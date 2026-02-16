use std::path::Path;

use aivi::{parse_modules, BlockItem, BlockKind, DiagnosticSeverity, Expr, Literal, ModuleItem};

#[test]
fn parses_effect_block_let_rhs_as_literal() {
    let src = r#"
module tmp
export main

main : Effect Text Int
main = effect {
  x = 1
  pure x
}
"#;

    let (modules, diags) = parse_modules(Path::new("<test>"), src);
    assert!(
        diags.iter()
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

    let Expr::Block { kind, items, .. } = &def.expr else {
        panic!(
            "expected main body to be an effect block, got: {:?}",
            def.expr
        );
    };
    assert!(matches!(kind, BlockKind::Effect));

    let Some(BlockItem::Let { expr, .. }) = items.first() else {
        panic!("expected first block item to be Let, got: {:?}", items.first());
    };
    assert!(
        matches!(expr, Expr::Literal(Literal::Number { .. })),
        "expected let RHS to parse as a number literal, got: {expr:?}"
    );
}
