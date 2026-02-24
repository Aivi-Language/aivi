use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;

use uuid::Uuid;

use super::values::{shape_record, KeyValue, TaggedValue};
use super::*;

fn expect_ok<T>(result: Result<T, RuntimeError>, msg: &str) -> T {
    match result {
        Ok(value) => value,
        Err(_) => panic!("{msg}"),
    }
}

fn runtime_from_source(source: &str) -> Runtime {
    let (modules, diags) = crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.diagnostic.severity == crate::diagnostics::DiagnosticSeverity::Error)
        .collect();
    assert!(errors.is_empty(), "unexpected diagnostics: {errors:?}");

    let program = crate::hir::desugar_modules(&modules);
    let module = program.modules.into_iter().next().expect("expected module");

    let globals = Env::new(None);
    register_builtins(&globals);

    let mut grouped: HashMap<String, Vec<HirExpr>> = HashMap::new();
    for def in module.defs {
        grouped.entry(def.name).or_default().push(def.expr);
    }
    for (name, exprs) in grouped {
        if exprs.len() == 1 {
            let thunk = ThunkValue {
                expr: Arc::new(exprs.into_iter().next().unwrap()),
                env: globals.clone(),
                cached: Mutex::new(None),
                in_progress: AtomicBool::new(false),
            };
            globals.set(name, Value::Thunk(Arc::new(thunk)));
        } else {
            let mut clauses = Vec::new();
            for expr in exprs {
                let thunk = ThunkValue {
                    expr: Arc::new(expr),
                    env: globals.clone(),
                    cached: Mutex::new(None),
                    in_progress: AtomicBool::new(false),
                };
                clauses.push(Value::Thunk(Arc::new(thunk)));
            }
            globals.set(name, Value::MultiClause(clauses));
        }
    }

    let mut constructor_ordinals = core_constructor_ordinals();
    for (name, ordinal) in collect_surface_constructor_ordinals(&modules) {
        match ordinal {
            Some(idx) => insert_constructor_ordinal(&mut constructor_ordinals, name, idx),
            None => {
                constructor_ordinals.insert(name, None);
            }
        }
    }

    let ctx = Arc::new(RuntimeContext::new_with_constructor_ordinals(
        globals,
        constructor_ordinals,
    ));
    let cancel = CancelToken::root();
    Runtime::new(ctx, cancel)
}

fn runtime_from_source_with_stdlib(source: &str) -> Runtime {
    let (mut modules, diags) =
        crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.diagnostic.severity == crate::diagnostics::DiagnosticSeverity::Error)
        .collect();
    assert!(errors.is_empty(), "unexpected diagnostics: {errors:?}");

    let mut stdlib_modules = crate::stdlib::embedded_stdlib_modules();
    stdlib_modules.append(&mut modules);
    let program = crate::hir::desugar_modules(&stdlib_modules);
    build_runtime_from_program(program).expect("runtime")
}

#[test]
fn jit_globals_include_int_function_definitions() {
    let (mut modules, diags) = crate::surface::parse_modules(
        std::path::Path::new("test.aivi"),
        r#"
module app.main
add : Int -> Int -> Int
add = a b => a + b

main : Effect Text Unit
main = do Effect {
  println "{add 1 2}"
}
"#,
    );
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.diagnostic.severity == crate::diagnostics::DiagnosticSeverity::Error)
        .collect();
    assert!(errors.is_empty(), "unexpected diagnostics: {errors:?}");

    let mut stdlib_modules = crate::stdlib::embedded_stdlib_modules();
    stdlib_modules.append(&mut modules);
    let program = crate::hir::desugar_modules(&stdlib_modules);
    let jitted = build_jitted_globals(program).expect("build jitted globals");
    let Some(Value::Builtin(builtin)) = jitted.get("app.main.add") else {
        panic!(
            "expected `app.main.add` to be jitted builtin; keys: {:?}",
            jitted.keys().take(20).collect::<Vec<_>>()
        );
    };
    assert!(
        builtin.imp.name.starts_with("__jit|"),
        "unexpected builtin name: {}",
        builtin.imp.name
    );
}

#[test]
fn jit_globals_include_non_numeric_function_shapes() {
    let (mut modules, diags) = crate::surface::parse_modules(
        std::path::Path::new("test.aivi"),
        r#"
module app.main
greet : Text -> Text
greet = name => "hello {name}"

main : Effect Text Unit
main = do Effect {
  println (greet "aivi")
}
"#,
    );
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.diagnostic.severity == crate::diagnostics::DiagnosticSeverity::Error)
        .collect();
    assert!(errors.is_empty(), "unexpected diagnostics: {errors:?}");

    let mut stdlib_modules = crate::stdlib::embedded_stdlib_modules();
    stdlib_modules.append(&mut modules);
    let program = crate::hir::desugar_modules(&stdlib_modules);
    let jitted = build_jitted_globals(program).expect("build jitted globals");
    let Some(Value::Builtin(builtin)) = jitted.get("greet") else {
        panic!(
            "expected `greet` to be jitted builtin; keys: {:?}",
            jitted.keys().take(20).collect::<Vec<_>>()
        );
    };
    assert!(
        builtin.imp.name.starts_with("__jit|"),
        "unexpected builtin name: {}",
        builtin.imp.name
    );
}

#[test]
fn jit_globals_include_zero_arity_definitions() {
    let (mut modules, diags) = crate::surface::parse_modules(
        std::path::Path::new("test.aivi"),
        r#"
module app.main
answer : Int
answer = 42

main : Effect Text Unit
main = do Effect {
  println "{answer}"
}
"#,
    );
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.diagnostic.severity == crate::diagnostics::DiagnosticSeverity::Error)
        .collect();
    assert!(errors.is_empty(), "unexpected diagnostics: {errors:?}");

    let mut stdlib_modules = crate::stdlib::embedded_stdlib_modules();
    stdlib_modules.append(&mut modules);
    let program = crate::hir::desugar_modules(&stdlib_modules);
    let jitted = build_jitted_globals(program).expect("build jitted globals");
    let Some(Value::Builtin(builtin)) = jitted.get("answer") else {
        panic!(
            "expected `answer` to be jitted builtin; keys: {:?}",
            jitted.keys().take(20).collect::<Vec<_>>()
        );
    };
    assert_eq!(builtin.imp.arity, 0);
}

#[test]
fn constructor_introspection_returns_name_and_ordinal() {
    let mut runtime = runtime_from_source(
        r#"
module test.constructorIntrospection
Status = Draft | Published | Archived

name = constructorName Published
ordinal = constructorOrdinal Published
"#,
    );

    let name = runtime.ctx.globals.get("name").expect("name binding");
    let name = expect_ok(runtime.force_value(name), "name evaluates");
    assert!(matches!(name, Value::Text(text) if text == "Published"));

    let ordinal = runtime.ctx.globals.get("ordinal").expect("ordinal binding");
    let ordinal = expect_ok(runtime.force_value(ordinal), "ordinal evaluates");
    assert!(matches!(ordinal, Value::Int(1)));
}

#[test]
fn file_json_source_loads_structured_values() {
    let temp = tempfile::NamedTempFile::new().expect("temp file");
    std::fs::write(
        temp.path(),
        r#"{"name":"Aivi","age":3,"enabled":true,"tags":["lang","dx"]}"#,
    )
    .expect("write json");
    let source = format!(
        r#"
module test.fileJson
value = do Effect {{
  data <- load (file.json "{}")
  pure data
}}
"#,
        temp.path().display()
    );
    let mut runtime = runtime_from_source(&source);
    let value = runtime.ctx.globals.get("value").expect("value binding");
    let effect = expect_ok(runtime.force_value(value), "force");
    let loaded = expect_ok(runtime.run_effect_value(effect), "load json");
    let Value::Record(fields) = loaded else {
        panic!("expected record, got {}", format_value(&loaded));
    };
    assert!(matches!(fields.get("name"), Some(Value::Text(name)) if name == "Aivi"));
    assert!(matches!(fields.get("age"), Some(Value::Int(3))));
}

#[test]
fn file_csv_source_loads_row_list() {
    let temp = tempfile::NamedTempFile::new().expect("temp file");
    std::fs::write(temp.path(), "id,name\n1,A\n2,B\n").expect("write csv");
    let source = format!(
        r#"
module test.fileCsv
rows = do Effect {{
  data <- load (file.csv "{}")
  pure data
}}
"#,
        temp.path().display()
    );
    let mut runtime = runtime_from_source(&source);
    let value = runtime.ctx.globals.get("rows").expect("rows binding");
    let effect = expect_ok(runtime.force_value(value), "force");
    let loaded = expect_ok(runtime.run_effect_value(effect), "load csv");
    let Value::List(rows) = loaded else {
        panic!("expected list, got {}", format_value(&loaded));
    };
    assert_eq!(rows.len(), 2);
}

#[test]
fn env_decode_source_collects_prefix() {
    std::env::set_var("AIVI_DECODE_PORT", "8080");
    std::env::set_var("AIVI_DECODE_DEBUG", "true");
    let source = r#"
module test.envDecode
cfg = do Effect {
  data <- load (env.decode "AIVI_DECODE")
  pure data
}
"#;
    let mut runtime = runtime_from_source(source);
    let value = runtime.ctx.globals.get("cfg").expect("cfg binding");
    let effect = expect_ok(runtime.force_value(value), "force");
    let loaded = expect_ok(runtime.run_effect_value(effect), "decode env");
    let Value::Record(fields) = loaded else {
        panic!("expected record, got {}", format_value(&loaded));
    };
    assert!(fields.contains_key("port"));
    assert!(fields.contains_key("debug"));
    std::env::remove_var("AIVI_DECODE_PORT");
    std::env::remove_var("AIVI_DECODE_DEBUG");
}

#[test]
fn static_json_decorator_embeds_value_at_parse_time() {
    let temp = tempfile::NamedTempFile::new().expect("temp file");
    std::fs::write(temp.path(), r#"{"embedded":"ok"}"#).expect("write json");
    let source = format!(
        r#"
@no_prelude
module test.staticJson

use aivi

@static
payload = file.json "{}"
"#,
        temp.path().display()
    );
    let (modules, diags) =
        crate::surface::parse_modules(std::path::Path::new("test_static.aivi"), &source);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.diagnostic.severity == crate::diagnostics::DiagnosticSeverity::Error)
        .collect();
    assert!(errors.is_empty(), "unexpected diagnostics: {errors:?}");
    let crate::surface::ModuleItem::Def(def) = &modules[0].items[0] else {
        panic!("expected first item to be a definition");
    };
    assert!(matches!(def.expr, crate::surface::Expr::Record { .. }));
}

#[test]
fn cleanups_run_even_when_cancelled() {
    let globals = Env::new(None);
    register_builtins(&globals);
    let ctx = Arc::new(RuntimeContext::new(globals));
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
fn boolean_operators_short_circuit_and_unary_not() {
    let source = r#"
module test.bool
x = False && ([1][10] == 0)
y = True || ([1][10] == 0)
z = !True
w = !False
"#;
    let mut runtime = runtime_from_source(source);

    let x = runtime.ctx.globals.get("x").unwrap();
    let y = runtime.ctx.globals.get("y").unwrap();
    let z = runtime.ctx.globals.get("z").unwrap();
    let w = runtime.ctx.globals.get("w").unwrap();

    let x = expect_ok(runtime.force_value(x), "failed to evaluate x");
    let y = expect_ok(runtime.force_value(y), "failed to evaluate y");
    let z = expect_ok(runtime.force_value(z), "failed to evaluate z");
    let w = expect_ok(runtime.force_value(w), "failed to evaluate w");

    assert!(
        matches!(x, Value::Bool(false)),
        "expected False for x, got {}",
        format_value(&x)
    );
    assert!(
        matches!(y, Value::Bool(true)),
        "expected True for y, got {}",
        format_value(&y)
    );
    assert!(
        matches!(z, Value::Bool(false)),
        "expected False for z, got {}",
        format_value(&z)
    );
    assert!(
        matches!(w, Value::Bool(true)),
        "expected True for w, got {}",
        format_value(&w)
    );
}

#[test]
fn range_operator_builds_lists_and_spreads_in_list_literals() {
    let source = r#"
module test.range

// `..` constructs a list of Ints, inclusive.
xs = 1..3

// Inside list literals, range expressions behave like implicit spreads.
ys = [0, 1..3, 4]
zs = [1..3]
"#;
    let mut runtime = runtime_from_source(source);

    let xs = runtime.ctx.globals.get("xs").unwrap();
    let ys = runtime.ctx.globals.get("ys").unwrap();
    let zs = runtime.ctx.globals.get("zs").unwrap();

    let xs = expect_ok(runtime.force_value(xs), "failed to evaluate xs");
    let ys = expect_ok(runtime.force_value(ys), "failed to evaluate ys");
    let zs = expect_ok(runtime.force_value(zs), "failed to evaluate zs");

    fn expect_int_list(value: Value, expected: &[i64], label: &str) {
        let Value::List(items) = value else {
            panic!("expected list for {label}, got {}", format_value(&value));
        };
        if items.len() != expected.len() {
            panic!(
                "expected {} items for {label}, got {} ({})",
                expected.len(),
                items.len(),
                format_value(&Value::List(items))
            );
        }
        for (idx, item) in items.iter().enumerate() {
            let Value::Int(n) = item else {
                panic!("expected Int at {label}[{idx}], got {}", format_value(item));
            };
            if *n != expected[idx] {
                panic!("expected {label}[{idx}] = {}, got {}", expected[idx], n);
            }
        }
    }

    expect_int_list(xs, &[1, 2, 3], "xs");

    expect_int_list(ys, &[0, 1, 2, 3, 4], "ys");

    expect_int_list(zs, &[1, 2, 3], "zs");
}

#[test]
fn text_interpolation_evaluates() {
    let source = r#"
module test.interpolation
s = "Count: {1 + 2}"
n = -1
t = "negative{n}"
u = "brace \{x\}"
v = "user: { { name: \"A\" }.name }"
"#;
    let mut runtime = runtime_from_source(source);

    let s = runtime.ctx.globals.get("s").unwrap();
    let t = runtime.ctx.globals.get("t").unwrap();
    let u = runtime.ctx.globals.get("u").unwrap();
    let v = runtime.ctx.globals.get("v").unwrap();

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
    let v = match runtime.force_value(v) {
        Ok(Value::Text(value)) => value,
        Ok(_) => panic!("expected Text for v"),
        Err(_) => panic!("failed to evaluate v"),
    };

    assert_eq!(s, "Count: 3");
    assert_eq!(t, "negative-1");
    assert_eq!(u, "brace {x}");
    assert_eq!(v, "user: A");
}

#[test]
fn i18n_sigils_evaluate_to_compiled_records() {
    let source = r#"
module test.i18nSigils
k = ~k"app.welcome"
m = ~m"Hello, {name:Text}!"
rendered = i18n.render m { name: "Alice" } or "ERR"
"#;
    let mut runtime = runtime_from_source(source);

    let k = runtime.ctx.globals.get("k").unwrap();
    let k = expect_ok(runtime.force_value(k), "evaluate k");
    let Value::Record(k_rec) = k else {
        panic!("expected k to be a Record");
    };
    assert!(matches!(k_rec.get("tag"), Some(Value::Text(t)) if t == "k"));
    assert!(matches!(k_rec.get("body"), Some(Value::Text(t)) if t == "app.welcome"));

    let m = runtime.ctx.globals.get("m").unwrap();
    let m = expect_ok(runtime.force_value(m), "evaluate m");
    let Value::Record(m_rec) = m else {
        panic!("expected m to be a Record");
    };
    assert!(matches!(m_rec.get("tag"), Some(Value::Text(t)) if t == "m"));
    assert!(matches!(m_rec.get("body"), Some(Value::Text(t)) if t == "Hello, {name:Text}!"));
    assert!(matches!(m_rec.get("parts"), Some(Value::List(_))));

    let rendered = runtime.ctx.globals.get("rendered").unwrap();
    let rendered = expect_ok(runtime.force_value(rendered), "evaluate rendered");
    assert!(matches!(rendered, Value::Text(t) if t == "Hello, Alice!"));
}

#[test]
fn structured_sigils_evaluate_to_map_and_set() {
    let source = r#"
module test.structuredSigils
m = ~map{ "a" => 1, "b" => 2 }
s = ~set["x", "y", "x"]
a = m["a"]
n = Set.size s
"#;
    let mut runtime = runtime_from_source(source);

    let m = runtime.ctx.globals.get("m").unwrap();
    let m = expect_ok(runtime.force_value(m), "evaluate m");
    assert!(matches!(m, Value::Map(_)));

    let s = runtime.ctx.globals.get("s").unwrap();
    let s = expect_ok(runtime.force_value(s), "evaluate s");
    assert!(matches!(s, Value::Set(_)));

    let a = runtime.ctx.globals.get("a").unwrap();
    let a = expect_ok(runtime.force_value(a), "evaluate a");
    assert!(matches!(a, Value::Int(1)));

    let n = runtime.ctx.globals.get("n").unwrap();
    let n = expect_ok(runtime.force_value(n), "evaluate n");
    assert!(matches!(n, Value::Int(2)));
}

#[test]
fn url_and_path_sigils_evaluate_to_typed_records() {
    let source = r#"
module test.urlAndPathSigils

use aivi.path

u = ~url(https://example.com/a/b?q=1#frag)
p = ~path[/a/./b/../c]
joined = toString (~path[/a/b] / ~path[../c])
"#;
    let (mut modules, diags) =
        crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");

    let mut stdlib_modules = crate::stdlib::embedded_stdlib_modules();
    stdlib_modules.append(&mut modules);
    let program = crate::hir::desugar_modules(&stdlib_modules);
    let mut runtime = build_runtime_from_program(program).expect("runtime");

    let u = runtime.ctx.globals.get("u").unwrap();
    let u = expect_ok(runtime.force_value(u), "evaluate u");
    let Value::Record(u_rec) = u else {
        panic!("expected u to be a Record");
    };
    assert!(matches!(u_rec.get("protocol"), Some(Value::Text(t)) if t == "https"));
    assert!(matches!(u_rec.get("host"), Some(Value::Text(t)) if t == "example.com"));
    assert!(matches!(u_rec.get("path"), Some(Value::Text(t)) if t == "/a/b"));

    let p = runtime.ctx.globals.get("p").unwrap();
    let p = expect_ok(runtime.force_value(p), "evaluate p");
    let Value::Record(p_rec) = p else {
        panic!("expected p to be a Record");
    };
    assert!(matches!(p_rec.get("absolute"), Some(Value::Bool(true))));
    let Some(Value::List(segments)) = p_rec.get("segments") else {
        panic!("expected p.segments to be a List");
    };
    let seg_texts: Vec<String> = segments
        .iter()
        .map(|v| match v {
            Value::Text(s) => s.clone(),
            _ => panic!("expected segment to be Text"),
        })
        .collect();
    assert_eq!(seg_texts, vec!["a".to_string(), "c".to_string()]);

    let joined = runtime.ctx.globals.get("joined").unwrap();
    let joined = expect_ok(runtime.force_value(joined), "evaluate joined");
    let Value::Text(joined) = joined else {
        panic!("expected joined to be Text");
    };
    assert_eq!(joined, "/a/c");
}

#[test]
fn layout_suffix_literals_find_domain_templates_at_runtime() {
    let source = r#"
module test.layoutSuffix

use aivi.ui.layout

x = 10px
"#;

    let (mut modules, diags) =
        crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");

    let mut stdlib_modules = crate::stdlib::embedded_stdlib_modules();
    assert!(
        stdlib_modules
            .iter()
            .any(|m| m.name.name == "aivi.ui.layout"),
        "expected embedded stdlib to include aivi.ui.layout"
    );
    stdlib_modules.append(&mut modules);
    let program = crate::hir::desugar_modules(&stdlib_modules);
    let has_template = program
        .modules
        .iter()
        .any(|m| m.defs.iter().any(|d| d.name == "1px"));
    assert!(has_template, "expected embedded stdlib to define `1px`");

    let mut runtime = build_runtime_from_program(program).expect("runtime");
    let x = runtime.ctx.globals.get("x").expect("x");
    let value = runtime.force_value(x);
    if let Err(err) = value {
        panic!(
            "expected `10px` to evaluate via a `1px` template binding, got: {}",
            format_runtime_error(err)
        );
    }
}

#[test]
fn database_persists_rows_in_sqlite_memory() {
    let source = r#"
module test.databaseSqlite

Driver = Sqlite | Postgresql | Mysql
DbConfig = { driver: Driver, url: Text }

User = { id: Int, name: Text }

userTable = database.table "users" []

main = do Effect {
  _ <- database.configure { driver: Sqlite, url: ":memory:" }
  _ <- database.runMigrations [ userTable ]
  _ <- database.applyDelta userTable (database.ins { id: 1, name: "A" })
  database.load userTable
}
"#;

    let mut runtime = runtime_from_source(source);
    let main = runtime.ctx.globals.get("main").unwrap();
    let main = expect_ok(runtime.force_value(main), "evaluate main");
    let Value::Effect(effect) = main else {
        panic!("expected main to be an Effect");
    };

    let result = match runtime.run_effect_value(Value::Effect(effect)) {
        Ok(value) => value,
        Err(RuntimeError::Cancelled) => panic!("run main effect: cancelled"),
        Err(RuntimeError::Message(message)) => panic!("run main effect: {message}"),
        Err(RuntimeError::Error(value)) => panic!("run main effect: {}", format_value(&value)),
    };
    let Value::List(items) = result else {
        panic!("expected List result");
    };
    assert_eq!(items.len(), 1);
    let Value::Record(fields) = &items[0] else {
        panic!("expected record row");
    };
    assert!(matches!(fields.get("id"), Some(Value::Int(1))));
    assert!(matches!(fields.get("name"), Some(Value::Text(t)) if t == "A"));
}

#[test]
fn database_upsert_inserts_then_updates() {
    let source = r#"
module test.databaseUpsert

Driver = Sqlite | Postgresql | Mysql
DbConfig = { driver: Driver, url: Text }

User = { id: Int, name: Text, active: Bool }

userTable = database.table "users" []

main = do Effect {
  _ <- database.configure { driver: Sqlite, url: ":memory:" }
  _ <- database.runMigrations [ userTable ]
  _ <- database.applyDelta userTable (database.ups (u => u.id == 1) { id: 1, name: "A", active: False } (u => u <| { name: "B" }))
  table1 <- database.applyDelta userTable (database.ups (u => u.id == 1) { id: 1, name: "X", active: False } (u => u <| { name: "B", active: True }))
  database.load table1
}
"#;

    let mut runtime = runtime_from_source(source);
    let main = runtime.ctx.globals.get("main").unwrap();
    let main = expect_ok(runtime.force_value(main), "evaluate main");
    let Value::Effect(effect) = main else {
        panic!("expected main to be an Effect");
    };

    let result = match runtime.run_effect_value(Value::Effect(effect)) {
        Ok(value) => value,
        Err(RuntimeError::Cancelled) => panic!("run main effect: cancelled"),
        Err(RuntimeError::Message(message)) => panic!("run main effect: {message}"),
        Err(RuntimeError::Error(value)) => panic!("run main effect: {}", format_value(&value)),
    };
    let Value::List(items) = result else {
        panic!("expected List result");
    };
    assert_eq!(items.len(), 1);
    let Value::Record(fields) = &items[0] else {
        panic!("expected record row");
    };
    assert!(matches!(fields.get("id"), Some(Value::Int(1))));
    assert!(matches!(fields.get("name"), Some(Value::Text(t)) if t == "B"));
    assert!(matches!(fields.get("active"), Some(Value::Bool(true))));
}

#[test]
fn concurrent_par_observes_parent_cancellation() {
    let globals = Env::new(None);
    register_builtins(&globals);
    let ctx = Arc::new(RuntimeContext::new(globals));
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
    let ctx = Arc::new(RuntimeContext::new(globals));
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
fn crypto_sha256_randoms() {
    let globals = Env::new(None);
    register_builtins(&globals);
    let ctx = Arc::new(RuntimeContext::new(globals));
    let cancel = CancelToken::root();
    let mut runtime = Runtime::new(ctx, cancel);

    let crypto_record = runtime.ctx.globals.get("crypto").expect("crypto record");
    let Value::Record(fields) = crypto_record else {
        panic!("expected crypto record");
    };

    let sha256 = fields.get("sha256").expect("sha256").clone();
    let digest = runtime
        .apply(sha256, Value::Text("hello".to_string()))
        .unwrap_or_else(|_| panic!("sha256 applied"));
    let Value::Text(digest) = digest else {
        panic!("expected sha256 output");
    };
    assert_eq!(
        digest,
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );

    let random_bytes = fields.get("randomBytes").expect("randomBytes").clone();
    let effect = runtime
        .apply(random_bytes, Value::Int(16))
        .unwrap_or_else(|_| panic!("randomBytes applied"));
    let value = runtime
        .run_effect_value(effect)
        .unwrap_or_else(|_| panic!("randomBytes run"));
    let Value::Bytes(bytes) = value else {
        panic!("expected Bytes");
    };
    assert_eq!(bytes.len(), 16);

    let random_uuid = fields.get("randomUuid").expect("randomUuid").clone();
    let effect = runtime
        .apply(random_uuid, Value::Unit)
        .unwrap_or_else(|_| panic!("randomUuid applied"));
    let value = runtime
        .run_effect_value(effect)
        .unwrap_or_else(|_| panic!("randomUuid run"));
    let Value::Text(text) = value else {
        panic!("expected Text uuid");
    };
    Uuid::parse_str(&text).expect("uuid parse");
}

#[test]
fn collections_map_set_queue_heap() {
    let globals = Env::new(None);
    register_builtins(&globals);
    let ctx = Arc::new(RuntimeContext::new(globals));
    let cancel = CancelToken::root();
    let mut runtime = Runtime::new(ctx, cancel);

    let map_record = runtime.ctx.globals.get("Map").expect("Map record");
    let Value::Record(map_fields) = map_record else {
        panic!("Map record missing");
    };
    let from_list = map_fields.get("fromList").expect("fromList").clone();
    let union = map_fields.get("union").expect("union").clone();

    let list1 = Value::List(Arc::new(vec![
        Value::Tuple(vec![Value::Int(1), Value::Text("one".to_string())]),
        Value::Tuple(vec![Value::Int(2), Value::Text("two".to_string())]),
    ]));
    let map1 = expect_ok(runtime.apply(from_list.clone(), list1), "map1");
    let list2 = Value::List(Arc::new(vec![Value::Tuple(vec![
        Value::Int(2),
        Value::Text("dos".to_string()),
    ])]));
    let map2 = expect_ok(runtime.apply(from_list, list2), "map2");
    let union_left = expect_ok(runtime.apply(union, map1), "union left");
    let unioned = expect_ok(runtime.apply(union_left, map2), "unioned");
    let Value::Map(map) = unioned else {
        panic!("expected map");
    };
    match map.get(&KeyValue::Int(2)) {
        Some(Value::Text(value)) => assert_eq!(value, "dos"),
        _ => panic!("expected map entry"),
    }

    let set_record = runtime.ctx.globals.get("Set").expect("Set record");
    let Value::Record(set_fields) = set_record else {
        panic!("Set record missing");
    };
    let from_list = set_fields.get("fromList").expect("fromList").clone();
    let has = set_fields.get("has").expect("has").clone();
    let set_list = Value::List(Arc::new(vec![Value::Int(1), Value::Int(2)]));
    let set = expect_ok(runtime.apply(from_list, set_list), "set");
    let has_key = expect_ok(runtime.apply(has, Value::Int(2)), "has key");
    let has = expect_ok(runtime.apply(has_key, set), "has");
    assert!(matches!(has, Value::Bool(true)));

    let queue_record = runtime.ctx.globals.get("Queue").expect("Queue record");
    let Value::Record(queue_fields) = queue_record else {
        panic!("Queue record missing");
    };
    let enqueue = queue_fields.get("enqueue").expect("enqueue").clone();
    let dequeue = queue_fields.get("dequeue").expect("dequeue").clone();
    let empty = queue_fields.get("empty").expect("empty").clone();
    let enqueue_first = expect_ok(
        runtime.apply(enqueue.clone(), Value::Text("first".to_string())),
        "enqueue arg1",
    );
    let queue = expect_ok(runtime.apply(enqueue_first, empty), "enqueue arg2");
    let dequeued = expect_ok(runtime.apply(dequeue, queue), "dequeue");
    match dequeued {
        Value::Constructor { name, args } if name == "Some" => {
            assert!(
                matches!(args.as_slice(), [Value::Tuple(values)] if matches!(values.as_slice(), [Value::Text(value), _] if value == "first"))
            );
        }
        _ => panic!("expected Some from dequeue"),
    }

    let heap_record = runtime.ctx.globals.get("Heap").expect("Heap record");
    let Value::Record(heap_fields) = heap_record else {
        panic!("Heap record missing");
    };
    let push = heap_fields.get("push").expect("push").clone();
    let pop_min = heap_fields.get("popMin").expect("popMin").clone();
    let empty = heap_fields.get("empty").expect("empty").clone();
    let push_three = expect_ok(runtime.apply(push.clone(), Value::Int(3)), "push arg1");
    let heap = expect_ok(runtime.apply(push_three, empty), "push arg2");
    let push_one = expect_ok(runtime.apply(push, Value::Int(1)), "push arg1");
    let heap = expect_ok(runtime.apply(push_one, heap), "push arg2");
    let popped = expect_ok(runtime.apply(pop_min, heap), "popMin");
    match popped {
        Value::Constructor { name, args } if name == "Some" => {
            assert!(
                matches!(args.as_slice(), [Value::Tuple(values)] if matches!(values.as_slice(), [Value::Int(1), _]))
            );
        }
        _ => panic!("expected Some from heap pop"),
    }
}

#[test]
fn list_core_ops() {
    let globals = Env::new(None);
    register_builtins(&globals);
    let ctx = Arc::new(RuntimeContext::new(globals));
    let cancel = CancelToken::root();
    let mut runtime = Runtime::new(ctx, cancel);

    let list_record = runtime.ctx.globals.get("List").expect("List record");
    let Value::Record(fields) = list_record else {
        panic!("List record missing");
    };

    let empty = fields.get("empty").expect("empty").clone();
    assert!(matches!(empty, Value::List(items) if items.is_empty()));

    let length = fields.get("length").expect("length").clone();
    let len = expect_ok(
        runtime.apply(
            length,
            Value::List(Arc::new(vec![Value::Int(1), Value::Int(2), Value::Int(3)])),
        ),
        "List.length",
    );
    assert!(matches!(len, Value::Int(3)));

    let take = fields.get("take").expect("take").clone();
    let take = expect_ok(runtime.apply(take, Value::Int(2)), "List.take arg1");
    let taken = runtime.apply(
        take,
        Value::List(Arc::new(vec![Value::Int(1), Value::Int(2), Value::Int(3)])),
    );
    let taken = expect_ok(taken, "List.take arg2");
    assert!(matches!(taken, Value::List(items) if items.len() == 2));

    let chunk = fields.get("chunk").expect("chunk").clone();
    let chunk = expect_ok(runtime.apply(chunk, Value::Int(2)), "List.chunk arg1");
    let chunked = runtime.apply(
        chunk,
        Value::List(Arc::new(vec![Value::Int(1), Value::Int(2), Value::Int(3)])),
    );
    let chunked = expect_ok(chunked, "List.chunk arg2");
    let Value::List(chunks) = chunked else {
        panic!("expected List of chunks");
    };
    assert_eq!(chunks.len(), 2);
    assert!(matches!(&chunks[0], Value::List(items) if items.len() == 2));
    assert!(matches!(&chunks[1], Value::List(items) if items.len() == 1));
}

#[test]
fn map_new_ops() {
    let globals = Env::new(None);
    register_builtins(&globals);
    let ctx = Arc::new(RuntimeContext::new(globals));
    let cancel = CancelToken::root();
    let mut runtime = Runtime::new(ctx, cancel);

    let map_record = runtime.ctx.globals.get("Map").expect("Map record");
    let Value::Record(fields) = map_record else {
        panic!("Map record missing");
    };

    let empty = fields.get("empty").expect("empty").clone();
    let insert = fields.get("insert").expect("insert").clone();
    let insert = expect_ok(
        runtime.apply(insert, Value::Text("a".to_string())),
        "insert k",
    );
    let insert = expect_ok(runtime.apply(insert, Value::Int(1)), "insert v");
    let map = expect_ok(runtime.apply(insert, empty), "insert map");

    let get_or_else = fields.get("getOrElse").expect("getOrElse").clone();
    let applied = runtime.apply(get_or_else, Value::Text("a".to_string()));
    let applied = expect_ok(applied, "getOrElse k");
    let applied = expect_ok(runtime.apply(applied, Value::Int(9)), "getOrElse default");
    let got = expect_ok(runtime.apply(applied, map.clone()), "getOrElse map");
    assert!(matches!(got, Value::Int(1)));

    let alter = fields.get("alter").expect("alter").clone();
    let alter = expect_ok(
        runtime.apply(alter, Value::Text("a".to_string())),
        "alter key",
    );
    let remove_fn = Value::Builtin(crate::runtime::values::BuiltinValue {
        imp: Arc::new(crate::runtime::values::BuiltinImpl {
            name: "<remove>".to_string(),
            arity: 1,
            func: Arc::new(|_args, _runtime| {
                Ok(Value::Constructor {
                    name: "None".to_string(),
                    args: Vec::new(),
                })
            }),
        }),
        args: Vec::new(),
        tagged_args: Some(Vec::new()),
    });
    let alter = expect_ok(runtime.apply(alter, remove_fn), "alter f");
    let map2 = expect_ok(runtime.apply(alter, map), "alter map");

    let has = fields.get("has").expect("has").clone();
    let has = expect_ok(runtime.apply(has, Value::Text("a".to_string())), "has k");
    let present = expect_ok(runtime.apply(has, map2), "has map");
    assert!(matches!(present, Value::Bool(false)));
}

#[test]
fn database_pool_withconn_releases_on_failure() {
    let source = r#"
module test.poolRelease

use aivi.database.pool as Pool

Conn = { id: Int }

cfg : Pool.Config Conn
cfg = {
  maxSize: 1,
  minIdle: 0,
  acquireTimeout: { millis: 200 },
  idleTimeout: None,
  maxLifetime: None,
  healthCheckInterval: None,
  backoffPolicy: Pool.Fixed ({ millis: 10 }),
  queuePolicy: Pool.Fifo,
  acquire: _ => pure { id: 1 },
  release: _ => pure Unit,
  healthCheck: _ => pure True
}

main : Effect Text (Pool.PoolStats, Pool.PoolStats)
main = do Effect {
  poolRes <- Pool.create cfg
  pool = poolRes match | Ok p => p | Err _ => { acquire: _ => pure (Err Pool.Closed), release: _ => pure Unit, stats: _ => pure { size: 0, idle: 0, inUse: 0, waiters: 0, closed: True }, drain: _ => pure Unit, close: _ => pure Unit }
  before <- Pool.stats pool
  _ <- attempt (Pool.withConn pool (_ => fail "boom"))
  after <- Pool.stats pool
  pure (before, after)
}
"#;

    let mut runtime = runtime_from_source_with_stdlib(source);
    let main = runtime.ctx.globals.get("main").unwrap();
    let main = expect_ok(runtime.force_value(main), "evaluate main");
    let Value::Effect(effect) = main else {
        panic!("expected main to be an Effect");
    };
    let result = expect_ok(
        runtime.run_effect_value(Value::Effect(effect)),
        "run main effect",
    );
    let Value::Tuple(items) = result else {
        panic!("expected stats tuple");
    };
    assert_eq!(items.len(), 2);
    let Value::Record(after) = &items[1] else {
        panic!("expected after stats record");
    };
    let in_use = after.get("inUse").cloned().unwrap_or(Value::Unit);
    let idle = after.get("idle").cloned().unwrap_or(Value::Unit);
    let size = after.get("size").cloned().unwrap_or(Value::Unit);
    assert!(
        matches!(in_use, Value::Int(0)),
        "expected pool inUse==0 after failure; got inUse={}, idle={}, size={}",
        crate::runtime::format_value(&in_use),
        crate::runtime::format_value(&idle),
        crate::runtime::format_value(&size)
    );
}

#[test]
fn database_pool_acquire_times_out_when_full() {
    let source = r#"
module test.poolTimeout

use aivi.database.pool as Pool

Conn = { id: Int }

cfg : Pool.Config Conn
cfg = {
  maxSize: 1,
  minIdle: 0,
  acquireTimeout: { millis: 0 },
  idleTimeout: None,
  maxLifetime: None,
  healthCheckInterval: None,
  backoffPolicy: Pool.Fixed ({ millis: 10 }),
  queuePolicy: Pool.Fifo,
  acquire: _ => pure { id: 1 },
  release: _ => pure Unit,
  healthCheck: _ => pure True
}

main : Effect Text (Result Pool.PoolError Conn, Result Pool.PoolError Conn)
main = do Effect {
  poolRes <- Pool.create cfg
  pool = poolRes match | Ok p => p | Err _ => { acquire: _ => pure (Err Pool.Closed), release: _ => pure Unit, stats: _ => pure { size: 0, idle: 0, inUse: 0, waiters: 0, closed: True }, drain: _ => pure Unit, close: _ => pure Unit }
  c1 <- Pool.acquire pool
  c2 <- Pool.acquire pool
  pure (c1, c2)
}
"#;

    let mut runtime = runtime_from_source_with_stdlib(source);
    let main = runtime.ctx.globals.get("main").unwrap();
    let main = expect_ok(runtime.force_value(main), "evaluate main");
    let Value::Effect(effect) = main else {
        panic!("expected main to be an Effect");
    };
    let result = expect_ok(
        runtime.run_effect_value(Value::Effect(effect)),
        "run main effect",
    );
    let Value::Tuple(items) = result else {
        panic!("expected tuple");
    };
    assert_eq!(items.len(), 2);

    let Value::Constructor { name, args } = &items[0] else {
        panic!("expected Result for c1");
    };
    assert_eq!(name, "Ok");
    assert_eq!(args.len(), 1);

    let Value::Constructor { name, args } = &items[1] else {
        panic!("expected Result for c2");
    };
    assert_eq!(name, "Err");
    assert_eq!(args.len(), 1);
    assert!(
        matches!(&args[0], Value::Constructor { name, args } if name == "Timeout" && args.is_empty())
    );
}

#[test]
fn linalg_dot_and_graph_shortest_path() {
    let globals = Env::new(None);
    register_builtins(&globals);
    let ctx = Arc::new(RuntimeContext::new(globals));
    let cancel = CancelToken::root();
    let mut runtime = Runtime::new(ctx, cancel);

    let linalg_record = runtime.ctx.globals.get("linalg").expect("linalg record");
    let Value::Record(linalg_fields) = linalg_record else {
        panic!("linalg record missing");
    };
    let dot = linalg_fields.get("dot").expect("dot").clone();

    let vec_a = {
        let mut fields = HashMap::new();
        fields.insert("size".to_string(), Value::Int(3));
        fields.insert(
            "data".to_string(),
            Value::List(Arc::new(vec![
                Value::Float(1.0),
                Value::Float(2.0),
                Value::Float(3.0),
            ])),
        );
        Value::Record(Arc::new(fields))
    };
    let vec_b = {
        let mut fields = HashMap::new();
        fields.insert("size".to_string(), Value::Int(3));
        fields.insert(
            "data".to_string(),
            Value::List(Arc::new(vec![
                Value::Float(2.0),
                Value::Float(0.0),
                Value::Float(1.0),
            ])),
        );
        Value::Record(Arc::new(fields))
    };
    let dot_left = expect_ok(runtime.apply(dot, vec_a), "dot arg1");
    let dot = expect_ok(runtime.apply(dot_left, vec_b), "dot");
    assert!(matches!(dot, Value::Float(value) if (value - 5.0).abs() < 1e-9));

    let graph_record = runtime.ctx.globals.get("graph").expect("graph record");
    let Value::Record(graph_fields) = graph_record else {
        panic!("graph record missing");
    };
    let shortest_path = graph_fields
        .get("shortestPath")
        .expect("shortestPath")
        .clone();

    let graph = {
        let mut fields = HashMap::new();
        fields.insert(
            "nodes".to_string(),
            Value::List(Arc::new(vec![Value::Int(1), Value::Int(2), Value::Int(3)])),
        );
        let edge = |from: i64, to: i64, weight: f64| {
            let mut edge_fields = HashMap::new();
            edge_fields.insert("from".to_string(), Value::Int(from));
            edge_fields.insert("to".to_string(), Value::Int(to));
            edge_fields.insert("weight".to_string(), Value::Float(weight));
            Value::Record(Arc::new(edge_fields))
        };
        fields.insert(
            "edges".to_string(),
            Value::List(Arc::new(vec![
                edge(1, 2, 1.0),
                edge(2, 3, 1.0),
                edge(1, 3, 5.0),
            ])),
        );
        Value::Record(Arc::new(fields))
    };
    let path_left = expect_ok(runtime.apply(shortest_path, graph), "path arg1");
    let path_mid = expect_ok(runtime.apply(path_left, Value::Int(1)), "path arg2");
    let path = expect_ok(runtime.apply(path_mid, Value::Int(3)), "path");
    let Value::List(nodes) = path else {
        panic!("expected list from shortestPath");
    };
    assert!(matches!(
        nodes.as_slice(),
        [Value::Int(1), Value::Int(2), Value::Int(3)]
    ));
}

#[test]
fn https_rejects_non_https_urls() {
    let globals = Env::new(None);
    register_builtins(&globals);
    let ctx = Arc::new(RuntimeContext::new(globals));
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
    let load = runtime.ctx.globals.get("load").expect("load");
    let loaded = match runtime.apply(load, applied) {
        Ok(value) => value,
        Err(_) => panic!("apply load failed"),
    };
    let result = runtime.run_effect_value(loaded);

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
    let ctx = Arc::new(RuntimeContext::new(globals));
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

#[test]
fn deep_recursion_does_not_overflow() {
    // A recursive countdown: count = n => n match | 0 => 0 | n => count (n - 1)
    // With n = 100_000, this would overflow the Rust stack without the trampoline.
    let mut runtime = runtime_from_source(
        r#"
@no_prelude
module test
count = n => n match
  | 0 => 0
  | n => count (n - 1)
result = count 100000
"#,
    );
    runtime.fuel = Some(1_000_000);
    let value = runtime.ctx.globals.get("result").expect("result defined");
    let value = runtime
        .force_value(value)
        .unwrap_or_else(|e| panic!("result evaluates: {}", format_runtime_error(e)));
    assert!(matches!(value, Value::Int(0)));
}

#[test]
fn shaped_record_lookup_matches_hash_lookup() {
    let mut fields = HashMap::new();
    fields.insert("x".to_string(), Value::Int(1));
    fields.insert("y".to_string(), Value::Int(2));
    fields.insert("name".to_string(), Value::Text("p".to_string()));
    let shaped = shape_record(&fields);
    assert!(matches!(shaped.get("x"), Some(Value::Int(1))));
    assert!(matches!(shaped.get("y"), Some(Value::Int(2))));
    assert!(matches!(shaped.get("name"), Some(Value::Text(text)) if text == "p"));
}

#[test]
fn tagged_value_encodes_scalars() {
    let int_tag = TaggedValue::from_value(&Value::Int(42)).expect("int tag");
    let bool_tag = TaggedValue::from_value(&Value::Bool(true)).expect("bool tag");
    let none_tag = TaggedValue::from_value(&Value::Float(3.5));
    assert_ne!(int_tag, bool_tag);
    assert!(none_tag.is_none());
}

#[test]
fn deep_effect_chain_does_not_overflow() {
    // Build a chain of effect binds that is deep enough to overflow without trampoline.
    // Each `pure` is an `Effect` value, so `x <- pure 1` forces and binds.
    let mut runtime = runtime_from_source_with_stdlib(
        r#"
@no_prelude
module test
use aivi

// Recursive helper that nests effect binds
loop = n => do Effect {
  if n <= 0 then
    pure 0
  else
    loop (n - 1)
}

main = do Effect {
  result <- loop 10000
  assert (result == 0)
}
"#,
    );
    runtime.fuel = Some(10_000_000);
    let main = runtime.ctx.globals.get("main").expect("main defined");
    let main = runtime
        .force_value(main)
        .unwrap_or_else(|e| panic!("main resolves: {}", format_runtime_error(e)));
    let result = runtime.run_effect_value(main);
    result.unwrap_or_else(|e| panic!("deep effect chain failed: {}", format_runtime_error(e)));
}
