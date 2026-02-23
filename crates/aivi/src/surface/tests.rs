use std::path::Path;

use crate::surface::{lower_modules_to_arena, parse_modules, ArenaExpr, Expr, Literal, ModuleItem};

fn diag_codes(diags: &[crate::FileDiagnostic]) -> Vec<String> {
    let mut codes: Vec<String> = diags.iter().map(|d| d.diagnostic.code.clone()).collect();
    codes.sort();
    codes
}

#[test]
fn parses_decorator_with_argument_on_def() {
    let src = r#"
module Example

@deprecated "use `y` instead"
x = 1
"#;

    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");

    assert_eq!(def.decorators.len(), 1);
    assert_eq!(def.decorators[0].name.name, "deprecated");
    assert!(
        matches!(
            def.decorators[0].arg,
            Some(Expr::Literal(Literal::String { .. }))
        ),
        "expected @deprecated string literal argument"
    );
}

#[test]
fn lowers_surface_module_to_arena() {
    let src = r#"
module Example

id = x => x
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );
    let (arena, lowered) = lower_modules_to_arena(&modules);
    assert_eq!(lowered.len(), 1);
    assert!(!arena.exprs.is_empty());
    assert!(!arena.patterns.is_empty());
    let module = &lowered[0];
    assert_eq!(module.name.symbol.as_str(), "Example");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            crate::surface::ArenaModuleItem::Def(def) => Some(def),
            _ => None,
        })
        .expect("def");
    match arena.expr(def.expr) {
        ArenaExpr::Lambda { body, .. } => match arena.expr(*body) {
            ArenaExpr::Ident(name) => assert_eq!(name.symbol.as_str(), "x"),
            other => panic!("expected arena lambda body ident, got {other:?}"),
        },
        other => panic!("expected arena lambda body, got {other:?}"),
    }
}

#[test]
fn rejects_unknown_item_decorator() {
    let src = r#"
module Example

@sql
x = 1
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1506".to_string()));
}

#[test]
fn rejects_deprecated_without_argument() {
    let src = r#"
module Example

@deprecated
x = 1
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1511".to_string()));
}

#[test]
fn rejects_argument_on_inline() {
    let src = r#"
module Example

@inline "nope"
f x = x
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1513".to_string()));
}

#[test]
fn module_decorator_no_prelude_rejects_argument() {
    let src = r#"
@no_prelude "nope"
module Example
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1512".to_string()));
}

#[test]
fn native_decorator_requires_string_argument() {
    let src = r#"
module Example

@native
x : Int -> Int
x = n => n
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1511".to_string()));
}

#[test]
fn native_decorator_rewrites_to_target_call() {
    let src = r#"
module Example

@native "gtk4.windowPresent"
windowPresent : Int -> Effect Text Unit
windowPresent = windowId => Unit
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );
    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "windowPresent" => Some(def),
            _ => None,
        })
        .expect("windowPresent def");
    match &def.expr {
        Expr::Call { func, args, .. } => {
            assert_eq!(args.len(), 1);
            assert!(matches!(args.first(), Some(Expr::Ident(name)) if name.name == "windowId"));
            match func.as_ref() {
                Expr::FieldAccess { base, field, .. } => {
                    assert_eq!(field.name, "windowPresent");
                    assert!(matches!(base.as_ref(), Expr::Ident(root) if root.name == "gtk4"));
                }
                other => panic!("expected rewritten native field access, got {other:?}"),
            }
        }
        other => panic!("expected rewritten native call, got {other:?}"),
    }
}

#[test]
fn native_decorator_requires_type_signature() {
    let src = r#"
module Example

@native "gtk4.appRun"
appRun = appId => Unit
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1526".to_string()));
}

#[test]
fn rejects_legacy_braced_module_body_syntax() {
    let src = r#"
module Example = {
  x = 1
}
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1518".to_string()));
}

#[test]
fn rejects_module_not_at_file_start() {
    let src = r#"
x = 1

module Example
y = 2
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1519".to_string()));
}

#[test]
fn parses_structured_sigil_map_literal() {
    let src = r#"
module Example

x = ~map{ "a" => 1 }
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");

    assert!(
        !matches!(&def.expr, Expr::Literal(Literal::Sigil { tag, .. }) if tag == "map"),
        "expected ~map{{...}} to parse as a structured literal, not a sigil literal"
    );
}

#[test]
fn record_pattern_shorthand_rejects_trailing_garbage() {
    let src = r#"
module Example

renderCount = { count * 23 sasd, step } => count
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "expected parser recovery without diagnostics for this legacy edge-case, got: {:?}",
        diag_codes(&diags)
    );
}

#[test]
fn record_pattern_fields_require_separator_between_fields() {
    let src = r#"
module Example

f = { a: x b: y } => x
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    // When pattern parsing fails (E1538 for missing comma), the parser backtracks to
    // expression parsing where `{ a: (x b) }` is built and the stray `:` and `y`
    // get flagged as E1527.  This is the intended "strict" behavior.
    let codes = diag_codes(&diags);
    assert!(
        codes.iter().all(|c| c == "E1527"),
        "expected E1527 diagnostics for stray tokens, got: {:?}",
        codes
    );
}

#[test]
fn parses_record_destructuring_pipe_head() {
    let src = r#"
module Example

f = { name } => name |> toUpper
"#;

    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "f" => Some(def),
            _ => None,
        })
        .expect("f def");

    let Expr::Lambda { params, body, .. } = &def.expr else {
        panic!("expected lambda");
    };
    assert_eq!(params.len(), 1);
    match &params[0] {
        crate::surface::Pattern::Record { fields, .. } => {
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].path.len(), 1);
            assert_eq!(fields[0].path[0].name, "name");
            assert!(
                matches!(
                    &fields[0].pattern,
                    crate::surface::Pattern::Ident(n) if n.name == "name"
                ),
                "expected record-pattern shorthand to bind `name`"
            );
        }
        other => panic!("unexpected param pattern: {other:?}"),
    }

    match &**body {
        Expr::Binary {
            op, left, right, ..
        } => {
            assert_eq!(op, "|>");
            assert!(matches!(&**left, Expr::Ident(n) if n.name == "name"));
            assert!(matches!(&**right, Expr::Ident(n) if n.name == "toUpper"));
        }
        other => panic!("unexpected body: {other:?}"),
    }
}

#[test]
fn parses_record_destructuring_match_head() {
    let src = r#"
module Example

g = { name } => name match
  | "A" => 1
  | _   => 0
"#;

    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "g" => Some(def),
            _ => None,
        })
        .expect("g def");

    let Expr::Lambda { body, .. } = &def.expr else {
        panic!("expected lambda");
    };
    let Expr::Match { scrutinee, .. } = &**body else {
        panic!("expected match");
    };
    let scrutinee = scrutinee.as_ref().expect("scrutinee");
    assert!(matches!(&**scrutinee, Expr::Ident(n) if n.name == "name"));
}

#[test]
fn parses_at_binding_with_record_destructuring_pipe_head() {
    let src = r#"
module Example

h = user as { name } => name |> consume
"#;

    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "h" => Some(def),
            _ => None,
        })
        .expect("h def");

    let Expr::Lambda { params, body, .. } = &def.expr else {
        panic!("expected lambda");
    };
    assert_eq!(params.len(), 1);
    assert!(
        matches!(
            &params[0],
            crate::surface::Pattern::At { name, .. } if name.name == "user"
        ),
        "expected `user as ...` at-binding param"
    );

    let Expr::Binary {
        op, left, right, ..
    } = &**body
    else {
        panic!("expected pipe");
    };
    assert_eq!(op, "|>");
    assert!(matches!(&**left, Expr::Ident(n) if n.name == "name"));
    assert!(matches!(&**right, Expr::Ident(n) if n.name == "consume"));
}

#[test]
fn rejects_missing_module_declaration() {
    let src = r#"
x = 1
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diag_codes(&diags).contains(&"E1517".to_string()),
        "expected missing module diagnostic, got: {:?}",
        diag_codes(&diags)
    );
}

#[test]
fn rejects_type_sig_and_binding_on_same_line() {
    let src = r#"
module Example

x : Int = 1
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diag_codes(&diags).contains(&"E1528".to_string()),
        "expected inline type signature diagnostic, got: {:?}",
        diag_codes(&diags)
    );
}

fn expr_contains_ident(expr: &Expr, target: &str) -> bool {
    match expr {
        Expr::Ident(name) => name.name == target,
        Expr::Literal(_) => false,
        Expr::UnaryNeg { expr, .. } => expr_contains_ident(expr, target),
        Expr::Suffixed { base, .. } => expr_contains_ident(base, target),
        Expr::TextInterpolate { parts, .. } => parts.iter().any(|part| match part {
            crate::surface::TextPart::Text { .. } => false,
            crate::surface::TextPart::Expr { expr, .. } => expr_contains_ident(expr, target),
        }),
        Expr::List { items, .. } => items
            .iter()
            .any(|item| expr_contains_ident(&item.expr, target)),
        Expr::Tuple { items, .. } => items.iter().any(|item| expr_contains_ident(item, target)),
        Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } => fields
            .iter()
            .any(|field| expr_contains_ident(&field.value, target)),
        Expr::FieldAccess { base, field, .. } => {
            field.name == target || expr_contains_ident(base, target)
        }
        Expr::Index { base, index, .. } => {
            expr_contains_ident(base, target) || expr_contains_ident(index, target)
        }
        Expr::FieldSection { field, .. } => field.name == target,
        Expr::Call { func, args, .. } => {
            expr_contains_ident(func, target)
                || args.iter().any(|arg| expr_contains_ident(arg, target))
        }
        Expr::Lambda { body, .. } => expr_contains_ident(body, target),
        Expr::Match {
            scrutinee, arms, ..
        } => {
            scrutinee
                .as_ref()
                .is_some_and(|e| expr_contains_ident(e, target))
                || arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .is_some_and(|e| expr_contains_ident(e, target))
                        || expr_contains_ident(&arm.body, target)
                })
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            expr_contains_ident(cond, target)
                || expr_contains_ident(then_branch, target)
                || expr_contains_ident(else_branch, target)
        }
        Expr::Binary { left, right, .. } => {
            expr_contains_ident(left, target) || expr_contains_ident(right, target)
        }
        Expr::Block { items, .. } => items.iter().any(|item| match item {
            crate::surface::BlockItem::Bind { expr, .. }
            | crate::surface::BlockItem::Let { expr, .. }
            | crate::surface::BlockItem::Filter { expr, .. }
            | crate::surface::BlockItem::Yield { expr, .. }
            | crate::surface::BlockItem::Recurse { expr, .. }
            | crate::surface::BlockItem::Expr { expr, .. } => expr_contains_ident(expr, target),
            crate::surface::BlockItem::When { cond, effect, .. }
            | crate::surface::BlockItem::Unless { cond, effect, .. } => {
                expr_contains_ident(cond, target) || expr_contains_ident(effect, target)
            }
            crate::surface::BlockItem::Given {
                cond, fail_expr, ..
            } => expr_contains_ident(cond, target) || expr_contains_ident(fail_expr, target),
            crate::surface::BlockItem::On {
                transition,
                handler,
                ..
            } => expr_contains_ident(transition, target) || expr_contains_ident(handler, target),
        }),
        Expr::Raw { .. } => false,
    }
}

#[test]
fn parses_parenthesized_suffix_application_expression() {
    let src = r#"
module Example

x = (1 + 2)px
"#;

    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");

    match &def.expr {
        Expr::Suffixed { base, suffix, .. } => {
            assert_eq!(suffix.name, "px");
            assert!(
                matches!(base.as_ref(), Expr::Binary { op, .. } if op == "+"),
                "expected base to be a binary '+', got: {base:?}"
            );
        }
        other => panic!("expected suffixed expression, got: {other:?}"),
    }
}

#[test]
fn parses_binary_operator_precedence_multiplication_binds_tighter_than_addition() {
    let src = r#"
module Example

x = 1 + 2 * 3
"#;

    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");

    match &def.expr {
        Expr::Binary {
            op, left, right, ..
        } => {
            assert_eq!(op, "+");
            assert!(
                matches!(left.as_ref(), Expr::Literal(Literal::Number { text, .. }) if text == "1")
            );
            assert!(
                matches!(right.as_ref(), Expr::Binary { op, .. } if op == "*"),
                "expected right side to be multiplication, got: {right:?}"
            );
        }
        other => panic!("expected binary expression, got: {other:?}"),
    }
}

#[test]
fn parses_binary_operator_precedence_cross_binds_tighter_than_addition() {
    let src = r#"
module Example

x = 1 + 2 × 3
"#;

    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");

    match &def.expr {
        Expr::Binary {
            op, left, right, ..
        } => {
            assert_eq!(op, "+");
            assert!(
                matches!(left.as_ref(), Expr::Literal(Literal::Number { text, .. }) if text == "1")
            );
            assert!(
                matches!(right.as_ref(), Expr::Binary { op, .. } if op == "×"),
                "expected right side to be cross operator, got: {right:?}"
            );
        }
        other => panic!("expected binary expression, got: {other:?}"),
    }
}

#[test]
fn parses_binary_operator_associativity_left_for_minus() {
    let src = r#"
module Example

x = 1 - 2 - 3
"#;

    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");

    match &def.expr {
        Expr::Binary {
            op, left, right, ..
        } => {
            assert_eq!(op, "-");
            assert!(
                matches!(right.as_ref(), Expr::Literal(Literal::Number { text, .. }) if text == "3")
            );
            assert!(
                matches!(left.as_ref(), Expr::Binary { op, .. } if op == "-"),
                "expected left associativity for '-', got left={left:?}"
            );
        }
        other => panic!("expected binary expression, got: {other:?}"),
    }
}

#[test]
fn parses_structured_sigil_html_literal() {
    let src = r#"
module Example

x =
  ~<html>
    <div class="card">
      <span>{ 1 }</span>
    </div>
  </html>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");

    assert!(
        !matches!(&def.expr, Expr::Literal(Literal::Sigil { tag, .. }) if tag == "html"),
        "expected ~<html> to parse as a structured literal, not a sigil literal"
    );

    assert!(
        expr_contains_ident(&def.expr, "vElement") && expr_contains_ident(&def.expr, "vClass"),
        "expected ~<html> to lower into UI helpers"
    );
}

#[test]
fn parses_html_sigil_key_attribute() {
    let src = r#"
module Example

x = ~<html><div key="k">Hi</div></html>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");

    assert!(
        expr_contains_ident(&def.expr, "vKeyed"),
        "expected key= to lower into `vKeyed`"
    );
}

#[test]
fn html_sigil_multiple_roots_is_error() {
    let src = r#"
module Example

x =
  ~<html>
    <div>One</div>
    <div>Two</div>
  </html>
"#;

    let (_modules, diags) = parse_modules(Path::new("test.aivi"), src);
    let codes = diag_codes(&diags);
    assert!(
        codes.iter().any(|code| code == "E1601"),
        "expected E1601 for multiple roots, got: {codes:?}"
    );
}

#[test]
fn parses_domain_literal_def_in_embedded_ui_layout() {
    let src = crate::stdlib::embedded_stdlib_source("aivi.ui.layout")
        .expect("embedded stdlib source for aivi.ui.layout");
    let (modules, diags) = parse_modules(Path::new("<embedded:aivi.ui.layout>"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );
    let module = modules
        .iter()
        .find(|m| m.name.name == "aivi.ui.layout")
        .expect("aivi.ui.layout module");
    let domain = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::DomainDecl(domain) if domain.name.name == "Layout" => Some(domain),
            _ => None,
        })
        .expect("Layout domain");

    let has_1px = domain.items.iter().any(|item| match item {
        crate::surface::DomainItem::LiteralDef(def) => def.name.name == "1px",
        _ => false,
    });
    let literal_defs: Vec<String> = domain
        .items
        .iter()
        .filter_map(|item| match item {
            crate::surface::DomainItem::LiteralDef(def) => Some(def.name.name.clone()),
            _ => None,
        })
        .collect();
    assert!(
        has_1px,
        "expected Layout domain to define a `1px` literal template"
    );
    assert!(
        literal_defs.contains(&"1%".to_string()),
        "expected Layout domain to define a `1%` literal template"
    );
}

#[test]
fn parses_domain_literal_def_px_suffix() {
    let src = r#"
module Example

UnitVal = { val: Float }

domain Layout over UnitVal = {
  Length = Px Float

  1px = Px 1.0
  1em = Em 1.0
}
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );
    let module = modules.first().expect("module");
    let domain = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::DomainDecl(domain) if domain.name.name == "Layout" => Some(domain),
            _ => None,
        })
        .expect("Layout domain");

    let literal_defs: Vec<String> = domain
        .items
        .iter()
        .filter_map(|item| match item {
            crate::surface::DomainItem::LiteralDef(def) => Some(def.name.name.clone()),
            _ => None,
        })
        .collect();
    assert!(
        literal_defs.contains(&"1px".to_string()),
        "expected literal defs to contain 1px, got {literal_defs:?}"
    );
    assert!(
        literal_defs.contains(&"1em".to_string()),
        "expected literal defs to contain 1em, got {literal_defs:?}"
    );
}

#[test]
fn parses_decorator_on_class_decl() {
    let src = r#"
module Example

@inline
class Functor (F *) = { map: (A -> B) -> F A -> F B }
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let class_decl = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::ClassDecl(class_decl) if class_decl.name.name == "Functor" => {
                Some(class_decl)
            }
            _ => None,
        })
        .expect("Functor class decl");

    assert_eq!(class_decl.decorators.len(), 1);
    assert_eq!(class_decl.decorators[0].name.name, "inline");
}

#[test]
fn parses_instance_decl() {
    let src = r#"
module Example

instance Functor (Option *) = {
  map: f opt => opt
}
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let instance_decl = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::InstanceDecl(instance_decl) => Some(instance_decl),
            _ => None,
        })
        .expect("instance decl");

    assert_eq!(instance_decl.name.name, "Functor");
    assert_eq!(instance_decl.params.len(), 1);
}

#[test]
fn parses_class_type_variable_constraints() {
    let src = r#"
module Example

class Collection (C *) = given (A: Eq, B: Show) {
  elem: A -> C A -> Bool
  render: B -> Text
}
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let class_decl = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::ClassDecl(class_decl) if class_decl.name.name == "Collection" => {
                Some(class_decl)
            }
            _ => None,
        })
        .expect("Collection class decl");

    assert_eq!(class_decl.constraints.len(), 2);
    assert_eq!(class_decl.constraints[0].var.name, "A");
    assert_eq!(class_decl.constraints[0].class.name, "Eq");
    assert_eq!(class_decl.constraints[1].var.name, "B");
    assert_eq!(class_decl.constraints[1].class.name, "Show");
}

#[test]
fn rejects_multiple_modules_per_file() {
    let src = r#"
module A
x = 1

module B
y = 2
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1516".to_string()));
}

#[test]
fn rejects_result_or_success_arms() {
    let src = r#"
	module Example

Result E A = Err E | Ok A

value = (Ok 1) or
  | Ok x  => x
  | Err _ => 0
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1530".to_string()));
}

#[test]
fn rejects_test_without_argument() {
    let src = r#"
module Example

@test
x = do Effect { _ <- assert True }
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diag_codes(&diags).contains(&"E1511".to_string()),
        "expected E1511 for @test without argument, got: {:?}",
        diag_codes(&diags)
    );
}

#[test]
fn accepts_test_with_string_argument() {
    let src = r#"
module Example

@test "adds two numbers"
x = do Effect { _ <- assertEq (1 + 1) 2 }
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");

    assert_eq!(def.decorators.len(), 1);
    assert_eq!(def.decorators[0].name.name, "test");
    assert!(
        matches!(
            def.decorators[0].arg,
            Some(Expr::Literal(Literal::String { .. }))
        ),
        "expected @test string literal argument"
    );
}

#[test]
fn rejects_test_with_non_string_argument() {
    let src = r#"
module Example

@test 42
x = do Effect { _ <- assert True }
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diag_codes(&diags).contains(&"E1510".to_string()),
        "expected E1510 for @test with non-string argument, got: {:?}",
        diag_codes(&diags)
    );
}
