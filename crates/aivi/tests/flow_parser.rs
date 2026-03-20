use std::path::Path;

use aivi::surface::{FlowLine, FlowStepKind};
use aivi::{parse_modules, Expr, ModuleItem};

fn parse_main_expr(source: &str) -> Expr {
    let (modules, diags) = parse_modules(Path::new("test.aivi"), source);
    assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:#?}");
    let module = modules
        .iter()
        .find(|m| m.name.name == "app.main")
        .expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "main" => Some(def),
            _ => None,
        })
        .expect("main def");
    def.expr.clone()
}

#[test]
fn multiline_flow_rhs_parses_as_one_flow() {
    let expr = parse_main_expr(
        r#"module app.main
export main

main =
  1
    ~|> _ + 1
     |> _ + 0
"#,
    );

    let Expr::Flow { root, lines, .. } = expr else {
        panic!("expected flow rhs");
    };
    assert!(matches!(root.as_ref(), Expr::Literal(_)));
    assert_eq!(lines.len(), 2);
    assert!(matches!(lines[0], FlowLine::Step(ref step) if step.kind == FlowStepKind::Tap));
    assert!(matches!(lines[1], FlowLine::Step(ref step) if step.kind == FlowStepKind::Flow));
}

#[test]
fn lambda_body_keeps_flow_inside_lambda() {
    let expr = parse_main_expr(
        r#"module app.main
export main

main = config =>
  config
    ~|> (_ => init Unit)
     |> (_ => appNew config.appId)
"#,
    );

    let Expr::Lambda { body, .. } = expr else {
        panic!("expected lambda");
    };
    let Expr::Flow { root, lines, .. } = body.as_ref() else {
        panic!("expected flow lambda body");
    };
    assert!(matches!(root.as_ref(), Expr::Ident(_)));
    assert_eq!(lines.len(), 2);
    assert!(matches!(lines[0], FlowLine::Step(ref step) if step.kind == FlowStepKind::Tap));
    assert!(matches!(lines[1], FlowLine::Step(ref step) if step.kind == FlowStepKind::Flow));
}

#[test]
fn flow_step_lambda_body_does_not_capture_sibling_lines() {
    let expr = parse_main_expr(
        r#"module app.main
export main

main =
  Unit
    |> _ => pure 5
    |> _ + 1
    |> assertEq 6
"#,
    );

    let Expr::Flow { root, lines, .. } = expr else {
        panic!("expected outer flow");
    };
    assert!(matches!(root.as_ref(), Expr::Ident(_)));
    assert_eq!(lines.len(), 3);

    let FlowLine::Step(first_step) = &lines[0] else {
        panic!("expected first flow step");
    };
    assert_eq!(first_step.kind, FlowStepKind::Flow);

    let Expr::Lambda { body, .. } = &first_step.expr else {
        panic!("expected first step to be a lambda");
    };
    assert!(matches!(body.as_ref(), Expr::Call { .. }));
    assert!(!matches!(body.as_ref(), Expr::Flow { .. }));

    assert!(matches!(lines[1], FlowLine::Step(ref step) if step.kind == FlowStepKind::Flow));
    assert!(matches!(lines[2], FlowLine::Step(ref step) if step.kind == FlowStepKind::Flow));
}

#[test]
fn attempt_and_recover_share_one_flow() {
    let expr = parse_main_expr(
        r#"module app.main
export main

main =
  42
    ?|> risky
    !|> "forty-two" => 7
"#,
    );

    let Expr::Flow { lines, .. } = expr else {
        panic!("expected flow rhs");
    };
    assert_eq!(lines.len(), 2);
    assert!(matches!(lines[0], FlowLine::Step(ref step) if step.kind == FlowStepKind::Attempt));
    assert!(matches!(lines[1], FlowLine::Recover(_)));
}

#[test]
fn operator_first_flow_uses_implicit_unit_root() {
    let expr = parse_main_expr(
        r#"module app.main
export main

main = &|> assertEq 1 1
       &|> assertEq 2 2
"#,
    );

    let Expr::Flow { root, lines, .. } = expr else {
        panic!("expected flow rhs");
    };
    let Expr::Ident(root_name) = root.as_ref() else {
        panic!("expected implicit Unit root");
    };
    assert_eq!(root_name.name, "Unit");
    assert_eq!(lines.len(), 2);
    assert!(matches!(lines[0], FlowLine::Step(ref step) if step.kind == FlowStepKind::Applicative));
    assert!(matches!(lines[1], FlowLine::Step(ref step) if step.kind == FlowStepKind::Applicative));
}

#[test]
fn fanout_uses_explicit_end_marker() {
    let expr = parse_main_expr(
        r#"module app.main
export main

main =
  users
    *|> _ #user
    >|> user.active
     |> user.id
    *-|
     |> toSet
"#,
    );

    let Expr::Flow { root, lines, .. } = expr else {
        panic!("expected flow rhs");
    };
    assert!(matches!(root.as_ref(), Expr::Ident(_)));
    assert_eq!(lines.len(), 2);
    let FlowLine::Step(step) = &lines[0] else {
        panic!("expected fan-out step");
    };
    assert_eq!(step.kind, FlowStepKind::FanOut);
    assert_eq!(step.subflow.len(), 2);
    assert!(matches!(step.subflow[0], FlowLine::Guard(_)));
    assert!(
        matches!(step.subflow[1], FlowLine::Step(ref inner) if inner.kind == FlowStepKind::Flow)
    );
    assert!(matches!(lines[1], FlowLine::Step(ref step) if step.kind == FlowStepKind::Flow));
}

#[test]
fn removed_continuation_binding_reports_parse_diag() {
    let (_, diags) = parse_modules(
        Path::new("test.aivi"),
        r#"module app.main
export main

main =
  token
    &|> fetchProfile token #profile!
"#,
    );
    assert!(
        diags
            .iter()
            .any(|diag| diag.diagnostic.message.contains("`#name!` was removed")),
        "expected removed continuation binding diagnostic, got: {diags:#?}"
    );
}

#[test]
fn missing_fanout_end_reports_parse_diag() {
    let (_, diags) = parse_modules(
        Path::new("test.aivi"),
        r#"module app.main
export main

main =
  users
    *|> _
     |> _.id
"#,
    );
    assert!(
        diags
            .iter()
            .any(|diag| diag.diagnostic.message.contains("expected `*-|`")),
        "expected missing fan-out terminator diagnostic, got: {diags:#?}"
    );
}
