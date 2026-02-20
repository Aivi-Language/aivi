impl Runtime {
    fn new(ctx: Arc<RuntimeContext>, cancel: Arc<CancelToken>) -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|dur| dur.as_nanos() as u64)
            .unwrap_or(0x1234_5678);
        Self {
            ctx,
            cancel,
            cancel_mask: 0,
            fuel: None,
            rng_state: seed ^ 0x9E37_79B9_7F4A_7C15,
            debug_stack: Vec::new(),
            check_counter: 0,
        }
    }

    fn check_cancelled(&mut self) -> Result<(), RuntimeError> {
        if self.cancel_mask > 0 {
            return Ok(());
        }
        // Amortize the atomic load: only check the cancel token every 64 evals.
        self.check_counter = self.check_counter.wrapping_add(1);
        if self.check_counter & 0x3F != 0 {
            // Still do fuel accounting every call if fuel is set.
            if let Some(fuel) = self.fuel.as_mut() {
                if *fuel == 0 {
                    return Err(RuntimeError::Cancelled);
                }
                *fuel = fuel.saturating_sub(1);
            }
            return Ok(());
        }
        if let Some(fuel) = self.fuel.as_mut() {
            if *fuel == 0 {
                return Err(RuntimeError::Cancelled);
            }
            *fuel = fuel.saturating_sub(1);
        }
        if self.cancel.is_cancelled() {
            Err(RuntimeError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn uncancelable<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        self.cancel_mask = self.cancel_mask.saturating_add(1);
        let result = f(self);
        self.cancel_mask = self.cancel_mask.saturating_sub(1);
        result
    }

    fn next_u64(&mut self) -> u64 {
        self.rng_state = self
            .rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        self.rng_state
    }

    fn force_value(&mut self, value: Value) -> Result<Value, RuntimeError> {
        match value {
            Value::Thunk(thunk) => self.eval_thunk(thunk),
            other => Ok(other),
        }
    }

    fn eval_thunk(&mut self, thunk: Arc<ThunkValue>) -> Result<Value, RuntimeError> {
        let cached = thunk.cached.lock().expect("thunk cache lock");
        if let Some(value) = cached.clone() {
            return Ok(value);
        }
        drop(cached);
        if thunk.in_progress.swap(true, Ordering::Acquire) {
            return Err(RuntimeError::Message(
                "recursive definition detected".to_string(),
            ));
        }
        let value = self.eval_expr(&thunk.expr, &thunk.env)?;
        let mut cached = thunk.cached.lock().expect("thunk cache lock");
        *cached = Some(value.clone());
        thunk.in_progress.store(false, Ordering::Release);
        Ok(value)
    }

    fn eval_expr(&mut self, expr: &HirExpr, env: &Env) -> Result<Value, RuntimeError> {
        self.check_cancelled()?;
        match expr {
            HirExpr::Var { name, .. } => {
                if let Some(value) = env.get(name) {
                    return self.force_value(value);
                }
                if let Some(ctor) = constructor_segment(name) {
                    return Ok(Value::Constructor {
                        name: ctor.to_string(),
                        args: Vec::new(),
                    });
                }
                Err(RuntimeError::Message(format!("unknown name {name}")))
            }
            HirExpr::LitNumber { text, .. } => {
                if let Some(value) = parse_number_value(text) {
                    return Ok(value);
                }
                let value = env.get(text).ok_or_else(|| {
                    RuntimeError::Message(format!("unknown numeric literal {text}"))
                })?;
                self.force_value(value)
            }
            HirExpr::LitString { text, .. } => Ok(Value::Text(text.clone())),
            HirExpr::TextInterpolate { parts, .. } => {
                let mut out = String::new();
                for part in parts {
                    match part {
                        HirTextPart::Text { text } => out.push_str(text),
                        HirTextPart::Expr { expr } => {
                            let value = self.eval_expr(expr, env)?;
                            out.push_str(&format_value(&value));
                        }
                    }
                }
                Ok(Value::Text(out))
            }
            HirExpr::LitSigil {
                tag, body, flags, ..
            } => match tag.as_str() {
                // Keep the runtime behavior aligned with `specs/02_syntax/13_sigils.md` and
                // `specs/05_stdlib/00_core/29_i18n.md`:
                // - ~k/~m are record-shaped values.
                // - ~m includes compiled `parts` for `i18n.render`.
                "r" => {
                    let mut builder = RegexBuilder::new(body);
                    for flag in flags.chars() {
                        match flag {
                            'i' => {
                                builder.case_insensitive(true);
                            }
                            'm' => {
                                builder.multi_line(true);
                            }
                            's' => {
                                builder.dot_matches_new_line(true);
                            }
                            'x' => {
                                builder.ignore_whitespace(true);
                            }
                            _ => {}
                        }
                    }
                    let regex = builder.build().map_err(|err| {
                        RuntimeError::Message(format!("invalid regex literal: {err}"))
                    })?;
                    Ok(Value::Regex(Arc::new(regex)))
                }
                "u" | "url" => {
                    let parsed = Url::parse(body).map_err(|err| {
                        RuntimeError::Message(format!("invalid url literal: {err}"))
                    })?;
                    Ok(Value::Record(Arc::new(url_to_record(&parsed))))
                }
                "p" | "path" => {
                    let cleaned = body.trim().replace('\\', "/");
                    if cleaned.contains('\0') {
                        return Err(RuntimeError::Message(
                            "invalid path literal: contains NUL byte".to_string(),
                        ));
                    }
                    let absolute = cleaned.starts_with('/');
                    let mut segments: Vec<String> = Vec::new();
                    for raw in cleaned.split('/') {
                        if raw.is_empty() || raw == "." {
                            continue;
                        }
                        if raw == ".." {
                            if let Some(last) = segments.last() {
                                if last != ".." {
                                    segments.pop();
                                    continue;
                                }
                            }
                            if !absolute {
                                segments.push("..".to_string());
                            }
                            continue;
                        }
                        segments.push(raw.to_string());
                    }

                    let mut map = HashMap::new();
                    map.insert("absolute".to_string(), Value::Bool(absolute));
                    map.insert(
                        "segments".to_string(),
                        Value::List(Arc::new(
                            segments.into_iter().map(Value::Text).collect::<Vec<_>>(),
                        )),
                    );
                    Ok(Value::Record(Arc::new(map)))
                }
                "d" => {
                    let date = NaiveDate::parse_from_str(body, "%Y-%m-%d").map_err(|err| {
                        RuntimeError::Message(format!("invalid date literal: {err}"))
                    })?;
                    Ok(Value::Record(Arc::new(date_to_record(date))))
                }
                "t" | "dt" => {
                    let _ = chrono::DateTime::parse_from_rfc3339(body).map_err(|err| {
                        RuntimeError::Message(format!("invalid datetime literal: {err}"))
                    })?;
                    Ok(Value::DateTime(body.clone()))
                }
                "tz" => {
                    let zone_id = body.trim();
                    let _: chrono_tz::Tz = zone_id.parse().map_err(|_| {
                        RuntimeError::Message(format!("invalid timezone id: {zone_id}"))
                    })?;
                    let mut map = HashMap::new();
                    map.insert("id".to_string(), Value::Text(zone_id.to_string()));
                    Ok(Value::Record(Arc::new(map)))
                }
                "zdt" => {
                    let text = body.trim();
                    let (dt_text, zone_id) = parse_zdt_parts(text)?;
                    let tz: chrono_tz::Tz = zone_id.parse().map_err(|_| {
                        RuntimeError::Message(format!("invalid timezone id: {zone_id}"))
                    })?;

                    let zdt = if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(dt_text) {
                        parsed.with_timezone(&tz)
                    } else {
                        let naive = parse_naive_datetime(dt_text)?;
                        tz.from_local_datetime(&naive)
                            .single()
                            .ok_or_else(|| {
                                RuntimeError::Message(
                                    "ambiguous or invalid local time".to_string(),
                                )
                            })?
                    };

                    let offset_millis =
                        i64::from(chrono::offset::Offset::fix(zdt.offset()).local_minus_utc())
                            * 1000;

                    let mut dt_map = HashMap::new();
                    dt_map.insert("year".to_string(), Value::Int(zdt.year() as i64));
                    dt_map.insert("month".to_string(), Value::Int(zdt.month() as i64));
                    dt_map.insert("day".to_string(), Value::Int(zdt.day() as i64));
                    dt_map.insert("hour".to_string(), Value::Int(zdt.hour() as i64));
                    dt_map.insert("minute".to_string(), Value::Int(zdt.minute() as i64));
                    dt_map.insert("second".to_string(), Value::Int(zdt.second() as i64));
                    dt_map.insert(
                        "millisecond".to_string(),
                        Value::Int(zdt.timestamp_subsec_millis() as i64),
                    );

                    let mut zone_map = HashMap::new();
                    zone_map.insert("id".to_string(), Value::Text(zone_id.to_string()));

                    let mut offset_map = HashMap::new();
                    offset_map.insert("millis".to_string(), Value::Int(offset_millis));

                    let mut map = HashMap::new();
                    map.insert("dateTime".to_string(), Value::Record(Arc::new(dt_map)));
                    map.insert("zone".to_string(), Value::Record(Arc::new(zone_map)));
                    map.insert("offset".to_string(), Value::Record(Arc::new(offset_map)));
                    Ok(Value::Record(Arc::new(map)))
                }
                "k" => {
                    validate_key_text(body).map_err(|msg| {
                        RuntimeError::Message(format!("invalid i18n key literal: {msg}"))
                    })?;
                    let mut map = HashMap::new();
                    map.insert("tag".to_string(), Value::Text(tag.clone()));
                    map.insert("body".to_string(), Value::Text(body.trim().to_string()));
                    map.insert("flags".to_string(), Value::Text(flags.clone()));
                    Ok(Value::Record(Arc::new(map)))
                }
                "m" => {
                    let parsed = parse_message_template(body).map_err(|msg| {
                        RuntimeError::Message(format!("invalid i18n message literal: {msg}"))
                    })?;
                    let mut map = HashMap::new();
                    map.insert("tag".to_string(), Value::Text(tag.clone()));
                    map.insert("body".to_string(), Value::Text(body.clone()));
                    map.insert("flags".to_string(), Value::Text(flags.clone()));
                    map.insert("parts".to_string(), i18n_message_parts_value(&parsed.parts));
                    Ok(Value::Record(Arc::new(map)))
                }
                _ => {
                    let mut map = HashMap::new();
                    map.insert("tag".to_string(), Value::Text(tag.clone()));
                    map.insert("body".to_string(), Value::Text(body.clone()));
                    map.insert("flags".to_string(), Value::Text(flags.clone()));
                    Ok(Value::Record(Arc::new(map)))
                }
            },
            HirExpr::LitBool { value, .. } => Ok(Value::Bool(*value)),
            HirExpr::LitDateTime { text, .. } => Ok(Value::DateTime(text.clone())),
            HirExpr::Lambda { param, body, .. } => Ok(Value::Closure(Arc::new(ClosureValue {
                param: param.clone(),
                body: Arc::new((**body).clone()),
                env: env.clone(),
            }))),
            HirExpr::App { func, arg, .. } => {
                let func_value = self.eval_expr(func, env)?;
                let arg_value = self.eval_expr(arg, env)?;
                self.apply(func_value, arg_value)
            }
            HirExpr::Call { func, args, .. } => {
                let mut func_value = self.eval_expr(func, env)?;
                for arg in args {
                    let arg_value = self.eval_expr(arg, env)?;
                    func_value = self.apply(func_value, arg_value)?;
                }
                Ok(func_value)
            }
            HirExpr::DebugFn {
                fn_name,
                arg_vars,
                log_args,
                log_return,
                log_time,
                body,
                ..
            } => {
                let call_id = self.ctx.next_debug_call_id();
                let start = log_time.then(std::time::Instant::now);

                let ts = log_time.then(now_unix_ms);
                let args_json = if *log_args {
                    Some(
                        arg_vars
                            .iter()
                            .map(|name| {
                                env.get(name)
                                    .as_ref()
                                    .map(|v| debug_value_to_json(v, 0))
                                    .unwrap_or(serde_json::Value::Null)
                            })
                            .collect::<Vec<_>>(),
                    )
                } else {
                    None
                };

                self.debug_stack.push(DebugFrame {
                    fn_name: fn_name.clone(),
                    call_id,
                    start,
                });

                let mut enter = serde_json::Map::new();
                enter.insert("kind".to_string(), serde_json::Value::String("fn.enter".to_string()));
                enter.insert("fn".to_string(), serde_json::Value::String(fn_name.clone()));
                enter.insert(
                    "callId".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(call_id)),
                );
                if let Some(args_json) = args_json {
                    enter.insert("args".to_string(), serde_json::Value::Array(args_json));
                }
                if let Some(ts) = ts {
                    enter.insert(
                        "ts".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(ts)),
                    );
                }
                emit_debug_event(serde_json::Value::Object(enter));

                let result = self.eval_expr(body, env);

                let frame = self.debug_stack.pop();
                if let Some(frame) = frame {
                    let dur_ms = if *log_time {
                        frame
                            .start
                            .map(|s| s.elapsed().as_millis() as u64)
                            .unwrap_or(0)
                    } else {
                        0
                    };

                    let mut exit = serde_json::Map::new();
                    exit.insert("kind".to_string(), serde_json::Value::String("fn.exit".to_string()));
                    exit.insert("fn".to_string(), serde_json::Value::String(frame.fn_name));
                    exit.insert(
                        "callId".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(frame.call_id)),
                    );
                    if *log_return {
                        if let Ok(ref value) = result {
                            exit.insert("ret".to_string(), debug_value_to_json(value, 0));
                        }
                    }
                    if *log_time {
                        exit.insert(
                            "durMs".to_string(),
                            serde_json::Value::Number(serde_json::Number::from(dur_ms)),
                        );
                    }
                    emit_debug_event(serde_json::Value::Object(exit));
                }

                result
            }
            HirExpr::Pipe {
                pipe_id,
                step,
                label,
                log_time,
                func,
                arg,
                ..
            } => {
                let func_value = self.eval_expr(func, env)?;
                let arg_value = self.eval_expr(arg, env)?;

                let Some(frame) = self.debug_stack.last().cloned() else {
                    return self.apply(func_value, arg_value);
                };

                let ts_in = log_time.then(now_unix_ms);
                let mut pipe_in = serde_json::Map::new();
                pipe_in.insert("kind".to_string(), serde_json::Value::String("pipe.in".to_string()));
                pipe_in.insert("fn".to_string(), serde_json::Value::String(frame.fn_name.clone()));
                pipe_in.insert(
                    "callId".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(frame.call_id)),
                );
                pipe_in.insert(
                    "pipeId".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(*pipe_id)),
                );
                pipe_in.insert(
                    "step".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(*step)),
                );
                pipe_in.insert("label".to_string(), serde_json::Value::String(label.clone()));
                pipe_in.insert("value".to_string(), debug_value_to_json(&arg_value, 0));
                if let Some(ts) = ts_in {
                    pipe_in.insert(
                        "ts".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(ts)),
                    );
                }
                emit_debug_event(serde_json::Value::Object(pipe_in));

                let step_start = log_time.then(std::time::Instant::now);
                let out_value = self.apply(func_value, arg_value)?;

                let dur_ms = if *log_time {
                    step_start
                        .map(|s| s.elapsed().as_millis() as u64)
                        .unwrap_or(0)
                } else {
                    0
                };
                let shape = debug_shape_tag(&out_value);

                let mut pipe_out = serde_json::Map::new();
                pipe_out.insert(
                    "kind".to_string(),
                    serde_json::Value::String("pipe.out".to_string()),
                );
                pipe_out.insert("fn".to_string(), serde_json::Value::String(frame.fn_name));
                pipe_out.insert(
                    "callId".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(frame.call_id)),
                );
                pipe_out.insert(
                    "pipeId".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(*pipe_id)),
                );
                pipe_out.insert(
                    "step".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(*step)),
                );
                pipe_out.insert("label".to_string(), serde_json::Value::String(label.clone()));
                pipe_out.insert("value".to_string(), debug_value_to_json(&out_value, 0));
                if *log_time {
                    pipe_out.insert(
                        "durMs".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(dur_ms)),
                    );
                }
                if let Some(shape) = shape {
                    pipe_out.insert("shape".to_string(), serde_json::Value::String(shape));
                }
                emit_debug_event(serde_json::Value::Object(pipe_out));

                Ok(out_value)
            }
            HirExpr::List { items, .. } => self.eval_list(items, env),
            HirExpr::Tuple { items, .. } => {
                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    values.push(self.eval_expr(item, env)?);
                }
                Ok(Value::Tuple(values))
            }
            HirExpr::Record { fields, .. } => self.eval_record(fields, env),
            HirExpr::Patch { target, fields, .. } => self.eval_patch(target, fields, env),
            HirExpr::FieldAccess { base, field, .. } => {
                let base_value = self.eval_expr(base, env)?;
                match base_value {
                    Value::Record(map) => map
                        .get(field)
                        .cloned()
                        .ok_or_else(|| RuntimeError::Message(format!("missing field {field}"))),
                    _ => Err(RuntimeError::Message(format!(
                        "field access on non-record {field}"
                    ))),
                }
            }
            HirExpr::Index { base, index, .. } => {
                let base_value = self.eval_expr(base, env)?;
                let index_value = self.eval_expr(index, env)?;
                match base_value {
                    Value::List(items) => {
                        let Value::Int(idx) = index_value else {
                            return Err(RuntimeError::Message(
                                "list index expects an Int".to_string(),
                            ));
                        };
                        let idx = idx as usize;
                        items
                            .get(idx)
                            .cloned()
                            .ok_or_else(|| RuntimeError::Message("index out of bounds".to_string()))
                    }
                    Value::Tuple(items) => {
                        let Value::Int(idx) = index_value else {
                            return Err(RuntimeError::Message(
                                "tuple index expects an Int".to_string(),
                            ));
                        };
                        let idx = idx as usize;
                        items
                            .get(idx)
                            .cloned()
                            .ok_or_else(|| RuntimeError::Message("index out of bounds".to_string()))
                    }
                    Value::Map(entries) => {
                        let Some(key) = KeyValue::try_from_value(&index_value) else {
                            return Err(RuntimeError::Message(format!(
                                "map key is not a valid key type: {}",
                                format_value(&index_value)
                            )));
                        };
                        entries
                            .get(&key)
                            .cloned()
                            .ok_or_else(|| RuntimeError::Message("missing map key".to_string()))
                    }
                    _ => Err(RuntimeError::Message(
                        "index on unsupported value".to_string(),
                    )),
                }
            }
            HirExpr::Match {
                scrutinee, arms, ..
            } => {
                let value = self.eval_expr(scrutinee, env)?;
                self.eval_match(&value, arms, env)
            }
            HirExpr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                let cond_value = self.eval_expr(cond, env)?;
                if matches!(cond_value, Value::Bool(true)) {
                    self.eval_expr(then_branch, env)
                } else {
                    self.eval_expr(else_branch, env)
                }
            }
            HirExpr::Binary {
                op, left, right, ..
            } => {
                // Short-circuit logical operators: avoid evaluating the right
                // operand when the left already determines the result.
                if op == "&&" {
                    let left_value = self.eval_expr(left, env)?;
                    return match left_value {
                        Value::Bool(false) => Ok(Value::Bool(false)),
                        Value::Bool(true) => self.eval_expr(right, env),
                        _ => {
                            let right_value = self.eval_expr(right, env)?;
                            self.eval_binary(op, left_value, right_value, env)
                        }
                    };
                }
                if op == "||" {
                    let left_value = self.eval_expr(left, env)?;
                    return match left_value {
                        Value::Bool(true) => Ok(Value::Bool(true)),
                        Value::Bool(false) => self.eval_expr(right, env),
                        _ => {
                            let right_value = self.eval_expr(right, env)?;
                            self.eval_binary(op, left_value, right_value, env)
                        }
                    };
                }
                let left_value = self.eval_expr(left, env)?;
                let right_value = self.eval_expr(right, env)?;
                self.eval_binary(op, left_value, right_value, env)
            }
            HirExpr::Block {
                block_kind, items, ..
            } => match block_kind {
                crate::hir::HirBlockKind::Plain => self.eval_plain_block(items, env),
                crate::hir::HirBlockKind::Do { ref monad } if monad == "Effect" => {
                    Ok(Value::Effect(Arc::new(EffectValue::Block {
                        env: env.clone(),
                        items: Arc::new(items.clone()),
                    })))
                }
                crate::hir::HirBlockKind::Do { ref monad } => {
                    self.eval_generic_do_block(monad, items, env)
                }
                crate::hir::HirBlockKind::Resource => {
                    Ok(Value::Resource(Arc::new(ResourceValue {
                        items: Arc::new(items.clone()),
                    })))
                }
                crate::hir::HirBlockKind::Generate => self.eval_generate_block(items, env),
            },
            HirExpr::Raw { .. } => Err(RuntimeError::Message(
                "raw expressions are not supported in native runtime yet".to_string(),
            )),
        }
    }

    fn apply(&mut self, func: Value, arg: Value) -> Result<Value, RuntimeError> {
        let func = self.force_value(func)?;
        match func {
            Value::Closure(closure) => {
                let new_env = Env::new(Some(closure.env.clone()));
                new_env.set(closure.param.clone(), arg);
                self.eval_expr(&closure.body, &new_env)
            }
            Value::Builtin(builtin) => builtin.apply(arg, self),
            Value::MultiClause(clauses) => self.apply_multi_clause(clauses, arg),
            Value::Constructor { name, mut args } => {
                args.push(arg);
                Ok(Value::Constructor { name, args })
            }
            other => Err(RuntimeError::Message(format!(
                "attempted to call a non-function: {}",
                format_value(&other)
            ))),
        }
    }
}

fn parse_zdt_parts(text: &str) -> Result<(&str, &str), RuntimeError> {
    let (dt_text, zone_part) = text.rsplit_once('[').ok_or_else(|| {
        RuntimeError::Message("invalid zoned datetime literal: missing [Zone]".to_string())
    })?;
    let zone_id = zone_part.strip_suffix(']').ok_or_else(|| {
        RuntimeError::Message("invalid zoned datetime literal: missing closing ]".to_string())
    })?;
    let dt_text = dt_text.trim();
    let zone_id = zone_id.trim();
    if dt_text.is_empty() || zone_id.is_empty() {
        return Err(RuntimeError::Message(
            "invalid zoned datetime literal".to_string(),
        ));
    }
    Ok((dt_text, zone_id))
}

fn parse_naive_datetime(text: &str) -> Result<chrono::NaiveDateTime, RuntimeError> {
    let (date_part, time_part) = text.split_once('T').ok_or_else(|| {
        RuntimeError::Message("invalid zoned datetime literal".to_string())
    })?;

    let mut date_iter = date_part.splitn(3, '-');
    let year = parse_i32(date_iter.next())?;
    let month = parse_u32(date_iter.next())?;
    let day = parse_u32(date_iter.next())?;
    if date_iter.next().is_some() {
        return Err(RuntimeError::Message(
            "invalid zoned datetime literal".to_string(),
        ));
    }

    let (time_main, frac_part) = time_part.split_once('.').unwrap_or((time_part, ""));
    let time_main = time_main.strip_suffix('Z').unwrap_or(time_main);
    let mut time_iter = time_main.splitn(3, ':');
    let hour = parse_u32(time_iter.next())?;
    let minute = parse_u32(time_iter.next())?;
    let second = parse_u32(time_iter.next())?;
    if time_iter.next().is_some() {
        return Err(RuntimeError::Message(
            "invalid zoned datetime literal".to_string(),
        ));
    }

    let millis = parse_millis(frac_part)?;
    chrono::NaiveDate::from_ymd_opt(year, month, day)
        .and_then(|d| d.and_hms_milli_opt(hour, minute, second, millis))
        .ok_or_else(|| RuntimeError::Message("invalid zoned datetime literal".to_string()))
}

fn parse_i32(value: Option<&str>) -> Result<i32, RuntimeError> {
    let value = value.ok_or_else(|| {
        RuntimeError::Message("invalid zoned datetime literal".to_string())
    })?;
    value.parse::<i32>().map_err(|_| {
        RuntimeError::Message("invalid zoned datetime literal".to_string())
    })
}

fn parse_u32(value: Option<&str>) -> Result<u32, RuntimeError> {
    let value = value.ok_or_else(|| {
        RuntimeError::Message("invalid zoned datetime literal".to_string())
    })?;
    value.parse::<u32>().map_err(|_| {
        RuntimeError::Message("invalid zoned datetime literal".to_string())
    })
}

fn parse_millis(text: &str) -> Result<u32, RuntimeError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(0);
    }
    if !trimmed.chars().all(|ch| ch.is_ascii_digit()) || trimmed.len() > 3 {
        return Err(RuntimeError::Message(
            "invalid zoned datetime literal".to_string(),
        ));
    }
    let value: u32 = trimmed.parse().map_err(|_| {
        RuntimeError::Message("invalid zoned datetime literal".to_string())
    })?;
    let scale = 10u32.pow((3 - trimmed.len()) as u32);
    Ok(value.saturating_mul(scale))
}
