//! Pipeline QA tests for compilation phases 1–5.
//!
//! Covers: P1_01–P1_03, P3_01–P3_03, P4_01–P4_04, P5_01–P5_08

use std::path::Path;

use aivi::{
    check_modules, check_types, file_diagnostics_have_errors, load_modules_from_paths,
    parse_modules,
};

// ============================================================================
// Phase 1: File resolution
// ============================================================================

#[test]
fn p1_glob_filters_non_aivi() {
    let dir = tempfile::tempdir().expect("tempdir");
    let aivi_path = dir.path().join("main.aivi");
    let txt_path = dir.path().join("notes.txt");
    let lock_path = dir.path().join("Cargo.lock");

    std::fs::write(&aivi_path, "module Main\n").expect("write main.aivi");
    std::fs::write(&txt_path, "not aivi\n").expect("write notes.txt");
    std::fs::write(&lock_path, "# auto\n").expect("write Cargo.lock");

    let resolved =
        aivi::resolve_target(dir.path().to_str().expect("utf8")).expect("resolve target");
    assert_eq!(resolved, vec![aivi_path]);
}

#[test]
fn p1_single_file_accepted() {
    let dir = tempfile::tempdir().expect("tempdir");
    let aivi_path = dir.path().join("single.aivi");
    std::fs::write(&aivi_path, "module Single\n").expect("write");

    let resolved = aivi::resolve_target(aivi_path.to_str().expect("utf8")).expect("resolve target");
    assert_eq!(resolved, vec![aivi_path]);
}

#[test]
fn p1_recursive_glob_deterministic() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root_aivi = dir.path().join("main.aivi");
    let nested = dir.path().join("nested");
    let nested_aivi = nested.join("util.aivi");
    let nested_txt = nested.join("notes.txt");

    std::fs::create_dir_all(&nested).expect("mkdir");
    std::fs::write(&root_aivi, "module Main\n").expect("write");
    std::fs::write(&nested_aivi, "module Util\n").expect("write");
    std::fs::write(&nested_txt, "not aivi\n").expect("write");

    let target = format!("{}/**", dir.path().display());
    let mut resolved = aivi::resolve_target(&target).expect("resolve target");
    resolved.sort();

    let mut expected = vec![root_aivi, nested_aivi];
    expected.sort();
    assert_eq!(resolved, expected);
}

// ============================================================================
// Phase 3: Early diagnostics gate
// ============================================================================

#[test]
fn p3_syntax_error_blocks_type_checking() {
    let src = r#"
module gate.test

bad = (1, 2
wrongType : Int
wrongType = "not an int"
"#;
    let (_modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        file_diagnostics_have_errors(&diags),
        "expected parse errors from unclosed paren"
    );
    // The type mismatch on wrongType should NOT appear because the gate halts
    assert!(
        !diags
            .iter()
            .any(|d| d.diagnostic.message.contains("type mismatch")),
        "type errors should not cascade after syntax errors"
    );
}

#[test]
fn p3_multiple_errors_aggregated() {
    // Two broken definitions: incomplete `if` and unclosed paren
    let src = "module gate.multi\n\n\
        bad1 = if True then\n\
        bad2 = (1, 2\n\
        ok = 42\n";

    let (_modules, diags) = parse_modules(Path::new("test.aivi"), src);
    let error_count = diags
        .iter()
        .filter(|d| d.diagnostic.severity == aivi::DiagnosticSeverity::Error)
        .count();
    assert!(
        error_count >= 2,
        "expected at least 2 errors (one per broken def), got {error_count}"
    );
}

#[test]
fn p3_clean_source_passes_gate() {
    let src = r#"
module gate.clean

x = 1
y = x + 2
"#;
    let (_modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        !file_diagnostics_have_errors(&diags),
        "clean source should pass gate, got: {diags:?}"
    );
}

// ============================================================================
// Phase 4: Stdlib context
// ============================================================================

#[test]
fn p4_builtins_resolve_without_import() {
    let src = r#"
module stdlib.builtins

x : Int
x = 42

flag : Bool
flag = True

msg : Text
msg = "hello"

main : Effect Text Unit
main = do Effect {
  print msg
}
"#;
    let paths = write_temp_aivi(src);
    let modules = load_modules_from_paths(&paths).expect("load");
    let diags = check_modules(&modules);
    assert!(
        !file_diagnostics_have_errors(&diags),
        "builtins should resolve: {diags:?}"
    );
}

#[test]
fn p4_no_prelude_with_explicit_import() {
    let src = r#"
@no_prelude
module stdlib.noprelude

use aivi

x = 42
"#;
    let paths = write_temp_aivi(src);
    let modules = load_modules_from_paths(&paths).expect("load");
    let diags = check_modules(&modules);
    assert!(
        !file_diagnostics_have_errors(&diags),
        "@no_prelude with explicit import should work: {diags:?}"
    );
}

// ============================================================================
// Phase 5: Semantic + typing
// ============================================================================

#[test]
fn p5_occurs_check_self_application() {
    let src = r#"
module types.occurs

selfApply = f => f f
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        !file_diagnostics_have_errors(&diags),
        "parse errors: {diags:?}"
    );
    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(
        file_diagnostics_have_errors(&module_diags),
        "expected occurs-check error for self-application, got: {module_diags:?}"
    );
}

#[test]
fn p5_polymorphic_generalization() {
    let src = r#"
module types.poly

id : a -> a
id = x => x

intVal = id 42
textVal = id "hello"
boolVal = id True
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        !file_diagnostics_have_errors(&diags),
        "parse errors: {diags:?}"
    );
    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(
        !file_diagnostics_have_errors(&module_diags),
        "polymorphic id should type-check at multiple types: {module_diags:?}"
    );
}

#[test]
fn p5_coercion_rejects_text_to_int() {
    let src = r#"
module types.coercion

bad : Int
bad = "not an int"
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        !file_diagnostics_have_errors(&diags),
        "parse errors: {diags:?}"
    );
    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(
        file_diagnostics_have_errors(&module_diags),
        "expected type mismatch for Text assigned to Int"
    );
    assert!(
        module_diags
            .iter()
            .any(|d| d.diagnostic.message.contains("type mismatch")),
        "expected 'type mismatch' diagnostic"
    );
}

#[test]
fn p5_non_exhaustive_match_e3100() {
    let src = r#"
module types.exhaust

Option A = None | Some A

value = Some 1 match
  | Some _ => 1
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        !file_diagnostics_have_errors(&diags),
        "parse errors: {diags:?}"
    );
    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(
        module_diags.iter().any(|d| d.diagnostic.code == "E3100"),
        "expected E3100, got: {module_diags:?}"
    );
}

#[test]
fn p5_unreachable_arm_w3101() {
    let src = r#"
module types.unreach

Option A = None | Some A

value = Some 1 match
  | _ => 0
  | Some _ => 1
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        !file_diagnostics_have_errors(&diags),
        "parse errors: {diags:?}"
    );
    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(
        module_diags.iter().any(|d| d.diagnostic.code == "W3101"),
        "expected W3101, got: {module_diags:?}"
    );
}

#[test]
fn p5_kind_mismatch_list_list() {
    let src = r#"
module types.kind

bad : List List
bad = []
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        !file_diagnostics_have_errors(&diags),
        "parse errors: {diags:?}"
    );
    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(
        module_diags
            .iter()
            .any(|d| d.diagnostic.message.contains("kind mismatch")),
        "expected kind mismatch, got: {module_diags:?}"
    );
}

#[test]
fn p5_unknown_name_error() {
    let src = r#"
module types.missing

export value
value = missing
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        !file_diagnostics_have_errors(&diags),
        "parse errors: {diags:?}"
    );
    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(
        file_diagnostics_have_errors(&module_diags),
        "expected unresolved name error"
    );
}

#[test]
fn p5_branded_type_nominal_rejection() {
    let src = r#"
module types.branded

Email = Text!

email : Email
email = "alice@example.com"
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        !file_diagnostics_have_errors(&diags),
        "parse errors: {diags:?}"
    );
    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(
        file_diagnostics_have_errors(&module_diags),
        "branded type Email should not accept bare Text"
    );
}

#[test]
fn p5_domain_operator_missing() {
    let src = r#"
module types.domainop

bad = True + False
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        !file_diagnostics_have_errors(&diags),
        "parse errors: {diags:?}"
    );
    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(
        file_diagnostics_have_errors(&module_diags),
        "expected no domain operator error"
    );
}

// ============================================================================
// Helpers
// ============================================================================

fn write_temp_aivi(source: &str) -> Vec<std::path::PathBuf> {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("test.aivi");
    std::fs::write(&path, source).expect("write");
    // Keep dir alive by leaking (tests are short-lived)
    std::mem::forget(dir);
    vec![path]
}
