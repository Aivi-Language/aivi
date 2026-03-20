use std::path::Path;

use aivi::surface::FlowLine;
use aivi::{parse_modules, DiagnosticSeverity, Expr, Literal, ModuleItem};

#[test]
fn parses_flow_binding_rhs_as_literal() {
    let src = r#"
module tmp
export main

main : Effect Text Int
main =
   |> pure 1#x
   |> pure x
"#;

    let (modules, diags) = parse_modules(Path::new("<test>"), src);
    assert!(
        diags
            .iter()
            .all(|d| d.diagnostic.severity != DiagnosticSeverity::Error),
        "unexpected parse diagnostics: {diags:?}"
    );

    let module = modules
        .iter()
        .find(|m| m.name.name == "tmp")
        .expect("module tmp");

    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "main" => Some(def),
            _ => None,
        })
        .expect("def main");

    let Expr::Flow { lines, .. } = &def.expr else {
        panic!("expected main body to be a flow, got: {:?}", def.expr);
    };
    let Some(FlowLine::Step(step)) = lines.first() else {
        panic!(
            "expected first flow line to be a step, got: {:?}",
            lines.first()
        );
    };
    assert!(
        step.binding
            .as_ref()
            .is_some_and(|binding| binding.name.name == "x"),
        "expected first flow line to bind x, got: {step:?}"
    );
    assert!(
        matches!(
            &step.expr,
            Expr::Call { func, args, .. }
                if matches!(func.as_ref(), Expr::Ident(name) if name.name == "pure")
                    && matches!(args.first(), Some(Expr::Literal(Literal::Number { .. })))
        ),
        "expected first flow expression to be `pure 1`, got: {:?}",
        step.expr
    );
}
