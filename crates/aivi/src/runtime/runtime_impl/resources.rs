impl Runtime {
    fn acquire_resource(
        &mut self,
        resource: Arc<ResourceValue>,
        env: &Env,
    ) -> Result<(Value, Value), RuntimeError> {
        let local_env = Env::new(Some(env.clone()));
        let items = resource.items.as_ref();
        let mut yielded = None;
        let mut cleanup_start = None;

        for (index, item) in items.iter().enumerate() {
            self.check_cancelled()?;
            match item {
                HirBlockItem::Bind { pattern, expr } => {
                    let value = self.eval_expr(expr, &local_env)?;
                    match value {
                        Value::Effect(_) => {
                            let value = self.run_effect_value(value)?;
                            let bindings =
                                collect_pattern_bindings(pattern, &value).ok_or_else(|| {
                                    RuntimeError::Message("pattern match failed".to_string())
                                })?;
                            for (name, value) in bindings {
                                local_env.set(name, value);
                            }
                        }
                        _ => {
                            return Err(RuntimeError::Message(
                                "resource bind expects Effect".to_string(),
                            ))
                        }
                    }
                }
                HirBlockItem::Yield { expr } => {
                    let value = self.eval_expr(expr, &local_env)?;
                    yielded = Some(value);
                    cleanup_start = Some(index + 1);
                    break;
                }
                HirBlockItem::Expr { expr } => {
                    let value = self.eval_expr(expr, &local_env)?;
                    if let Value::Effect(_) = value {
                        let _ = self.run_effect_value(value)?;
                    }
                }
                HirBlockItem::Filter { .. } | HirBlockItem::Recurse { .. } => {
                    return Err(RuntimeError::Message(
                        "unsupported block item in resource block".to_string(),
                    ));
                }
            }
        }

        let value = yielded
            .ok_or_else(|| RuntimeError::Message("resource block missing yield".to_string()))?;
        let cleanup_items = if let Some(start) = cleanup_start {
            items[start..].to_vec()
        } else {
            Vec::new()
        };
        let cleanup_effect = Value::Effect(Arc::new(EffectValue::Block {
            env: local_env,
            items: Arc::new(cleanup_items),
        }));
        Ok((value, cleanup_effect))
    }

    fn run_cleanups(&mut self, cleanups: Vec<Value>) -> Result<(), RuntimeError> {
        for cleanup in cleanups.into_iter().rev() {
            let _ = self.uncancelable(|runtime| runtime.run_effect_value(cleanup))?;
        }
        Ok(())
    }
}
