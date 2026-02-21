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
        // Explicit work stack: each entry is (start_index, items_vec, env).
        // This replaces the recursive call in the Bind arm.
        let items_vec: Vec<HirBlockItem> = items.to_vec();
        let mut work_stack: Vec<(usize, Vec<HirBlockItem>, Env)> =
            vec![(0, items_vec, Env::new(Some(env.clone())))];

        while let Some((start, work_items, local_env)) = work_stack.pop() {
            let mut aborted = false;
            for idx in start..work_items.len() {
                let item = &work_items[idx];
                match item {
                    HirBlockItem::Yield { expr } => {
                        let value = self.eval_expr(expr, &local_env)?;
                        out.push(value);
                    }
                    HirBlockItem::Bind { pattern, expr } => {
                        let source = self.eval_expr(expr, &local_env)?;
                        let source_items = self.generator_to_list(source)?;
                        let rest: Vec<HirBlockItem> = work_items[idx + 1..].to_vec();
                        // Push work for each source element in reverse so the first
                        // element is processed first (LIFO stack).
                        for val in source_items.into_iter().rev() {
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
                            work_stack.push((0, rest.clone(), bind_env));
                        }
                        aborted = true;
                        break;
                    }
                    HirBlockItem::Filter { expr } => {
                        let cond = self.eval_expr(expr, &local_env)?;
                        if !matches!(cond, Value::Bool(true)) {
                            aborted = true;
                            break;
                        }
                    }
                    HirBlockItem::Expr { expr } => {
                        let sub = self.eval_expr(expr, &local_env)?;
                        let sub_items = self.generator_to_list(sub)?;
                        out.extend(sub_items);
                    }
                    HirBlockItem::Recurse { .. } => {
                        // Unsupported for now
                    }
                }
            }
            if aborted {
                continue;
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

    /// Execute an effect value.
    ///
    /// Delegates to the trampoline so that deeply-nested effect chains
    /// (e.g. recursive `do Effect { ... }`) do not overflow the Rust stack.
    fn run_effect_value(&mut self, value: Value) -> Result<Value, RuntimeError> {
        self.trampoline(Step::RunEffectValue { value })
    }
}
