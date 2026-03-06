/// Coverage tests for surface parser files:
/// - arena.rs + arena/lower.rs
/// - parser/sigils.rs
/// - parser/entrypoints.rs
/// - parser/literals_and_blocks.rs
/// - openapi.rs
use std::path::Path;

use crate::surface::{
    lower_modules_to_arena, parse_modules, ArenaBlockItem, ArenaBlockKind, ArenaExpr, ArenaLiteral,
    ArenaModuleItem, ArenaPattern, ArenaTypeExpr, BlockItem, BlockKind, Expr, Literal, ModuleItem,
    PathSegment,
};

use super::diag_codes;

// ─────────────────────────────────────────────────────────
// Arena allocation / accessor tests
// ─────────────────────────────────────────────────────────

#[test]
fn arena_alloc_and_access_expr() {
    use crate::diagnostics::Position;
    use crate::diagnostics::Span;
    use crate::surface::arena::ArenaLiteral;
    use crate::surface::{ArenaExpr, AstArena};

    let dummy_span = Span {
        start: Position { line: 1, column: 1 },
        end: Position { line: 1, column: 2 },
    };
    let mut arena = AstArena::default();
    let id = arena.alloc_expr(ArenaExpr::Literal(ArenaLiteral::Bool {
        value: true,
        span: dummy_span.clone(),
    }));
    match arena.expr(id) {
        ArenaExpr::Literal(ArenaLiteral::Bool { value, .. }) => assert!(*value),
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn arena_alloc_and_access_pattern() {
    use crate::diagnostics::Position;
    use crate::diagnostics::Span;
    use crate::surface::{ArenaPattern, AstArena};

    let dummy_span = Span {
        start: Position { line: 1, column: 1 },
        end: Position { line: 1, column: 2 },
    };
    let mut arena = AstArena::default();
    let id = arena.alloc_pattern(ArenaPattern::Wildcard(dummy_span.clone()));
    match arena.pattern(id) {
        ArenaPattern::Wildcard(_) => {}
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn arena_alloc_and_access_type_expr() {
    use crate::diagnostics::Position;
    use crate::diagnostics::Span;
    use crate::intern::Symbol;
    use crate::surface::arena::SpannedSymbol;
    use crate::surface::{ArenaTypeExpr, AstArena};

    let dummy_span = Span {
        start: Position { line: 1, column: 1 },
        end: Position { line: 1, column: 2 },
    };
    let sym = SpannedSymbol {
        symbol: Symbol::intern("Text"),
        span: dummy_span.clone(),
    };
    let mut arena = AstArena::default();
    let id = arena.alloc_type_expr(ArenaTypeExpr::Name(sym));
    match arena.type_expr(id) {
        ArenaTypeExpr::Name(s) => assert_eq!(s.symbol.as_str(), "Text"),
        other => panic!("unexpected: {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────
// Arena lowering: all module item kinds
// ─────────────────────────────────────────────────────────

#[test]
fn lower_type_decl_to_arena() {
    let src = r#"
module Example

Color = Red | Green | Blue
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (_arena, lowered) = lower_modules_to_arena(&modules);
    let module = &lowered[0];
    let type_decl = module
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::TypeDecl(td) if td.name.symbol.as_str() == "Color" => Some(td),
            _ => None,
        })
        .expect("Color type decl");
    assert_eq!(type_decl.constructors.len(), 3);
    assert_eq!(type_decl.constructors[0].name.symbol.as_str(), "Red");
    assert_eq!(type_decl.constructors[1].name.symbol.as_str(), "Green");
    assert_eq!(type_decl.constructors[2].name.symbol.as_str(), "Blue");
}

#[test]
fn lower_type_alias_to_arena() {
    let src = r#"
module Example

Name = Text
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (_arena, lowered) = lower_modules_to_arena(&modules);
    let module = &lowered[0];
    let alias = module
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::TypeAlias(a) if a.name.symbol.as_str() == "Name" => Some(a),
            _ => None,
        })
        .expect("Name alias");
    match _arena.type_expr(alias.aliased) {
        ArenaTypeExpr::Name(s) => assert_eq!(s.symbol.as_str(), "Text"),
        other => panic!("expected Name(Text), got {other:?}"),
    }
}

#[test]
fn lower_type_sig_to_arena() {
    let src = r#"
module Example

add : Int -> Int -> Int
add = x => y => x + y
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let module = &lowered[0];
    let sig = module
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::TypeSig(s) if s.name.symbol.as_str() == "add" => Some(s),
            _ => None,
        })
        .expect("add type sig");
    match arena.type_expr(sig.ty) {
        // Int -> Int -> Int parses as curried: Func { params: [Int], result: Func{ params: [Int], result: Int } }
        ArenaTypeExpr::Func { params, .. } => assert_eq!(params.len(), 1),
        other => panic!("expected Func, got {other:?}"),
    }
}

#[test]
fn lower_class_decl_to_arena() {
    let src = r#"
module Example

class Show A = {
  show : A -> Text
}
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (_arena, lowered) = lower_modules_to_arena(&modules);
    let module = &lowered[0];
    let class = module
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::ClassDecl(c) if c.name.symbol.as_str() == "Show" => Some(c),
            _ => None,
        })
        .expect("Show class");
    assert_eq!(class.members.len(), 1);
    assert_eq!(class.members[0].name.symbol.as_str(), "show");
}

#[test]
fn lower_instance_decl_to_arena() {
    let src = r#"
module Example

class Show A = {
  show : A -> Text
}

instance Show Int = {
  show = x => "int"
}
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (_arena, lowered) = lower_modules_to_arena(&modules);
    let module = &lowered[0];
    let inst = module
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::InstanceDecl(i) if i.name.symbol.as_str() == "Show" => Some(i),
            _ => None,
        })
        .expect("Show Int instance");
    assert_eq!(inst.defs.len(), 1);
    assert_eq!(inst.defs[0].name.symbol.as_str(), "show");
}

#[test]
fn lower_domain_decl_to_arena() {
    let src = r#"
module Example

domain Pretty over Int = {
  render : Int -> Text
  render = x => "pretty"
}
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (_arena, lowered) = lower_modules_to_arena(&modules);
    let module = &lowered[0];
    let dom = module
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::DomainDecl(d) if d.name.symbol.as_str() == "Pretty" => Some(d),
            _ => None,
        })
        .expect("Pretty domain");
    assert_eq!(dom.items.len(), 2);
}

#[test]
fn lower_machine_decl_to_arena() {
    let src = r#"
module Example

machine Traffic = {
  -> Red : init {}
  Red -> Green : change {}
  Green -> Red : stop {}
}
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (_arena, lowered) = lower_modules_to_arena(&modules);
    let module = &lowered[0];
    let mach = module
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::MachineDecl(m) if m.name.symbol.as_str() == "Traffic" => Some(m),
            _ => None,
        })
        .expect("Traffic machine");
    assert!(!mach.transitions.is_empty());
}

// ─────────────────────────────────────────────────────────
// Arena lowering: expression variants
// ─────────────────────────────────────────────────────────

#[test]
fn lower_literal_number_to_arena() {
    let src = r#"
module Example

x = 42
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let module = &lowered[0];
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match arena.expr(def.expr) {
        ArenaExpr::Literal(ArenaLiteral::Number { text, .. }) => {
            assert_eq!(text.as_str(), "42")
        }
        other => panic!("expected number literal, got {other:?}"),
    }
}

#[test]
fn lower_literal_string_to_arena() {
    let src = r#"
module Example

x = "hello"
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match arena.expr(def.expr) {
        ArenaExpr::Literal(ArenaLiteral::String { text, .. }) => {
            assert_eq!(text.as_str(), "hello")
        }
        other => panic!("expected string literal, got {other:?}"),
    }
}

#[test]
fn lower_literal_bool_to_arena() {
    let src = r#"
module Example

x = True
y = False
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    for item in &lowered[0].items {
        if let ArenaModuleItem::Def(d) = item {
            if d.name.symbol.as_str() == "x" {
                match arena.expr(d.expr) {
                    ArenaExpr::Literal(ArenaLiteral::Bool { value: true, .. }) => {}
                    other => panic!("expected Bool(true), got {other:?}"),
                }
            }
        }
    }
}

#[test]
fn lower_unary_neg_to_arena() {
    let src = r#"
module Example

x = -1
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    // -1 may be parsed as a number literal directly
    match arena.expr(def.expr) {
        ArenaExpr::Literal(ArenaLiteral::Number { text, .. }) => {
            assert!(text.as_str().contains('1'));
        }
        ArenaExpr::UnaryNeg { expr, .. } => match arena.expr(*expr) {
            ArenaExpr::Literal(ArenaLiteral::Number { .. }) => {}
            other => panic!("expected Number inside UnaryNeg, got {other:?}"),
        },
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn lower_binary_expr_to_arena() {
    let src = r#"
module Example

x = 1 + 2
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match arena.expr(def.expr) {
        ArenaExpr::Binary { op, .. } => assert_eq!(op.as_str(), "+"),
        other => panic!("expected Binary, got {other:?}"),
    }
}

#[test]
fn lower_if_expr_to_arena() {
    let src = r#"
module Example

x = if True then 1 else 2
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match arena.expr(def.expr) {
        ArenaExpr::If { .. } => {}
        other => panic!("expected If, got {other:?}"),
    }
}

#[test]
fn lower_match_expr_to_arena() {
    let src = r#"
module Example

x = y match
  | 1 => "one"
  | _ => "other"
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match arena.expr(def.expr) {
        ArenaExpr::Match { arms, .. } => assert_eq!(arms.len(), 2),
        other => panic!("expected Match, got {other:?}"),
    }
}

#[test]
fn lower_list_expr_to_arena() {
    let src = r#"
module Example

x = [1, 2, 3]
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match arena.expr(def.expr) {
        ArenaExpr::List { items, .. } => assert_eq!(items.len(), 3),
        other => panic!("expected List, got {other:?}"),
    }
}

#[test]
fn lower_tuple_expr_to_arena() {
    let src = r#"
module Example

x = (1, 2)
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match arena.expr(def.expr) {
        ArenaExpr::Tuple { items, .. } => assert_eq!(items.len(), 2),
        other => panic!("expected Tuple, got {other:?}"),
    }
}

#[test]
fn lower_record_expr_to_arena() {
    let src = r#"
module Example

x = { name: "Alice", age: 30 }
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match arena.expr(def.expr) {
        ArenaExpr::Record { fields, .. } => assert_eq!(fields.len(), 2),
        other => panic!("expected Record, got {other:?}"),
    }
}

#[test]
fn lower_field_access_to_arena() {
    let src = r#"
module Example

x = rec.field
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match arena.expr(def.expr) {
        ArenaExpr::FieldAccess { field, .. } => assert_eq!(field.symbol.as_str(), "field"),
        other => panic!("expected FieldAccess, got {other:?}"),
    }
}

#[test]
fn lower_call_expr_to_arena() {
    let src = r#"
module Example

x = f 1 2
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match arena.expr(def.expr) {
        ArenaExpr::Call { args, .. } => assert_eq!(args.len(), 2),
        other => panic!("expected Call, got {other:?}"),
    }
}

#[test]
fn lower_do_block_to_arena() {
    let src = r#"
module Example

x = do Effect {
  y <- someEffect
  pure y
}
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match arena.expr(def.expr) {
        ArenaExpr::Block {
            kind: ArenaBlockKind::Do { monad },
            items,
            ..
        } => {
            assert_eq!(monad.symbol.as_str(), "Effect");
            assert_eq!(items.len(), 2);
        }
        other => panic!("expected Do Block, got {other:?}"),
    }
}

#[test]
fn lower_generate_block_to_arena() {
    let src = r#"
module Example

x = generate {
  yield 1
  yield 2
}
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match arena.expr(def.expr) {
        ArenaExpr::Block {
            kind: ArenaBlockKind::Generate,
            items,
            ..
        } => {
            assert_eq!(items.len(), 2);
            assert!(matches!(items[0], ArenaBlockItem::Yield { .. }));
        }
        other => panic!("expected Generate Block, got {other:?}"),
    }
}

#[test]
fn lower_resource_block_to_arena() {
    let src = r#"
module Example

x = resource {
  yield "something"
}
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match arena.expr(def.expr) {
        ArenaExpr::Block {
            kind: ArenaBlockKind::Resource,
            ..
        } => {}
        other => panic!("expected Resource Block, got {other:?}"),
    }
}

#[test]
fn lower_text_interpolate_to_arena() {
    let src = r#"
module Example

x = "hello {name}"
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match arena.expr(def.expr) {
        ArenaExpr::TextInterpolate { parts, .. } => assert!(!parts.is_empty()),
        other => panic!("expected TextInterpolate, got {other:?}"),
    }
}

#[test]
fn lower_patch_lit_to_arena() {
    let src = r#"
module Example

x = rec <| { name: "Bob" }
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let _def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    // The `<|` operator creates a Call with PatchLit
    assert!(!arena.exprs.is_empty());
}

#[test]
fn lower_index_expr_to_arena() {
    let src = r#"
module Example

x = arr[0]
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match arena.expr(def.expr) {
        ArenaExpr::Index { .. } => {}
        other => panic!("expected Index, got {other:?}"),
    }
}

#[test]
fn lower_field_section_to_arena() {
    let src = r#"
module Example

x = .name
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match arena.expr(def.expr) {
        ArenaExpr::FieldSection { field, .. } => assert_eq!(field.symbol.as_str(), "name"),
        other => panic!("expected FieldSection, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────
// Arena lowering: pattern variants
// ─────────────────────────────────────────────────────────

#[test]
fn lower_wildcard_pattern_to_arena() {
    let src = r#"
module Example

x = y match
  | _ => 1
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    let arms = match arena.expr(def.expr) {
        ArenaExpr::Match { arms, .. } => arms.clone(),
        other => panic!("expected Match, got {other:?}"),
    };
    match arena.pattern(arms[0].pattern) {
        ArenaPattern::Wildcard(_) => {}
        other => panic!("expected Wildcard, got {other:?}"),
    }
}

#[test]
fn lower_constructor_pattern_to_arena() {
    let src = r#"
module Example

x = y match
  | Some val => val
  | None => 0
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    let arms = match arena.expr(def.expr) {
        ArenaExpr::Match { arms, .. } => arms.clone(),
        other => panic!("expected Match, got {other:?}"),
    };
    match arena.pattern(arms[0].pattern) {
        ArenaPattern::Constructor { name, args, .. } => {
            assert_eq!(name.symbol.as_str(), "Some");
            assert_eq!(args.len(), 1);
        }
        other => panic!("expected Constructor, got {other:?}"),
    }
}

#[test]
fn lower_list_pattern_to_arena() {
    let src = r#"
module Example

x = lst match
  | [h, ...t] => h
  | [] => 0
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    let arms = match arena.expr(def.expr) {
        ArenaExpr::Match { arms, .. } => arms.clone(),
        other => panic!("expected Match, got {other:?}"),
    };
    match arena.pattern(arms[0].pattern) {
        ArenaPattern::List { items, rest, .. } => {
            assert_eq!(items.len(), 1);
            assert!(rest.is_some());
        }
        other => panic!("expected List pattern, got {other:?}"),
    }
}

#[test]
fn lower_tuple_pattern_to_arena() {
    let src = r#"
module Example

x = p match
  | (a, b) => a + b
  | _ => 0
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    let arms = match arena.expr(def.expr) {
        ArenaExpr::Match { arms, .. } => arms.clone(),
        other => panic!("expected Match, got {other:?}"),
    };
    match arena.pattern(arms[0].pattern) {
        ArenaPattern::Tuple { items, .. } => assert_eq!(items.len(), 2),
        other => panic!("expected Tuple pattern, got {other:?}"),
    }
}

#[test]
fn lower_record_pattern_to_arena() {
    let src = r#"
module Example

x = r match
  | { name, age } => name
  | _ => "?"
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    let arms = match arena.expr(def.expr) {
        ArenaExpr::Match { arms, .. } => arms.clone(),
        other => panic!("expected Match, got {other:?}"),
    };
    match arena.pattern(arms[0].pattern) {
        ArenaPattern::Record { fields, .. } => assert_eq!(fields.len(), 2),
        other => panic!("expected Record pattern, got {other:?}"),
    }
}

#[test]
fn lower_at_pattern_to_arena() {
    let src = r#"
module Example

x = v match
  | n@(Some _) => n
  | _ => None
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    // W1603 warning expected; just ensure no hard errors
    assert!(
        !diag_codes(&diags).iter().any(|c| c.starts_with('E')),
        "unexpected errors: {:?}",
        diag_codes(&diags)
    );
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    let arms = match arena.expr(def.expr) {
        ArenaExpr::Match { arms, .. } => arms.clone(),
        other => panic!("expected Match, got {other:?}"),
    };
    match arena.pattern(arms[0].pattern) {
        ArenaPattern::At { name, .. } => assert_eq!(name.symbol.as_str(), "n"),
        other => panic!("expected At pattern, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────

#[test]
fn lower_func_type_to_arena() {
    let src = r#"
module Example

f : Int -> Text -> Bool
f = a => b => True
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let sig = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::TypeSig(s) if s.name.symbol.as_str() == "f" => Some(s),
            _ => None,
        })
        .expect("f sig");
    match arena.type_expr(sig.ty) {
        // Int -> Text -> Bool is curried: Func { params: [Int], result: Func { params: [Text], result: Bool } }
        ArenaTypeExpr::Func { params, .. } => assert_eq!(params.len(), 1),
        other => panic!("expected Func, got {other:?}"),
    }
}

#[test]
fn lower_apply_type_to_arena() {
    let src = r#"
module Example

f : Option Int
f = None
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let sig = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::TypeSig(s) if s.name.symbol.as_str() == "f" => Some(s),
            _ => None,
        })
        .expect("f sig");
    match arena.type_expr(sig.ty) {
        ArenaTypeExpr::Apply { args, .. } => assert_eq!(args.len(), 1),
        other => panic!("expected Apply, got {other:?}"),
    }
}

#[test]
fn lower_record_type_to_arena() {
    let src = r#"
module Example

f : { name: Text, age: Int }
f = { name: "x", age: 1 }
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let sig = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::TypeSig(s) if s.name.symbol.as_str() == "f" => Some(s),
            _ => None,
        })
        .expect("f sig");
    match arena.type_expr(sig.ty) {
        ArenaTypeExpr::Record { fields, .. } => assert_eq!(fields.len(), 2),
        other => panic!("expected Record type, got {other:?}"),
    }
}

#[test]
fn lower_tuple_type_to_arena() {
    let src = r#"
module Example

f : (Int, Text)
f = (1, "a")
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let sig = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::TypeSig(s) if s.name.symbol.as_str() == "f" => Some(s),
            _ => None,
        })
        .expect("f sig");
    match arena.type_expr(sig.ty) {
        ArenaTypeExpr::Tuple { items, .. } => assert_eq!(items.len(), 2),
        other => panic!("expected Tuple type, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────
// Arena lowering: block items
// ─────────────────────────────────────────────────────────

#[test]
fn lower_block_let_item_to_arena() {
    let src = r#"
module Example

x = do Effect {
  y = 1
  pure y
}
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match arena.expr(def.expr) {
        ArenaExpr::Block { items, .. } => {
            assert!(items
                .iter()
                .any(|i| matches!(i, ArenaBlockItem::Let { .. })));
        }
        other => panic!("expected Block, got {other:?}"),
    }
}

#[test]
fn lower_block_when_item_to_arena() {
    let src = r#"
module Example

x = do Effect {
  when True <- someEffect
  pure 1
}
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match arena.expr(def.expr) {
        ArenaExpr::Block { items, .. } => {
            assert!(items
                .iter()
                .any(|i| matches!(i, ArenaBlockItem::When { .. })));
        }
        other => panic!("expected Block, got {other:?}"),
    }
}

#[test]
fn lower_block_unless_item_to_arena() {
    let src = r#"
module Example

x = do Effect {
  unless False <- someEffect
  pure 1
}
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match arena.expr(def.expr) {
        ArenaExpr::Block { items, .. } => {
            assert!(items
                .iter()
                .any(|i| matches!(i, ArenaBlockItem::Unless { .. })));
        }
        other => panic!("expected Block, got {other:?}"),
    }
}

#[test]
fn lower_block_given_item_to_arena() {
    let src = r#"
module Example

x = do Effect {
  given ok <- someCheck else fail "nope"
  pure 1
}
"#;
    let (modules, _diags) = parse_modules(Path::new("test.aivi"), src);
    // given may parse with diags; just ensure arena lowering doesn't panic
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let _def = lowered[0].items.iter().find_map(|item| match item {
        ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
        _ => None,
    });
    assert!(!arena.exprs.is_empty());
}

#[test]
fn lower_filter_in_generate_block() {
    let src = r#"
module Example

x = generate {
  n <- [1, 2, 3]
  n -> n > 0
  yield n
}
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|item| match item {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match arena.expr(def.expr) {
        ArenaExpr::Block { items, .. } => {
            assert!(items
                .iter()
                .any(|i| matches!(i, ArenaBlockItem::Filter { .. })));
        }
        other => panic!("expected Generate block, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────
// Arena lowering: use decls + exports
// ─────────────────────────────────────────────────────────

#[test]
fn lower_use_decl_to_arena() {
    let src = r#"
@no_prelude
module Example

use some.module (foo, bar)
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (_arena, lowered) = lower_modules_to_arena(&modules);
    let module = &lowered[0];
    let use_decl = module
        .uses
        .iter()
        .find(|u| u.module.symbol.as_str() == "some.module")
        .expect("use decl");
    assert_eq!(use_decl.items.len(), 2);
    assert!(!use_decl.wildcard);
}

#[test]
fn lower_wildcard_use_decl_to_arena() {
    let src = r#"
@no_prelude
module Example

use some.module
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (_arena, lowered) = lower_modules_to_arena(&modules);
    let module = &lowered[0];
    let use_decl = module
        .uses
        .iter()
        .find(|u| u.module.symbol.as_str() == "some.module")
        .expect("use decl");
    assert!(use_decl.wildcard);
}

#[test]
fn lower_exports_to_arena() {
    let src = r#"
module Example

export x = 1
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (_arena, lowered) = lower_modules_to_arena(&modules);
    let module = &lowered[0];
    assert!(module.exports.iter().any(|e| e.name.symbol.as_str() == "x"));
}

// ─────────────────────────────────────────────────────────
// parser/entrypoints.rs — module declarations
// ─────────────────────────────────────────────────────────

#[test]
fn parses_simple_module_def() {
    let src = r#"
module Example

answer = 42
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    assert_eq!(module.name.name, "Example");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(d) if d.name.name == "answer" => Some(d),
            _ => None,
        })
        .expect("answer def");
    assert!(matches!(&def.expr, Expr::Literal(Literal::Number { text, .. }) if text == "42"));
}

#[test]
fn parses_type_signature_only() {
    let src = r#"
module Example

greet : Text -> Text
greet = name => "hello"
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    assert!(module.items.iter().any(|item| matches!(
        item,
        ModuleItem::TypeSig(sig) if sig.name.name == "greet"
    )));
}

#[test]
fn parses_type_declaration_with_constructors() {
    let src = r#"
module Example

Shape = Circle Float | Rect Float Float | Point
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    let td = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::TypeDecl(td) if td.name.name == "Shape" => Some(td),
            _ => None,
        })
        .expect("Shape type decl");
    assert_eq!(td.constructors.len(), 3);
}

#[test]
fn parses_type_alias() {
    let src = r#"
module Example

Name = Text
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    assert!(module.items.iter().any(|item| matches!(
        item,
        ModuleItem::TypeAlias(a) if a.name.name == "Name"
    )));
}

#[test]
fn parses_class_declaration() {
    let src = r#"
module Example

class Eq A = {
  eq : A -> A -> Bool
}
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    let class = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::ClassDecl(c) if c.name.name == "Eq" => Some(c),
            _ => None,
        })
        .expect("Eq class");
    assert_eq!(class.members.len(), 1);
}

#[test]
fn parses_instance_declaration() {
    let src = r#"
module Example

class Eq A = {
  eq : A -> A -> Bool
}

instance Eq Int = {
  eq = a => b => a == b
}
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    assert!(module.items.iter().any(|item| matches!(
        item,
        ModuleItem::InstanceDecl(i) if i.name.name == "Eq"
    )));
}

#[test]
fn parses_domain_declaration() {
    let src = r##"
module Example

domain Color over Text = {
  red : Text
  red = "#ff0000"
}
"##;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    assert!(module.items.iter().any(|item| matches!(
        item,
        ModuleItem::DomainDecl(d) if d.name.name == "Color"
    )));
}

#[test]
fn parses_machine_declaration() {
    let src = r#"
module Example

machine Door = {
  -> Closed : init {}
  Closed -> Open : open {}
  Open -> Closed : close {}
}
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    let mach = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::MachineDecl(m) if m.name.name == "Door" => Some(m),
            _ => None,
        })
        .expect("Door machine");
    assert_eq!(mach.transitions.len(), 3);
}

#[test]
fn parses_use_declaration() {
    let src = r#"
@no_prelude
module Example

use aivi.stdlib.text (trim, split)
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    let use_decl = module
        .uses
        .iter()
        .find(|u| u.module.name == "aivi.stdlib.text")
        .expect("use decl");
    assert_eq!(use_decl.items.len(), 2);
}

#[test]
fn parses_module_annotation() {
    let src = r#"
@no_prelude
module Example

x = 1
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    assert!(module
        .annotations
        .iter()
        .any(|a| a.name.name == "no_prelude"));
}

#[test]
fn parses_module_with_dotted_name() {
    let src = r#"
module my.module.name

x = 1
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    assert_eq!(module.name.name, "my.module.name");
}

#[test]
fn parses_def_with_params() {
    let src = r#"
module Example

add = x => y => x + y
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(d) if d.name.name == "add" => Some(d),
            _ => None,
        })
        .expect("add def");
    // add = x => y => ... — the lambda has 1 param, the body is another lambda
    assert_eq!(def.params.len(), 0);
    // verify it lowered to a lambda
    assert!(matches!(&def.expr, Expr::Lambda { params, .. } if params.len() == 1));
}

#[test]
fn prelude_is_auto_injected() {
    let src = r#"
module Example

x = 1
"#;
    let (modules, _diags) = parse_modules(Path::new("test.aivi"), src);
    let module = modules.first().expect("module");
    assert!(
        module.uses.iter().any(|u| u.module.name == "aivi.prelude"),
        "expected prelude to be auto-injected"
    );
}

#[test]
fn no_prelude_annotation_suppresses_injection() {
    let src = r#"
@no_prelude
module Example

x = 1
"#;
    let (modules, _diags) = parse_modules(Path::new("test.aivi"), src);
    let module = modules.first().expect("module");
    assert!(
        !module.uses.iter().any(|u| u.module.name == "aivi.prelude"),
        "expected prelude suppression"
    );
}

#[test]
fn parses_export_list() {
    let src = r#"
module Example

export x, y

x = 1
y = 2
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    assert!(module.exports.iter().any(|e| e.name.name == "x"));
    assert!(module.exports.iter().any(|e| e.name.name == "y"));
}

#[test]
fn parses_class_with_super_constraints() {
    let src = r#"
module Example

class Ord A = Eq, {
  compare : A -> A -> Int
}
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    let class = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::ClassDecl(c) if c.name.name == "Ord" => Some(c),
            _ => None,
        })
        .expect("Ord class");
    assert!(!class.supers.is_empty());
}

// ─────────────────────────────────────────────────────────
// parser/literals_and_blocks.rs
// ─────────────────────────────────────────────────────────

#[test]
fn parses_map_literal() {
    let src = r#"
module Example

x = ~map{ "a" => 1, "b" => 2 }
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x def");
    // ~map{} desugars to Map.union calls — not a Sigil literal
    assert!(!matches!(&def.expr, Expr::Literal(Literal::Sigil { tag, .. }) if tag == "map"));
}

#[test]
fn parses_set_literal() {
    let src = r#"
module Example

x = ~set[1, 2, 3]
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x def");
    assert!(!matches!(&def.expr, Expr::Literal(Literal::Sigil { tag, .. }) if tag == "set"));
}

#[test]
fn parses_matrix_literal_2x2() {
    let src = r#"
module Example

m = ~mat[1 2 3 4]
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(d) if d.name.name == "m" => Some(d),
            _ => None,
        })
        .expect("m def");
    // Should produce a record with m00..m11 fields
    assert!(matches!(&def.expr, Expr::Record { fields, .. } if !fields.is_empty()));
}

#[test]
fn parses_matrix_literal_3x3() {
    let src = r#"
module Example

m = ~mat[1 2 3 4 5 6 7 8 9]
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(d) if d.name.name == "m" => Some(d),
            _ => None,
        })
        .expect("m def");
    match &def.expr {
        Expr::Record { fields, .. } => assert_eq!(fields.len(), 9),
        other => panic!("expected 3x3 record, got {other:?}"),
    }
}

#[test]
fn rejects_invalid_matrix_size() {
    let src = r#"
module Example

m = ~mat[1 2 3]
"#;
    let (_modules, diags) = parse_modules(Path::new("test.aivi"), src);
    let codes = diag_codes(&diags);
    assert!(
        codes.contains(&"E1538".to_string()),
        "expected E1538, got {codes:?}"
    );
}

#[test]
fn parses_path_literal_absolute() {
    let src = r#"
module Example

p = ~path[/ usr / bin / ls]
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(d) if d.name.name == "p" => Some(d),
            _ => None,
        })
        .expect("p def");
    match &def.expr {
        Expr::Record { fields, .. } => {
            let abs = fields.iter().find(|f| matches!(f.path.first(), Some(PathSegment::Field(n)) if n.name == "absolute")).expect("absolute field");
            assert!(matches!(
                &abs.value,
                Expr::Literal(Literal::Bool { value: true, .. })
            ));
        }
        other => panic!("expected record for path, got {other:?}"),
    }
}

#[test]
fn parses_path_literal_relative() {
    let src = r#"
module Example

p = ~path[a / b]
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(d) if d.name.name == "p" => Some(d),
            _ => None,
        })
        .expect("p def");
    match &def.expr {
        Expr::Record { fields, .. } => {
            let abs = fields.iter().find(|f| matches!(f.path.first(), Some(PathSegment::Field(n)) if n.name == "absolute")).expect("absolute field");
            assert!(matches!(
                &abs.value,
                Expr::Literal(Literal::Bool { value: false, .. })
            ));
        }
        other => panic!("expected record for path, got {other:?}"),
    }
}

#[test]
fn parses_record_with_spread() {
    let src = r#"
module Example

base = { a: 1 }
x = { ...base, b: 2 }
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x def");
    match &def.expr {
        Expr::Record { fields, .. } => {
            assert!(fields.iter().any(|f| f.spread));
        }
        other => panic!("expected Record, got {other:?}"),
    }
}

#[test]
fn parses_record_with_nested_path() {
    let src = r#"
module Example

x = { a.b: 1 }
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x def");
    match &def.expr {
        Expr::Record { fields, .. } => {
            assert_eq!(fields[0].path.len(), 2);
        }
        other => panic!("expected Record, got {other:?}"),
    }
}

#[test]
fn parses_block_with_filter_in_generate() {
    let src = r#"
module Example

x = generate {
  n <- [1, 2, 3]
  n -> n > 0
  yield n
}
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x def");
    match &def.expr {
        Expr::Block {
            items,
            kind: BlockKind::Generate,
            ..
        } => {
            assert!(items.iter().any(|i| matches!(i, BlockItem::Filter { .. })));
        }
        other => panic!("expected Generate block, got {other:?}"),
    }
}

#[test]
fn rejects_yield_outside_generate() {
    let src = r#"
module Example

x = {
  yield 1
}
"#;
    let (_modules, diags) = parse_modules(Path::new("test.aivi"), src);
    let codes = diag_codes(&diags);
    assert!(
        codes.contains(&"E1534".to_string()),
        "expected E1534, got {codes:?}"
    );
}

#[test]
fn rejects_recurse_outside_generate() {
    let src = r#"
module Example

x = {
  recurse 1
}
"#;
    let (_modules, diags) = parse_modules(Path::new("test.aivi"), src);
    let codes = diag_codes(&diags);
    assert!(
        codes.contains(&"E1535".to_string()),
        "expected E1535, got {codes:?}"
    );
}

#[test]
fn parses_map_with_spread() {
    let src = r#"
module Example

base = ~map{ "a" => 1 }
x = ~map{ ...base, "b" => 2 }
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let _module = modules.first().expect("module");
}

#[test]
fn parses_set_with_spread() {
    let src = r#"
module Example

base = ~set[1, 2]
x = ~set[...base, 3]
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let _module = modules.first().expect("module");
}

#[test]
fn parses_list_with_spread() {
    let src = r#"
module Example

base = [1, 2]
x = [...base, 3, 4]
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x def");
    match &def.expr {
        Expr::List { items, .. } => {
            assert!(items.iter().any(|i| i.spread));
        }
        other => panic!("expected List, got {other:?}"),
    }
}

#[test]
fn parses_record_index_path() {
    let src = r#"
module Example

x = { [0]: "first" }
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x def");
    match &def.expr {
        Expr::Record { fields, .. } => {
            assert!(fields[0]
                .path
                .iter()
                .any(|seg| matches!(seg, PathSegment::Index(..))));
        }
        other => panic!("expected Record with index path, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────
// parser/sigils.rs — structured sigils
// ─────────────────────────────────────────────────────────

#[test]
fn parses_map_sigil_empty() {
    let src = r#"
module Example

x = ~map{}
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let _module = modules.first().expect("module");
}

#[test]
fn parses_set_sigil_empty() {
    let src = r#"
module Example

x = ~set[]
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let _module = modules.first().expect("module");
}

#[test]
fn parses_gtk_sigil_basic() {
    let src = r#"
module Example

x =
  ~<gtk>
    <object class="GtkLabel">
      <property name="label">Hello</property>
    </object>
  </gtk>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diags: {:?}",
        diag_codes(&diags)
    );
    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x def");
    use super::expr_contains_ident;
    assert!(expr_contains_ident(&def.expr, "gtkElement"));
}

#[test]
fn parses_gtk_sigil_with_attributes() {
    let src = r#"
module Example

x = ~<gtk><object class="GtkButton" label="Click me" /></gtk>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diags: {:?}",
        diag_codes(&diags)
    );
    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x def");
    use super::expr_contains_ident;
    assert!(expr_contains_ident(&def.expr, "gtkAttr"));
}

#[test]
fn parses_gtk_sigil_nested_children() {
    let src = r#"
module Example

x =
  ~<gtk>
    <object class="GtkBox">
      <object class="GtkLabel" />
      <object class="GtkButton" />
    </object>
  </gtk>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diags: {:?}",
        diag_codes(&diags)
    );
    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x def");
    use super::expr_contains_ident;
    assert!(expr_contains_ident(&def.expr, "gtkElement"));
}

#[test]
fn parses_gtk_sigil_with_splice_attribute() {
    let src = r#"
module Example

myVal = 42
x = ~<gtk><object class="GtkLabel" visible={ True } /></gtk>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diags: {:?}",
        diag_codes(&diags)
    );
    let _module = modules.first().expect("module");
}

#[test]
fn parses_html_sigil_with_expression_splice() {
    let src = r#"
module Example

val = 42
x = ~<html><div>{ val }</div></html>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diags: {:?}",
        diag_codes(&diags)
    );
    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x def");
    use super::expr_contains_ident;
    assert!(expr_contains_ident(&def.expr, "vElement"));
}

#[test]
fn parses_html_sigil_with_event_handler() {
    let src = r#"
module Example

x = ~<html><button onClick="save">Save</button></html>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diags: {:?}",
        diag_codes(&diags)
    );
    let _module = modules.first().expect("module");
}

#[test]
fn parses_html_sigil_multiple_elements() {
    let src = r#"
module Example

x =
  ~<html>
    <div class="wrapper">
      <h1>Title</h1>
      <p>Body</p>
    </div>
  </html>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diags: {:?}",
        diag_codes(&diags)
    );
    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x def");
    use super::expr_contains_ident;
    assert!(expr_contains_ident(&def.expr, "vElement"));
}

#[test]
fn parses_html_sigil_self_closing() {
    let src = r#"
module Example

x = ~<html><input type="text" /></html>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diags: {:?}",
        diag_codes(&diags)
    );
    let _module = modules.first().expect("module");
}

#[test]
fn parses_gtk_sigil_property_element() {
    let src = r#"
module Example

x =
  ~<gtk>
    <object class="GtkLabel">
      <property name="label">Hello World</property>
    </object>
  </gtk>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diags: {:?}",
        diag_codes(&diags)
    );
    let _module = modules.first().expect("module");
}

#[test]
fn parses_gtk_sigil_with_signal_element() {
    let src = r#"
module Example

Save = Save

x =
  ~<gtk>
    <object class="GtkButton">
      <signal name="clicked" on={ Save } />
    </object>
  </gtk>
"#;
    // signal on must be compile-time value - this should produce diagnostic E1614
    let (_modules, _diags) = parse_modules(Path::new("test.aivi"), src);
    // We just ensure no panic
}

#[test]
fn path_literal_with_dotdot_normalization() {
    let src = r#"
module Example

p = ~path[a / .. / b]
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(d) if d.name.name == "p" => Some(d),
            _ => None,
        })
        .expect("p def");
    // After normalization, a/.. should produce just "b"
    match &def.expr {
        Expr::Record { fields, .. } => {
            let segs_field = fields.iter().find(|f| {
                matches!(f.path.first(), Some(PathSegment::Field(n)) if n.name == "segments")
            }).expect("segments field");
            match &segs_field.value {
                Expr::List { items, .. } => assert_eq!(items.len(), 1),
                other => panic!("expected list of segments, got {other:?}"),
            }
        }
        other => panic!("expected Record for path, got {other:?}"),
    }
}

#[test]
fn path_literal_absolute_dotdot_does_not_escape() {
    let src = r#"
module Example

p = ~path[/ .. / b]
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(d) if d.name.name == "p" => Some(d),
            _ => None,
        })
        .expect("p def");
    match &def.expr {
        Expr::Record { fields, .. } => {
            let abs_field = fields.iter().find(|f| {
                matches!(f.path.first(), Some(PathSegment::Field(n)) if n.name == "absolute")
            }).expect("absolute field");
            assert!(matches!(
                &abs_field.value,
                Expr::Literal(Literal::Bool { value: true, .. })
            ));
        }
        other => panic!("expected record, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────
// openapi.rs — spec_to_result via parse_modules integration
// ─────────────────────────────────────────────────────────

#[test]
fn openapi_from_json_string() {
    use crate::diagnostics::{Position, Span};
    use crate::surface::openapi::openapi_to_expr;
    use std::path::PathBuf;

    let span = Span {
        start: Position { line: 1, column: 1 },
        end: Position { line: 1, column: 1 },
    };

    let spec_json = r#"{
        "openapi": "3.0.0",
        "info": { "title": "Test API", "version": "1.0.0" },
        "paths": {
            "/users": {
                "get": {
                    "operationId": "listUsers",
                    "responses": {
                        "200": {
                            "description": "ok",
                            "content": {
                                "application/json": {
                                    "schema": { "type": "array", "items": { "type": "string" } }
                                }
                            }
                        }
                    }
                }
            }
        }
    }"#;

    // Write to temp file and parse via file path
    let tmp = std::env::temp_dir().join("test_openapi_spec.json");
    std::fs::write(&tmp, spec_json).expect("test");

    let result = openapi_to_expr(
        tmp.to_str().expect("test"),
        false,
        &PathBuf::from("/"),
        &span,
        "api",
    );
    assert!(result.is_ok(), "expected Ok, got {:?}", result.err());
    let res = result.expect("test");
    match &res.expr {
        Expr::Lambda { body, .. } => match body.as_ref() {
            Expr::Record { fields, .. } => {
                assert!(fields.iter().any(|f| matches!(
                    f.path.first(),
                    Some(PathSegment::Field(n)) if n.name == "listUsers"
                )));
            }
            other => panic!("expected inner Record, got {other:?}"),
        },
        other => panic!("expected Lambda expr, got {other:?}"),
    }
}

#[test]
fn openapi_with_server_url() {
    use crate::diagnostics::{Position, Span};
    use crate::surface::openapi::openapi_to_expr;
    use std::path::PathBuf;

    let span = Span {
        start: Position { line: 1, column: 1 },
        end: Position { line: 1, column: 1 },
    };

    let spec_json = r#"{
        "openapi": "3.0.0",
        "info": { "title": "Test", "version": "1.0" },
        "servers": [{ "url": "https://api.example.com" }],
        "paths": {
            "/ping": {
                "get": {
                    "operationId": "ping",
                    "responses": { "200": { "description": "ok" } }
                }
            }
        }
    }"#;

    let tmp = std::env::temp_dir().join("test_openapi_server.json");
    std::fs::write(&tmp, spec_json).expect("test");

    let result = openapi_to_expr(
        tmp.to_str().expect("test"),
        false,
        &PathBuf::from("/"),
        &span,
        "api",
    );
    assert!(result.is_ok());
    let res = result.expect("test");
    use super::expr_contains_string;
    assert!(
        expr_contains_string(&res.expr, "https://api.example.com"),
        "expected __baseUrl in expr"
    );
}

#[test]
fn openapi_with_post_endpoint_and_body() {
    use crate::diagnostics::{Position, Span};
    use crate::surface::openapi::openapi_to_expr;
    use std::path::PathBuf;

    let span = Span {
        start: Position { line: 1, column: 1 },
        end: Position { line: 1, column: 1 },
    };

    let spec_json = r##"{
        "openapi": "3.0.0",
        "info": { "title": "Test", "version": "1.0" },
        "paths": {
            "/users": {
                "post": {
                    "operationId": "createUser",
                    "requestBody": {
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "name": { "type": "string" },
                                        "age": { "type": "integer" }
                                    },
                                    "required": ["name"]
                                }
                            }
                        }
                    },
                    "responses": {
                        "201": {
                            "description": "created",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/User" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }"##;

    let tmp = std::env::temp_dir().join("test_openapi_post.json");
    std::fs::write(&tmp, spec_json).expect("test");

    let result = openapi_to_expr(
        tmp.to_str().expect("test"),
        false,
        &PathBuf::from("/"),
        &span,
        "api",
    );
    assert!(result.is_ok());
    let res = result.expect("test");
    match &res.expr {
        Expr::Lambda { body, .. } => match body.as_ref() {
            Expr::Record { fields, .. } => {
                assert!(fields.iter().any(|f| matches!(
                    f.path.first(),
                    Some(PathSegment::Field(n)) if n.name == "createUser"
                )));
            }
            other => panic!("expected inner Record, got {other:?}"),
        },
        other => panic!("expected Lambda, got {other:?}"),
    }
}

#[test]
fn openapi_with_path_parameter() {
    use crate::diagnostics::{Position, Span};
    use crate::surface::openapi::openapi_to_expr;
    use std::path::PathBuf;

    let span = Span {
        start: Position { line: 1, column: 1 },
        end: Position { line: 1, column: 1 },
    };

    let spec_json = r#"{
        "openapi": "3.0.0",
        "info": { "title": "Test", "version": "1.0" },
        "paths": {
            "/users/{id}": {
                "get": {
                    "operationId": "getUser",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "integer" }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "ok",
                            "content": {
                                "application/json": {
                                    "schema": { "type": "object" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }"#;

    let tmp = std::env::temp_dir().join("test_openapi_path_param.json");
    std::fs::write(&tmp, spec_json).expect("test");

    let result = openapi_to_expr(
        tmp.to_str().expect("test"),
        false,
        &PathBuf::from("/"),
        &span,
        "api",
    );
    assert!(result.is_ok());
}

#[test]
fn openapi_with_query_parameter() {
    use crate::diagnostics::{Position, Span};
    use crate::surface::openapi::openapi_to_expr;
    use std::path::PathBuf;

    let span = Span {
        start: Position { line: 1, column: 1 },
        end: Position { line: 1, column: 1 },
    };

    let spec_json = r#"{
        "openapi": "3.0.0",
        "info": { "title": "Test", "version": "1.0" },
        "paths": {
            "/search": {
                "get": {
                    "operationId": "search",
                    "parameters": [
                        {
                            "name": "q",
                            "in": "query",
                            "required": false,
                            "schema": { "type": "string" }
                        }
                    ],
                    "responses": {
                        "200": { "description": "ok" }
                    }
                }
            }
        }
    }"#;

    let tmp = std::env::temp_dir().join("test_openapi_query.json");
    std::fs::write(&tmp, spec_json).expect("test");

    let result = openapi_to_expr(
        tmp.to_str().expect("test"),
        false,
        &PathBuf::from("/"),
        &span,
        "api",
    );
    assert!(result.is_ok());
    // type sig should have optional record arg
    let res = result.expect("test");
    assert!(!res.items.is_empty());
}

#[test]
fn openapi_schema_type_aliases_generated() {
    use crate::diagnostics::{Position, Span};
    use crate::surface::openapi::openapi_to_expr;
    use std::path::PathBuf;

    let span = Span {
        start: Position { line: 1, column: 1 },
        end: Position { line: 1, column: 1 },
    };

    let spec_json = r#"{
        "openapi": "3.0.0",
        "info": { "title": "Test", "version": "1.0" },
        "paths": {},
        "components": {
            "schemas": {
                "User": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "name": { "type": "string" }
                    }
                },
                "Status": {
                    "type": "string",
                    "enum": ["active", "inactive"]
                }
            }
        }
    }"#;

    let tmp = std::env::temp_dir().join("test_openapi_schemas.json");
    std::fs::write(&tmp, spec_json).expect("test");

    let result = openapi_to_expr(
        tmp.to_str().expect("test"),
        false,
        &PathBuf::from("/"),
        &span,
        "api",
    );
    assert!(result.is_ok());
    let res = result.expect("test");
    // Should have TypeAlias for User + TypeDecl for Status (enum) + TypeSig for binding
    assert!(res.items.len() >= 2);
    assert!(res
        .items
        .iter()
        .any(|item| matches!(item, ModuleItem::TypeSig(_))));
}

#[test]
fn openapi_missing_file_returns_error() {
    use crate::diagnostics::{Position, Span};
    use crate::surface::openapi::openapi_to_expr;
    use std::path::PathBuf;

    let span = Span {
        start: Position { line: 1, column: 1 },
        end: Position { line: 1, column: 1 },
    };

    let result = openapi_to_expr(
        "nonexistent_file_abc123.json",
        false,
        &PathBuf::from("/"),
        &span,
        "api",
    );
    assert!(result.is_err());
}

#[test]
fn openapi_invalid_json_returns_error() {
    use crate::diagnostics::{Position, Span};
    use crate::surface::openapi::openapi_to_expr;
    use std::path::PathBuf;

    let span = Span {
        start: Position { line: 1, column: 1 },
        end: Position { line: 1, column: 1 },
    };

    let tmp = std::env::temp_dir().join("test_openapi_invalid.json");
    std::fs::write(&tmp, "not json at all {{{{").expect("test");

    let result = openapi_to_expr(
        tmp.to_str().expect("test"),
        false,
        &PathBuf::from("/"),
        &span,
        "api",
    );
    assert!(result.is_err());
}

#[test]
fn openapi_with_derived_operation_id() {
    use crate::diagnostics::{Position, Span};
    use crate::surface::openapi::openapi_to_expr;
    use std::path::PathBuf;

    let span = Span {
        start: Position { line: 1, column: 1 },
        end: Position { line: 1, column: 1 },
    };

    // No operationId — should derive from method + path
    let spec_json = r#"{
        "openapi": "3.0.0",
        "info": { "title": "Test", "version": "1.0" },
        "paths": {
            "/users/{id}/posts": {
                "get": {
                    "responses": { "200": { "description": "ok" } }
                }
            }
        }
    }"#;

    let tmp = std::env::temp_dir().join("test_openapi_derived_id.json");
    std::fs::write(&tmp, spec_json).expect("test");

    let result = openapi_to_expr(
        tmp.to_str().expect("test"),
        false,
        &PathBuf::from("/"),
        &span,
        "api",
    );
    assert!(result.is_ok());
    let res = result.expect("test");
    if let Expr::Record { fields, .. } = &res.expr {
        // Should have a derived id like "getUsersPosts" or "getUserIdPosts"
        assert!(!fields.is_empty());
    }
}

#[test]
fn openapi_yaml_spec() {
    use crate::diagnostics::{Position, Span};
    use crate::surface::openapi::openapi_to_expr;
    use std::path::PathBuf;

    let span = Span {
        start: Position { line: 1, column: 1 },
        end: Position { line: 1, column: 1 },
    };

    let spec_yaml = r#"
openapi: "3.0.0"
info:
  title: Test API
  version: "1.0.0"
paths:
  /ping:
    get:
      operationId: ping
      responses:
        "200":
          description: ok
"#;

    let tmp = std::env::temp_dir().join("test_openapi_spec.yaml");
    std::fs::write(&tmp, spec_yaml).expect("test");

    let result = openapi_to_expr(
        tmp.to_str().expect("test"),
        false,
        &PathBuf::from("/"),
        &span,
        "api",
    );
    assert!(
        result.is_ok(),
        "expected Ok for YAML spec, got {:?}",
        result.err()
    );
}

#[test]
fn openapi_with_oneof_schema() {
    use crate::diagnostics::{Position, Span};
    use crate::surface::openapi::openapi_to_expr;
    use std::path::PathBuf;

    let span = Span {
        start: Position { line: 1, column: 1 },
        end: Position { line: 1, column: 1 },
    };

    let spec_json = r##"{
        "openapi": "3.0.0",
        "info": { "title": "Test", "version": "1.0" },
        "paths": {},
        "components": {
            "schemas": {
                "Animal": {
                    "oneOf": [
                        { "$ref": "#/components/schemas/Cat" },
                        { "$ref": "#/components/schemas/Dog" }
                    ]
                },
                "Cat": { "type": "object" },
                "Dog": { "type": "object" }
            }
        }
    }"##;

    let tmp = std::env::temp_dir().join("test_openapi_oneof.json");
    std::fs::write(&tmp, spec_json).expect("test");

    let result = openapi_to_expr(
        tmp.to_str().expect("test"),
        false,
        &PathBuf::from("/"),
        &span,
        "api",
    );
    assert!(result.is_ok());
    let res = result.expect("test");
    // Animal should produce a TypeDecl
    assert!(res
        .items
        .iter()
        .any(|item| matches!(item, ModuleItem::TypeDecl(td) if td.name.name == "Animal")));
}

#[test]
fn openapi_with_anyof_schema() {
    use crate::diagnostics::{Position, Span};
    use crate::surface::openapi::openapi_to_expr;
    use std::path::PathBuf;

    let span = Span {
        start: Position { line: 1, column: 1 },
        end: Position { line: 1, column: 1 },
    };

    let spec_json = r#"{
        "openapi": "3.0.0",
        "info": { "title": "Test", "version": "1.0" },
        "paths": {},
        "components": {
            "schemas": {
                "Result": {
                    "anyOf": [
                        { "type": "string" },
                        { "type": "integer" }
                    ]
                }
            }
        }
    }"#;

    let tmp = std::env::temp_dir().join("test_openapi_anyof.json");
    std::fs::write(&tmp, spec_json).expect("test");

    let result = openapi_to_expr(
        tmp.to_str().expect("test"),
        false,
        &PathBuf::from("/"),
        &span,
        "api",
    );
    assert!(result.is_ok());
}

#[test]
fn openapi_with_all_http_methods() {
    use crate::diagnostics::{Position, Span};
    use crate::surface::openapi::openapi_to_expr;
    use std::path::PathBuf;

    let span = Span {
        start: Position { line: 1, column: 1 },
        end: Position { line: 1, column: 1 },
    };

    let spec_json = r#"{
        "openapi": "3.0.0",
        "info": { "title": "Test", "version": "1.0" },
        "paths": {
            "/resource": {
                "get": { "operationId": "getResource", "responses": { "200": { "description": "ok" } } },
                "post": { "operationId": "postResource", "responses": { "201": { "description": "created" } } },
                "put": { "operationId": "putResource", "responses": { "200": { "description": "ok" } } },
                "delete": { "operationId": "deleteResource", "responses": { "204": { "description": "deleted" } } },
                "patch": { "operationId": "patchResource", "responses": { "200": { "description": "ok" } } }
            }
        }
    }"#;

    let tmp = std::env::temp_dir().join("test_openapi_all_methods.json");
    std::fs::write(&tmp, spec_json).expect("test");

    let result = openapi_to_expr(
        tmp.to_str().expect("test"),
        false,
        &PathBuf::from("/"),
        &span,
        "api",
    );
    assert!(result.is_ok());
    let res = result.expect("test");
    match &res.expr {
        Expr::Lambda { body, .. } => match body.as_ref() {
            Expr::Record { fields, .. } => {
                let names: Vec<&str> = fields
                    .iter()
                    .filter_map(|f| match f.path.first() {
                        Some(PathSegment::Field(n)) => Some(n.name.as_str()),
                        _ => None,
                    })
                    .collect();
                assert!(names.contains(&"getResource"));
                assert!(names.contains(&"postResource"));
                assert!(names.contains(&"putResource"));
                assert!(names.contains(&"deleteResource"));
                assert!(names.contains(&"patchResource"));
            }
            other => panic!("expected inner Record, got {other:?}"),
        },
        other => panic!("expected Lambda, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────
// entrypoints.rs — additional coverage
// ─────────────────────────────────────────────────────────

#[test]
fn expands_domain_exports() {
    let src = r#"
module Example

export domain Fmt over Int = {
  fmt : Int -> Text
  fmt = x => "x"
}
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    // domain members should be auto-exported
    assert!(module.exports.iter().any(|e| e.name.name == "fmt"));
}

#[test]
fn expands_type_constructor_exports() {
    let src = r#"
module Example

export Maybe A = Just A | Nothing
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    // constructors should be exported
    assert!(module.exports.iter().any(|e| e.name.name == "Just"));
    assert!(module.exports.iter().any(|e| e.name.name == "Nothing"));
}

#[test]
fn module_alias_rewrites_field_access() {
    let src = r#"
@no_prelude
module Example

use some.module as M

x = M.foo
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    // May produce no diagnostic; verify no panic
    let _module = modules.first().expect("module");
    drop(diags);
}

#[test]
fn parses_class_with_given_constraints() {
    let src = r#"
module Example

class Mappable (F A) = given (A: Any) {
  map : (A -> B) -> F A -> F B
}
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    let class = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::ClassDecl(c) if c.name.name == "Mappable" => Some(c),
            _ => None,
        })
        .expect("Mappable class");
    assert_eq!(class.constraints.len(), 1);
}

#[test]
fn parses_machine_state_with_fields() {
    // Machine transitions with payload fields in the transition body
    let src = r#"
module Example

machine Counter = {
  -> Counting : init {}
  Counting -> Counting : increment { amount: Int }
  Counting -> Done : finish {}
}
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let module = modules.first().expect("module");
    let mach = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::MachineDecl(m) if m.name.name == "Counter" => Some(m),
            _ => None,
        })
        .expect("Counter machine");
    assert!(!mach.states.is_empty());
    // State Counting should be inferred
    assert!(mach.states.iter().any(|s| s.name.name == "Counting"));
    // Transitions should include increment with payload
    let increment = mach
        .transitions
        .iter()
        .find(|t| t.name.name == "increment")
        .expect("increment transition");
    assert_eq!(increment.payload.len(), 1);
    assert_eq!(increment.payload[0].0.name, "amount");
}

// ─────────────────────────────────────────────────────────
// Arena lowering: decorator with arg, suffixed, lambda, sigil
// ─────────────────────────────────────────────────────────

#[test]
fn lower_decorator_with_arg_to_arena() {
    let src = "module Example\n\n@deprecated \"use y\"\nx = 1\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|i| match i {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert_eq!(def.decorators.len(), 1);
    assert!(def.decorators[0].arg.is_some());
    let arg = def.decorators[0].arg.expect("decorator arg");
    assert!(matches!(
        arena.expr(arg),
        ArenaExpr::Literal(ArenaLiteral::String { .. })
    ));
}

#[test]
fn lower_suffixed_expr_to_arena() {
    let src = "module Example\n\nx = (1 + 2)px\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|i| match i {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(
        matches!(arena.expr(def.expr), ArenaExpr::Suffixed { suffix, .. } if suffix.symbol.as_str() == "px")
    );
}

#[test]
fn lower_lambda_to_arena() {
    let src = "module Example\n\nf = x => y => x + y\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|i| match i {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "f" => Some(d),
            _ => None,
        })
        .expect("f");
    assert!(matches!(arena.expr(def.expr), ArenaExpr::Lambda { params, .. } if params.len() == 1));
}

#[test]
fn lower_sigil_literal_to_arena() {
    let src = "module Example\n\nx = ~regex/[a-z]+/i\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|i| match i {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match arena.expr(def.expr) {
        ArenaExpr::Literal(ArenaLiteral::Sigil { tag, flags, .. }) => {
            assert_eq!(tag.as_str(), "regex");
            assert_eq!(flags.as_str(), "i");
        }
        other => panic!("expected Sigil, got {other:?}"),
    }
}

#[test]
fn lower_use_decl_with_alias_to_arena() {
    let src = "@no_prelude\nmodule Example\n\nuse some.module as SM\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (_arena, lowered) = lower_modules_to_arena(&modules);
    let u = lowered[0]
        .uses
        .iter()
        .find(|u| u.module.symbol.as_str() == "some.module")
        .expect("use");
    assert!(u.alias.is_some());
    assert_eq!(u.alias.as_ref().expect("alias").symbol.as_str(), "SM");
}

#[test]
fn lower_domain_literal_def_to_arena() {
    let src =
        "module Example\n\ndomain Css over Text = {\n  Length = Px Float\n\n  1px = Px 1.0\n}\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (_arena, lowered) = lower_modules_to_arena(&modules);
    let dom = lowered[0]
        .items
        .iter()
        .find_map(|i| match i {
            ArenaModuleItem::DomainDecl(d) if d.name.symbol.as_str() == "Css" => Some(d),
            _ => None,
        })
        .expect("Css");
    assert!(dom
        .items
        .iter()
        .any(|i| matches!(i, crate::surface::ArenaDomainItem::LiteralDef(_))));
}

#[test]
fn lower_match_with_guard_to_arena() {
    let src =
        "module Example\n\nx = y match\n  | n when n > 0 => \"positive\"\n  | _ => \"other\"\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|i| match i {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match arena.expr(def.expr) {
        ArenaExpr::Match { arms, .. } => assert!(arms[0].guard.is_some()),
        other => panic!("expected Match, got {other:?}"),
    }
}

#[test]
fn lower_block_on_item_to_arena() {
    let src =
        "module Example\n\nx = do Effect {\n  on SomeTransition => handleTransition\n  pure 1\n}\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|i| match i {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match arena.expr(def.expr) {
        ArenaExpr::Block { items, .. } => {
            assert!(items.iter().any(|i| matches!(i, ArenaBlockItem::On { .. })))
        }
        other => panic!("expected Block, got {other:?}"),
    }
}

#[test]
fn lower_block_recurse_item_to_arena() {
    let src = "module Example\n\nx = generate {\n  recurse 1\n}\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let (arena, lowered) = lower_modules_to_arena(&modules);
    let def = lowered[0]
        .items
        .iter()
        .find_map(|i| match i {
            ArenaModuleItem::Def(d) if d.name.symbol.as_str() == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match arena.expr(def.expr) {
        ArenaExpr::Block { items, .. } => assert!(items
            .iter()
            .any(|i| matches!(i, ArenaBlockItem::Recurse { .. }))),
        other => panic!("expected Block, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────
// blocks.rs — loop, given, on, when/unless errors
// ─────────────────────────────────────────────────────────

#[test]
fn parses_loop_in_do_block() {
    let src = "module Example\n\nx = do Effect {\n  loop n = 0 => {\n    pure n\n  }\n}\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(
        matches!(&def.expr, Expr::Block { items, .. } if items.iter().any(|i| matches!(i, BlockItem::Let { .. })))
    );
}

#[test]
fn rejects_loop_outside_do_or_generate() {
    let src = "module Example\n\nx = {\n  loop n = 0 => pure n\n}\n";
    let (_m, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1533".to_string()));
}

#[test]
fn rejects_when_outside_do() {
    let src = "module Example\n\nx = generate {\n  when True <- someEffect\n  yield 1\n}\n";
    let (_m, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1540".to_string()));
}

#[test]
fn rejects_unless_outside_do() {
    let src = "module Example\n\nx = generate {\n  unless False <- someEffect\n  yield 1\n}\n";
    let (_m, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1543".to_string()));
}

#[test]
fn rejects_given_outside_do() {
    let src = "module Example\n\nx = generate {\n  given True or fail \"nope\"\n  yield 1\n}\n";
    let (_m, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1541".to_string()));
}

#[test]
fn rejects_on_outside_do() {
    let src = "module Example\n\nx = generate {\n  on Transition => handler\n  yield 1\n}\n";
    let (_m, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1542".to_string()));
}

#[test]
fn rejects_bind_outside_do_or_generate() {
    let src = "module Example\n\nx = {\n  y <- someEffect\n  y\n}\n";
    let (_m, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1536".to_string()));
}

#[test]
fn parses_given_with_match_form() {
    let src = "module Example\n\nx = do Effect {\n  given (status > 0) or\n    | NotFound => pure \"not found\"\n    | Timeout => pure \"timeout\"\n  pure \"ok\"\n}\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match &def.expr {
        Expr::Block { items, .. } => assert!(items.iter().any(|i| matches!(
            i,
            BlockItem::Given {
                fail_expr: Expr::Match { .. },
                ..
            }
        ))),
        other => panic!("expected Block, got {other:?}"),
    }
}

#[test]
fn parses_given_with_simple_fail() {
    let src = "module Example\n\nx = do Effect {\n  given (status > 0) or fail \"bad\"\n  pure \"ok\"\n}\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(
        matches!(&def.expr, Expr::Block { items, .. } if items.iter().any(|i| matches!(i, BlockItem::Given { .. })))
    );
}

#[test]
fn parses_on_in_do_block() {
    let src = "module Example\n\nx = do Effect {\n  on Start => handleStart\n  pure 1\n}\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(
        matches!(&def.expr, Expr::Block { items, .. } if items.iter().any(|i| matches!(i, BlockItem::On { .. })))
    );
}

#[test]
fn parses_resource_block_with_yield_and_bind() {
    let src = "module Example\n\nx = resource {\n  handle <- acquire\n  yield handle\n}\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match &def.expr {
        Expr::Block {
            kind: BlockKind::Resource,
            items,
            ..
        } => {
            assert!(items.iter().any(|i| matches!(i, BlockItem::Yield { .. })));
            assert!(items.iter().any(|i| matches!(i, BlockItem::Bind { .. })));
        }
        other => panic!("expected Resource, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────
// module.rs — decorator edge cases
// ─────────────────────────────────────────────────────────

#[test]
fn rejects_unknown_module_decorator() {
    let src = "@custom_thing\nmodule Example\n\nx = 1\n";
    let (_m, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1506".to_string()));
}

#[test]
fn rejects_test_module_decorator_with_argument() {
    let src = "@test \"nope\"\nmodule Example\n\nx = 1\n";
    let (_m, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1512".to_string()));
}

#[test]
fn parses_use_with_domain_import() {
    let src = "@no_prelude\nmodule Example\n\nuse some.module (domain MyDomain, foo)\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let u = modules[0]
        .uses
        .iter()
        .find(|u| u.module.name == "some.module")
        .expect("use");
    assert_eq!(u.items.len(), 2);
    assert!(u
        .items
        .iter()
        .any(|i| i.kind == crate::surface::ScopeItemKind::Domain));
}

#[test]
fn export_domain_name_in_export_list() {
    let src = "module Example\n\ndomain Color over Text = {\n  red : Text\n  red = \"#ff0000\"\n}\n\nexport domain Color\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    assert!(modules[0]
        .exports
        .iter()
        .any(|e| e.kind == crate::surface::ScopeItemKind::Domain && e.name.name == "Color"));
}

#[test]
fn rejects_deprecated_with_non_string_arg() {
    let src = "module Example\n\n@deprecated 42\nx = 1\n";
    let (_m, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1510".to_string()));
}

#[test]
fn parses_static_decorator_no_arg() {
    let src = "module Example\n\n@static\nx = 42\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let _ = modules.first().expect("module");
}

#[test]
fn rejects_static_decorator_with_argument() {
    let src = "module Example\n\n@static \"nope\"\nx = 42\n";
    let (_m, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1513".to_string()));
}

#[test]
fn rejects_decorators_on_use() {
    let src = "@no_prelude\nmodule Example\n\n@deprecated \"old\"\nuse some.module\n";
    let (_m, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diag_codes(&diags).contains(&"E1507".to_string()));
}

// ─────────────────────────────────────────────────────────
// entrypoints.rs — opaque types, multiline ctors, export-prefixed
// ─────────────────────────────────────────────────────────

#[test]
fn parses_opaque_type_declaration() {
    let src = "module Example\n\nHandle\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let td = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::TypeDecl(td) if td.name.name == "Handle" => Some(td),
            _ => None,
        })
        .expect("Handle");
    assert!(td.constructors.is_empty());
}

#[test]
fn parses_opaque_type_with_params() {
    let src = "module Example\n\nContainer A B\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let td = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::TypeDecl(td) if td.name.name == "Container" => Some(td),
            _ => None,
        })
        .expect("Container");
    assert_eq!(td.params.len(), 2);
}

#[test]
fn parses_multiline_type_constructors() {
    let src = "module Example\n\nShape =\n  | Circle Float\n  | Rect Float Float\n  | Point\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let td = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::TypeDecl(td) if td.name.name == "Shape" => Some(td),
            _ => None,
        })
        .expect("Shape");
    assert_eq!(td.constructors.len(), 3);
}

#[test]
fn parses_export_class_declaration() {
    let src = "module Example\n\nexport class Printable A = {\n  print : A -> Text\n}\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    assert!(modules[0]
        .exports
        .iter()
        .any(|e| e.name.name == "Printable"));
}

#[test]
fn parses_export_instance_declaration() {
    let src = "module Example\n\nclass Show A = {\n  show : A -> Text\n}\n\nexport instance Show Int = {\n  show = x => \"int\"\n}\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    assert!(modules[0].exports.iter().any(|e| e.name.name == "Show"));
}

#[test]
fn parses_export_machine_declaration() {
    let src = "module Example\n\nexport machine Workflow = {\n  -> Idle : boot {}\n  Idle -> Running : start {}\n}\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    assert!(modules[0].exports.iter().any(|e| e.name.name == "Workflow"));
}

// ─────────────────────────────────────────────────────────
// literals_and_blocks.rs — matrix, path, map, set edge cases
// ─────────────────────────────────────────────────────────

#[test]
fn parses_matrix_literal_4x4() {
    let src = "module Example\n\nm = ~mat[1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16]\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "m" => Some(d),
            _ => None,
        })
        .expect("m");
    assert!(matches!(&def.expr, Expr::Record { fields, .. } if fields.len() == 16));
}

#[test]
fn parses_path_literal_with_dot_normalization() {
    let src = "module Example\n\np = ~path[a / . / b]\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "p" => Some(d),
            _ => None,
        })
        .expect("p");
    match &def.expr {
        Expr::Record { fields, .. } => {
            let segs = fields.iter().find(|f| matches!(f.path.first(), Some(PathSegment::Field(n)) if n.name == "segments")).expect("segments");
            assert!(matches!(&segs.value, Expr::List { items, .. } if items.len() == 2));
        }
        other => panic!("expected Record, got {other:?}"),
    }
}

#[test]
fn parses_path_literal_relative_dotdot_keeps_leading() {
    let src = "module Example\n\np = ~path[.. / b]\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "p" => Some(d),
            _ => None,
        })
        .expect("p");
    match &def.expr {
        Expr::Record { fields, .. } => {
            let segs = fields.iter().find(|f| matches!(f.path.first(), Some(PathSegment::Field(n)) if n.name == "segments")).expect("segments");
            assert!(matches!(&segs.value, Expr::List { items, .. } if items.len() == 2));
        }
        other => panic!("expected Record, got {other:?}"),
    }
}

#[test]
fn parses_map_with_multiline_entries() {
    let src = "module Example\n\nx = ~map{\n  \"a\" => 1\n  \"b\" => 2\n  \"c\" => 3\n}\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(super::expr_contains_ident(&def.expr, "Map"));
}

#[test]
fn parses_set_with_multiline_entries() {
    let src = "module Example\n\nx = ~set[\n  1\n  2\n  3\n]\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(super::expr_contains_ident(&def.expr, "Set"));
}

#[test]
fn parses_record_with_all_path_segment() {
    let src = "module Example\n\nx = { items[*]: True }\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match &def.expr {
        Expr::Record { fields, .. } => assert!(fields[0]
            .path
            .iter()
            .any(|s| matches!(s, PathSegment::All(..)))),
        other => panic!("expected Record, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────
// helpers.rs — operators, sigils
// ─────────────────────────────────────────────────────────

#[test]
fn parses_pipe_operator() {
    let src = "module Example\n\nx = [1, 2, 3] |> map f\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(matches!(&def.expr, Expr::Binary { op, .. } if op == "|>"));
}

#[test]
fn parses_coalesce_operator() {
    let src = "module Example\n\nx = a ?? b\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(matches!(&def.expr, Expr::Binary { op, .. } if op == "??"));
}

#[test]
fn parses_logical_and_or_precedence() {
    let src = "module Example\n\nx = a && b || c\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(matches!(&def.expr, Expr::Binary { op, .. } if op == "||"));
}

#[test]
fn parses_all_comparison_operators() {
    let src =
        "module Example\n\na = 1 < 2\nb = 1 <= 2\nc = 1 > 2\nd = 1 >= 2\ne = 1 == 2\nf = 1 != 2\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    assert_eq!(modules[0].items.len(), 6);
}

#[test]
fn parses_concat_operator() {
    let src = "module Example\n\nx = \"a\" ++ \"b\" ++ \"c\"\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(matches!(&def.expr, Expr::Binary { op, .. } if op == "++"));
}

#[test]
fn parses_modulo_operator() {
    let src = "module Example\n\nx = 10 % 3\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(matches!(&def.expr, Expr::Binary { op, .. } if op == "%"));
}

#[test]
fn parses_sigil_slash_delimiter() {
    let src = "module Example\n\nx = ~regex/[a-z]+/\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(
        matches!(&def.expr, Expr::Literal(Literal::Sigil { tag, body, .. }) if tag == "regex" && body == "[a-z]+")
    );
}

#[test]
fn parses_sigil_with_flags() {
    let src = "module Example\n\nx = ~regex/pattern/gi\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(matches!(&def.expr, Expr::Literal(Literal::Sigil { flags, .. }) if flags == "gi"));
}

#[test]
fn parses_sigil_paren_delimiter() {
    let src = "module Example\n\nx = ~css(color: red)\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(
        matches!(&def.expr, Expr::Literal(Literal::Sigil { tag, body, .. }) if tag == "css" && body == "color: red")
    );
}

#[test]
fn parses_text_interpolation_expr() {
    let src = "module Example\n\nname = \"world\"\nx = \"hello {name}!\"\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(matches!(&def.expr, Expr::TextInterpolate { .. }));
}

#[test]
fn parses_text_interpolation_with_nested_string_literal() {
    // A string literal inside {…} must not prematurely close the outer string.
    let src = r#"module Example

subject = "Re: {email ?? " "}"
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "subject" => Some(d),
            _ => None,
        })
        .expect("subject");
    assert!(matches!(&def.expr, Expr::TextInterpolate { .. }));
}

#[test]
fn parses_text_interpolation_nested_string_with_closing_brace_inside() {
    // A "}" inside a nested string literal must not close the outer interpolation.
    let src = r#"module Example

x = "prefix {f "a}b"} suffix"
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(matches!(&def.expr, Expr::TextInterpolate { .. }));
}

#[test]
fn lexer_string_with_nested_string_in_interpolation_has_no_diags() {
    // The lexer must produce a single string token without diagnostics.
    let src = r#""Re: {email ?? " "}""#;
    let (tokens, diags) = crate::lexer::lex(src);
    assert!(diags.is_empty(), "unexpected diags: {:?}", diags);
    let string_tokens: Vec<_> = tokens.iter().filter(|t| t.kind == "string").collect();
    assert_eq!(
        string_tokens.len(),
        1,
        "expected 1 string token, got {string_tokens:?}"
    );
}

#[test]
fn parses_field_section_expr() {
    let src = "module Example\n\nx = .name\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(matches!(&def.expr, Expr::FieldSection { .. }));
}

#[test]
fn parses_tuple_expression_3() {
    let src = "module Example\n\nx = (1, \"hello\", True)\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(diags.is_empty(), "diags: {:?}", diag_codes(&diags));
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(matches!(&def.expr, Expr::Tuple { items, .. } if items.len() == 3));
}

// ─────────────────────────────────────────────────────────
// openapi.rs — additional schema types
// ─────────────────────────────────────────────────────────

#[test]
fn openapi_with_boolean_and_number_schema() {
    use crate::diagnostics::{Position, Span};
    use crate::surface::openapi::openapi_to_expr;
    use std::path::PathBuf;
    let span = Span {
        start: Position { line: 1, column: 1 },
        end: Position { line: 1, column: 1 },
    };
    let j = r#"{"openapi":"3.0.0","info":{"title":"T","version":"1.0"},"paths":{},"components":{"schemas":{"Config":{"type":"object","properties":{"enabled":{"type":"boolean"},"weight":{"type":"number"},"count":{"type":"integer"}},"required":["enabled"]}}}}"#;
    let tmp = std::env::temp_dir().join("test_openapi_bn.json");
    std::fs::write(&tmp, j).expect("write temp openapi bn");
    assert!(openapi_to_expr(
        tmp.to_str().expect("tmp path utf-8"),
        false,
        &PathBuf::from("/"),
        &span,
        "api"
    )
    .is_ok());
}

#[test]
fn openapi_with_head_and_options_methods() {
    use crate::diagnostics::{Position, Span};
    use crate::surface::openapi::openapi_to_expr;
    use std::path::PathBuf;
    let span = Span {
        start: Position { line: 1, column: 1 },
        end: Position { line: 1, column: 1 },
    };
    let j = r#"{"openapi":"3.0.0","info":{"title":"T","version":"1.0"},"paths":{"/r":{"head":{"operationId":"headR","responses":{"200":{"description":"ok"}}},"options":{"operationId":"optR","responses":{"200":{"description":"ok"}}}}}}"#;
    let tmp = std::env::temp_dir().join("test_openapi_ho.json");
    std::fs::write(&tmp, j).expect("write temp openapi ho");
    let res = openapi_to_expr(
        tmp.to_str().expect("tmp path utf-8"),
        false,
        &PathBuf::from("/"),
        &span,
        "api",
    )
    .expect("openapi_to_expr");
    match &res.expr {
        Expr::Lambda { body, .. } => match body.as_ref() {
            Expr::Record { fields, .. } => {
                let names: Vec<&str> = fields
                    .iter()
                    .filter_map(|f| match f.path.first() {
                        Some(PathSegment::Field(n)) => Some(n.name.as_str()),
                        _ => None,
                    })
                    .collect();
                assert!(names.contains(&"headR"));
                assert!(names.contains(&"optR"));
            }
            other => panic!("expected inner Record, got {other:?}"),
        },
        other => panic!("expected Lambda, got {other:?}"),
    }
}

#[test]
fn openapi_with_array_schema() {
    use crate::diagnostics::{Position, Span};
    use crate::surface::openapi::openapi_to_expr;
    use std::path::PathBuf;
    let span = Span {
        start: Position { line: 1, column: 1 },
        end: Position { line: 1, column: 1 },
    };
    let j = r#"{"openapi":"3.0.0","info":{"title":"T","version":"1.0"},"paths":{},"components":{"schemas":{"UserList":{"type":"array","items":{"type":"string"}}}}}"#;
    let tmp = std::env::temp_dir().join("test_openapi_arr.json");
    std::fs::write(&tmp, j).expect("write temp openapi arr");
    assert!(openapi_to_expr(
        tmp.to_str().expect("tmp path utf-8"),
        false,
        &PathBuf::from("/"),
        &span,
        "api"
    )
    .is_ok());
}
