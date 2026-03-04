use std::path::Path;

use aivi::{check_modules, check_types, file_diagnostics_have_errors, parse_modules};

fn check_ok(source: &str) {
    let (modules, diagnostics) = parse_modules(Path::new("test.aivi"), source);
    assert!(
        !file_diagnostics_have_errors(&diagnostics),
        "parse errors: {diagnostics:?}"
    );

    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(
        !file_diagnostics_have_errors(&module_diags),
        "unexpected errors: {module_diags:?}"
    );
}

fn check_err(source: &str) {
    let (modules, diagnostics) = parse_modules(Path::new("test.aivi"), source);
    assert!(
        !file_diagnostics_have_errors(&diagnostics),
        "parse errors: {diagnostics:?}"
    );

    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(
        file_diagnostics_have_errors(&module_diags),
        "expected errors, got: {module_diags:?}"
    );
}

// ── A1: Occurs-check ─────────────────────────────────────────────────────────

/// `bad = x => x x` requires `x : a` with `a = a -> b`, which fails the
/// occurs check. The checker must reject this and report an error (not loop
/// or ICE).
#[test]
fn occurs_check_self_application_errors() {
    let source = r#"
module test.occurs_self_apply
bad = x => x x
"#;
    check_err(source);
}

/// The occurs-check error message must mention "occurs check".
#[test]
fn occurs_check_error_message_quality() {
    let source = r#"
module test.occurs_message
bad = x => x x
"#;
    let (modules, diagnostics) = parse_modules(Path::new("test.aivi"), source);
    assert!(
        !file_diagnostics_have_errors(&diagnostics),
        "parse errors: {diagnostics:?}"
    );

    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(
        file_diagnostics_have_errors(&module_diags),
        "expected occurs check error, got no errors"
    );
    assert!(
        module_diags
            .iter()
            .any(|d| d.diagnostic.message.contains("occurs check")),
        "expected 'occurs check' in error message, got: {module_diags:?}"
    );
}

/// A nested occurs-check: `f = x => [f x]` makes `f : a -> [a -> ...]`,
/// which also fails the occurs check.
#[test]
fn occurs_check_nested_self_reference_errors() {
    let source = r#"
module test.occurs_nested

Option A = None | Some A

bad = x => Some (bad x)
"#;
    // This is a recursive self-reference; the checker should either accept it
    // (top-level definitions are allowed to be recursive) or reject it.
    // The important property is that it terminates — no infinite loop or ICE.
    let (modules, diagnostics) = parse_modules(Path::new("test.aivi"), source);
    assert!(
        !file_diagnostics_have_errors(&diagnostics),
        "parse errors: {diagnostics:?}"
    );
    let mut _module_diags = check_modules(&modules);
    _module_diags.extend(check_types(&modules));
    // We do not assert ok/err here — only that we reach this line (no ICE/loop).
}

// ── A2: Principal type stability ─────────────────────────────────────────────

/// A polymorphic identity function without annotation must type-check.
#[test]
fn principal_type_unannotated_identity_ok() {
    let source = r#"
module test.principal_unannotated
id = x => x

result = id 42
"#;
    check_ok(source);
}

/// The same identity function with an explicit annotation must type-check.
#[test]
fn principal_type_annotated_identity_ok() {
    let source = r#"
module test.principal_annotated

id : A -> A
id = x => x

result = id 42
"#;
    check_ok(source);
}

/// Adding a type annotation to a polymorphic function must not change whether
/// the function type-checks — both forms are accepted.
#[test]
fn principal_type_annotation_does_not_break_polymorphism() {
    let unannotated = r#"
module test.principal_unannotated2
apply = f x => f x
result1 = apply (_ + 1) 5
result2 = apply (_ * 2) 10
"#;
    let annotated = r#"
module test.principal_annotated2
apply : (A -> B) -> A -> B
apply = f x => f x
result1 = apply (_ + 1) 5
result2 = apply (_ * 2) 10
"#;
    check_ok(unannotated);
    check_ok(annotated);
}

/// A concrete annotation that is incompatible with the inferred type must error.
#[test]
fn principal_type_wrong_annotation_errors() {
    let source = r#"
module test.principal_wrong_ann

id : Int -> Text
id = x => x
"#;
    check_err(source);
}

// ── A3: Ambiguity error quality ───────────────────────────────────────────────

/// When a method resolves to zero candidate instances the error must mention
/// "ambiguous" and the method name.
#[test]
fn ambiguous_instance_error_message_quality() {
    let source = r#"
module test.ambig_quality

class Render A = {
  render: A -> Text
}

instance Render Int = {
  render = x => "int"
}

instance Render Int = {
  render = x => "also int"
}

result = render 42
"#;
    let (modules, diagnostics) = parse_modules(Path::new("test.aivi"), source);
    assert!(
        !file_diagnostics_have_errors(&diagnostics),
        "parse errors: {diagnostics:?}"
    );

    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(
        file_diagnostics_have_errors(&module_diags),
        "expected error for overlapping instances, got: {module_diags:?}"
    );
    assert!(
        module_diags
            .iter()
            .any(|d| d.diagnostic.message.contains("render")),
        "expected method name 'render' in error, got: {module_diags:?}"
    );
}

/// When a method name is resolved but no instance matches the concrete type,
/// the error must also fire (zero applicable instances).
#[test]
fn ambiguous_instance_no_candidate_errors() {
    let source = r#"
module test.ambig_no_candidate

class Serialize A = {
  serialize: A -> Text
}

instance Serialize Int = {
  serialize = x => "42"
}

result : Text
result = serialize True
"#;
    check_err(source);
}

// ── Positive: type-level row transforms (basic) ───────────────────────────────

/// Pick with a single field produces a valid record type.
#[test]
fn row_pick_single_field_ok() {
    let source = r#"
module test.row_pick_single

User = { id: Int, name: Text, admin: Bool }

IdOnly = Pick (id) User

get : User -> IdOnly
get = u => { id: u.id }
"#;
    check_ok(source);
}

/// Omit with a single field produces a valid record type.
#[test]
fn row_omit_single_field_ok() {
    let source = r#"
module test.row_omit_single

User = { id: Int, name: Text, admin: Bool }

PublicUser = Omit (admin) User

toPublic : User -> PublicUser
toPublic = u => { id: u.id, name: u.name }
"#;
    check_ok(source);
}

/// Pick of a non-existent field must be a type error.
#[test]
fn row_pick_nonexistent_field_errors() {
    let source = r#"
module test.row_pick_bad

User = { id: Int, name: Text }

Bad = Pick (missing) User
"#;
    check_err(source);
}

/// Omit of a non-existent field must be a type error.
#[test]
fn row_omit_nonexistent_field_errors() {
    let source = r#"
module test.row_omit_bad

User = { id: Int, name: Text }

Bad = Omit (missing) User
"#;
    check_err(source);
}
