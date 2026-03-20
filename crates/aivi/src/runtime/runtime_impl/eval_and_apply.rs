impl Runtime {
    fn apply_multi_clause(
        &mut self,
        clauses: Vec<Value>,
        arg: Value,
    ) -> Result<Value, RuntimeError> {
        let mut callables = Vec::new();
        let mut match_failures = 0;
        let mut last_error = None;
        let saved_suppress = self.jit_suppress_warnings;
        self.jit_suppress_warnings = true;
        for clause in clauses.into_iter() {
            let a = arg.clone();
            match self.apply(clause, a) {
                Ok(value) => {
                    if is_callable(&value) {
                        // Callable (partial application): collect but keep trying.
                        callables.push(value);
                    } else {
                        // First concrete (non-callable) result wins immediately.
                        // This ensures HKT dispatch is correct: e.g. `filter pred
                        // list` returns the filtered list rather than a partially
                        // applied callable produced during method dispatch.
                        self.jit_suppress_warnings = saved_suppress;
                        return Ok(value);
                    }
                }
                Err(RuntimeError::NonExhaustiveMatch { .. }) => {
                    match_failures += 1;
                }
                Err(RuntimeError::Message(ref message)) if is_match_failure_message(message) => {
                    match_failures += 1;
                }
                Err(err) => {
                    last_error = Some(err);
                }
            }
        }
        self.jit_suppress_warnings = saved_suppress;
        // No concrete result found. If there are callables (partial applications),
        // return them so the caller can continue applying arguments.
        if !callables.is_empty() {
            if callables.len() == 1 {
                return Ok(callables.remove(0));
            }
            return Ok(Value::MultiClause(callables));
        }
        if match_failures > 0 && last_error.is_none() {
            return Err(RuntimeError::NonExhaustiveMatch {
                scrutinee: None,
            });
        }
        Err(last_error.unwrap_or_else(|| RuntimeError::NonExhaustiveMatch {
            scrutinee: None,
        }))
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

    /// Execute an effect value directly using an explicit continuation stack so
    /// long-running effect loops stay stack-safe.
    pub(crate) fn run_effect_value(&mut self, value: Value) -> Result<Value, RuntimeError> {
        enum Continuation {
            Apply(Value),
            PopResourceScope,
        }

        enum Step {
            Eval(Value),
            Return(Value),
        }

        fn unwind_effect_error(
            runtime: &mut Runtime,
            continuations: &mut Vec<Continuation>,
            err: RuntimeError,
        ) -> Result<Value, RuntimeError> {
            while let Some(continuation) = continuations.pop() {
                if matches!(continuation, Continuation::PopResourceScope) {
                    runtime.pop_resource_scope();
                }
            }
            Err(err)
        }

        fn is_effect_value(value: &Value) -> bool {
            matches!(value, Value::Effect(_) | Value::Source(_) | Value::Resource(_))
        }

        let mut step = Step::Eval(value);
        let mut continuations = Vec::new();

        loop {
            self.check_cancelled()?;
            match step {
                Step::Eval(current) => match current {
                    Value::Resource(resource) => {
                        match (resource.acquire)(self) {
                            Ok(Value::Tuple(mut items)) if items.len() == 2 => {
                                let cleanup_fn = items.pop().unwrap();
                                let result = items.pop().unwrap();
                                let cleanup = Arc::new(
                                    move |runtime: &mut crate::runtime::Runtime| {
                                        let cleanup_effect =
                                            runtime.apply(cleanup_fn.clone(), Value::Unit)?;
                                        runtime.run_effect_value(cleanup_effect)
                                    },
                                );
                                self.resource_cleanups
                                    .push(ResourceCleanupEntry::Cleanup { cleanup });
                                step = Step::Return(result);
                            }
                            Ok(other) => {
                                return unwind_effect_error(
                                    self,
                                    &mut continuations,
                                    RuntimeError::TypeError {
                                        context: "resource acquisition".to_string(),
                                        expected:
                                            "Tuple (yielded value, cleanup closure)".to_string(),
                                        got: format_value(&other),
                                    },
                                );
                            }
                            Err(err) => {
                                return unwind_effect_error(self, &mut continuations, err);
                            }
                        }
                    }
                    Value::Effect(effect) => match effect.as_ref() {
                        EffectValue::Thunk { func } => match func(self) {
                            Ok(result) => {
                                step = Step::Return(result);
                            }
                            Err(err) => {
                                return unwind_effect_error(self, &mut continuations, err);
                            }
                        },
                        EffectValue::Bind { effect, func } => {
                            continuations.push(Continuation::Apply(func.clone()));
                            step = Step::Eval(effect.clone());
                        }
                        EffectValue::WithResourceScope { effect } => {
                            self.push_resource_scope();
                            continuations.push(Continuation::PopResourceScope);
                            step = Step::Eval(effect.clone());
                        }
                    },
                    Value::Source(effect_source) => {
                        step = Step::Eval(Value::Effect(effect_source.effect.clone()));
                    }
                    other => {
                        return Err(RuntimeError::TypeError {
                            context: "effect execution".to_string(),
                            expected: "Effect".to_string(),
                            got: format_value(&other),
                        });
                    }
                },
                Step::Return(mut value) => loop {
                    match continuations.pop() {
                        Some(Continuation::PopResourceScope) => {
                            self.pop_resource_scope();
                        }
                        Some(Continuation::Apply(func)) => match self.apply(func, value)? {
                            next if is_effect_value(&next) => {
                                step = Step::Eval(next);
                                break;
                            }
                            next => {
                                value = next;
                            }
                        },
                        None => return Ok(value),
                    }
                }
            };
        }
    }

    /// Push a scope marker onto the resource cleanup stack.
    /// Called at the start of a do-block.
    pub(crate) fn push_resource_scope(&mut self) {
        self.resource_cleanups.push(ResourceCleanupEntry::ScopeBoundary);
    }

    /// Run all resource cleanups registered since the last scope marker (LIFO).
    /// Called at the end of a do-block. Cleanup errors are suppressed;
    /// the original result/error takes priority.
    pub(crate) fn pop_resource_scope(&mut self) {
        // Save any pending JIT error so cleanup code doesn't clear it
        // (make_jit_builtin resets jit_pending_error on entry).
        let saved_error = self.jit_pending_error.take();
        let saved_snapshot = self.jit_pending_snapshot.take();
        while let Some(entry) = self.resource_cleanups.pop() {
            match entry {
                ResourceCleanupEntry::ScopeBoundary => break,
                ResourceCleanupEntry::Cleanup { cleanup } => {
                    self.uncancelable(|runtime| {
                        let _ = cleanup(runtime);
                        runtime.clear_pending_runtime_error();
                    });
                }
            }
        }
        // Restore the original pending error
        if saved_error.is_some() {
            self.jit_pending_error = saved_error;
            self.jit_pending_snapshot = saved_snapshot;
        }
    }
}
