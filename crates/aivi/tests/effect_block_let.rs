use std::path::Path;

use aivi::hir::{HirBlockItem, HirExpr, HirTextPart};
use aivi::{
    desugar_modules, parse_modules, BlockItem, BlockKind, DiagnosticSeverity, Expr, Literal,
    ModuleItem,
};

#[test]
fn parses_effect_block_let_rhs_as_literal() {
    let src = r#"
module tmp
export main

main : Effect Text Int
main = do Effect {
  x = 1
  pure x
}
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

    let Expr::Block { kind, items, .. } = &def.expr else {
        panic!(
            "expected main body to be an effect block, got: {:?}",
            def.expr
        );
    };
    assert!(matches!(kind, BlockKind::Do { .. }));

    let Some(BlockItem::Let { expr, .. }) = items.first() else {
        panic!(
            "expected first block item to be Let, got: {:?}",
            items.first()
        );
    };
    assert!(
        matches!(expr, Expr::Literal(Literal::Number { .. })),
        "expected let RHS to parse as a number literal, got: {expr:?}"
    );
}

#[test]
fn parses_generate_loop_without_stub_raw_expr() {
    let src = r#"
module tmp
export g

g : Generator Int
g = generate {
  loop n = 0 => {
    recurse (n + 1)
  }
}
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
            ModuleItem::Def(def) if def.name.name == "g" => Some(def),
            _ => None,
        })
        .expect("def g");

    let Expr::Block { kind, items, .. } = &def.expr else {
        panic!("expected generate block, got: {:?}", def.expr);
    };
    assert!(matches!(kind, BlockKind::Generate));
    assert!(
        matches!(items.first(), Some(BlockItem::Let { .. })),
        "expected loop desugaring let binder, got: {items:?}"
    );
    assert!(
        matches!(
            items.get(1),
            Some(BlockItem::Expr {
                expr: Expr::Call { .. },
                ..
            })
        ),
        "expected loop desugaring call item, got: {items:?}"
    );

    let hir = desugar_modules(&modules);
    let hir_module = hir
        .modules
        .iter()
        .find(|m| m.name == "tmp")
        .expect("hir tmp");
    let hir_def = hir_module
        .defs
        .iter()
        .find(|d| d.name == "g")
        .expect("hir g");
    assert!(
        !hir_expr_has_recurse(&hir_def.expr),
        "expected generate loop to be lowered before HIR recurse handling"
    );
}

fn hir_expr_has_recurse(expr: &HirExpr) -> bool {
    match expr {
        HirExpr::Block { items, .. } => items.iter().any(|item| match item {
            HirBlockItem::Recurse { .. } => true,
            HirBlockItem::Bind { expr, .. } | HirBlockItem::Expr { expr } => {
                hir_expr_has_recurse(expr)
            }
            HirBlockItem::Yield { expr } | HirBlockItem::Filter { expr } => {
                hir_expr_has_recurse(expr)
            }
        }),
        HirExpr::Lambda { body, .. } => hir_expr_has_recurse(body),
        HirExpr::App { func, arg, .. } => hir_expr_has_recurse(func) || hir_expr_has_recurse(arg),
        HirExpr::Call { func, args, .. } => {
            hir_expr_has_recurse(func) || args.iter().any(hir_expr_has_recurse)
        }
        HirExpr::Match {
            scrutinee, arms, ..
        } => {
            hir_expr_has_recurse(scrutinee)
                || arms.iter().any(|arm| hir_expr_has_recurse(&arm.body))
        }
        HirExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            hir_expr_has_recurse(cond)
                || hir_expr_has_recurse(then_branch)
                || hir_expr_has_recurse(else_branch)
        }
        HirExpr::Binary { left, right, .. } => {
            hir_expr_has_recurse(left) || hir_expr_has_recurse(right)
        }
        HirExpr::Tuple { items, .. } => items.iter().any(hir_expr_has_recurse),
        HirExpr::List { items, .. } => items.iter().any(|item| hir_expr_has_recurse(&item.expr)),
        HirExpr::Record { fields, .. } => fields
            .iter()
            .any(|field| hir_expr_has_recurse(&field.value)),
        HirExpr::Patch { target, fields, .. } => {
            hir_expr_has_recurse(target)
                || fields
                    .iter()
                    .any(|field| hir_expr_has_recurse(&field.value))
        }
        HirExpr::FieldAccess { base, .. } => hir_expr_has_recurse(base),
        HirExpr::Index { base, index, .. } => {
            hir_expr_has_recurse(base) || hir_expr_has_recurse(index)
        }
        HirExpr::TextInterpolate { parts, .. } => parts.iter().any(|part| match part {
            HirTextPart::Expr { expr } => hir_expr_has_recurse(expr),
            _ => false,
        }),
        HirExpr::DebugFn { body, .. } => hir_expr_has_recurse(body),
        HirExpr::Pipe { func, arg, .. } => hir_expr_has_recurse(func) || hir_expr_has_recurse(arg),
        HirExpr::Var { .. }
        | HirExpr::LitNumber { .. }
        | HirExpr::LitString { .. }
        | HirExpr::LitSigil { .. }
        | HirExpr::LitBool { .. }
        | HirExpr::LitDateTime { .. }
        | HirExpr::Raw { .. } => false,
    }
}
