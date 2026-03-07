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
            capability_handlers: Vec::new(),
            fuel: None,
            rng_state: seed ^ 0x9E37_79B9_7F4A_7C15,
            check_counter: 0,
            jit_call_depth: 0,
            jit_max_call_depth: 1_000,
            jit_match_failed: false,
            jit_pending_error: None,
            jit_current_fn: None,
            jit_current_loc: None,
            jit_rt_warning_count: 0,
            jit_suppress_warnings: false,
            jit_binary_op_dispatching: false,
            update_snapshots: false,
            current_test_name: None,
            project_root: None,
            snapshot_recordings: HashMap::new(),
            snapshot_replay_cursors: HashMap::new(),
            snapshot_failure: None,
            resource_cleanups: Vec::new(),
            reactive_host: None,
        }
    }

    pub(crate) fn check_cancelled(&mut self) -> Result<(), RuntimeError> {
        if self.cancel_mask > 0 {
            return Ok(());
        }
        self.check_counter = self.check_counter.wrapping_add(1);
        if self.check_counter & 0x3F != 0 {
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

    pub(crate) fn push_capability_scope(&mut self, scope: CapabilityHandlerScope) {
        self.capability_handlers.push(scope);
    }

    pub(crate) fn pop_capability_scope(&mut self) {
        self.capability_handlers.pop();
    }

    pub(crate) fn capture_capability_scopes(&self) -> Vec<CapabilityHandlerScope> {
        self.capability_handlers.clone()
    }

    pub(crate) fn with_capability_scope<T>(
        &mut self,
        scope: CapabilityHandlerScope,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.push_capability_scope(scope);
        let result = f(self);
        self.pop_capability_scope();
        result
    }

    pub(crate) fn with_capability_scopes<T>(
        &mut self,
        scopes: &[CapabilityHandlerScope],
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let start = self.capability_handlers.len();
        self.capability_handlers.extend(scopes.iter().cloned());
        let result = f(self);
        self.capability_handlers.truncate(start);
        result
    }

    pub(crate) fn wrap_value_with_capability_scope(
        &self,
        value: Value,
        scope: CapabilityHandlerScope,
    ) -> Value {
        match value {
            Value::Builtin(builtin) if builtin.imp.arity == 0 && builtin.args.is_empty() => {
                let wrapped_builtin = BuiltinValue {
                    imp: Arc::new(BuiltinImpl {
                        name: format!("{}|capabilityScope", builtin.imp.name),
                        arity: 0,
                        func: Arc::new(move |_, runtime| {
                            let value = runtime.with_capability_scope(scope.clone(), |runtime| {
                                runtime.force_value(Value::Builtin(builtin.clone()))
                            })?;
                            Ok(runtime.wrap_value_with_capability_scope(value, scope.clone()))
                        }),
                    }),
                    args: Vec::new(),
                    tagged_args: Some(Vec::new()),
                };
                Value::Builtin(wrapped_builtin)
            }
            Value::Effect(effect) => {
                let wrapped = EffectValue::Thunk {
                    func: Arc::new(move |runtime| {
                        runtime.with_capability_scope(scope.clone(), |runtime| {
                            runtime.run_effect_value(Value::Effect(effect.clone()))
                        })
                    }),
                };
                Value::Effect(Arc::new(wrapped))
            }
            Value::Source(source) => {
                let source_for_effect = source.clone();
                Value::Source(Arc::new(SourceValue {
                    kind: source.kind.clone(),
                    effect: Arc::new(EffectValue::Thunk {
                        func: Arc::new(move |runtime| {
                            runtime.with_capability_scope(scope.clone(), |runtime| {
                                runtime.run_effect_value(Value::Source(source_for_effect.clone()))
                            })
                        }),
                    }),
                    schema: source.schema.clone(),
                    raw_text: source.raw_text.clone(),
                }))
            }
            Value::Resource(resource) => {
                let resource_for_acquire = resource.clone();
                Value::Resource(Arc::new(ResourceValue {
                    acquire: Arc::new({
                        let scope = scope.clone();
                        move |runtime| {
                            runtime.with_capability_scope(scope.clone(), |runtime| {
                                (resource_for_acquire.acquire)(runtime)
                            })
                        }
                    }),
                    cleanup: Arc::new(move |runtime| {
                        runtime.with_capability_scope(scope.clone(), |runtime| {
                            (resource.cleanup)(runtime)
                        })
                    }),
                }))
            }
            other => other,
        }
    }

    pub(crate) fn dispatch_capability_handler(
        &mut self,
        capability: &str,
        args: &[Value],
    ) -> Result<Option<Value>, RuntimeError> {
        let Some(handler) = self.resolve_capability_handler(capability)? else {
            return Ok(None);
        };
        let mut value = handler;
        for arg in args.iter().cloned() {
            value = self.apply(value, arg)?;
        }
        Ok(Some(value))
    }

    fn resolve_capability_handler(&mut self, capability: &str) -> Result<Option<Value>, RuntimeError> {
        let segments: Vec<&str> = capability.split('.').collect();
        for index in (0..self.capability_handlers.len()).rev() {
            let scope = self.capability_handlers[index].clone();
            if let Some(handler) = scope.get(capability) {
                return Ok(Some(handler.clone()));
            }
            for prefix_len in (1..segments.len()).rev() {
                let prefix = segments[..prefix_len].join(".");
                let remainder = &segments[prefix_len..];
                let Some(handler) = scope.get(&prefix) else {
                    continue;
                };
                if let Some(resolved) = self.resolve_handler_member(handler.clone(), remainder)? {
                    return Ok(Some(resolved));
                }
            }
        }
        Ok(None)
    }

    fn resolve_handler_member(
        &mut self,
        handler: Value,
        remainder: &[&str],
    ) -> Result<Option<Value>, RuntimeError> {
        let mut current = handler;
        for segment in remainder {
            let value = self.force_value(current)?;
            let Value::Record(fields) = value else {
                return Ok(None);
            };
            let Some(next) = fields.get(*segment).cloned() else {
                return Ok(None);
            };
            current = next;
        }
        Ok(Some(current))
    }

    fn next_u64(&mut self) -> u64 {
        self.rng_state = self
            .rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        self.rng_state
    }

    pub(crate) fn force_value(&mut self, value: Value) -> Result<Value, RuntimeError> {
        match value {
            Value::Thunk(thunk) => {
                let cached = thunk.cached.lock().expect("thunk cache lock");
                if let Some(value) = cached.clone() {
                    return Ok(value);
                }
                drop(cached);
                Err(RuntimeError::Message(
                    "cannot force non-JIT thunk: interpreter has been removed".to_string(),
                ))
            }
            Value::Builtin(builtin)
                if builtin.imp.arity == 0
                    && builtin.args.is_empty()
                    && builtin.imp.name.starts_with("__jit|") =>
            {
                (builtin.imp.func)(Vec::new(), self)
            }
            other => Ok(other),
        }
    }

    pub(crate) fn apply(&mut self, func: Value, arg: Value) -> Result<Value, RuntimeError> {
        let func = self.force_value(func)?;
        match func {
            Value::Builtin(builtin) => builtin.apply(arg, self),
            Value::MultiClause(clauses) => self.apply_multi_clause(clauses, arg),
            Value::Constructor { name, mut args } => {
                args.push(arg);
                Ok(Value::Constructor { name, args })
            }
            other => Err(RuntimeError::TypeError {
                context: "function application".to_string(),
                expected: "Function".to_string(),
                got: format_value(&other),
            }),
        }
    }
}

fn parse_zdt_parts(text: &str) -> Result<(&str, &str), RuntimeError> {
    let (dt_text, zone_part) = text.rsplit_once('[').ok_or_else(|| {
        RuntimeError::ParseError {
            context: "zoned datetime literal".to_string(),
            input: "missing [Zone]".to_string(),
        }
    })?;
    let zone_id = zone_part.strip_suffix(']').ok_or_else(|| {
        RuntimeError::ParseError {
            context: "zoned datetime literal".to_string(),
            input: "missing closing ]".to_string(),
        }
    })?;
    let dt_text = dt_text.trim();
    let zone_id = zone_id.trim();
    if dt_text.is_empty() || zone_id.is_empty() {
        return Err(RuntimeError::ParseError {
            context: "zoned datetime literal".to_string(),
            input: text.to_string(),
        });
    }
    Ok((dt_text, zone_id))
}

fn zdt_parse_error(input: &str) -> RuntimeError {
    RuntimeError::ParseError {
        context: "zoned datetime literal".to_string(),
        input: input.to_string(),
    }
}

fn parse_naive_datetime(text: &str) -> Result<chrono::NaiveDateTime, RuntimeError> {
    let (date_part, time_part) = text.split_once('T').ok_or_else(|| zdt_parse_error(text))?;

    let mut date_iter = date_part.splitn(3, '-');
    let year = parse_i32(date_iter.next(), text)?;
    let month = parse_u32(date_iter.next(), text)?;
    let day = parse_u32(date_iter.next(), text)?;
    if date_iter.next().is_some() {
        return Err(zdt_parse_error(text));
    }

    let (time_main, frac_part) = time_part.split_once('.').unwrap_or((time_part, ""));
    let time_main = time_main.strip_suffix('Z').unwrap_or(time_main);
    let mut time_iter = time_main.splitn(3, ':');
    let hour = parse_u32(time_iter.next(), text)?;
    let minute = parse_u32(time_iter.next(), text)?;
    let second = parse_u32(time_iter.next(), text)?;
    if time_iter.next().is_some() {
        return Err(zdt_parse_error(text));
    }

    let millis = parse_millis(frac_part, text)?;
    chrono::NaiveDate::from_ymd_opt(year, month, day)
        .and_then(|d| d.and_hms_milli_opt(hour, minute, second, millis))
        .ok_or_else(|| zdt_parse_error(text))
}

fn parse_i32(value: Option<&str>, input: &str) -> Result<i32, RuntimeError> {
    let value = value.ok_or_else(|| zdt_parse_error(input))?;
    value.parse::<i32>().map_err(|_| zdt_parse_error(input))
}

fn parse_u32(value: Option<&str>, input: &str) -> Result<u32, RuntimeError> {
    let value = value.ok_or_else(|| zdt_parse_error(input))?;
    value.parse::<u32>().map_err(|_| zdt_parse_error(input))
}

fn parse_millis(text: &str, input: &str) -> Result<u32, RuntimeError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(0);
    }
    if !trimmed.chars().all(|ch| ch.is_ascii_digit()) || trimmed.len() > 3 {
        return Err(zdt_parse_error(input));
    }
    let value: u32 = trimmed.parse().map_err(|_| zdt_parse_error(input))?;
    let scale = 10u32.pow((3 - trimmed.len()) as u32);
    Ok(value.saturating_mul(scale))
}
