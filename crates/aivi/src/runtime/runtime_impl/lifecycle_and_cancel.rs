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
            check_counter: 0,
            jit_call_depth: 0,
            jit_max_call_depth: 1_000,
            jit_match_failed: false,
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
