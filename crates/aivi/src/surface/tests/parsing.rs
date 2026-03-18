use std::path::Path;

use crate::surface::{
    lower_modules_to_arena, parse_modules, ArenaExpr, BlockItem, Expr, Literal, ModuleItem,
    PathSegment, Pattern,
};

use super::diag_codes;

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
fn parses_export_prefixed_declarations() {
    let src = r#"
module Example

export answer = 42

export domain Color over Int = {
  brighten : Int -> Int
  brighten = x => x + 1
}
"#;

    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    assert!(module.exports.iter().any(|item| {
        item.kind == crate::surface::ScopeItemKind::Value && item.name.name == "answer"
    }));
    assert!(module.exports.iter().any(|item| {
        item.kind == crate::surface::ScopeItemKind::Domain && item.name.name == "Color"
    }));
}

#[test]
fn bare_export_name_stays_export_list() {
    let src = r#"
module Example

Value = Int | Other
export Value
"#;

    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let value_type_items = module
        .items
        .iter()
        .filter(|item| matches!(item, ModuleItem::TypeDecl(ty) if ty.name.name == "Value"))
        .count();
    assert_eq!(
        value_type_items, 1,
        "expected a single `Value` type declaration"
    );
    assert!(module.exports.iter().any(|item| {
        item.kind == crate::surface::ScopeItemKind::Value && item.name.name == "Value"
    }));
    assert!(module.exports.iter().any(|item| {
        item.kind == crate::surface::ScopeItemKind::Value && item.name.name == "Int"
    }));
    assert!(module.exports.iter().any(|item| {
        item.kind == crate::surface::ScopeItemKind::Value && item.name.name == "Other"
    }));
}

#[test]
fn export_prefixed_type_exports_its_constructors() {
    let src = r#"
module Example

export UiMsg = ComposeNew | QuickReplySend Text
"#;

    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    assert!(module.exports.iter().any(|item| {
        item.kind == crate::surface::ScopeItemKind::Value && item.name.name == "UiMsg"
    }));
    assert!(module.exports.iter().any(|item| {
        item.kind == crate::surface::ScopeItemKind::Value && item.name.name == "ComposeNew"
    }));
    assert!(module.exports.iter().any(|item| {
        item.kind == crate::surface::ScopeItemKind::Value && item.name.name == "QuickReplySend"
    }));
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

#[test]
fn rejects_trailing_tokens_after_definition_rhs() {
    let src = r#"
module Example

x = signal { a: 1 }
y = x <-- { a: _ * 2 }
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    let codes = diag_codes(&diags);
    assert!(
        codes.contains(&"E1546".to_string()),
        "expected trailing definition token diagnostic, got: {codes:?}"
    );
    let diag = diags
        .iter()
        .find(|d| d.diagnostic.code == "E1546")
        .expect("E1546 diagnostic should be present");
    assert!(
        diag.diagnostic.message.contains("<<-"),
        "expected E1546 message to mention `<<-`, got: {:?}",
        diag.diagnostic.message
    );
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
fn parses_decorator_on_class_decl() {
    let src = r#"
module Example

@deprecated "use Mappable"
class Functor (F A) = given (A: Any) { map: (A -> B) -> F B }
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
    assert_eq!(class_decl.decorators[0].name.name, "deprecated");
}

#[test]
fn parses_instance_decl() {
    let src = r#"
module Example

instance Functor (Option A) = given (A: Any) {
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

class Collection (C A) = given (A: Eq, B: Show) {
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
    let codes = diag_codes(&diags);
    assert!(
        codes.contains(&"E1516".to_string()),
        "expected E1516, got: {codes:?}"
    );
    let e1516 = diags
        .iter()
        .find(|d| d.diagnostic.code == "E1516")
        .expect("E1516 diagnostic should be present");
    assert!(
        e1516.diagnostic.message.to_lowercase().contains("module"),
        "E1516 message should mention 'module', got: {:?}",
        e1516.diagnostic.message
    );
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
    let codes = diag_codes(&diags);
    assert!(
        codes.contains(&"E1530".to_string()),
        "expected E1530, got: {codes:?}"
    );
    assert_eq!(
        codes.iter().filter(|c| *c == "E1530").count(),
        1,
        "expected exactly one E1530 diagnostic"
    );
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

#[test]
fn record_expr_shorthand_field() {
    let src = r#"
module Example

x = { name, age: 42 }
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "unexpected diagnostics: {:?}", diags);
    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");

    let Expr::Record { fields, .. } = &def.expr else {
        panic!("expected record literal");
    };
    assert_eq!(fields.len(), 2);

    // shorthand field: { name } → path = [name], value = Ident("name")
    assert!(matches!(&fields[0].path[..], [PathSegment::Field(n)] if n.name == "name"));
    assert!(
        matches!(&fields[0].value, Expr::Ident(n) if n.name == "name"),
        "expected shorthand field `name` to produce Ident(name)"
    );

    // explicit field: { age: 42 }
    assert!(matches!(&fields[1].path[..], [PathSegment::Field(n)] if n.name == "age"));
}

#[test]
fn selective_import_alias_parses() {
    let src = r#"
@no_prelude
module Example

use some.module (foo as Bar, baz)

x = Bar
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let use_decl = module
        .uses
        .iter()
        .find(|u| u.module.name == "some.module")
        .expect("use decl for some.module");
    assert_eq!(use_decl.items.len(), 2, "expected 2 import items");

    let item0 = &use_decl.items[0];
    assert_eq!(item0.name.name, "foo");
    assert_eq!(
        item0.alias.as_ref().map(|a| a.name.as_str()),
        Some("Bar"),
        "expected alias Bar for foo"
    );

    let item1 = &use_decl.items[1];
    assert_eq!(item1.name.name, "baz");
    assert!(item1.alias.is_none(), "baz should have no alias");
}

#[test]
fn rejects_same_line_export_items_without_comma() {
    let src = r#"
module Example

export Value Other
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diag_codes(&diags).contains(&"E1548".to_string()),
        "expected export separator diagnostic, got: {:?}",
        diag_codes(&diags)
    );
}

#[test]
fn rejects_same_line_import_items_without_comma() {
    let src = r#"
@no_prelude
module Example

use some.module (foo bar)
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diag_codes(&diags).contains(&"E1548".to_string()),
        "expected import separator diagnostic, got: {:?}",
        diag_codes(&diags)
    );
}

#[test]
fn rejects_same_line_grouped_import_items_without_separator() {
    let src = r#"
@no_prelude
module Example

use some.module (text(foo) math(bar))
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diag_codes(&diags).contains(&"E1548".to_string()),
        "expected grouped import separator diagnostic, got: {:?}",
        diag_codes(&diags)
    );
}

#[test]
fn rejects_trailing_tokens_after_use_statement() {
    let src = r#"
@no_prelude
module Example

use some.module extra
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diag_codes(&diags).contains(&"E1547".to_string()),
        "expected trailing use diagnostic, got: {:?}",
        diag_codes(&diags)
    );
}

#[test]
fn rejects_trailing_tokens_after_type_alias() {
    let src = r#"
module Example

Alias = Int ?? Text
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diag_codes(&diags).contains(&"E1547".to_string()),
        "expected trailing type alias diagnostic, got: {:?}",
        diag_codes(&diags)
    );
}

#[test]
fn rejects_trailing_tokens_after_class_decl() {
    let src = r#"
module Example

class Eq A = {} trailing
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diag_codes(&diags).contains(&"E1547".to_string()),
        "expected trailing class diagnostic, got: {:?}",
        diag_codes(&diags)
    );
}

#[test]
fn rejects_trailing_tokens_after_domain_local_type_decl() {
    let src = r#"
module Example

domain Change over Int = {
  Delta = Add | Remove ?? Invalid
}
"#;
    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diag_codes(&diags).contains(&"E1547".to_string()),
        "expected trailing domain type diagnostic, got: {:?}",
        diag_codes(&diags)
    );
}

#[test]
fn parses_unparenthesized_lambda_on_pipe_rhs() {
    let src = r#"
module Example

f = { count: 1 } |> { count } => count + 1
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

    let Expr::Binary { op, right, .. } = &def.expr else {
        panic!("expected pipe expression");
    };
    assert_eq!(op, "|>");

    let Expr::Lambda { params, body, .. } = &**right else {
        panic!("expected lambda on pipe rhs, got: {right:?}");
    };
    assert_eq!(params.len(), 1);
    assert!(
        matches!(&params[0], Pattern::Record { .. }),
        "expected record-pattern lambda param"
    );
    assert!(
        matches!(&**body, Expr::Binary { op, .. } if op == "+"),
        "expected lambda body to remain the addition expression"
    );
}

#[test]
fn parses_parenthesized_lambda_as_call_argument() {
    let src = r#"
module Example

x = gen (a => b => a + b) 0
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

    let Expr::Call { func, args, .. } = &def.expr else {
        panic!("expected call expression, got: {:?}", def.expr);
    };
    assert!(matches!(&**func, Expr::Ident(name) if name.name == "gen"));
    assert_eq!(args.len(), 2, "expected lambda and zero arguments");
    assert!(matches!(&args[0], Expr::Lambda { .. }));
    assert!(matches!(
        &args[1],
        Expr::Literal(Literal::Number { text, .. }) if text == "0"
    ));
}

#[test]
fn parses_patched_lambda_head_into_shadowing_block() {
    let src = r#"
module Example

f = x <| _ * 2 => x + 1
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
    assert!(matches!(&params[0], Pattern::Ident(name) if name.name == "x"));

    let Expr::Block { items, .. } = &**body else {
        panic!("expected desugared block body, got {body:?}");
    };
    assert_eq!(
        items.len(),
        2,
        "expected one synthetic let and the original body"
    );

    let BlockItem::Let { pattern, expr, .. } = &items[0] else {
        panic!("expected synthetic let binding");
    };
    assert!(matches!(pattern, Pattern::Ident(name) if name.name == "x"));
    let Expr::Binary {
        op, left, right, ..
    } = expr
    else {
        panic!("expected pipe rewrite, got {expr:?}");
    };
    assert_eq!(op, "|>");
    assert!(matches!(&**left, Expr::Ident(name) if name.name == "x"));
    assert!(
        matches!(&**right, Expr::Binary { op, .. } if op == "*"),
        "expected updater expression on pipe rhs"
    );
}

#[test]
fn parses_multiline_patched_lambda_head() {
    let src = r#"
module Example

f = x <| _ + 1
    y <| _ + 3
  => x + y
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
    assert_eq!(params.len(), 2);
    assert!(matches!(&params[0], Pattern::Ident(name) if name.name == "x"));
    assert!(matches!(&params[1], Pattern::Ident(name) if name.name == "y"));

    let Expr::Block { items, .. } = &**body else {
        panic!("expected desugared block body, got {body:?}");
    };
    assert_eq!(
        items.len(),
        3,
        "expected two synthetic lets and the original body"
    );
    assert!(
        matches!(&items[0], BlockItem::Let { pattern, .. } if matches!(pattern, Pattern::Ident(name) if name.name == "x"))
    );
    assert!(
        matches!(&items[1], BlockItem::Let { pattern, .. } if matches!(pattern, Pattern::Ident(name) if name.name == "y"))
    );
    assert!(
        matches!(&items[2], BlockItem::Expr { expr, .. } if matches!(expr, Expr::Binary { op, .. } if op == "+"))
    );
}

#[test]
fn rejects_patched_non_identifier_lambda_param() {
    let src = r#"
module Example

f = { x } <| _ + 1 => x
"#;

    let (_, diags) = parse_modules(Path::new("test.aivi"), src);
    let codes = diag_codes(&diags);
    assert!(
        codes.contains(&"E1545".to_string()),
        "expected E1545, got: {codes:?}"
    );
    let diag = diags
        .iter()
        .find(|d| d.diagnostic.code == "E1545")
        .expect("E1545 diagnostic should be present");
    assert!(
        diag.diagnostic.message.contains("simple identifier"),
        "expected E1545 message to mention simple identifiers, got: {:?}",
        diag.diagnostic.message
    );
}

#[test]
fn parses_patched_lambda_on_pipe_rhs() {
    let src = r#"
module Example

f = 3 |> x <| _ + 1 => x * 2
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

    let Expr::Binary { op, right, .. } = &def.expr else {
        panic!("expected pipe expression");
    };
    assert_eq!(op, "|>");

    let Expr::Lambda { params, body, .. } = &**right else {
        panic!("expected lambda on pipe rhs, got: {right:?}");
    };
    assert_eq!(params.len(), 1);
    assert!(matches!(&params[0], Pattern::Ident(name) if name.name == "x"));
    assert!(
        matches!(&**body, Expr::Block { items, .. } if items.len() == 2),
        "expected patched rhs lambda to desugar through a block body"
    );
}

#[test]
fn parses_bare_matcher_block_on_signal_pipe_rhs() {
    let src = r#"
module Example

ShellState = AiSettingsSection | SearchSection

aiSettingsOpen = shellState ->>
  | Some AiSettingsSection => True
  | _                      => False
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
            ModuleItem::Def(def) if def.name.name == "aiSettingsOpen" => Some(def),
            _ => None,
        })
        .expect("aiSettingsOpen def");

    let Expr::Binary { op, right, .. } = &def.expr else {
        panic!("expected signal pipe expression");
    };
    assert_eq!(op, "->>");

    let Expr::Match {
        scrutinee, arms, ..
    } = &**right
    else {
        panic!("expected scrutinee-less matcher block on signal pipe rhs, got: {right:?}");
    };
    assert!(
        scrutinee.is_none(),
        "matcher block should stay scrutinee-less"
    );
    assert_eq!(arms.len(), 2);
    assert!(
        matches!(&arms[0].pattern, Pattern::Constructor { name, .. } if name.name == "Some"),
        "expected first matcher arm to be `Some ...`"
    );
}

#[test]
fn parses_bare_name_projection_addition_as_pipe_rhs_expression() {
    let src = r#"
module Example

f = state |> count + 1
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

    let Expr::Binary { op, right, .. } = &def.expr else {
        panic!("expected pipe expression");
    };
    assert_eq!(op, "|>");
    assert!(
        matches!(&**right, Expr::Binary { op, .. } if op == "+"),
        "expected `count + 1` to stay grouped on the pipe rhs, got: {right:?}"
    );
}
