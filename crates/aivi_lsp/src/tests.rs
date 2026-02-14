use std::collections::HashMap;
use std::path::PathBuf;

use aivi::{format_text_with_options, parse_modules, FormatOptions, ModuleItem, Span};
use tower_lsp::lsp_types::{
    CodeActionOrCommand, DiagnosticSeverity, HoverContents, NumberOrString, Position, Url,
};

use crate::backend::Backend;
use crate::doc_index::DocIndex;
use crate::state::IndexedModule;

fn sample_text() -> &'static str {
    r#"@no_prelude
	module examples.compiler.math = {
	  export add, sub, run

	  add : Number -> Number -> Number
	  sub : Number -> Number -> Number

	  add = x y => x + y
	  sub = x y => x - y
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

fn position_after(text: &str, needle: &str) -> Position {
    let pos = position_for(text, needle);
    Position::new(pos.line, pos.character + needle.chars().count() as u32)
}

fn workspace_with_stdlib(names: &[&str]) -> HashMap<String, IndexedModule> {
    let mut workspace = HashMap::new();
    let modules = aivi::embedded_stdlib_modules();
    for name in names {
        let Some(module) = modules.iter().find(|m| m.name.name == *name) else {
            panic!("expected embedded stdlib module {name}");
        };
        workspace.insert(
            (*name).to_string(),
            IndexedModule {
                uri: Backend::stdlib_uri(name),
                module: module.clone(),
                text: None,
            },
        );
    }
    workspace
}

fn find_symbol_span(text: &str, name: &str) -> Span {
    let path = PathBuf::from("test.aivi");
    let (modules, _) = parse_modules(&path, text);
    for module in modules {
        for item in module.items.iter() {
            if let Some(span) = match item {
                ModuleItem::Def(def) if def.name.name == name => Some(def.name.span.clone()),
                ModuleItem::TypeSig(sig) if sig.name.name == name => Some(sig.name.span.clone()),
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
            if export.name.name == name {
                return export.name.span.clone();
            }
        }
    }
    panic!("symbol not found: {name}");
}

#[test]
fn completion_items_include_keywords_and_defs() {
    let text = sample_text();
    let uri = sample_uri();
    let items = Backend::build_completion_items(text, &uri, Position::new(0, 0), &HashMap::new());
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    assert!(labels.contains(&"module"));
    assert!(labels.contains(&"examples.compiler.math"));
    assert!(labels.contains(&"add"));
}

fn collect_aivi_files(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_aivi_files(&path, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("aivi") {
            out.push(path);
        }
    }
}

#[test]
fn examples_open_without_lsp_errors() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("repo root");
    let examples_dir = repo_root.join("examples");

    let mut files = Vec::new();
    collect_aivi_files(&examples_dir, &mut files);
    files.sort();
    assert!(!files.is_empty(), "expected examples/**/*.aivi");

    // Build a workspace index from all example modules so `use ...` across examples resolves.
    let mut workspace = HashMap::new();
    for path in &files {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let (modules, _diags) = parse_modules(path, &text);
        let Ok(uri) = Url::from_file_path(path) else {
            continue;
        };
        for module in modules {
            workspace.insert(
                module.name.name.clone(),
                IndexedModule {
                    uri: uri.clone(),
                    module,
                    text: Some(text.clone()),
                },
            );
        }
    }

    let mut failures = Vec::new();
    for path in files {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(uri) = Url::from_file_path(&path) else {
            continue;
        };
        let diags = Backend::build_diagnostics_with_workspace(&text, &uri, &workspace);
        let errors: Vec<_> = diags
            .into_iter()
            .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
            .collect();
        if errors.is_empty() {
            continue;
        }
        let mut msg = format!("{}:", path.display());
        for diag in errors.iter().take(5) {
            msg.push_str(&format!(" {}", diag.message));
        }
        failures.push(msg);
    }

    assert!(
        failures.is_empty(),
        "expected no ERROR diagnostics from aivi-lsp for examples; got:\n{}",
        failures.join("\n")
    );
}

#[test]
fn specs_snippets_open_without_lsp_diagnostics() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("repo root");
    let snippets_dir = repo_root.join("specs").join("snippets");

    let mut files = Vec::new();
    collect_aivi_files(&snippets_dir, &mut files);
    files.sort();
    assert!(!files.is_empty(), "expected specs/snippets/**/*.aivi");

    let mut failures = Vec::new();
    for path in files {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(uri) = Url::from_file_path(&path) else {
            continue;
        };
        let diags = Backend::build_diagnostics_with_workspace(&text, &uri, &HashMap::new());
        if diags.is_empty() {
            continue;
        }
        let mut msg = format!("{}:", path.display());
        for diag in diags.iter().take(5) {
            msg.push_str(&format!(" {}", diag.message));
        }
        failures.push(msg);
    }

    assert!(
        failures.is_empty(),
        "expected no diagnostics from aivi-lsp for specs snippets; got:\n{}",
        failures.join("\n")
    );
}

#[test]
fn completion_after_use_suggests_modules() {
    let text = "module examples.app\nuse aivi.t";
    let uri = sample_uri();
    let workspace = workspace_with_stdlib(&["aivi", "aivi.text"]);
    let position = position_after(text, "use aivi.t");
    let items = Backend::build_completion_items(text, &uri, position, &workspace);
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    assert!(labels.contains(&"aivi.text"));
}

#[test]
fn completion_inside_use_import_list_suggests_remaining_exports() {
    let text = "module examples.app\nuse aivi.text (length, isE";
    let uri = sample_uri();
    let workspace = workspace_with_stdlib(&["aivi.text"]);
    let position = position_after(text, "use aivi.text (length, isE");
    let items = Backend::build_completion_items(text, &uri, position, &workspace);
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    assert!(
        !labels.contains(&"length"),
        "already imported export should be filtered"
    );
    assert!(
        labels.contains(&"isEmpty"),
        "expected export completion from module"
    );
}

#[test]
fn completion_after_qualified_module_name_suggests_exports() {
    let text = "module examples.app\nrun = aivi.text.";
    let uri = sample_uri();
    let workspace = workspace_with_stdlib(&["aivi.text"]);
    let position = position_after(text, "aivi.text.");
    let items = Backend::build_completion_items(text, &uri, position, &workspace);
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    assert!(labels.contains(&"length"));
    assert!(labels.contains(&"isEmpty"));
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
module examples.compiler.math
export add
add = x y => x + y"#;
    let app_text = r#"@no_prelude
module examples.compiler.app
export run
use examples.compiler.math (add)
run = add 1 2"#;

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
                text: None,
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
    let doc_index = DocIndex::default();
    let hover = Backend::build_hover(text, &uri, position, &doc_index).expect("hover found");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected markup hover");
    };
    assert!(markup.value.contains("`add`"));
    assert!(markup.value.contains(":"));
}

#[test]
fn build_hover_reports_type_signature_across_files_via_use() {
    let math_text = r#"@no_prelude
module examples.compiler.math
export add
add : Number -> Number -> Number
add = x y => x + y"#;
    let app_text = r#"@no_prelude
module examples.compiler.app
export run
use examples.compiler.math (add)
run = add 1 2"#;

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
                text: None,
            },
        );
    }

    let position = position_for(app_text, "add 1 2");
    let doc_index = DocIndex::default();
    let hover =
        Backend::build_hover_with_workspace(app_text, &app_uri, position, &workspace, &doc_index)
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
module examples.docs
// Identity function.
id = x => x

run = id 1"#;
    let uri = sample_uri();
    let position = position_for(text, "id 1");
    let doc_index = DocIndex::default();
    let hover = Backend::build_hover(text, &uri, position, &doc_index).expect("hover found");
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
module examples.compiler.math
export add
add : Number -> Number -> Number
add = x y => x + y"#;
    let app_text = r#"@no_prelude
module examples.compiler.app
export run
use examples.compiler.math (add)
run = add 1 2"#;

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
                text: None,
            },
        );
    }

    let position = position_for(app_text, "1 2");
    let help =
        Backend::build_signature_help_with_workspace(app_text, &app_uri, position, &workspace)
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
module examples.compiler.math
export add
add : Number -> Number -> Number
add = x y => x + y"#;
    let app_text = r#"@no_prelude
module examples.compiler.app
export run
use examples.compiler.math (add)
run = add 1 2"#;

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
                text: Some(math_text.to_string()),
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
                text: Some(app_text.to_string()),
            },
        );
    }

    let position = position_for(app_text, "add 1 2");
    let locations =
        Backend::build_references_with_workspace(app_text, &app_uri, position, true, &workspace);

    assert!(locations.iter().any(|loc| loc.uri == math_uri));
    assert!(locations.iter().any(|loc| loc.uri == app_uri));
}

#[test]
fn build_rename_edits_across_modules() {
    let math_text = r#"@no_prelude
module examples.compiler.math
export add
add : Number -> Number -> Number
add = x y => x + y"#;
    let app_text = r#"@no_prelude
module examples.compiler.app
export run
use examples.compiler.math (add)
run = add 1 2"#;

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
                text: Some(math_text.to_string()),
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
                text: Some(app_text.to_string()),
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
fn diagnostics_report_missing_module_declaration() {
    let text = "x = 1\n";
    let uri = sample_uri();
    let diagnostics = Backend::build_diagnostics(text, &uri);
    assert!(diagnostics.iter().any(|diag| {
        matches!(diag.code.as_ref(), Some(NumberOrString::String(code)) if code == "E1517")
    }));
}

#[test]
fn diagnostics_report_non_exhaustive_match() {
    let text = r#"module demo

Option A = None | Some A

value = Some 1 ?
  | Some _ => 1
"#;
    let uri = sample_uri();
    let diagnostics = Backend::build_diagnostics(text, &uri);
    assert!(diagnostics.iter().any(|diag| {
        matches!(diag.code.as_ref(), Some(NumberOrString::String(code)) if code == "E3100")
    }));
}

#[test]
#[ignore]
fn diagnostics_report_missing_list_comma() {
    let text = "module demo\n\nitems = [1 2]";
    let uri = sample_uri();
    let diagnostics = Backend::build_diagnostics(text, &uri);
    assert!(diagnostics.iter().any(|diag| {
        matches!(diag.code.as_ref(), Some(NumberOrString::String(code)) if code == "E1524")
    }));
}

#[test]
fn formatting_edits_match_formatter_output() {
    let text = "module demo\n\nmain = effect { _<-print \"hi\" }\n";
    let options = FormatOptions::default();
    let edits = Backend::build_formatting_edits(text, options);
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].range, Backend::full_document_range(text));
    assert_eq!(edits[0].new_text, format_text_with_options(text, options));
}

#[test]
fn formatting_edits_respect_indent_size() {
    let text = "module demo\n\nmain = effect {\n_<-print \"hi\"\n}\n";
    let options = FormatOptions {
        indent_size: 4,
        max_blank_lines: 1,
    };
    let edits = Backend::build_formatting_edits(text, options);
    let formatted = &edits[0].new_text;
    let inner_line = formatted
        .lines()
        .nth(3)
        .expect("expected formatted inner effect line");
    assert!(inner_line.starts_with("    "));
}

#[test]
#[ignore]
fn diagnostics_report_missing_record_comma() {
    let text = "module demo\n\nrecord = { a: 1 b: 2 }";
    let uri = sample_uri();
    let diagnostics = Backend::build_diagnostics(text, &uri);
    assert!(diagnostics.iter().any(|diag| {
        matches!(diag.code.as_ref(), Some(NumberOrString::String(code)) if code == "E1525")
    }));
}

#[test]
fn diagnostics_report_unclosed_brace() {
    let text = "module demo = {";
    let uri = sample_uri();
    let diagnostics = Backend::build_diagnostics(text, &uri);
    assert!(diagnostics.iter().any(|diag| {
        matches!(diag.code.as_ref(), Some(NumberOrString::String(code)) if code == "E1004")
    }));
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

fn collect_semantic_token_texts(text: &str) -> Vec<(u32, String)> {
    let tokens = Backend::build_semantic_tokens(text);
    let lines: Vec<&str> = text.lines().collect();

    let mut abs_line = 0u32;
    let mut abs_start = 0u32;
    let mut out = Vec::new();

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
        out.push((token.token_type, text));
    }

    out
}

#[test]
fn semantic_tokens_split_i18n_sigils_into_delimiters_and_string_content() {
    let text = r#"module Test.i18n
x = ~k"app.welcome"
y = ~m"Hello, {name:Text}!"
"#;

    let tokens = collect_semantic_token_texts(text);

    assert!(
        tokens
            .iter()
            .any(|(ty, s)| *ty == Backend::SEM_TOKEN_SIGIL && s == "~k\""),
        "expected `~k\\\"` prefix to be a sigil token, got: {tokens:?}"
    );
    assert!(
        tokens
            .iter()
            .any(|(ty, s)| *ty == Backend::SEM_TOKEN_STRING && s == "app.welcome"),
        "expected `~k` body to be a string token, got: {tokens:?}"
    );
    assert!(
        tokens
            .iter()
            .any(|(ty, s)| *ty == Backend::SEM_TOKEN_SIGIL && s == "\""),
        "expected closing quote to be a sigil token, got: {tokens:?}"
    );

    assert!(
        tokens
            .iter()
            .any(|(ty, s)| *ty == Backend::SEM_TOKEN_SIGIL && s == "~m\""),
        "expected `~m\\\"` prefix to be a sigil token, got: {tokens:?}"
    );
    assert!(
        tokens.iter().any(|(ty, s)| *ty == Backend::SEM_TOKEN_STRING
            && s == "Hello, {name:Text}!"),
        "expected `~m` body to be a string token, got: {tokens:?}"
    );
}

#[test]
fn semantic_tokens_highlight_html_inside_html_sigil() {
    let text = r#"module Test.html
x = ~html~> <div class="a">{ foo }</div> <~html
"#;

    let tokens = collect_semantic_token_texts(text);

    assert!(
        tokens
            .iter()
            .any(|(ty, s)| *ty == Backend::SEM_TOKEN_TYPE && s == "div"),
        "expected tag name to be highlighted as a type token, got: {tokens:?}"
    );
    assert!(
        tokens
            .iter()
            .any(|(ty, s)| *ty == Backend::SEM_TOKEN_PROPERTY && s == "class"),
        "expected attribute name to be highlighted as a property token, got: {tokens:?}"
    );
    assert!(
        tokens
            .iter()
            .any(|(ty, s)| *ty == Backend::SEM_TOKEN_STRING && s == "\"a\""),
        "expected attribute value to be highlighted as a string token, got: {tokens:?}"
    );
}

#[test]
fn semantic_tokens_highlight_paths_and_calls() {
    let text = r#"use aivi.net.https (get)
main = effect {
  xs = [1, 2]
  ys = xs |> map inc
}
nested = { title: "Report", stats: { count: 3, avg: 1.5 }, tags: ["a"] }
Queue.isEmpty
"#;
    let tokens = Backend::build_semantic_tokens(text);
    let lines: Vec<&str> = text.lines().collect();

    let mut abs_line = 0u32;
    let mut abs_start = 0u32;
    let mut saw_path_head = false;
    let mut saw_path_mid = false;
    let mut saw_path_tail = false;
    let mut saw_map_function = false;
    let mut saw_tags_property = false;
    let mut saw_queue_type = false;
    let mut is_empty_token = None;

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
            (Backend::SEM_TOKEN_PATH_HEAD, "aivi") => saw_path_head = true,
            (Backend::SEM_TOKEN_PATH_MID, "net") => saw_path_mid = true,
            (Backend::SEM_TOKEN_PATH_TAIL, "https") => saw_path_tail = true,
            (Backend::SEM_TOKEN_FUNCTION, "map") => saw_map_function = true,
            (Backend::SEM_TOKEN_PROPERTY, "tags") => saw_tags_property = true,
            (Backend::SEM_TOKEN_TYPE, "Queue") => saw_queue_type = true,
            (_, "isEmpty") => is_empty_token = Some(token.token_type),
            _ => {}
        }
    }

    assert!(saw_path_head);
    assert!(saw_path_mid);
    assert!(saw_path_tail);
    assert!(saw_map_function);
    assert!(saw_tags_property);
    assert!(saw_queue_type);
    if let Some(token_type) = is_empty_token {
        assert!(
            token_type != Backend::SEM_TOKEN_PATH_HEAD
                && token_type != Backend::SEM_TOKEN_PATH_MID
                && token_type != Backend::SEM_TOKEN_PATH_TAIL
        );
    } else {
        panic!("expected isEmpty token");
    }
}

#[test]
fn semantic_tokens_treat_value_signatures_like_function_signatures() {
    let text = r#"module Andreas.test
User = { name: String, age: Int }
user1 : User
user1 = { name: "Alice", age: 3 }
"#;
    let tokens = Backend::build_semantic_tokens(text);
    let lines: Vec<&str> = text.lines().collect();

    let mut abs_line = 0u32;
    let mut abs_start = 0u32;
    let mut user1_token_type = None;

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
        if abs_line == 2 && text == "user1" {
            user1_token_type = Some(token.token_type);
            break;
        }
    }

    assert_eq!(
        user1_token_type,
        Some(Backend::SEM_TOKEN_FUNCTION),
        "expected value signature name to be highlighted like a function signature name",
    );
}

#[test]
fn goto_definition_from_record_field_label_jumps_to_type_alias_field() {
    let text = r#"module Andreas.test
User = { name: String, age: Int }
user1 : User
user1 = { name: "Alice", age: 3 }
"#;
    let uri = sample_uri();

    let position = position_for(text, "name: \"Alice\"");
    let location = Backend::build_definition(text, &uri, position).expect("definition found");

    let expected_position = position_for(text, "name: String");
    assert_eq!(location.uri, uri);
    assert_eq!(location.range.start, expected_position);
}
