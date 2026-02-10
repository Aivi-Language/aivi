use std::collections::HashMap;
use std::path::PathBuf;

use aivi::{parse_modules, ModuleItem, Span};
use tower_lsp::lsp_types::{
    CodeActionOrCommand, DiagnosticSeverity, HoverContents, Position, Url,
};

use crate::backend::Backend;
use crate::state::IndexedModule;

fn sample_text() -> &'static str {
    r#"@no_prelude
module examples.compiler.math = {
  export add, sub

  add : Number -> Number -> Number
  sub : Number -> Number -> Number

  add = x y => x + y
  sub = x y => x - y
}

module examples.compiler.app = {
  export run

  use examples.compiler.math (add)

  run = add 1 2
}
"#
}

fn sample_uri() -> Url {
    Url::parse("file:///test.aivi").expect("valid test uri")
}

fn position_for(text: &str, needle: &str) -> Position {
    let offset = text.find(needle).expect("needle exists");
    let mut line = 0u32;
    let mut column = 0u32;
    for (idx, ch) in text.char_indices() {
        if idx == offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }
    Position::new(line, column)
}

fn find_symbol_span(text: &str, name: &str) -> Span {
    let path = PathBuf::from("test.aivi");
    let (modules, _) = parse_modules(&path, text);
    for module in modules {
        for item in module.items.iter() {
            if let Some(span) = match item {
                ModuleItem::Def(def) if def.name.name == name => Some(def.name.span.clone()),
                ModuleItem::TypeSig(sig) if sig.name.name == name => {
                    Some(sig.name.span.clone())
                }
                ModuleItem::TypeDecl(decl) if decl.name.name == name => {
                    Some(decl.name.span.clone())
                }
                ModuleItem::TypeAlias(alias) if alias.name.name == name => {
                    Some(alias.name.span.clone())
                }
                ModuleItem::ClassDecl(class_decl) if class_decl.name.name == name => {
                    Some(class_decl.name.span.clone())
                }
                ModuleItem::InstanceDecl(instance_decl) if instance_decl.name.name == name => {
                    Some(instance_decl.name.span.clone())
                }
                ModuleItem::DomainDecl(domain_decl) if domain_decl.name.name == name => {
                    Some(domain_decl.name.span.clone())
                }
                _ => None,
            } {
                return span;
            }
        }
        for export in module.exports.iter() {
            if export.name == name {
                return export.span.clone();
            }
        }
    }
    panic!("symbol not found: {name}");
}

#[test]
fn completion_items_include_keywords_and_defs() {
    let text = sample_text();
    let uri = sample_uri();
    let items = Backend::build_completion_items(text, &uri, &HashMap::new());
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    assert!(labels.contains(&"module"));
    assert!(labels.contains(&"examples.compiler.math"));
    assert!(labels.contains(&"add"));
}

#[test]
fn build_definition_resolves_def() {
    let text = sample_text();
    let uri = sample_uri();
    let position = position_for(text, "add 1 2");
    let location = Backend::build_definition(text, &uri, position).expect("definition found");
    let expected_span = find_symbol_span(text, "add");
    let expected_range = Backend::span_to_range(expected_span);
    assert_eq!(location.range, expected_range);
}

#[test]
fn build_definition_resolves_def_across_files_via_use() {
    let math_text = r#"@no_prelude
module examples.compiler.math = {
  export add
  add = x y => x + y
}
"#;
    let app_text = r#"@no_prelude
module examples.compiler.app = {
  export run
  use examples.compiler.math (add)
  run = add 1 2
}
"#;

    let math_uri = Url::parse("file:///math.aivi").expect("valid uri");
    let app_uri = Url::parse("file:///app.aivi").expect("valid uri");

    let mut workspace = HashMap::new();
    let math_path = PathBuf::from("math.aivi");
    let (math_modules, _) = parse_modules(&math_path, math_text);
    for module in math_modules {
        workspace.insert(
            module.name.name.clone(),
            IndexedModule {
                uri: math_uri.clone(),
                module,
            },
        );
    }

    let position = position_for(app_text, "add 1 2");
    let location =
        Backend::build_definition_with_workspace(app_text, &app_uri, position, &workspace)
            .expect("definition found");

    let expected_span = find_symbol_span(math_text, "add");
    let expected_range = Backend::span_to_range(expected_span);
    assert_eq!(location.uri, math_uri);
    assert_eq!(location.range, expected_range);
}

#[test]
fn build_hover_reports_type_signature() {
    let text = sample_text();
    let uri = sample_uri();
    let position = position_for(text, "add 1 2");
    let hover = Backend::build_hover(text, &uri, position).expect("hover found");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected markup hover");
    };
    assert!(markup.value.contains("`add`"));
    assert!(markup.value.contains(":"));
}

#[test]
fn build_hover_reports_type_signature_across_files_via_use() {
    let math_text = r#"@no_prelude
module examples.compiler.math = {
  export add
  add : Number -> Number -> Number
  add = x y => x + y
}
"#;
    let app_text = r#"@no_prelude
module examples.compiler.app = {
  export run
  use examples.compiler.math (add)
  run = add 1 2
}
"#;

    let math_uri = Url::parse("file:///math.aivi").expect("valid uri");
    let app_uri = Url::parse("file:///app.aivi").expect("valid uri");

    let mut workspace = HashMap::new();
    let math_path = PathBuf::from("math.aivi");
    let (math_modules, _) = parse_modules(&math_path, math_text);
    for module in math_modules {
        workspace.insert(
            module.name.name.clone(),
            IndexedModule {
                uri: math_uri.clone(),
                module,
            },
        );
    }

    let position = position_for(app_text, "add 1 2");
    let hover = Backend::build_hover_with_workspace(app_text, &app_uri, position, &workspace)
        .expect("hover found");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected markup hover");
    };
    assert!(markup.value.contains("`add`"));
    assert!(markup.value.contains("Number"));
}

#[test]
fn build_hover_includes_docs_and_inferred_types() {
    let text = r#"@no_prelude
module examples.docs = {
  // Identity function.
  id = x => x

  run = id 1
}
"#;
    let uri = sample_uri();
    let position = position_for(text, "id 1");
    let hover = Backend::build_hover(text, &uri, position).expect("hover found");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected markup hover");
    };
    assert!(markup.value.contains("`id`"));
    assert!(markup.value.contains(":"));
    assert!(markup.value.contains("Identity function."));
}

#[test]
fn build_references_finds_symbol_mentions() {
    let text = sample_text();
    let uri = sample_uri();
    let position = position_for(text, "add 1 2");
    let locations = Backend::build_references(text, &uri, position, true);
    let expected_span = find_symbol_span(text, "add");
    let expected_range = Backend::span_to_range(expected_span);
    assert!(locations
        .iter()
        .any(|location| location.range == expected_range));
    assert!(locations.len() >= 2);
}

#[test]
fn build_signature_help_resolves_imported_type_sig() {
    let math_text = r#"@no_prelude
module examples.compiler.math = {
  export add
  add : Number -> Number -> Number
  add = x y => x + y
}
"#;
    let app_text = r#"@no_prelude
module examples.compiler.app = {
  export run
  use examples.compiler.math (add)
  run = add 1 2
}
"#;

    let math_uri = Url::parse("file:///math.aivi").expect("valid uri");
    let app_uri = Url::parse("file:///app.aivi").expect("valid uri");

    let mut workspace = HashMap::new();
    let math_path = PathBuf::from("math.aivi");
    let (math_modules, _) = parse_modules(&math_path, math_text);
    for module in math_modules {
        workspace.insert(
            module.name.name.clone(),
            IndexedModule {
                uri: math_uri.clone(),
                module,
            },
        );
    }

    let position = position_for(app_text, "1 2");
    let help = Backend::build_signature_help_with_workspace(app_text, &app_uri, position, &workspace)
        .expect("signature help");

    assert_eq!(help.active_signature, Some(0));
    assert_eq!(help.active_parameter, Some(0));
    assert_eq!(help.signatures.len(), 1);
    assert!(help.signatures[0].label.contains("`add`"));
    assert!(help.signatures[0].label.contains("Number"));
}

#[test]
fn build_references_searches_across_modules() {
    let math_text = r#"@no_prelude
module examples.compiler.math = {
  export add
  add : Number -> Number -> Number
  add = x y => x + y
}
"#;
    let app_text = r#"@no_prelude
module examples.compiler.app = {
  export run
  use examples.compiler.math (add)
  run = add 1 2
}
"#;

    let math_uri = Url::parse("file:///math.aivi").expect("valid uri");
    let app_uri = Url::parse("file:///app.aivi").expect("valid uri");

    let mut workspace = HashMap::new();
    let math_path = PathBuf::from("math.aivi");
    let (math_modules, _) = parse_modules(&math_path, math_text);
    for module in math_modules {
        workspace.insert(
            module.name.name.clone(),
            IndexedModule {
                uri: math_uri.clone(),
                module,
            },
        );
    }
    let app_path = PathBuf::from("app.aivi");
    let (app_modules, _) = parse_modules(&app_path, app_text);
    for module in app_modules {
        workspace.insert(
            module.name.name.clone(),
            IndexedModule {
                uri: app_uri.clone(),
                module,
            },
        );
    }

    let position = position_for(app_text, "add 1 2");
    let locations = Backend::build_references_with_workspace(
        app_text, &app_uri, position, true, &workspace,
    );

    assert!(locations.iter().any(|loc| loc.uri == math_uri));
    assert!(locations.iter().any(|loc| loc.uri == app_uri));
}

#[test]
fn build_rename_edits_across_modules() {
    let math_text = r#"@no_prelude
module examples.compiler.math = {
  export add
  add : Number -> Number -> Number
  add = x y => x + y
}
"#;
    let app_text = r#"@no_prelude
module examples.compiler.app = {
  export run
  use examples.compiler.math (add)
  run = add 1 2
}
"#;

    let math_uri = Url::parse("file:///math.aivi").expect("valid uri");
    let app_uri = Url::parse("file:///app.aivi").expect("valid uri");

    let mut workspace = HashMap::new();
    let math_path = PathBuf::from("math.aivi");
    let (math_modules, _) = parse_modules(&math_path, math_text);
    for module in math_modules {
        workspace.insert(
            module.name.name.clone(),
            IndexedModule {
                uri: math_uri.clone(),
                module,
            },
        );
    }
    let app_path = PathBuf::from("app.aivi");
    let (app_modules, _) = parse_modules(&app_path, app_text);
    for module in app_modules {
        workspace.insert(
            module.name.name.clone(),
            IndexedModule {
                uri: app_uri.clone(),
                module,
            },
        );
    }

    let position = position_for(app_text, "add 1 2");
    let edit =
        Backend::build_rename_with_workspace(app_text, &app_uri, position, "sum", &workspace)
            .expect("rename edit");

    let changes = edit.changes.expect("changes");
    assert!(changes.contains_key(&math_uri));
    assert!(changes.contains_key(&app_uri));
    assert!(changes
        .values()
        .flatten()
        .all(|edit| edit.new_text == "sum"));
}

#[test]
fn build_diagnostics_reports_error() {
    let text = "module broken = {";
    let uri = sample_uri();
    let diagnostics = Backend::build_diagnostics(text, &uri);
    assert!(!diagnostics.is_empty());
    assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
    assert_eq!(diagnostics[0].source.as_deref(), Some("aivi"));
}

#[test]
fn code_actions_offer_quick_fix_for_unclosed_delimiter() {
    let text = "module broken = {";
    let uri = sample_uri();
    let diagnostics = Backend::build_diagnostics(text, &uri);
    let actions = Backend::build_code_actions(text, &uri, &diagnostics);
    let expected_pos = Backend::end_position(text);

    let mut saw_fix = false;
    for action in actions {
        let CodeActionOrCommand::CodeAction(action) = action else {
            continue;
        };
        if !action.title.contains("Insert missing") {
            continue;
        }
        let Some(edit) = action.edit else {
            continue;
        };
        let Some(changes) = edit.changes else {
            continue;
        };
        let Some(edits) = changes.get(&uri) else {
            continue;
        };
        if edits.iter().any(|edit| {
            edit.new_text == "}"
                && edit.range.start == expected_pos
                && edit.range.end == expected_pos
        }) {
            saw_fix = true;
            break;
        }
    }

    assert!(saw_fix);
}

#[test]
fn document_symbols_include_module_and_children() {
    let text = sample_text();
    let uri = sample_uri();
    let symbols = Backend::build_document_symbols(text, &uri);
    let module = symbols
        .iter()
        .find(|symbol| symbol.name == "examples.compiler.math")
        .expect("module symbol exists");
    let children = module.children.as_ref().expect("module has children");
    let child_names: Vec<&str> = children.iter().map(|child| child.name.as_str()).collect();
    assert!(child_names.contains(&"add"));
    assert!(child_names.contains(&"sub"));
}

#[test]
fn semantic_tokens_highlight_keywords_types_and_literals() {
    let text = sample_text();
    let tokens = Backend::build_semantic_tokens(text);
    let lines: Vec<&str> = text.lines().collect();

    let mut abs_line = 0u32;
    let mut abs_start = 0u32;
    let mut seen_module_keyword = false;
    let mut seen_type_name = false;
    let mut seen_number = false;
    let mut seen_decorator = false;

    for token in tokens.data.iter() {
        abs_line += token.delta_line;
        if token.delta_line == 0 {
            abs_start += token.delta_start;
        } else {
            abs_start = token.delta_start;
        }
        let line = lines.get(abs_line as usize).copied().unwrap_or_default();
        let text: String = line
            .chars()
            .skip(abs_start as usize)
            .take(token.length as usize)
            .collect();

        match (token.token_type, text.as_str()) {
            (Backend::SEM_TOKEN_KEYWORD, "module") => seen_module_keyword = true,
            (Backend::SEM_TOKEN_TYPE, "Number") => seen_type_name = true,
            (Backend::SEM_TOKEN_NUMBER, "1") => seen_number = true,
            (Backend::SEM_TOKEN_DECORATOR, "@") => seen_decorator = true,
            _ => {}
        }
    }

    assert!(seen_module_keyword);
    assert!(seen_type_name);
    assert!(seen_number);
    assert!(seen_decorator);
}
