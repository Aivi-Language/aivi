
fn sample_text() -> &'static str {
    r#"@no_prelude
module examples.compiler.math
export add, sub, run

add : Number -> Number -> Number
sub : Number -> Number -> Number

add = x y => x + y
sub = x y => x - y
run = add 1 2
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
#[ignore]
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
        let diags = Backend::build_diagnostics_with_workspace(
            &text,
            &uri,
            &workspace,
            false,
            &crate::strict::StrictConfig::default(),
        );
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
        let diags = Backend::build_diagnostics_with_workspace(
            &text,
            &uri,
            &HashMap::new(),
            false,
            &crate::strict::StrictConfig::default(),
        );
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
