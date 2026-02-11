use std::sync::mpsc;
use std::time::Duration;

use rudo_gc::GcMutex;

use super::*;

#[test]
fn cleanups_run_even_when_cancelled() {
    let globals = Env::new(None);
    register_builtins(&globals);
    let ctx = Arc::new(RuntimeContext { globals });
    let cancel = CancelToken::root();
    let mut runtime = Runtime::new(ctx, cancel.clone());

    let ran = Arc::new(AtomicBool::new(false));
    let ran_clone = ran.clone();
    let cleanup = Value::Effect(Arc::new(EffectValue::Thunk {
        func: Arc::new(move |_| {
            ran_clone.store(true, Ordering::SeqCst);
            Ok(Value::Unit)
        }),
    }));

    cancel.cancel();
    assert!(runtime.run_cleanups(vec![cleanup]).is_ok());
    assert!(ran.load(Ordering::SeqCst));
}

#[test]
fn text_interpolation_evaluates() {
    let source = r#"
module test.interpolation = {
  s = "Count: {1 + 2}"
  n = -1
  t = "negative{n}"
  u = "brace \{x\}"
}
"#;

    let (modules, diags) = crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");

    let program = crate::hir::desugar_modules(&modules);
    let module = program.modules.into_iter().next().expect("expected module");

    let globals = Env::new(None);
    register_builtins(&globals);
    assert!(globals.get("println").is_some());

    let mut grouped: HashMap<String, Vec<HirExpr>> = HashMap::new();
    for def in module.defs {
        grouped.entry(def.name).or_default().push(def.expr);
    }
    for (name, exprs) in grouped {
        if exprs.len() == 1 {
            let thunk = ThunkValue {
                expr: Arc::new(exprs.into_iter().next().unwrap()),
                env: globals.clone(),
                cached: GcMutex::new(None),
                in_progress: AtomicBool::new(false),
            };
            globals.set(name, Value::Thunk(Arc::new(thunk)));
        } else {
            let mut clauses = Vec::new();
            for expr in exprs {
                let thunk = ThunkValue {
                    expr: Arc::new(expr),
                    env: globals.clone(),
                    cached: GcMutex::new(None),
                    in_progress: AtomicBool::new(false),
                };
                clauses.push(Value::Thunk(Arc::new(thunk)));
            }
            globals.set(name, Value::MultiClause(clauses));
        }
    }

    let ctx = Arc::new(RuntimeContext { globals });
    let cancel = CancelToken::root();
    let mut runtime = Runtime::new(ctx, cancel);

    let s = runtime.ctx.globals.get("s").unwrap();
    let t = runtime.ctx.globals.get("t").unwrap();
    let u = runtime.ctx.globals.get("u").unwrap();

    let s = match runtime.force_value(s) {
        Ok(Value::Text(value)) => value,
        Ok(_) => panic!("expected Text for s"),
        Err(_) => panic!("failed to evaluate s"),
    };
    let t = match runtime.force_value(t) {
        Ok(Value::Text(value)) => value,
        Ok(_) => panic!("expected Text for t"),
        Err(_) => panic!("failed to evaluate t"),
    };
    let u = match runtime.force_value(u) {
        Ok(Value::Text(value)) => value,
        Ok(_) => panic!("expected Text for u"),
        Err(_) => panic!("failed to evaluate u"),
    };

    assert_eq!(s, "Count: 3");
    assert_eq!(t, "negative-1");
    assert_eq!(u, "brace {x}");
}

#[test]
fn concurrent_par_observes_parent_cancellation() {
    let globals = Env::new(None);
    register_builtins(&globals);
    let ctx = Arc::new(RuntimeContext { globals });
    let cancel = CancelToken::root();

    let (started_left_tx, started_left_rx) = mpsc::channel();
    let (started_right_tx, started_right_rx) = mpsc::channel();

    let left = Value::Effect(Arc::new(EffectValue::Thunk {
        func: Arc::new(move |runtime| {
            let _ = started_left_tx.send(());
            loop {
                runtime.check_cancelled()?;
                std::hint::spin_loop();
            }
        }),
    }));
    let right = Value::Effect(Arc::new(EffectValue::Thunk {
        func: Arc::new(move |runtime| {
            let _ = started_right_tx.send(());
            loop {
                runtime.check_cancelled()?;
                std::hint::spin_loop();
            }
        }),
    }));

    let (result_tx, result_rx) = mpsc::channel();
    let ctx_clone = ctx.clone();
    let cancel_clone = cancel.clone();
    std::thread::spawn(move || {
        let mut runtime = Runtime::new(ctx_clone, cancel_clone);
        let concurrent = super::builtins::build_concurrent_record();
        let Value::Record(fields) = concurrent else {
            panic!("expected concurrent record");
        };
        let par = fields.get("par").expect("par").clone();
        let applied = match runtime.apply(par, left) {
            Ok(value) => value,
            Err(_) => panic!("apply left failed"),
        };
        let applied = match runtime.apply(applied, right) {
            Ok(value) => value,
            Err(_) => panic!("apply right failed"),
        };
        let result = runtime.run_effect_value(applied);
        let _ = result_tx.send(result);
    });

    started_left_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("left started");
    started_right_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("right started");

    cancel.cancel();

    let result = result_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("par returned");
    assert!(matches!(result, Err(RuntimeError::Cancelled)));
}

#[test]
fn text_bytes_roundtrip() {
    let globals = Env::new(None);
    register_builtins(&globals);
    let ctx = Arc::new(RuntimeContext { globals });
    let cancel = CancelToken::root();
    let mut runtime = Runtime::new(ctx, cancel);

    let text_record = runtime.ctx.globals.get("text").expect("text record");
    let Value::Record(fields) = text_record else {
        panic!("text record missing");
    };

    let length = fields.get("length").expect("length").clone();
    let len_value = runtime
        .apply(length, Value::Text("hello".to_string()))
        .unwrap_or_else(|_| panic!("length applied"));
    assert!(matches!(len_value, Value::Int(5)));

    let to_bytes = fields.get("toBytes").expect("toBytes").clone();
    let utf8 = Value::Constructor {
        name: "Utf8".to_string(),
        args: Vec::new(),
    };
    let to_bytes = runtime
        .apply(to_bytes, utf8)
        .unwrap_or_else(|_| panic!("toBytes arg1"));
    let bytes = runtime
        .apply(to_bytes, Value::Text("ping".to_string()))
        .unwrap_or_else(|_| panic!("toBytes arg2"));
    let Value::Bytes(bytes) = bytes else {
        panic!("expected Bytes");
    };
    assert_eq!(bytes.as_ref(), b"ping");

    let from_bytes = fields.get("fromBytes").expect("fromBytes").clone();
    let utf8 = Value::Constructor {
        name: "Utf8".to_string(),
        args: Vec::new(),
    };
    let from_bytes = runtime
        .apply(from_bytes, utf8)
        .unwrap_or_else(|_| panic!("fromBytes arg1"));
    let decoded = runtime
        .apply(from_bytes, Value::Bytes(bytes))
        .unwrap_or_else(|_| panic!("fromBytes arg2"));
    let Value::Constructor { name, args } = decoded else {
        panic!("expected Result constructor");
    };
    assert_eq!(name, "Ok");
    assert_eq!(args.len(), 1);
    assert!(matches!(args[0], Value::Text(ref value) if value == "ping"));
}

#[test]
fn https_rejects_non_https_urls() {
    let globals = Env::new(None);
    register_builtins(&globals);
    let ctx = Arc::new(RuntimeContext { globals });
    let cancel = CancelToken::root();
    let mut runtime = Runtime::new(ctx, cancel);

    let https_record = runtime.ctx.globals.get("https").expect("https record");
    let Value::Record(fields) = https_record else {
        panic!("https record missing");
    };
    let get = fields.get("get").expect("get").clone();

    let mut url_fields = HashMap::new();
    url_fields.insert("protocol".to_string(), Value::Text("http".to_string()));
    url_fields.insert("host".to_string(), Value::Text("example.com".to_string()));
    url_fields.insert(
        "port".to_string(),
        Value::Constructor {
            name: "None".to_string(),
            args: Vec::new(),
        },
    );
    url_fields.insert("path".to_string(), Value::Text("/".to_string()));
    url_fields.insert("query".to_string(), Value::List(Arc::new(Vec::new())));
    url_fields.insert(
        "hash".to_string(),
        Value::Constructor {
            name: "None".to_string(),
            args: Vec::new(),
        },
    );
    let url = Value::Record(Arc::new(url_fields));

    let applied = match runtime.apply(get, url) {
        Ok(value) => value,
        Err(_) => panic!("apply get failed"),
    };
    let result = runtime.run_effect_value(applied);

    match result {
        Err(RuntimeError::Message(message)) => {
            assert!(message.contains("https"), "unexpected error: {message}");
        }
        _ => panic!("expected https error"),
    }
}

#[test]
fn regex_compile_and_match() {
    let globals = Env::new(None);
    register_builtins(&globals);
    let ctx = Arc::new(RuntimeContext { globals });
    let cancel = CancelToken::root();
    let mut runtime = Runtime::new(ctx, cancel);

    let regex_record = runtime.ctx.globals.get("regex").expect("regex record");
    let Value::Record(fields) = regex_record else {
        panic!("regex record missing");
    };

    let compile = fields.get("compile").expect("compile").clone();
    let compiled = runtime
        .apply(compile, Value::Text("[a-z]+".to_string()))
        .unwrap_or_else(|_| panic!("compile applied"));
    let Value::Constructor { name, args } = compiled else {
        panic!("expected Result");
    };
    assert_eq!(name, "Ok");
    let regex = args[0].clone();

    let test = fields.get("test").expect("test").clone();
    let test = runtime
        .apply(test, regex)
        .unwrap_or_else(|_| panic!("test applied"));
    let verdict = runtime
        .apply(test, Value::Text("caa".to_string()))
        .unwrap_or_else(|_| panic!("test value"));
    assert!(matches!(verdict, Value::Bool(true)));
}
