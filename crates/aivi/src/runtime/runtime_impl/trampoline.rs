/// Trampoline types and evaluation loop.
///
/// The interpreter drives evaluation via an explicit `Step`/`Frame` state machine
/// instead of Rust call-stack recursion.  This prevents user programs from
/// overflowing the OS thread stack.

/// What to evaluate next.
enum Step {
    /// Evaluate an expression in the given environment.
    Eval { expr: Arc<HirExpr>, env: Env },
    /// Apply a function value to an argument value.
    Apply { func: Value, arg: Value },
    /// Execute an effect value (force an `Effect` / `Source`).
    RunEffectValue { value: Value },
    /// Evaluation is finished — bubble the value up through the frame stack.
    Return(Value),
}

/// Pending work saved on the explicit stack while a sub-evaluation runs.
enum Frame {
    /// Stepping through a plain `do { ... }` block.
    PlainBlockStep {
        items: Arc<Vec<HirBlockItem>>,
        index: usize,
        env: Env,
    },

    /// Just evaluated a bind expression in a plain block; need to apply bindings.
    PlainBlockBindReady {
        pattern: HirPattern,
        items: Arc<Vec<HirBlockItem>>,
        index: usize,
        env: Env,
    },

    /// Stepping through a `do Effect { ... }` block.
    #[allow(dead_code)]
    EffectBlockStep {
        items: Arc<Vec<HirBlockItem>>,
        index: usize,
        env: Env,
        cleanups: Vec<Value>,
    },

    /// Evaluated the expression for a bind in an effect block; decide how to handle it.
    EffectBlockBindExprReady {
        pattern: HirPattern,
        items: Arc<Vec<HirBlockItem>>,
        index: usize,
        env: Env,
        cleanups: Vec<Value>,
    },

    /// Forced an Effect/Source bind value; need to apply pattern bindings.
    EffectBlockBindForced {
        pattern: HirPattern,
        items: Arc<Vec<HirBlockItem>>,
        index: usize,
        env: Env,
        cleanups: Vec<Value>,
    },

    /// Evaluated an expression-item in an effect block; decide how to handle it.
    EffectBlockExprReady {
        items: Arc<Vec<HirBlockItem>>,
        index: usize,
        env: Env,
        cleanups: Vec<Value>,
        is_last: bool,
    },

    /// A non-last effect expression was forced; discard value and continue stepping.
    EffectBlockExprForced {
        items: Arc<Vec<HirBlockItem>>,
        index: usize,
        env: Env,
        cleanups: Vec<Value>,
    },

    /// Effect block finished with a result; need to run cleanups.
    EffectBlockCleanup {
        cleanups: Vec<Value>,
    },
}

impl Runtime {
    /// Convert an `apply` call into a `Step` without recursing.
    fn apply_step(&mut self, func: Value, arg: Value) -> Result<Step, RuntimeError> {
        let func = self.force_value(func)?;
        match func {
            Value::Closure(closure) => {
                let new_env = Env::new(Some(closure.env.clone()));
                new_env.set(closure.param.clone(), arg);
                Ok(Step::Eval {
                    expr: closure.body.clone(),
                    env: new_env,
                })
            }
            Value::Builtin(builtin) => {
                let value = builtin.apply(arg, self)?;
                Ok(Step::Return(value))
            }
            Value::MultiClause(clauses) => {
                let value = self.apply_multi_clause(clauses, arg)?;
                Ok(Step::Return(value))
            }
            Value::Constructor { name, mut args } => {
                args.push(arg);
                Ok(Step::Return(Value::Constructor { name, args }))
            }
            other => Err(RuntimeError::Message(format!(
                "attempted to call a non-function: {}",
                format_value(&other)
            ))),
        }
    }

    /// Evaluate an expression, returning `Step` for tail positions.
    ///
    /// Handles `If`, `Match`, and `Block(Plain)` without recursion.
    /// All other expression types delegate to the existing `eval_expr`.
    fn eval_expr_step(
        &mut self,
        expr: &HirExpr,
        env: &Env,
        stack: &mut Vec<Frame>,
    ) -> Result<Step, RuntimeError> {
        self.check_cancelled()?;
        match expr {
            HirExpr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                let cond_value = self.eval_expr(cond, env)?;
                let branch = if matches!(cond_value, Value::Bool(true)) {
                    then_branch
                } else {
                    else_branch
                };
                Ok(Step::Eval {
                    expr: Arc::new((**branch).clone()),
                    env: env.clone(),
                })
            }
            HirExpr::Match {
                scrutinee, arms, ..
            } => {
                let value = self.eval_expr(scrutinee, env)?;
                self.eval_match_step(&value, arms, env)
            }
            // Increment 3: plain blocks step through items via frames.
            HirExpr::Block {
                block_kind, items, ..
            } if matches!(block_kind, crate::hir::HirBlockKind::Plain) => {
                if items.is_empty() {
                    return Ok(Step::Return(Value::Unit));
                }
                let items = Arc::new(items.clone());
                let local_env = Env::new(Some(env.clone()));
                // Push continuation frame for first item, then eval it.
                let is_first_last = items.len() == 1;
                self.push_plain_block_continuation(stack, &items, 0, &local_env, is_first_last);
                Ok(self.start_plain_block_item(&items, 0, local_env))
            }
            // App: evaluate func and arg, then delegate apply to the trampoline loop.
            HirExpr::App { func, arg, .. } => {
                let func_value = self.eval_expr(func, env)?;
                let arg_value = self.eval_expr(arg, env)?;
                Ok(Step::Apply {
                    func: func_value,
                    arg: arg_value,
                })
            }
            // Call: apply all args; the last one goes through the trampoline.
            HirExpr::Call { func, args, .. } => {
                let mut func_value = self.eval_expr(func, env)?;
                if args.is_empty() {
                    return Ok(Step::Return(func_value));
                }
                for arg in &args[..args.len() - 1] {
                    let arg_value = self.eval_expr(arg, env)?;
                    func_value = self.apply(func_value, arg_value)?;
                }
                let last_arg = self.eval_expr(args.last().unwrap(), env)?;
                Ok(Step::Apply {
                    func: func_value,
                    arg: last_arg,
                })
            }
            // All other expression types: delegate to existing recursive eval_expr.
            other => {
                let value = self.eval_expr(other, env)?;
                Ok(Step::Return(value))
            }
        }
    }

    /// Begin evaluating a single item in a plain block.
    fn start_plain_block_item(
        &self,
        items: &Arc<Vec<HirBlockItem>>,
        index: usize,
        env: Env,
    ) -> Step {
        match &items[index] {
            HirBlockItem::Bind { expr, .. } | HirBlockItem::Expr { expr } => Step::Eval {
                expr: Arc::new(expr.clone()),
                env,
            },
            _ => {
                // Filter/Yield/Recurse not supported in plain blocks.
                Step::Return(Value::Unit)
            }
        }
    }

    /// Pattern-match and return a `Step` for the matched arm body.
    fn eval_match_step(
        &mut self,
        value: &Value,
        arms: &[HirMatchArm],
        env: &Env,
    ) -> Result<Step, RuntimeError> {
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
                return Ok(Step::Eval {
                    expr: Arc::new(arm.body.clone()),
                    env: arm_env,
                });
            }
        }
        Err(RuntimeError::Message("non-exhaustive match".to_string()))
    }

    /// Drive evaluation to completion using an explicit stack.
    fn trampoline(&mut self, initial: Step) -> Result<Value, RuntimeError> {
        let mut step = initial;
        let mut stack: Vec<Frame> = Vec::new();

        loop {
            let next = self.trampoline_step(&step, &mut stack);
            match next {
                Ok(next_step) => {
                    step = next_step;
                    if let Step::Return(_) = step {
                        if stack.is_empty() {
                            // Take value out of step to avoid clone.
                            let Step::Return(value) = step else {
                                unreachable!()
                            };
                            return Ok(value);
                        }
                        // Continue through resume_stack on the next iteration.
                    }
                }
                Err(err) => {
                    // Before propagating the error, run any pending cleanups
                    // from effect block frames.
                    self.drain_cleanups_on_error(&mut stack);
                    return Err(err);
                }
            }
        }
    }

    /// Execute one step of the trampoline loop.
    fn trampoline_step(
        &mut self,
        step: &Step,
        stack: &mut Vec<Frame>,
    ) -> Result<Step, RuntimeError> {
        match step {
            Step::Eval { ref expr, ref env } => {
                let next = self.eval_expr_step(expr, env, stack)?;
                match next {
                    Step::Return(value) => self.resume_stack(stack, value),
                    other => Ok(other),
                }
            }
            Step::Apply { ref func, ref arg } => {
                self.check_cancelled()?;
                let next = self.apply_step(func.clone(), arg.clone())?;
                match next {
                    Step::Return(value) => self.resume_stack(stack, value),
                    other => Ok(other),
                }
            }
            Step::RunEffectValue { ref value } => {
                self.check_cancelled()?;
                // Increment 4: unpack EffectValue::Block into frame-based stepping.
                let effect: &EffectValue = match value {
                    Value::Effect(e) => e.as_ref(),
                    Value::Source(s) => s.effect.as_ref(),
                    _ => {
                        return Err(RuntimeError::Message(format!(
                            "expected Effect, got {}",
                            format_value(value)
                        )));
                    }
                };
                match effect {
                    EffectValue::Block { env, items } => {
                        let local_env = Env::new(Some(env.clone()));
                        if items.is_empty() {
                            self.resume_stack(stack, Value::Unit)
                        } else {
                            self.start_effect_block_item(
                                stack, items, 0, local_env, Vec::new(),
                            )
                        }
                    }
                    EffectValue::Thunk { func } => {
                        // Thunks are Rust closures — execute eagerly.
                        let result = func(self)?;
                        self.resume_stack(stack, result)
                    }
                }
            }
            Step::Return(ref value) => {
                // This is reached when step is Return but stack is non-empty
                // (the trampoline loop checks empty stack before calling this).
                self.resume_stack(stack, value.clone())
            }
        }
    }

    /// Run pending cleanups from effect block frames on error.
    ///
    /// When an error propagates through the trampoline, any effect block frames
    /// on the stack may hold cleanup values (from resource acquisitions). These
    /// must be run before the error is returned, mirroring the original
    /// `run_effect_block`'s unconditional cleanup behavior.
    fn drain_cleanups_on_error(&mut self, stack: &mut Vec<Frame>) {
        while let Some(frame) = stack.pop() {
            let cleanups = match frame {
                Frame::EffectBlockStep { cleanups, .. }
                | Frame::EffectBlockBindExprReady { cleanups, .. }
                | Frame::EffectBlockBindForced { cleanups, .. }
                | Frame::EffectBlockExprReady { cleanups, .. }
                | Frame::EffectBlockExprForced { cleanups, .. }
                | Frame::EffectBlockCleanup { cleanups, .. } => cleanups,
                _ => continue,
            };
            if !cleanups.is_empty() {
                let _ = self.run_cleanups(cleanups);
            }
        }
    }

    /// Begin evaluating a single item in an effect block.
    fn start_effect_block_item(
        &self,
        stack: &mut Vec<Frame>,
        items: &Arc<Vec<HirBlockItem>>,
        index: usize,
        env: Env,
        cleanups: Vec<Value>,
    ) -> Result<Step, RuntimeError> {
        let is_last = index + 1 == items.len();
        let trace_effect = std::env::var("AIVI_TRACE_EFFECT").is_ok_and(|v| v == "1");
        if trace_effect {
            eprintln!("[AIVI_TRACE_EFFECT] step {} / {}", index + 1, items.len());
        }

        match &items[index] {
            HirBlockItem::Bind { pattern, expr } => {
                // Push a frame so that when the bind expr is evaluated,
                // we come back to handle pattern matching and effect forcing.
                stack.push(Frame::EffectBlockBindExprReady {
                    pattern: pattern.clone(),
                    items: items.clone(),
                    index,
                    env: env.clone(),
                    cleanups,
                });
                Ok(Step::Eval {
                    expr: Arc::new(expr.clone()),
                    env,
                })
            }
            HirBlockItem::Expr { expr } => {
                stack.push(Frame::EffectBlockExprReady {
                    items: items.clone(),
                    index,
                    env: env.clone(),
                    cleanups,
                    is_last,
                });
                Ok(Step::Eval {
                    expr: Arc::new(expr.clone()),
                    env,
                })
            }
            HirBlockItem::Filter { .. }
            | HirBlockItem::Yield { .. }
            | HirBlockItem::Recurse { .. } => Err(RuntimeError::Message(
                "unsupported block item in effect block".to_string(),
            )),
        }
    }

    /// Advance to the next item in an effect block, or finish with cleanup.
    fn advance_effect_block(
        &self,
        stack: &mut Vec<Frame>,
        items: &Arc<Vec<HirBlockItem>>,
        index: usize,
        env: Env,
        cleanups: Vec<Value>,
    ) -> Result<Step, RuntimeError> {
        let next_index = index + 1;
        if next_index >= items.len() {
            // Block is done — push cleanup frame and return Unit.
            if cleanups.is_empty() {
                Ok(Step::Return(Value::Unit))
            } else {
                stack.push(Frame::EffectBlockCleanup { cleanups });
                Ok(Step::Return(Value::Unit))
            }
        } else {
            self.start_effect_block_item(stack, items, next_index, env, cleanups)
        }
    }

    /// Pop the top frame and produce the next `Step`.
    fn resume_stack(
        &mut self,
        stack: &mut Vec<Frame>,
        value: Value,
    ) -> Result<Step, RuntimeError> {
        let Some(frame) = stack.pop() else {
            return Ok(Step::Return(value));
        };
        match frame {
            // === Plain block frames (Increment 3) ===

            Frame::PlainBlockStep {
                items,
                index,
                env,
            } => {
                // The expression at `index` evaluated to `value`.
                let next_index = index + 1;
                if next_index >= items.len() {
                    // Last item — return the expression's value.
                    Ok(Step::Return(value))
                } else {
                    // Not last — discard value, continue to next item.
                    let is_next_last = next_index + 1 == items.len();
                    self.push_plain_block_continuation(
                        stack, &items, next_index, &env, is_next_last,
                    );
                    Ok(self.start_plain_block_item(&items, next_index, env))
                }
            }

            Frame::PlainBlockBindReady {
                pattern,
                items,
                index,
                env,
            } => {
                // Bind: apply pattern bindings, then continue to next item.
                let bindings = collect_pattern_bindings(&pattern, &value)
                    .ok_or_else(|| RuntimeError::Message("pattern match failed".to_string()))?;
                for (name, val) in bindings {
                    env.set(name, val);
                }
                let next_index = index + 1;
                if next_index >= items.len() {
                    // Last item was a bind → block returns Unit.
                    Ok(Step::Return(Value::Unit))
                } else {
                    let is_next_last = next_index + 1 == items.len();
                    self.push_plain_block_continuation(
                        stack, &items, next_index, &env, is_next_last,
                    );
                    Ok(self.start_plain_block_item(&items, next_index, env))
                }
            }

            // === Effect block frames (Increment 4) ===

            Frame::EffectBlockStep {
                items,
                index,
                env,
                cleanups,
            } => {
                // Previous non-last item was handled; continue.
                self.advance_effect_block(stack, &items, index, env, cleanups)
            }

            Frame::EffectBlockBindExprReady {
                pattern,
                items,
                index,
                env,
                cleanups,
            } => {
                // The bind expression has been evaluated to `value`. Decide what to do.
                match value {
                    Value::Resource(resource) => {
                        // Resource acquisition uses the old recursive path.
                        let (res_value, cleanup) =
                            self.acquire_resource(resource, &env)?;
                        let bindings = collect_pattern_bindings(&pattern, &res_value)
                            .ok_or_else(|| {
                                RuntimeError::Message(
                                    "pattern match failed in resource bind".to_string(),
                                )
                            })?;
                        for (name, val) in bindings {
                            env.set(name, val);
                        }
                        let mut cleanups = cleanups;
                        cleanups.push(cleanup);
                        self.advance_effect_block(stack, &items, index, env, cleanups)
                    }
                    Value::Effect(_) | Value::Source(_) => {
                        // Need to force the effect, then bind the result.
                        stack.push(Frame::EffectBlockBindForced {
                            pattern,
                            items,
                            index,
                            env,
                            cleanups,
                        });
                        Ok(Step::RunEffectValue { value })
                    }
                    other => {
                        // Plain value — direct bind.
                        let bindings = collect_pattern_bindings(&pattern, &other)
                            .ok_or_else(|| {
                                RuntimeError::Message("pattern match failed".to_string())
                            })?;
                        for (name, val) in bindings {
                            env.set(name, val);
                        }
                        self.advance_effect_block(stack, &items, index, env, cleanups)
                    }
                }
            }

            Frame::EffectBlockBindForced {
                pattern,
                items,
                index,
                env,
                cleanups,
            } => {
                // Effect was forced; `value` is the result. Bind it.
                let bindings = collect_pattern_bindings(&pattern, &value)
                    .ok_or_else(|| RuntimeError::Message("pattern match failed".to_string()))?;
                for (name, val) in bindings {
                    env.set(name, val);
                }
                self.advance_effect_block(stack, &items, index, env, cleanups)
            }

            Frame::EffectBlockExprReady {
                items,
                index,
                env,
                cleanups,
                is_last,
            } => {
                // Expression item evaluated to `value`.
                if is_last {
                    // Last expression must be an Effect — force it.
                    match value {
                        Value::Effect(_) | Value::Source(_) => {
                            // Push cleanup frame, then force the effect.
                            if !cleanups.is_empty() {
                                stack.push(Frame::EffectBlockCleanup { cleanups });
                            }
                            Ok(Step::RunEffectValue { value })
                        }
                        _ => {
                            // Run cleanups and return error.
                            let _ = self.run_cleanups(cleanups);
                            Err(RuntimeError::Message(
                                "final expression in effect block must be Effect".to_string(),
                            ))
                        }
                    }
                } else {
                    // Non-last expression must be an Effect — force and discard.
                    match value {
                        Value::Effect(_) | Value::Source(_) => {
                            stack.push(Frame::EffectBlockExprForced {
                                items,
                                index,
                                env,
                                cleanups,
                            });
                            Ok(Step::RunEffectValue { value })
                        }
                        _ => {
                            let _ = self.run_cleanups(cleanups);
                            Err(RuntimeError::Message(
                                "expression in effect block must be Effect".to_string(),
                            ))
                        }
                    }
                }
            }

            Frame::EffectBlockExprForced {
                items,
                index,
                env,
                cleanups,
            } => {
                // Non-last effect was forced and discarded. Continue.
                self.advance_effect_block(stack, &items, index, env, cleanups)
            }

            Frame::EffectBlockCleanup { cleanups } => {
                // Block finished (value is the final result). Run cleanups.
                let cleanup_result = self.run_cleanups(cleanups);
                match cleanup_result {
                    Err(err) => Err(err),
                    Ok(()) => Ok(Step::Return(value)),
                }
            }
        }
    }

    /// Push the right continuation frame for the current plain block item.
    fn push_plain_block_continuation(
        &self,
        stack: &mut Vec<Frame>,
        items: &Arc<Vec<HirBlockItem>>,
        index: usize,
        env: &Env,
        _is_last: bool,
    ) {
        match &items[index] {
            HirBlockItem::Bind { pattern, .. } => {
                stack.push(Frame::PlainBlockBindReady {
                    pattern: pattern.clone(),
                    items: items.clone(),
                    index,
                    env: env.clone(),
                });
            }
            HirBlockItem::Expr { .. } => {
                stack.push(Frame::PlainBlockStep {
                    items: items.clone(),
                    index,
                    env: env.clone(),
                });
            }
            _ => {
                // Filter/Yield/Recurse — shouldn't happen in plain blocks.
                stack.push(Frame::PlainBlockStep {
                    items: items.clone(),
                    index,
                    env: env.clone(),
                });
            }
        }
    }
}
