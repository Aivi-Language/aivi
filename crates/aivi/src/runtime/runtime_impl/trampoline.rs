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
///
/// Variants are added incrementally as recursion sites are converted.
#[allow(dead_code)]
enum Frame {
    /// Placeholder — removed once real variants exist.
    _Placeholder,
}

impl Runtime {
    /// Convert an `apply` call into a `Step` without recursing.
    ///
    /// For `Closure` values, this produces `Step::Eval` of the body —
    /// thereby breaking the `apply → eval_expr → apply` recursion chain.
    ///
    /// All other function-like values (`Builtin`, `MultiClause`, `Constructor`)
    /// are evaluated eagerly and produce `Step::Return`.
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

    /// Drive evaluation to completion using an explicit stack.
    ///
    /// The trampoline replaces Rust call-stack recursion for the
    /// `eval_expr → apply → eval_expr` chain.  Each sub-evaluation that
    /// would previously recurse now pushes a `Frame` and returns the next
    /// `Step`, which the loop processes iteratively.
    fn trampoline(&mut self, initial: Step) -> Result<Value, RuntimeError> {
        let mut step = initial;
        let mut stack: Vec<Frame> = Vec::new();

        loop {
            step = match step {
                Step::Eval { ref expr, ref env } => {
                    // Cancellation/fuel check happens inside eval_expr already.
                    let value = self.eval_expr(expr, env)?;
                    self.resume_stack(&mut stack, value)?
                }
                Step::Apply { func, arg } => {
                    self.check_cancelled()?;
                    // Use apply_step to avoid recursion at the closure boundary.
                    let next = self.apply_step(func, arg)?;
                    // apply_step returns either Step::Eval (for closures) or
                    // Step::Return (for builtins/constructors). When it returns
                    // Step::Eval, the loop continues without growing the Rust
                    // stack — this is where the trampoline effect happens.
                    match next {
                        Step::Return(value) => self.resume_stack(&mut stack, value)?,
                        other => other,
                    }
                }
                Step::RunEffectValue { value } => {
                    // Still delegates to recursive run_effect_value.
                    let value = self.run_effect_value(value)?;
                    self.resume_stack(&mut stack, value)?
                }
                Step::Return(value) => {
                    return Ok(value);
                }
            };
        }
    }

    /// Pop the top frame and produce the next `Step` based on the completed
    /// sub-evaluation result.
    ///
    /// When no frames remain, produces `Step::Return`.
    fn resume_stack(
        &mut self,
        stack: &mut Vec<Frame>,
        value: Value,
    ) -> Result<Step, RuntimeError> {
        let Some(frame) = stack.pop() else {
            return Ok(Step::Return(value));
        };
        match frame {
            Frame::_Placeholder => {
                unreachable!("_Placeholder frame should never be pushed")
            }
        }
    }
}
