#[cfg(test)]
mod handler_scope_tests {
    use super::*;

    fn test_runtime() -> Runtime {
        let globals = Env::new(None);
        register_builtins(&globals);
        let ctx = Arc::new(RuntimeContext::new_with_constructor_ordinals(
            globals,
            core_constructor_ordinals(),
        ));
        Runtime::new(ctx, CancelToken::root())
    }

    #[test]
    fn cleanup_uses_captured_handler_scope_while_cancelled() {
        let mut runtime = test_runtime();
        let observed = Arc::new(Mutex::new((String::new(), false, false)));

        let mut captured = HashMap::new();
        captured.insert("file.read".to_string(), Value::Text("outer".to_string()));
        let mut current = HashMap::new();
        current.insert("file.read".to_string(), Value::Text("inner".to_string()));

        runtime.push_resource_scope();
        runtime.resource_cleanups.push(ResourceCleanupEntry::Cleanup {
            cleanup: Arc::new({
                let observed = observed.clone();
                move |runtime| {
                    let handler = runtime
                        .resolve_capability_handler("file.read")?
                        .expect("captured handler");
                    let handler_name = match handler {
                        Value::Text(text) => text,
                        other => format_value(&other),
                    };
                    let cancel_ok = runtime.check_cancelled().is_ok();
                    let masked = runtime.cancel_mask > 0;
                    *observed.lock().expect("observe cleanup") = (handler_name, cancel_ok, masked);
                    Ok(Value::Unit)
                }
            }),
            handlers: vec![captured],
        });

        runtime.push_capability_scope(current);
        runtime.cancel.cancel();
        runtime.pop_resource_scope();

        let observed = observed.lock().expect("observed cleanup").clone();
        assert_eq!(observed.0, "outer");
        assert!(observed.1, "cleanup should run with cancellation masked");
        assert!(observed.2, "cleanup should run inside an uncancelable region");
    }
}
