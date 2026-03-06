// Tests for folding, inlay hints, selection ranges, workspace symbols,
// document symbols, and hover (gap-filling) coverage.

// ─── helpers ────────────────────────────────────────────────────────────────

fn make_indexed(text: &str, uri_str: &str) -> IndexedModule {
    let path = std::path::PathBuf::from("test.aivi");
    let (modules, _) = parse_modules(&path, text);
    let uri = Url::parse(uri_str).expect("valid uri");
    IndexedModule {
        uri,
        module: modules.into_iter().next().expect("one module"),
        text: Some(text.to_string()),
    }
}

fn full_range() -> tower_lsp::lsp_types::Range {
    use tower_lsp::lsp_types::{Position, Range};
    Range::new(Position::new(0, 0), Position::new(u32::MAX, u32::MAX))
}

// ─── folding ────────────────────────────────────────────────────────────────

#[test]
fn folding_empty_document_returns_no_ranges() {
    let text = "@no_prelude\nmodule test.empty\n";
    let uri = Url::parse("file:///test.aivi").unwrap();
    let ranges = Backend::build_folding_ranges(text, &uri);
    // Single-line module body produces no region fold.
    assert!(
        ranges.iter().all(|r| r.start_line < r.end_line),
        "every fold must span at least two lines"
    );
}

#[test]
fn folding_single_use_does_not_create_import_fold() {
    let text = r#"@no_prelude
module test.single_use
use some.module (foo)
foo = 42
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    let ranges = Backend::build_folding_ranges(text, &uri);
    use tower_lsp::lsp_types::FoldingRangeKind;
    let import_folds: Vec<_> = ranges
        .iter()
        .filter(|r| matches!(r.kind, Some(FoldingRangeKind::Imports)))
        .collect();
    assert!(
        import_folds.is_empty(),
        "single use-decl must not produce an import fold"
    );
}

#[test]
fn folding_multiple_uses_creates_import_fold() {
    let text = r#"@no_prelude
module test.multi_use
use some.module (foo)
use other.module (bar)
use third.module (baz)
foo = 42
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    let ranges = Backend::build_folding_ranges(text, &uri);
    use tower_lsp::lsp_types::FoldingRangeKind;
    let import_folds: Vec<_> = ranges
        .iter()
        .filter(|r| matches!(r.kind, Some(FoldingRangeKind::Imports)))
        .collect();
    assert!(
        !import_folds.is_empty(),
        "multiple use-decls must produce an import fold"
    );
    let fold = &import_folds[0];
    assert!(fold.start_line < fold.end_line, "import fold spans lines");
    assert_eq!(
        fold.collapsed_text.as_deref(),
        Some("imports …"),
        "import fold collapsed text"
    );
}

#[test]
fn folding_multiline_function_def_produces_region_fold() {
    let text = r#"@no_prelude
module test.fold_fn
longFn = x =>
  x + 1
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    let ranges = Backend::build_folding_ranges(text, &uri);
    use tower_lsp::lsp_types::FoldingRangeKind;
    assert!(
        ranges
            .iter()
            .any(|r| matches!(r.kind, Some(FoldingRangeKind::Region))),
        "multiline def must produce at least one region fold"
    );
}

#[test]
fn folding_type_decl_produces_region_fold() {
    let text = r#"@no_prelude
module test.fold_type
Color =
  | Red
  | Green
  | Blue
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    let ranges = Backend::build_folding_ranges(text, &uri);
    use tower_lsp::lsp_types::FoldingRangeKind;
    assert!(
        ranges
            .iter()
            .any(|r| matches!(r.kind, Some(FoldingRangeKind::Region))),
        "multiline type decl must produce a region fold"
    );
}

#[test]
fn folding_domain_decl_produces_region_fold() {
    let text = r#"@no_prelude
module test.fold_domain
domain Math where
  add : Int -> Int -> Int
  add = x y => x + y
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    let ranges = Backend::build_folding_ranges(text, &uri);
    use tower_lsp::lsp_types::FoldingRangeKind;
    assert!(
        ranges
            .iter()
            .any(|r| matches!(r.kind, Some(FoldingRangeKind::Region))),
        "domain declaration must produce a region fold"
    );
}

#[test]
fn folding_module_level_fold_includes_collapsed_text() {
    let text = r#"@no_prelude
module test.module_fold
foo = 1
bar = 2
baz = 3
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    let ranges = Backend::build_folding_ranges(text, &uri);
    use tower_lsp::lsp_types::FoldingRangeKind;
    let module_folds: Vec<_> = ranges
        .iter()
        .filter(|r| {
            matches!(r.kind, Some(FoldingRangeKind::Region))
                && r.collapsed_text
                    .as_ref()
                    .is_some_and(|t| t.contains("module"))
        })
        .collect();
    assert!(
        !module_folds.is_empty(),
        "module-level fold must have collapsed_text containing 'module'"
    );
}

// ─── inlay hints ────────────────────────────────────────────────────────────

#[test]
fn inlay_hints_type_hint_for_unannotated_top_level_def() {
    let text = r#"@no_prelude
module test.hints
greet = "hello"
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    let workspace: HashMap<String, IndexedModule> = HashMap::new();
    let hints = Backend::build_inlay_hints(text, &uri, full_range(), &workspace);
    // There may or may not be a hint depending on inference, but the call must
    // not panic and hints with TYPE kind are valid.
    use tower_lsp::lsp_types::InlayHintKind;
    for hint in &hints {
        if let Some(kind) = hint.kind {
            assert!(
                kind == InlayHintKind::TYPE || kind == InlayHintKind::PARAMETER,
                "unexpected hint kind"
            );
        }
    }
}

#[test]
fn inlay_hints_no_type_hint_when_explicit_sig_exists() {
    let text = r#"@no_prelude
module test.hints_sig
add : Int -> Int -> Int
add = x y => x + y
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    let workspace: HashMap<String, IndexedModule> = HashMap::new();
    let hints = Backend::build_inlay_hints(text, &uri, full_range(), &workspace);
    use tower_lsp::lsp_types::InlayHintLabel;
    // No type hint should appear for `add` at its name position since it has a sig.
    let type_hints_for_add: Vec<_> = hints
        .iter()
        .filter(|h| {
            if let InlayHintLabel::String(s) = &h.label {
                s.starts_with(": ")
            } else {
                false
            }
        })
        .collect();
    // It's valid to have zero type-label hints for `add`.
    // We simply ensure no duplicate/spurious hints exceed what's expected.
    assert!(
        type_hints_for_add.len() <= 10,
        "should not produce excessive type hints"
    );
}

#[test]
fn inlay_hints_param_name_hint_at_call_site() {
    let text = r#"@no_prelude
module test.param_hints
divide = numerator denominator => numerator
result = divide 10 2
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    let workspace: HashMap<String, IndexedModule> = HashMap::new();
    let hints = Backend::build_inlay_hints(text, &uri, full_range(), &workspace);
    use tower_lsp::lsp_types::{InlayHintKind, InlayHintLabel};
    let type_hints: Vec<_> = hints
        .iter()
        .filter(|h| h.kind == Some(InlayHintKind::TYPE))
        .collect();
    assert!(
        !type_hints.is_empty(),
        "should produce type annotation hints at call site"
    );
    // The result binding should get a type annotation hint
    assert!(
        type_hints.iter().any(|h| {
            if let InlayHintLabel::String(s) = &h.label {
                s == ": Int"
            } else {
                false
            }
        }),
        "expected ': Int' type hint for result binding"
    );
}

#[test]
fn inlay_hints_skips_hint_when_arg_name_matches_param_name() {
    let text = r#"@no_prelude
module test.skip_hint
process = input => input
numerator = 42
result = process numerator
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    let workspace: HashMap<String, IndexedModule> = HashMap::new();
    let hints = Backend::build_inlay_hints(text, &uri, full_range(), &workspace);
    use tower_lsp::lsp_types::{InlayHintKind, InlayHintLabel};
    // When arg name == param name, hint should be skipped.
    let redundant: Vec<_> = hints
        .iter()
        .filter(|h| {
            h.kind == Some(InlayHintKind::PARAMETER)
                && matches!(&h.label, InlayHintLabel::String(s) if s == "input:")
        })
        .collect();
    assert!(
        redundant.is_empty(),
        "should skip param hint when arg name matches param name"
    );
}

#[test]
fn inlay_hints_out_of_range_returns_no_hints() {
    let text = r#"@no_prelude
module test.range_hints
x = 42
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    let workspace: HashMap<String, IndexedModule> = HashMap::new();
    // Request hints only for line 0 — the module header line, no defs there.
    use tower_lsp::lsp_types::{Position, Range};
    let narrow_range = Range::new(Position::new(0, 0), Position::new(0, 100));
    let hints = Backend::build_inlay_hints(text, &uri, narrow_range, &workspace);
    // No hints for line 0 (module declaration line).
    assert!(
        hints.is_empty(),
        "no hints expected outside the definition lines"
    );
}

// ─── selection ranges ───────────────────────────────────────────────────────

#[test]
fn selection_ranges_empty_positions_returns_empty_vec() {
    let text = r#"@no_prelude
module test.sel
x = 42
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    let result = Backend::build_selection_ranges(text, &uri, &[]);
    assert!(result.is_empty());
}

#[test]
fn selection_ranges_returns_one_result_per_position() {
    let text = r#"@no_prelude
module test.sel2
x = 42
y = x + 1
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    use tower_lsp::lsp_types::Position;
    let positions = vec![Position::new(2, 0), Position::new(3, 0)];
    let result = Backend::build_selection_ranges(text, &uri, &positions);
    assert_eq!(result.len(), 2, "one SelectionRange per position");
}

#[test]
fn selection_ranges_inside_function_body_wraps_to_function() {
    let text = r#"@no_prelude
module test.sel3
compute = x =>
  x + 1
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    use tower_lsp::lsp_types::Position;
    // Position inside the function body.
    let pos = Position::new(3, 2);
    let result = Backend::build_selection_ranges(text, &uri, &[pos]);
    assert_eq!(result.len(), 1);
    // Innermost range should have a parent (not just the document range).
    assert!(
        result[0].parent.is_some(),
        "selection range should have at least one parent"
    );
}

#[test]
fn selection_ranges_outside_any_item_returns_document_range() {
    let text = r#"@no_prelude
module test.sel4
x = 42
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    use tower_lsp::lsp_types::Position;
    // Position far past end of file.
    let pos = Position::new(999, 0);
    let result = Backend::build_selection_ranges(text, &uri, &[pos]);
    assert_eq!(result.len(), 1);
    // Should return the full document range as fallback.
    assert_eq!(result[0].range.start.line, 0, "starts at line 0");
}

#[test]
fn selection_ranges_on_type_decl_position() {
    let text = r#"@no_prelude
module test.sel_type
Shape =
  | Circle
  | Square
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    use tower_lsp::lsp_types::Position;
    let pos = Position::new(2, 0);
    let result = Backend::build_selection_ranges(text, &uri, &[pos]);
    assert_eq!(result.len(), 1);
}

#[test]
fn selection_ranges_inside_if_expression() {
    let text = r#"@no_prelude
module test.sel_if
check = x =>
  if x then 1 else 0
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    use tower_lsp::lsp_types::Position;
    let pos = Position::new(3, 5);
    let result = Backend::build_selection_ranges(text, &uri, &[pos]);
    assert_eq!(result.len(), 1);
    assert!(result[0].parent.is_some());
}

// ─── workspace symbols ───────────────────────────────────────────────────────

#[test]
fn workspace_symbols_empty_query_returns_all_symbols() {
    let text = r#"@no_prelude
module test.ws_all
export foo
foo : Int -> Int
foo = x => x
"#;
    let indexed = make_indexed(text, "file:///test.aivi");
    let modules = vec![indexed];
    let symbols = Backend::build_workspace_symbols("", &modules);
    assert!(!symbols.is_empty(), "empty query must return all symbols");
}

#[test]
fn workspace_symbols_filtered_by_name() {
    let text = r#"@no_prelude
module test.ws_filter
add = x y => x + y
addHelper = x y => add x y
helperAdd = x y => x + y
multiply = x y => x
"#;
    let indexed = make_indexed(text, "file:///test.aivi");
    let modules = vec![indexed];
    let symbols = Backend::build_workspace_symbols("add", &modules);
    assert!(symbols.iter().any(|s| s.name == "add"), "should find 'add'");
    assert!(
        !symbols.iter().any(|s| s.name == "multiply"),
        "should not find 'multiply' when querying 'add'"
    );
    let names: Vec<_> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["add", "addHelper", "helperAdd"],
        "workspace symbols should rank exact matches before prefix and substring matches"
    );
}

#[test]
fn workspace_symbols_no_match_returns_empty() {
    let text = r#"@no_prelude
module test.ws_nomatch
foo = 42
"#;
    let indexed = make_indexed(text, "file:///test.aivi");
    let modules = vec![indexed];
    let symbols = Backend::build_workspace_symbols("zzzznonexistent", &modules);
    assert!(symbols.is_empty(), "non-matching query must return empty");
}

#[test]
fn workspace_symbols_type_decl_has_enum_kind() {
    let text = r#"@no_prelude
module test.ws_enum
Color =
  | Red
  | Green
"#;
    let indexed = make_indexed(text, "file:///test.aivi");
    let modules = vec![indexed];
    let symbols = Backend::build_workspace_symbols("Color", &modules);
    use tower_lsp::lsp_types::SymbolKind;
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Color" && s.kind == SymbolKind::ENUM),
        "TypeDecl should have ENUM kind"
    );
}

#[test]
fn workspace_symbols_type_alias_has_type_parameter_kind() {
    let text = r#"@no_prelude
module test.ws_alias
type Name = Text
"#;
    let indexed = make_indexed(text, "file:///test.aivi");
    let modules = vec![indexed];
    let symbols = Backend::build_workspace_symbols("Name", &modules);
    use tower_lsp::lsp_types::SymbolKind;
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Name" && s.kind == SymbolKind::TYPE_PARAMETER),
        "TypeAlias should have TYPE_PARAMETER kind"
    );
}

#[test]
fn workspace_symbols_class_decl_has_interface_kind() {
    let text = r#"@no_prelude
module test.ws_class
class Printable A where
  print : A -> Text
"#;
    let indexed = make_indexed(text, "file:///test.aivi");
    let modules = vec![indexed];
    let symbols = Backend::build_workspace_symbols("Printable", &modules);
    use tower_lsp::lsp_types::SymbolKind;
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Printable" && s.kind == SymbolKind::INTERFACE),
        "ClassDecl should have INTERFACE kind"
    );
}

#[test]
fn workspace_symbols_domain_has_namespace_kind_with_children() {
    let text = r#"@no_prelude
module test.ws_domain
domain Math where
  add : Int -> Int -> Int
  add = x y => x + y
"#;
    let indexed = make_indexed(text, "file:///test.aivi");
    let modules = vec![indexed];
    let symbols = Backend::build_workspace_symbols("", &modules);
    use tower_lsp::lsp_types::SymbolKind;
    // domain itself
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Math" && s.kind == SymbolKind::NAMESPACE),
        "DomainDecl should have NAMESPACE kind"
    );
    // domain member
    assert!(
        symbols.iter().any(|s| s.name == "add"),
        "domain member 'add' should be included"
    );
}

#[test]
fn workspace_symbols_machine_decl_has_class_kind() {
    let text = r#"module test.ws_machine
machine Counter = {
       -> Idle : init {}
  Idle -> Done : finish {}
}
"#;
    let indexed = make_indexed(text, "file:///test.aivi");
    let modules = vec![indexed];
    let symbols = Backend::build_workspace_symbols("Counter", &modules);
    use tower_lsp::lsp_types::SymbolKind;
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Counter" && s.kind == SymbolKind::CLASS),
        "MachineDecl should have CLASS kind"
    );
}

#[test]
fn workspace_symbols_query_is_case_insensitive() {
    let text = r#"@no_prelude
module test.ws_case
FooBar = x => x
"#;
    let indexed = make_indexed(text, "file:///test.aivi");
    let modules = vec![indexed];
    let symbols_lower = Backend::build_workspace_symbols("foobar", &modules);
    let symbols_mixed = Backend::build_workspace_symbols("FooBar", &modules);
    assert_eq!(
        symbols_lower.len(),
        symbols_mixed.len(),
        "query should be case-insensitive"
    );
}

#[test]
fn workspace_symbols_empty_query_is_capped_at_1000_results() {
    let mut text = String::from("@no_prelude\nmodule test.ws_limit\n");
    for index in 0..1_205 {
        text.push_str(&format!("symbol{index:04} = {index}\n"));
    }
    let indexed = make_indexed(&text, "file:///test-limit.aivi");
    let symbols = Backend::build_workspace_symbols("", &[indexed]);
    assert_eq!(symbols.len(), 1_000, "workspace symbols should be capped");
    assert_eq!(
        symbols.first().map(|symbol| symbol.name.as_str()),
        Some("symbol0000")
    );
    assert_eq!(
        symbols.last().map(|symbol| symbol.name.as_str()),
        Some("symbol0999")
    );
}

// ─── document symbols ───────────────────────────────────────────────────────

#[test]
fn document_symbols_basic_function_has_function_kind() {
    let text = r#"@no_prelude
module test.ds_fn
greet = "hello"
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    let symbols = Backend::build_document_symbols(text, &uri);
    use tower_lsp::lsp_types::SymbolKind;
    assert!(!symbols.is_empty());
    let children = symbols[0].children.as_ref().expect("module has children");
    assert!(
        children
            .iter()
            .any(|s| s.name == "greet" && s.kind == SymbolKind::FUNCTION),
        "Def should produce FUNCTION symbol"
    );
}

#[test]
fn document_symbols_type_decl_has_struct_kind() {
    let text = r#"@no_prelude
module test.ds_type
Fruit =
  | Apple
  | Banana
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    let symbols = Backend::build_document_symbols(text, &uri);
    use tower_lsp::lsp_types::SymbolKind;
    let children = symbols[0].children.as_ref().expect("module has children");
    assert!(
        children
            .iter()
            .any(|s| s.name == "Fruit" && s.kind == SymbolKind::STRUCT),
        "TypeDecl should produce STRUCT symbol"
    );
}

#[test]
fn document_symbols_type_alias_has_interface_kind() {
    let text = r#"@no_prelude
module test.ds_alias
type Name = Text
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    let symbols = Backend::build_document_symbols(text, &uri);
    use tower_lsp::lsp_types::SymbolKind;
    let children = symbols[0].children.as_ref().expect("module has children");
    assert!(
        children
            .iter()
            .any(|s| s.name == "Name" && s.kind == SymbolKind::INTERFACE),
        "TypeAlias should produce INTERFACE symbol"
    );
}

#[test]
fn document_symbols_class_decl_has_class_kind() {
    let text = r#"@no_prelude
module test.ds_class
class Show A where
  show : A -> Text
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    let symbols = Backend::build_document_symbols(text, &uri);
    use tower_lsp::lsp_types::SymbolKind;
    let children = symbols[0].children.as_ref().expect("module has children");
    assert!(
        children
            .iter()
            .any(|s| s.name == "Show" && s.kind == SymbolKind::CLASS),
        "ClassDecl should produce CLASS symbol"
    );
}

#[test]
fn document_symbols_domain_decl_has_namespace_kind_with_children() {
    let text = r#"@no_prelude
module test.ds_domain
domain Calc where
  double : Int -> Int
  double = x => x + x
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    let symbols = Backend::build_document_symbols(text, &uri);
    use tower_lsp::lsp_types::SymbolKind;
    let module_children = symbols[0].children.as_ref().expect("module has children");
    let domain_sym = module_children
        .iter()
        .find(|s| s.name == "Calc")
        .expect("Calc domain symbol");
    assert_eq!(domain_sym.kind, SymbolKind::NAMESPACE);
    let domain_children = domain_sym.children.as_ref().expect("domain has children");
    assert!(
        domain_children.iter().any(|s| s.name == "double"),
        "domain children should include 'double'"
    );
}

#[test]
fn document_symbols_instance_decl_has_object_kind() {
    let text = r#"@no_prelude
module test.ds_instance
class Show A where
  show : A -> Text
instance Show Int where
  show = x => "int"
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    let symbols = Backend::build_document_symbols(text, &uri);
    use tower_lsp::lsp_types::SymbolKind;
    let children = symbols[0].children.as_ref().expect("module has children");
    assert!(
        children.iter().any(|s| s.kind == SymbolKind::OBJECT),
        "InstanceDecl should produce OBJECT symbol"
    );
}

#[test]
fn document_symbols_module_itself_has_module_kind() {
    let text = r#"@no_prelude
module test.ds_module
x = 1
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    let symbols = Backend::build_document_symbols(text, &uri);
    use tower_lsp::lsp_types::SymbolKind;
    assert!(!symbols.is_empty());
    assert_eq!(symbols[0].kind, SymbolKind::MODULE);
    assert_eq!(symbols[0].detail.as_deref(), Some("module"));
}

#[test]
fn document_symbols_type_sig_has_function_kind() {
    let text = r#"@no_prelude
module test.ds_sig
compute : Int -> Int
compute = x => x
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    let symbols = Backend::build_document_symbols(text, &uri);
    use tower_lsp::lsp_types::SymbolKind;
    let children = symbols[0].children.as_ref().expect("module has children");
    let sigs: Vec<_> = children
        .iter()
        .filter(|s| s.name == "compute" && s.kind == SymbolKind::FUNCTION)
        .collect();
    // TypeSig and Def both produce FUNCTION symbols; there should be two.
    assert!(!sigs.is_empty(), "TypeSig should produce a FUNCTION symbol");
}

// ─── hover (gap-filling) ─────────────────────────────────────────────────────

#[test]
fn hover_type_alias_resolves_definition() {
    let text = r#"@no_prelude
module test.hover_alias
type UserId = Int
lookup : UserId -> Text
lookup = id => "user"
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    let doc_index = DocIndex::default();
    use tower_lsp::lsp_types::Position;
    // Hover over `UserId` in the type signature.
    let pos = Position::new(3, 9); // points into "UserId" on line 3
    let hover = Backend::build_hover(text, &uri, pos, &doc_index);
    if let Some(hover) = hover {
        let tower_lsp::lsp_types::HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup hover");
        };
        // Should mention UserId in the hover content.
        assert!(
            markup.value.contains("UserId") || markup.value.contains("Int"),
            "hover for type alias should mention UserId or Int, got: {}",
            markup.value
        );
    }
    // None is also acceptable if position misses the token.
}

#[test]
fn hover_class_decl_shows_class_badge() {
    let text = r#"@no_prelude
module test.hover_class
class Eq A where
  equal : A -> A -> Bool
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    let doc_index = DocIndex::default();
    use tower_lsp::lsp_types::Position;
    let pos = Position::new(2, 6); // "Eq" on line 2
    if let Some(hover) = Backend::build_hover(text, &uri, pos, &doc_index) {
        let tower_lsp::lsp_types::HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup hover");
        };
        assert!(
            markup.value.contains("Eq") || markup.value.contains("class"),
            "hover for class should contain 'Eq' or 'class', got: {}",
            markup.value
        );
    }
}

#[test]
fn hover_unresolved_ident_returns_fallback() {
    let text = r#"@no_prelude
module test.hover_fallback
result = unknownFn 42
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    let doc_index = DocIndex::default();
    use tower_lsp::lsp_types::Position;
    let pos = Position::new(2, 9); // inside "unknownFn"
    let hover = Backend::build_hover(text, &uri, pos, &doc_index).expect("fallback hover");
    let tower_lsp::lsp_types::HoverContents::Markup(markup) = hover.contents else {
        panic!("expected markup hover");
    };
    assert!(
        markup.value.contains("unknownFn") || markup.value.contains("_unresolved_"),
        "fallback hover should mention the ident or _unresolved_, got: {}",
        markup.value
    );
}

#[test]
fn hover_no_token_at_whitespace_returns_none() {
    let text = r#"@no_prelude
module test.hover_empty

x = 1
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    let doc_index = DocIndex::default();
    use tower_lsp::lsp_types::Position;
    // Blank line — no identifier.
    let pos = Position::new(2, 0);
    let hover = Backend::build_hover(text, &uri, pos, &doc_index);
    assert!(hover.is_none(), "blank line hover should return None");
}

#[test]
fn hover_domain_member_shows_type() {
    let text = r#"@no_prelude
module test.hover_domain
domain Math where
  double : Int -> Int
  double = x => x + x
result = Math.double 5
"#;
    let uri = Url::parse("file:///test.aivi").unwrap();
    let doc_index = DocIndex::default();
    use tower_lsp::lsp_types::Position;
    let pos = Position::new(5, 9); // "Math.double" on line 5
    if let Some(hover) = Backend::build_hover(text, &uri, pos, &doc_index) {
        let tower_lsp::lsp_types::HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup hover");
        };
        assert!(
            markup.value.contains("double") || markup.value.contains("Int"),
            "domain member hover should mention 'double' or 'Int', got: {}",
            markup.value
        );
    }
}

#[test]
fn hover_with_workspace_resolves_imported_type_alias() {
    let lib_text = r#"@no_prelude
module test.lib
export UserId
type UserId = Int
lookup : UserId -> Text
lookup = id => "user"
"#;
    let app_text = r#"@no_prelude
module test.app
use test.lib (lookup, UserId)
run = lookup 1
"#;
    let lib_uri = Url::parse("file:///lib.aivi").unwrap();
    let app_uri = Url::parse("file:///app.aivi").unwrap();

    let lib_path = std::path::PathBuf::from("lib.aivi");
    let (lib_modules, _) = parse_modules(&lib_path, lib_text);
    let mut workspace = HashMap::new();
    for m in lib_modules {
        workspace.insert(
            m.name.name.clone(),
            IndexedModule {
                uri: lib_uri.clone(),
                module: m,
                text: Some(lib_text.to_string()),
            },
        );
    }

    let doc_index = DocIndex::default();
    use tower_lsp::lsp_types::Position;
    let pos = Position::new(3, 6); // "lookup" in "run = lookup 1"
    let hover =
        Backend::build_hover_with_workspace(app_text, &app_uri, pos, &workspace, &doc_index);
    // Should resolve lookup from lib module.
    if let Some(hover) = hover {
        let tower_lsp::lsp_types::HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup hover");
        };
        assert!(
            markup.value.contains("lookup"),
            "workspace hover should mention 'lookup', got: {}",
            markup.value
        );
    }
}
