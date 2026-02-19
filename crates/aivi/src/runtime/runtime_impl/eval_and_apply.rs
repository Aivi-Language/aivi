impl Runtime {
    fn apply_multi_clause(
        &mut self,
        clauses: Vec<Value>,
        arg: Value,
    ) -> Result<Value, RuntimeError> {
        let mut results = Vec::new();
        let mut match_failures = 0;
        let mut last_error = None;
        let n = clauses.len();
        for (i, clause) in clauses.into_iter().enumerate() {
            let a = if i + 1 < n { arg.clone() } else { arg.clone() };
            match self.apply(clause, a) {
                Ok(value) => results.push(value),
                Err(RuntimeError::Message(message)) if is_match_failure_message(&message) => {
                    match_failures += 1;
                }
                Err(err) => {
                    last_error = Some(err);
                }
            }
        }
        if !results.is_empty() {
            let mut callable = results
                .iter()
                .filter(|value| is_callable(value))
                .cloned()
                .collect::<Vec<_>>();
            if !callable.is_empty() {
                if callable.len() == 1 {
                    return Ok(callable.remove(0));
                }
                return Ok(Value::MultiClause(callable));
            }
            return Ok(results.remove(0));
        }
        if match_failures > 0 && last_error.is_none() {
            return Err(RuntimeError::Message("non-exhaustive match".to_string()));
        }
        Err(last_error.unwrap_or_else(|| RuntimeError::Message("no matching clause".to_string())))
    }

    fn eval_plain_block(
        &mut self,
        items: &[HirBlockItem],
        env: &Env,
    ) -> Result<Value, RuntimeError> {
        let local_env = Env::new(Some(env.clone()));
        let mut last_value = Value::Unit;
        for (index, item) in items.iter().enumerate() {
            let last = index + 1 == items.len();
            match item {
                HirBlockItem::Bind { pattern, expr } => {
                    let value = self.eval_expr(expr, &local_env)?;
                    let bindings = collect_pattern_bindings(pattern, &value)
                        .ok_or_else(|| RuntimeError::Message("pattern match failed".to_string()))?;
                    for (name, value) in bindings {
                        local_env.set(name, value);
                    }
                    if last {
                        last_value = Value::Unit;
                    }
                }
                HirBlockItem::Expr { expr } => {
                    last_value = self.eval_expr(expr, &local_env)?;
                    if !last {
                        last_value = Value::Unit;
                    }
                }
                HirBlockItem::Filter { .. }
                | HirBlockItem::Yield { .. }
                | HirBlockItem::Recurse { .. } => {
                    return Err(RuntimeError::Message(
                        "unsupported block item in plain block".to_string(),
                    ));
                }
            }
        }
        Ok(last_value)
    }

    /// Evaluate a generic `do M { ... }` block (where M is not Effect).
    ///
    /// Since v0.1, generic `do M` blocks are desugared into nested `chain`/`of`
    /// calls at the HIR lowering stage. This method should no longer be reached
    /// for well-formed programs. It is kept as a defensive fallback.
    fn eval_generic_do_block(
        &mut self,
        monad: &str,
        _items: &[HirBlockItem],
        _env: &Env,
    ) -> Result<Value, RuntimeError> {
        Err(RuntimeError::Message(format!(
            "internal error: `do {monad} {{ ... }}` block was not desugared during HIR lowering; \
             this is a compiler bug"
        )))
    }

    fn eval_generate_block(
        &mut self,
        items: &[HirBlockItem],
        env: &Env,
    ) -> Result<Value, RuntimeError> {
        // Eagerly materialize the generator items into a Vec<Value>
        let mut values = Vec::new();
        self.materialize_generate(items, env, &mut values)?;

        // Return a builtin function: \k -> \z -> foldl k z values
        let values = Arc::new(values);
        Ok(Value::Builtin(BuiltinValue {
            imp: Arc::new(BuiltinImpl {
                name: "<generator>".to_string(),
                arity: 2,
                func: Arc::new(move |mut args, runtime| {
                    let z = args.pop().unwrap();
                    let k = args.pop().unwrap();
                    let mut acc = z;
                    for val in values.iter() {
                        // k(acc, x)
                        let partial = runtime.apply(k.clone(), acc)?;
                        acc = runtime.apply(partial, val.clone())?;
                    }
                    Ok(acc)
                }),
            }),
            args: Vec::new(),
        }))
    }

    fn materialize_generate(
        &mut self,
        items: &[HirBlockItem],
        env: &Env,
        out: &mut Vec<Value>,
    ) -> Result<(), RuntimeError> {
        let local_env = Env::new(Some(env.clone()));
        for item in items {
            match item {
                HirBlockItem::Yield { expr } => {
                    let value = self.eval_expr(expr, &local_env)?;
                    out.push(value);
                }
                HirBlockItem::Bind { pattern, expr } => {
                    let source = self.eval_expr(expr, &local_env)?;
                    // The source should be a generator (a builtin that takes k and z).
                    // We need to extract its elements. We do this by folding with a
                    // list-accumulate step function.
                    let source_items = self.generator_to_list(source)?;
                    // For each element from the source, bind it to the pattern
                    // and process the rest of the items in this scope.
                    let rest =
                        &items[items.iter().position(|i| std::ptr::eq(i, item)).unwrap() + 1..];
                    for val in source_items {
                        let bind_env = Env::new(Some(local_env.clone()));
                        let bindings =
                            collect_pattern_bindings(pattern, &val).ok_or_else(|| {
                                RuntimeError::Message(
                                    "pattern match failed in generator bind".to_string(),
                                )
                            })?;
                        for (name, bound_val) in bindings {
                            bind_env.set(name, bound_val);
                        }
                        self.materialize_generate(rest, &bind_env, out)?;
                    }
                    return Ok(());
                }
                HirBlockItem::Filter { expr } => {
                    let cond = self.eval_expr(expr, &local_env)?;
                    if !matches!(cond, Value::Bool(true)) {
                        return Ok(());
                    }
                }
                HirBlockItem::Expr { expr } => {
                    // An expression in a generate block acts as a sub-generator to spread
                    let sub = self.eval_expr(expr, &local_env)?;
                    let sub_items = self.generator_to_list(sub)?;
                    out.extend(sub_items);
                }
                HirBlockItem::Recurse { .. } => {
                    // Unsupported for now
                }
            }
        }
        Ok(())
    }

    fn generator_to_list(&mut self, gen: Value) -> Result<Vec<Value>, RuntimeError> {
        // A generator is a function (k -> z -> R).
        // We fold it with a list-append step: k = \acc x -> acc ++ [x], z = []
        // Cache the step builtin implementation to avoid re-allocating on every call.
        thread_local! {
            static GEN_STEP_IMPL: Arc<BuiltinImpl> = Arc::new(BuiltinImpl {
                name: "<gen_to_list_step>".to_string(),
                arity: 2,
                func: Arc::new(|mut args, _runtime| {
                    let x = args.pop().unwrap();
                    let acc = args.pop().unwrap();
                    let mut list = match acc {
                        Value::List(items) => (*items).clone(),
                        _ => {
                            return Err(RuntimeError::Message(
                                "expected list accumulator".to_string(),
                            ))
                        }
                    };
                    list.push(x);
                    Ok(Value::List(Arc::new(list)))
                }),
            });
        }
        let step = Value::Builtin(BuiltinValue {
            imp: GEN_STEP_IMPL.with(|imp| imp.clone()),
            args: Vec::new(),
        });
        let init = Value::List(Arc::new(Vec::new()));
        let with_step = self.apply(gen, step)?;
        let result = self.apply(with_step, init)?;
        match result {
            Value::List(items) => Ok((*items).clone()),
            _ => Err(RuntimeError::Message(
                "generator fold did not produce a list".to_string(),
            )),
        }
    }

    fn eval_match(
        &mut self,
        value: &Value,
        arms: &[HirMatchArm],
        env: &Env,
    ) -> Result<Value, RuntimeError> {
        for arm in arms {
            if let Some(bindings) = collect_pattern_bindings(&arm.pattern, value) {
                if let Some(guard) = &arm.guard {
                    let guard_env = Env::new(Some(env.clone()));
                    for (name, value) in bindings.clone() {
                        guard_env.set(name, value);
                    }
                    let guard_value = self.eval_expr(guard, &guard_env)?;
                    if !matches!(guard_value, Value::Bool(true)) {
                        continue;
                    }
                }
                let arm_env = Env::new(Some(env.clone()));
                for (name, value) in bindings {
                    arm_env.set(name, value);
                }
                return self.eval_expr(&arm.body, &arm_env);
            }
        }
        Err(RuntimeError::Message("non-exhaustive match".to_string()))
    }

    fn eval_list(&mut self, items: &[HirListItem], env: &Env) -> Result<Value, RuntimeError> {
        let mut values = Vec::new();
        for item in items {
            let value = self.eval_expr(&item.expr, env)?;
            if item.spread {
                match value {
                    Value::List(inner) => values.extend(inner.iter().cloned()),
                    _ => {
                        return Err(RuntimeError::Message(
                            "list spread expects a list".to_string(),
                        ))
                    }
                }
            } else {
                values.push(value);
            }
        }
        Ok(Value::List(Arc::new(values)))
    }

    fn eval_record(&mut self, fields: &[HirRecordField], env: &Env) -> Result<Value, RuntimeError> {
        let mut map = HashMap::new();
        for field in fields {
            let value = self.eval_expr(&field.value, env)?;
            if field.spread {
                match value {
                    Value::Record(inner) => {
                        for (k, v) in inner.as_ref().iter() {
                            map.insert(k.clone(), v.clone());
                        }
                    }
                    _ => {
                        return Err(RuntimeError::Message(
                            "record spread expects a record".to_string(),
                        ))
                    }
                }
                continue;
            }
            insert_record_path(&mut map, &field.path, value)?;
        }
        Ok(Value::Record(Arc::new(map)))
    }

    fn eval_patch(
        &mut self,
        target: &HirExpr,
        fields: &[HirRecordField],
        env: &Env,
    ) -> Result<Value, RuntimeError> {
        let base_value = self.eval_expr(target, env)?;
        let Value::Record(map) = base_value else {
            return Err(RuntimeError::Message(
                "patch target must be a record".to_string(),
            ));
        };
        let mut map = map.as_ref().clone();
        for field in fields {
            if field.spread {
                return Err(RuntimeError::Message(
                    "patch fields do not support record spread".to_string(),
                ));
            }
            self.apply_patch_field(&mut map, &field.path, &field.value, env)?;
        }
        Ok(Value::Record(Arc::new(map)))
    }

    fn apply_patch_field(
        &mut self,
        record: &mut HashMap<String, Value>,
        path: &[HirPathSegment],
        expr: &HirExpr,
        env: &Env,
    ) -> Result<(), RuntimeError> {
        if path.is_empty() {
            return Err(RuntimeError::Message(
                "patch field path must not be empty".to_string(),
            ));
        }
        let mut current = record;
        for segment in &path[..path.len() - 1] {
            match segment {
                HirPathSegment::Field(name) => {
                    let entry = current
                        .entry(name.clone())
                        .or_insert_with(|| Value::Record(Arc::new(HashMap::new())));
                    match entry {
                        Value::Record(map) => {
                            current = Arc::make_mut(map);
                        }
                        _ => {
                            return Err(RuntimeError::Message(format!(
                                "patch path conflict at {name}"
                            )))
                        }
                    }
                }
                HirPathSegment::Index(_) | HirPathSegment::All => {
                    return Err(RuntimeError::Message(
                        "patch index paths are not supported in native runtime yet".to_string(),
                    ))
                }
            }
        }
        let segment = path.last().unwrap();
        match segment {
            HirPathSegment::Field(name) => {
                let existing = current.get(name).cloned();
                let value = self.eval_expr(expr, env)?;
                let new_value = match existing {
                    Some(existing) if is_callable(&value) => self.apply(value, existing)?,
                    Some(_) | None if is_callable(&value) => {
                        return Err(RuntimeError::Message(format!(
                            "patch transform expects existing field {name}"
                        )));
                    }
                    _ => value,
                };
                current.insert(name.clone(), new_value);
                Ok(())
            }
            HirPathSegment::Index(_) | HirPathSegment::All => Err(RuntimeError::Message(
                "patch index paths are not supported in native runtime yet".to_string(),
            )),
        }
    }

    fn eval_binary(
        &mut self,
        op: &str,
        left: Value,
        right: Value,
        env: &Env,
    ) -> Result<Value, RuntimeError> {
        if let Some(result) = eval_binary_builtin(op, &left, &right) {
            return Ok(result);
        }
        let op_name = format!("({})", op);
        if let Some(op_value) = env.get(&op_name) {
            let applied = self.apply(op_value, left)?;
            return self.apply(applied, right);
        }
        Err(RuntimeError::Message(format!(
            "unsupported binary operator {op}"
        )))
    }

    fn run_effect_value(&mut self, value: Value) -> Result<Value, RuntimeError> {
        self.check_cancelled()?;
        match value {
            Value::Effect(effect) => match effect.as_ref() {
                EffectValue::Block { env, items } => {
                    self.run_effect_block(env.clone(), items.as_ref())
                }
                EffectValue::Thunk { func } => func(self),
            },
            // Sources can be executed anywhere an `Effect` is expected; `load` just makes this explicit.
            Value::Source(source) => match source.effect.as_ref() {
                EffectValue::Block { env, items } => {
                    self.run_effect_block(env.clone(), items.as_ref())
                }
                EffectValue::Thunk { func } => func(self),
            },
            other => Err(RuntimeError::Message(format!(
                "expected Effect, got {}",
                format_value(&other)
            ))),
        }
    }

    fn run_effect_block(
        &mut self,
        env: Env,
        items: &[HirBlockItem],
    ) -> Result<Value, RuntimeError> {
        let local_env = Env::new(Some(env));
        let mut cleanups: Vec<Value> = Vec::new();
        let mut result: Result<Value, RuntimeError> = Ok(Value::Unit);
        let trace_effect = std::env::var("AIVI_TRACE_EFFECT").is_ok_and(|v| v == "1");

        for (index, item) in items.iter().enumerate() {
            let last = index + 1 == items.len();
            if trace_effect {
                eprintln!("[AIVI_TRACE_EFFECT] step {} / {}", index + 1, items.len());
            }
            if let Err(err) = self.check_cancelled() {
                result = Err(err);
                break;
            }
            let step = match item {
                HirBlockItem::Bind { pattern, expr } => {
                    let value = self.eval_expr(expr, &local_env)?;
                    match value {
                        Value::Resource(resource) => {
                            let (res_value, cleanup) =
                                self.acquire_resource(resource, &local_env)?;
                            let bindings = collect_pattern_bindings(pattern, &res_value)
                                .ok_or_else(|| {
                                    RuntimeError::Message(
                                        "pattern match failed in resource bind".to_string(),
                                    )
                                })?;
                            for (name, value) in bindings {
                                local_env.set(name, value);
                            }
                            cleanups.push(cleanup);
                            Ok(Value::Unit)
                        }
                        Value::Effect(_) => {
                            let value = self.run_effect_value(value)?;
                            let bindings =
                                collect_pattern_bindings(pattern, &value).ok_or_else(|| {
                                    RuntimeError::Message("pattern match failed".to_string())
                                })?;
                            for (name, value) in bindings {
                                local_env.set(name, value);
                            }
                            Ok(Value::Unit)
                        }
                        other => {
                            let bindings =
                                collect_pattern_bindings(pattern, &other).ok_or_else(|| {
                                    RuntimeError::Message("pattern match failed".to_string())
                                })?;
                            for (name, value) in bindings {
                                local_env.set(name, value);
                            }
                            Ok(Value::Unit)
                        }
                    }
                }
                HirBlockItem::Expr { expr } => {
                    let value = self.eval_expr(expr, &local_env)?;
                    if last {
                        match value {
                            Value::Effect(_) => self.run_effect_value(value),
                            _ => Err(RuntimeError::Message(
                                "final expression in effect block must be Effect".to_string(),
                            )),
                        }
                    } else {
                        match value {
                            Value::Effect(_) => {
                                let _ = self.run_effect_value(value)?;
                                Ok(Value::Unit)
                            }
                            _ => Err(RuntimeError::Message(
                                "expression in effect block must be Effect".to_string(),
                            )),
                        }
                    }
                }
                HirBlockItem::Filter { .. }
                | HirBlockItem::Yield { .. }
                | HirBlockItem::Recurse { .. } => Err(RuntimeError::Message(
                    "unsupported block item in effect block".to_string(),
                )),
            };
            match step {
                Ok(value) => {
                    if last {
                        result = Ok(value);
                    }
                }
                Err(err) => {
                    result = Err(err);
                    break;
                }
            }
        }

        let cleanup_result = self.run_cleanups(cleanups);
        match (result, cleanup_result) {
            (Err(err), _) => Err(err),
            (Ok(_), Err(err)) => Err(err),
            (Ok(value), Ok(())) => Ok(value),
        }
    }
}
