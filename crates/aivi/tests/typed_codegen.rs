//! Tests for the typed codegen path.
//!
//! Verifies that definitions with closed types (Int, Float, Bool, etc.) produce additional
//! `_typed` function variants alongside the standard `Value`-returning functions.

mod native_fixture;

use aivi::{compile_rust_native_typed, desugar_target, infer_value_types_full, load_modules};
use native_fixture::write_aivi_source;
use std::sync::Mutex;
use tempfile::tempdir;

static BACKEND_ENV_LOCK: Mutex<()> = Mutex::new(());

/// Helper: compile AIVI source through the typed codegen pipeline and return the generated Rust.
fn compile_typed(source: &str) -> String {
    let _env_lock = BACKEND_ENV_LOCK.lock().expect("env lock");
    compile_typed_impl(source)
}

fn compile_typed_impl(source: &str) -> String {
    let dir = tempdir().expect("tempdir");
    let source_path_str = write_aivi_source(dir.path(), "main.aivi", source);

    // Desugar without full checking (consistent with other native codegen tests)
    let program = desugar_target(&source_path_str).expect("desugar");

    // Load modules (with stdlib) for type inference
    let modules = load_modules(&source_path_str).expect("load_modules");
    let infer_result = infer_value_types_full(&modules);

    compile_rust_native_typed(program, infer_result.cg_types).expect("compile_rust_native_typed")
}

fn compile_typed_with_backend(source: &str, backend: Option<&str>) -> String {
    let _env_lock = BACKEND_ENV_LOCK.lock().expect("env lock");
    let prev = std::env::var("AIVI_TYPED_BACKEND").ok();
    if let Some(backend) = backend {
        std::env::set_var("AIVI_TYPED_BACKEND", backend);
    } else {
        std::env::remove_var("AIVI_TYPED_BACKEND");
    }
    let rust = compile_typed_impl(source);
    if let Some(prev) = prev {
        std::env::set_var("AIVI_TYPED_BACKEND", prev);
    } else {
        std::env::remove_var("AIVI_TYPED_BACKEND");
    }
    rust
}

#[test]
fn typed_codegen_cg_types_collected() {
    // Verify that infer_value_types_full collects CgType info for a simple definition.
    let dir = tempdir().expect("tempdir");
    let source_path_str = write_aivi_source(
        dir.path(),
        "main.aivi",
        r#"module app.main
add : Int -> Int -> Int
add = a b => a + b

main : Effect Text Unit
main = do Effect {
  print "done"
}
"#,
    );
    let modules = load_modules(&source_path_str).expect("load_modules");
    let infer_result = infer_value_types_full(&modules);
    let cg_types = &infer_result.cg_types;

    // Should have a CgType entry for the app.main module containing "add"
    let mod_names: Vec<_> = cg_types.keys().collect();
    assert!(
        cg_types.contains_key("app.main"),
        "module app.main not found in cg_types keys: {mod_names:?}"
    );

    let mod_types = cg_types.get("app.main").expect("app.main module");
    assert!(
        mod_types.contains_key("add"),
        "add not found in module cg_types: {:?}",
        mod_types.keys().collect::<Vec<_>>()
    );
}

#[test]
fn typed_codegen_int_arithmetic_emits_typed_fn() {
    let rust = compile_typed(
        r#"module app.main
add : Int -> Int -> Int
add = a b => a + b

main : Effect Text Unit
main = do Effect {
  print "done"
}
"#,
    );

    // The typed path should emit a _typed variant for 'add'.
    // Global function names have the form `def_add__<hash>`, so the typed variant is
    // `def_add__<hash>_typed`.
    assert!(
        rust.contains("_typed(rt: &mut Runtime)"),
        "expected _typed function for a closed-type def; generated Rust:\n{rust}"
    );
}

#[test]
fn typed_codegen_still_has_value_fn() {
    // Typed codegen must always preserve the Value-returning function.
    let rust = compile_typed(
        r#"module app.main
add : Int -> Int -> Int
add = a b => a + b

main : Effect Text Unit
main = do Effect {
  print "done"
}
"#,
    );

    // Value-returning functions have the form: fn def_add__<hash>(rt: &mut Runtime) -> R
    assert!(
        rust.contains("(rt: &mut Runtime) -> R {"),
        "Value-returning function must always be present; generated Rust:\n{rust}"
    );
}

#[test]
fn typed_codegen_polymorphic_no_typed_fn() {
    // Polymorphic definitions should NOT get a _typed variant.
    let rust = compile_typed(
        r#"module app.main
identity : a -> a
identity = x => x

main : Effect Text Unit
main = do Effect {
  print "done"
}
"#,
    );

    assert!(
        !rust.contains("identity_typed"),
        "polymorphic function should not get _typed variant; generated Rust:\n{rust}"
    );
}

#[test]
fn typed_codegen_compiles_and_runs() {
    // End-to-end: typed codegen produces valid Rust that compiles and runs.
    let rust = compile_typed(
        r#"module app.main
main : Effect Text Unit
main = do Effect {
  print "Hello from typed codegen!"
}
"#,
    );

    assert!(rust.contains("fn main()"));

    let _lock = native_fixture::FIXTURE_LOCK.lock().expect("lock");
    let output = native_fixture::cargo_run_fixture(&rust);
    native_fixture::assert_cargo_success(&output);

    let stdout = native_fixture::stdout_text(&output);
    assert!(
        stdout.contains("Hello from typed codegen!"),
        "unexpected stdout: {stdout}"
    );
}

#[test]
fn typed_codegen_uses_mir_for_scalar_defs() {
    let rust = compile_typed(
        r#"module app.main
base : Int
base = 1

score : Int
score = base + 2

main : Effect Text Unit
main = do Effect {
  print "ok"
}
"#,
    );
    assert!(
        rust.contains("/* typed-mir */"),
        "expected typed MIR marker in generated Rust:\n{rust}"
    );
}

#[test]
fn typed_codegen_uses_mir_for_block_defs() {
    let rust = compile_typed(
        r#"module app.main
score : Int
score = {
  x = 1
  x + x
}

main : Effect Text Unit
main = do Effect {
  print "ok"
}
"#,
    );
    assert!(
        rust.contains("/* typed-mir */"),
        "expected typed MIR marker for block lowering:\n{rust}"
    );
}

#[test]
fn typed_codegen_uses_cranelift_backend_marker() {
    let rust = compile_typed_with_backend(
        r#"module app.main
base : Int
base = 1

score : Int
score = base + 2

main : Effect Text Unit
main = do Effect {
  print "ok"
}
"#,
        Some("cranelift"),
    );
    assert!(
        rust.contains("/* typed-clif */"),
        "expected typed Cranelift marker in generated Rust:\n{rust}"
    );
    assert!(
        rust.contains("clif.lowering.begin"),
        "expected Cranelift lowering comment in generated Rust:\n{rust}"
    );
}

#[test]
fn typed_codegen_cranelift_compiles_and_runs() {
    let rust = compile_typed_with_backend(
        r#"module app.main
base : Int
base = 21

score : Int
score = base + 21

main : Effect Text Unit
main = do Effect {
  print "Hello from typed cranelift!"
}
"#,
        Some("cranelift"),
    );
    assert!(rust.contains("/* typed-clif */"));

    let _lock = native_fixture::FIXTURE_LOCK.lock().expect("lock");
    let output = native_fixture::cargo_run_fixture(&rust);
    native_fixture::assert_cargo_success(&output);

    let stdout = native_fixture::stdout_text(&output);
    assert!(
        stdout.contains("Hello from typed cranelift!"),
        "unexpected stdout: {stdout}"
    );
}
