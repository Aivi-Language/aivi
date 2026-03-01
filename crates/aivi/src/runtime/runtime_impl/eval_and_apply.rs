impl Runtime {
    fn apply_multi_clause(
        &mut self,
        clauses: Vec<Value>,
        arg: Value,
    ) -> Result<Value, RuntimeError> {
        let mut results = Vec::new();
        let mut match_failures = 0;
        let mut last_error = None;
        for clause in clauses.into_iter() {
            let a = arg.clone();
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

    pub(crate) fn generator_to_list(&mut self, gen: Value) -> Result<Vec<Value>, RuntimeError> {
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
            tagged_args: Some(Vec::new()),
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

    /// Execute an effect value directly (no trampoline needed).
    pub(crate) fn run_effect_value(&mut self, value: Value) -> Result<Value, RuntimeError> {
        self.check_cancelled()?;
        match &value {
            Value::Resource(r) => {
                // Run acquire phase and push cleanup onto the resource stack
                let cleanup = r.cleanup.clone();
                let result = (r.acquire)(self)?;
                self.resource_cleanups.push(Some(cleanup));
                Ok(result)
            }
            _ => {
                let effect = match &value {
                    Value::Effect(e) => e.clone(),
                    Value::Source(s) => s.effect.clone(),
                    _ => {
                        return Err(RuntimeError::Message(format!(
                            "expected Effect, got {}",
                            format_value(&value)
                        )))
                    }
                };
                match effect.as_ref() {
                    EffectValue::Thunk { func } => func(self),
                }
            }
        }
    }

    /// Push a scope marker onto the resource cleanup stack.
    /// Called at the start of a do-block.
    pub(crate) fn push_resource_scope(&mut self) {
        self.resource_cleanups.push(None); // None = scope boundary
    }

    /// Run all resource cleanups registered since the last scope marker (LIFO).
    /// Called at the end of a do-block. Cleanup errors are suppressed;
    /// the original result/error takes priority.
    pub(crate) fn pop_resource_scope(&mut self) {
        // Save any pending JIT error so cleanup code doesn't clear it
        // (make_jit_builtin resets jit_pending_error on entry).
        let saved_error = self.jit_pending_error.take();
        while let Some(entry) = self.resource_cleanups.pop() {
            match entry {
                None => break, // reached scope boundary
                Some(cleanup) => {
                    let _ = cleanup(self); // suppress cleanup errors
                    // Discard any pending error from cleanup itself
                    self.jit_pending_error = None;
                }
            }
        }
        // Restore the original pending error
        if saved_error.is_some() {
            self.jit_pending_error = saved_error;
        }
    }
}
