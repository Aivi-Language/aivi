use std::path::Path;

use crate::surface::parse_modules;
use crate::typecheck::check_types;

fn has_errors(diags: &[crate::diagnostics::FileDiagnostic]) -> bool {
    diags
        .iter()
        .any(|d| d.diagnostic.severity == crate::diagnostics::DiagnosticSeverity::Error)
}

#[test]
fn class_member_constraints_allow_ambiguous_method_calls() {
    let src = r#"
module Example

class Eq A = {
  eq: A -> A -> Bool
}

// Two instances that are only distinguishable by constructor patterns at runtime.
instance Eq (Option X) = {
  eq: x y =>
    (x, y) ?
      | (None, None) => True
      | (Some _, Some _) => True
      | _ => False
}

instance Eq (Result E X) = {
  eq: x y =>
    (x, y) ?
      | (Ok _, Ok _) => True
      | (Err _, Err _) => True
      | _ => False
}

class NeedsEq = given (A: Eq) {
  same: A -> Bool
}

instance NeedsEq = {
  same: x => eq x x
}
"#;

    let (modules, parse_diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        !has_errors(&parse_diags),
        "unexpected parse errors: {parse_diags:?}"
    );

    let type_diags = check_types(&modules);
    assert!(
        !has_errors(&type_diags),
        "unexpected type errors: {type_diags:?}"
    );
}

#[test]
fn repeated_function_defs_require_explicit_signature() {
    let src = r#"
module Example

getNickName = ({ name: "Andreas" }) => "Andy"
getNickName = (_) => "Friend"
"#;

    let (modules, parse_diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        !has_errors(&parse_diags),
        "unexpected parse errors: {parse_diags:?}"
    );

    let type_diags = check_types(&modules);
    assert!(
        type_diags.iter().any(|diag| {
            diag.diagnostic.severity == crate::diagnostics::DiagnosticSeverity::Error
                && diag
                    .diagnostic
                    .message
                    .contains("requires an explicit type signature")
        }),
        "expected missing signature error for repeated defs, got: {type_diags:?}"
    );
}

#[test]
fn repeated_function_defs_with_signature_typecheck() {
    let src = r#"
module Example

getNickName : { name: Text } -> Text
getNickName = ({ name: "Andreas" }) => "Andy"
getNickName = (_) => "Friend"
"#;

    let (modules, parse_diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        !has_errors(&parse_diags),
        "unexpected parse errors: {parse_diags:?}"
    );

    let type_diags = check_types(&modules);
    assert!(
        !has_errors(&type_diags),
        "unexpected type errors: {type_diags:?}"
    );
}
