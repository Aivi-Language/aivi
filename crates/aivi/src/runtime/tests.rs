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
