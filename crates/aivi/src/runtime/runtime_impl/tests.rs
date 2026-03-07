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

    fn test_builtin(
        name: &str,
        arity: usize,
        func: impl Fn(Vec<Value>, &mut Runtime) -> Result<Value, RuntimeError> + Send + Sync + 'static,
    ) -> Value {
        Value::Builtin(BuiltinValue {
            imp: Arc::new(BuiltinImpl {
                name: name.to_string(),
                arity,
                func: Arc::new(func),
            }),
            args: Vec::new(),
            tagged_args: Some(Vec::new()),
        })
    }

    fn ok<T>(result: Result<T, RuntimeError>, context: &str) -> T {
        result.unwrap_or_else(|err| panic!("{context}: {}", format_runtime_error(err)))
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

    fn gtk4_field(runtime: &mut Runtime, name: &str) -> Value {
        let gtk4 = runtime
            .ctx
            .globals
            .get("gtk4")
            .expect("gtk4 builtin record should exist");
        let gtk4 = ok(runtime.force_value(gtk4), "force gtk4 record");
        let Value::Record(fields) = gtk4 else {
            panic!("gtk4 should be a record");
        };
        fields.get(name).cloned().expect("gtk4 field should exist")
    }

    fn apply2(runtime: &mut Runtime, func: Value, left: Value, right: Value) -> Value {
        let with_left = ok(runtime.apply(func, left), "first application");
        ok(runtime.apply(with_left, right), "second application")
    }

    fn run_effect(runtime: &mut Runtime, effect: Value) {
        ok(
            runtime.run_effect_value(effect),
            "reactive host bookkeeping effect should succeed",
        );
    }

    fn text_field(record: Value, field: &str) -> String {
        let Value::Record(fields) = record else {
            panic!("expected record model");
        };
        let value = fields
            .get(field)
            .cloned()
            .unwrap_or_else(|| panic!("missing field `{field}`"));
        let Value::Text(text) = value else {
            panic!("expected `{field}` to be Text");
        };
        text
    }

    fn make_model(query: &str, other: i64) -> Value {
        Value::Record(Arc::new(HashMap::from([
            ("query".to_string(), Value::Text(query.to_string())),
            ("other".to_string(), Value::Int(other)),
        ])))
    }

    #[test]
    fn computed_reuses_cache_until_a_dep_changes() {
        let mut runtime = test_runtime();
        let computed = gtk4_field(&mut runtime, "computed");
        let reactive_init = gtk4_field(&mut runtime, "reactiveInit");
        let reactive_commit = gtk4_field(&mut runtime, "reactiveCommit");
        let reads = Arc::new(Mutex::new(0usize));

        let derive = test_builtin("reactive.test.query", 1, {
            let reads = reads.clone();
            move |mut args: Vec<Value>, runtime: &mut Runtime| {
                runtime.reactive_note_root_field_access("query");
                *reads.lock().expect("read counter") += 1;
                Ok(Value::Text(text_field(args.remove(0), "query")))
            }
        });

        let signal = apply2(
            &mut runtime,
            computed,
            Value::Text("tests.query".to_string()),
            derive,
        );
        let model1 = make_model("alpha", 1);
        let model2 = make_model("alpha", 2);
        let model3 = make_model("beta", 2);

        let init_effect = ok(
            runtime.apply(reactive_init, model1.clone()),
            "reactiveInit application",
        );
        run_effect(&mut runtime, init_effect);

        let first = ok(runtime.apply(signal.clone(), model1.clone()), "first signal read");
        let second = ok(runtime.apply(signal.clone(), model1.clone()), "second signal read");

        assert!(matches!(first, Value::Text(ref text) if text == "alpha"));
        assert!(matches!(second, Value::Text(ref text) if text == "alpha"));
        assert_eq!(*reads.lock().expect("read counter"), 1);

        let commit_effect = apply2(
            &mut runtime,
            reactive_commit.clone(),
            model1.clone(),
            model2.clone(),
        );
        run_effect(&mut runtime, commit_effect);
        let unchanged_dep = ok(
            runtime.apply(signal.clone(), model2.clone()),
            "signal read after unrelated change",
        );
        assert!(matches!(unchanged_dep, Value::Text(ref text) if text == "alpha"));
        assert_eq!(*reads.lock().expect("read counter"), 1);

        let commit_effect = apply2(
            &mut runtime,
            reactive_commit,
            model2.clone(),
            model3.clone(),
        );
        run_effect(&mut runtime, commit_effect);
        let changed_dep = ok(
            runtime.apply(signal, model3),
            "signal read after dependency change",
        );
        assert!(matches!(changed_dep, Value::Text(ref text) if text == "beta"));
        assert_eq!(*reads.lock().expect("read counter"), 2);
    }

    #[test]
    fn serialize_attr_reads_computed_against_current_model() {
        let mut runtime = test_runtime();
        let computed = gtk4_field(&mut runtime, "computed");
        let reactive_init = gtk4_field(&mut runtime, "reactiveInit");
        let reactive_commit = gtk4_field(&mut runtime, "reactiveCommit");
        let serialize_attr = gtk4_field(&mut runtime, "serializeAttr");
        let reads = Arc::new(Mutex::new(0usize));

        let derive = test_builtin("reactive.test.attr", 1, {
            let reads = reads.clone();
            move |mut args: Vec<Value>, runtime: &mut Runtime| {
                runtime.reactive_note_root_field_access("query");
                *reads.lock().expect("read counter") += 1;
                Ok(Value::Text(text_field(args.remove(0), "query")))
            }
        });

        let signal = apply2(
            &mut runtime,
            computed,
            Value::Text("tests.attr".to_string()),
            derive,
        );
        let model1 = make_model("alpha", 1);
        let model2 = make_model("beta", 1);

        let init_effect = ok(
            runtime.apply(reactive_init, model1.clone()),
            "reactiveInit application",
        );
        run_effect(&mut runtime, init_effect);

        let first = ok(
            runtime.apply(serialize_attr.clone(), signal.clone()),
            "first attr serialization",
        );
        let second = ok(
            runtime.apply(serialize_attr.clone(), signal.clone()),
            "second attr serialization",
        );
        assert!(matches!(first, Value::Text(ref text) if text == "alpha"));
        assert!(matches!(second, Value::Text(ref text) if text == "alpha"));
        assert_eq!(*reads.lock().expect("read counter"), 1);

        let commit_effect = apply2(&mut runtime, reactive_commit, model1, model2);
        run_effect(&mut runtime, commit_effect);
        let updated = ok(runtime.apply(serialize_attr, signal), "updated attr serialization");
        assert!(matches!(updated, Value::Text(ref text) if text == "beta"));
        assert_eq!(*reads.lock().expect("read counter"), 2);
    }

    #[test]
    fn each_items_reads_signal_lists_against_current_model() {
        let mut runtime = test_runtime();
        let signal = gtk4_field(&mut runtime, "signal");
        let reactive_init = gtk4_field(&mut runtime, "reactiveInit");
        let each_items = gtk4_field(&mut runtime, "eachItems");

        let rows_derive = test_builtin("reactive.test.rows", 1, {
            move |mut args: Vec<Value>, runtime: &mut Runtime| {
                runtime.reactive_note_root_field_access("query");
                let query = text_field(args.remove(0), "query");
                Ok(Value::List(Arc::new(vec![
                    Value::Text(query),
                    Value::Text("tail".to_string()),
                ])))
            }
        });
        let rows_signal = ok(runtime.apply(signal, rows_derive), "signal creation");
        let template = test_builtin("reactive.test.rowTemplate", 1, |mut args, _| {
            Ok(Value::Constructor {
                name: "GtkTextNode".to_string(),
                args: vec![args.remove(0)],
            })
        });

        let model = make_model("alpha", 1);
        let init_effect = ok(
            runtime.apply(reactive_init, model),
            "reactiveInit application",
        );
        run_effect(&mut runtime, init_effect);

        let rows = apply2(&mut runtime, each_items, rows_signal, template);
        let Value::List(rows) = rows else {
            panic!("expected gtk4.eachItems to return a List");
        };
        assert_eq!(rows.len(), 2);
        assert!(matches!(
            &rows[0],
            Value::Constructor { name, args }
                if name == "GtkTextNode"
                    && matches!(args.first(), Some(Value::Text(text)) if text == "alpha")
        ));
        assert!(matches!(
            &rows[1],
            Value::Constructor { name, args }
                if name == "GtkTextNode"
                    && matches!(args.first(), Some(Value::Text(text)) if text == "tail")
        ));
    }

    #[test]
    fn computed_tracks_nested_signal_dependencies() {
        let mut runtime = test_runtime();
        let computed = gtk4_field(&mut runtime, "computed");
        let reactive_init = gtk4_field(&mut runtime, "reactiveInit");
        let reactive_commit = gtk4_field(&mut runtime, "reactiveCommit");
        let child_reads = Arc::new(Mutex::new(0usize));
        let parent_reads = Arc::new(Mutex::new(0usize));

        let child_derive = test_builtin("reactive.test.child", 1, {
            let child_reads = child_reads.clone();
            move |mut args: Vec<Value>, runtime: &mut Runtime| {
                runtime.reactive_note_root_field_access("query");
                *child_reads.lock().expect("child read counter") += 1;
                Ok(Value::Text(text_field(args.remove(0), "query")))
            }
        });
        let child = apply2(
            &mut runtime,
            computed.clone(),
            Value::Text("tests.child".to_string()),
            child_derive,
        );

        let parent_derive = test_builtin("reactive.test.parent", 1, {
            let child = child.clone();
            let parent_reads = parent_reads.clone();
            move |mut args: Vec<Value>, runtime: &mut Runtime| {
                *parent_reads.lock().expect("parent read counter") += 1;
                let model = args.remove(0);
                let child_value = runtime.apply(child.clone(), model)?;
                let Value::Text(text) = child_value else {
                    panic!("expected nested computed value to be Text");
                };
                Ok(Value::Text(format!("summary:{text}")))
            }
        });
        let parent = apply2(
            &mut runtime,
            computed,
            Value::Text("tests.parent".to_string()),
            parent_derive,
        );

        let model1 = make_model("alpha", 1);
        let model2 = make_model("alpha", 2);
        let model3 = make_model("beta", 2);

        let init_effect = ok(
            runtime.apply(reactive_init, model1.clone()),
            "reactiveInit application",
        );
        run_effect(&mut runtime, init_effect);

        let first = ok(runtime.apply(parent.clone(), model1.clone()), "first parent read");
        let second = ok(runtime.apply(parent.clone(), model1.clone()), "second parent read");
        assert!(matches!(first, Value::Text(ref text) if text == "summary:alpha"));
        assert!(matches!(second, Value::Text(ref text) if text == "summary:alpha"));
        assert_eq!(*child_reads.lock().expect("child reads"), 1);
        assert_eq!(*parent_reads.lock().expect("parent reads"), 1);

        let commit_effect = apply2(
            &mut runtime,
            reactive_commit.clone(),
            model1.clone(),
            model2.clone(),
        );
        run_effect(&mut runtime, commit_effect);
        let unchanged_dep = ok(
            runtime.apply(parent.clone(), model2.clone()),
            "parent read after unrelated change",
        );
        assert!(matches!(unchanged_dep, Value::Text(ref text) if text == "summary:alpha"));
        assert_eq!(*child_reads.lock().expect("child reads"), 1);
        assert_eq!(*parent_reads.lock().expect("parent reads"), 1);

        let commit_effect = apply2(&mut runtime, reactive_commit, model2, model3.clone());
        run_effect(&mut runtime, commit_effect);
        let changed_dep = ok(
            runtime.apply(parent, model3),
            "parent read after nested dependency change",
        );
        assert!(matches!(changed_dep, Value::Text(ref text) if text == "summary:beta"));
        assert_eq!(*child_reads.lock().expect("child reads"), 2);
        assert_eq!(*parent_reads.lock().expect("parent reads"), 2);
    }
}
