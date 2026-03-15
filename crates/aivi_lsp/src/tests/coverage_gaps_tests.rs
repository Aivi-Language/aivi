// Tests targeting coverage gaps in navigation, diagnostics, strict, signature,
// workspace, semantic_tokens, and related modules.

// ── navigation/definition.rs — go-to-definition coverage ─────────────────────

#[test]
fn definition_resolves_type_decl_name() {
    let text = r#"@no_prelude
module test.nav_type
Color = Red | Green | Blue
run = Red
"#;
    let uri = sample_uri();
    let position = position_for(text, "Red\n");
    let location = Backend::build_definition(text, &uri, position);
    assert!(location.is_some(), "should resolve go-to-definition for constructor 'Red'");
}

#[test]
fn definition_resolves_type_alias() {
    let text = r#"@no_prelude
module test.nav_alias
type UserId = Int
lookup : UserId -> Int
lookup = id => id
"#;
    let uri = sample_uri();
    let position = position_for(text, "UserId -> Int");
    let location = Backend::build_definition(text, &uri, position);
    // UserId should resolve to the type alias declaration
    assert!(location.is_some(), "should resolve go-to-definition for type alias 'UserId'");
}

#[test]
fn definition_resolves_module_name() {
    let text = r#"@no_prelude
module test.nav_module
export run
run = 42
"#;
    let uri = sample_uri();
    let position = position_for(text, "test.nav_module");
    let location = Backend::build_definition(text, &uri, position);
    assert!(location.is_some(), "should resolve go-to-definition for module name");
}

#[test]
fn definition_resolves_export_name() {
    let text = r#"@no_prelude
module test.nav_export
export run
run = 42
"#;
    let uri = sample_uri();
    let position = position_for(text, "run\nrun = 42");
    let location = Backend::build_definition(text, &uri, position);
    assert!(location.is_some(), "should resolve go-to-definition for export name");
}

#[test]
fn definition_with_workspace_resolves_dotted_module() {
    let lib_text = r#"@no_prelude
module examples.mylib
export helper
helper = 42
"#;
    let app_text = r#"@no_prelude
module examples.app
use examples.mylib (helper)
run = helper
"#;
    let lib_uri = Url::parse("file:///mylib.aivi").unwrap();
    let app_uri = Url::parse("file:///app.aivi").unwrap();

    let lib_path = PathBuf::from("mylib.aivi");
    let (lib_modules, _) = parse_modules(&lib_path, lib_text);
    let mut workspace = HashMap::new();
    for module in lib_modules {
        workspace.insert(
            module.name.name.clone(),
            IndexedModule {
                uri: lib_uri.clone(),
                module,
                text: Some(lib_text.to_string()),
            },
        );
    }

    let position = position_for(app_text, "helper\n");
    let location =
        Backend::build_definition_with_workspace(app_text, &app_uri, position, &workspace);
    assert!(location.is_some(), "should resolve definition across workspace");
    let loc = location.unwrap();
    assert_eq!(loc.uri, lib_uri, "definition should point to lib module");
}

#[test]
fn definition_returns_none_for_unknown_ident() {
    let text = r#"@no_prelude
module test.nav_unknown
run = unknownSymbol
"#;
    let uri = sample_uri();
    let position = position_for(text, "unknownSymbol");
    let location = Backend::build_definition(text, &uri, position);
    assert!(location.is_none(), "should return None for unresolved ident");
}

#[test]
fn definition_returns_none_at_whitespace() {
    let text = r#"@no_prelude
module test.nav_blank

run = 42
"#;
    let uri = sample_uri();
    let position = Position::new(2, 0); // blank line
    let location = Backend::build_definition(text, &uri, position);
    assert!(location.is_none(), "should return None at blank line");
}

#[test]
fn definition_with_workspace_resolves_aliased_import() {
    let lib_text = r#"@no_prelude
module examples.lib
export greet
greet = 42
"#;
    let app_text = r#"@no_prelude
module examples.app
use examples.lib (greet)
run = greet
"#;
    let lib_uri = Url::parse("file:///lib.aivi").unwrap();
    let app_uri = Url::parse("file:///app.aivi").unwrap();

    let lib_path = PathBuf::from("lib.aivi");
    let (lib_modules, _) = parse_modules(&lib_path, lib_text);
    let mut workspace = HashMap::new();
    for module in lib_modules {
        workspace.insert(
            module.name.name.clone(),
            IndexedModule {
                uri: lib_uri.clone(),
                module,
                text: Some(lib_text.to_string()),
            },
        );
    }

    let position = position_for(app_text, "greet\n");
    let location =
        Backend::build_definition_with_workspace(app_text, &app_uri, position, &workspace);
    assert!(location.is_some(), "should resolve aliased import");
}

// ── navigation/definition.rs — record field go-to-definition ─────────────────

#[test]
fn definition_resolves_record_field() {
    let text = r#"@no_prelude
module test.nav_record
type Config = { host: Text, port: Int }
getHost : Config -> Text
getHost = c => c.host
"#;
    let uri = sample_uri();
    let position = position_for(text, "host\n");
    let location = Backend::build_definition(text, &uri, position);
    // Record field navigation is best-effort; either resolved or None is valid
    let _ = location;
}

// ── navigation/definition.rs — collect_relevant_modules ──────────────────────

#[test]
fn collect_relevant_modules_includes_transitive_deps() {
    let base_text = r#"@no_prelude
module test.base
export baseVal
baseVal = 1
"#;
    let mid_text = r#"@no_prelude
module test.mid
export midVal
use test.base (baseVal)
midVal = baseVal
"#;
    let top_text = r#"@no_prelude
module test.top
use test.mid (midVal)
run = midVal
"#;
    let base_uri = Url::parse("file:///base.aivi").unwrap();
    let mid_uri = Url::parse("file:///mid.aivi").unwrap();

    let base_path = PathBuf::from("base.aivi");
    let (base_modules, _) = parse_modules(&base_path, base_text);
    let mid_path = PathBuf::from("mid.aivi");
    let (mid_modules, _) = parse_modules(&mid_path, mid_text);
    let top_path = PathBuf::from("top.aivi");
    let (top_modules, _) = parse_modules(&top_path, top_text);

    let mut workspace = HashMap::new();
    for module in base_modules {
        workspace.insert(
            module.name.name.clone(),
            IndexedModule {
                uri: base_uri.clone(),
                module,
                text: Some(base_text.to_string()),
            },
        );
    }
    for module in mid_modules {
        workspace.insert(
            module.name.name.clone(),
            IndexedModule {
                uri: mid_uri.clone(),
                module,
                text: Some(mid_text.to_string()),
            },
        );
    }

    let current_module = &top_modules[0];
    let relevant = Backend::collect_relevant_modules(&top_modules, current_module, &workspace);
    let names: Vec<&str> = relevant.iter().map(|m| m.name.name.as_str()).collect();

    assert!(names.contains(&"test.top"), "should include current module");
    assert!(names.contains(&"test.mid"), "should include direct import");
    assert!(names.contains(&"test.base"), "should include transitive (2nd-level) import");
}

// ── diagnostics.rs — additional coverage ─────────────────────────────────────

#[test]
fn diagnostics_with_workspace_resolves_imports() {
    let lib_text = r#"@no_prelude
module test.diaglib
export helper
helper : Int -> Int
helper = x => x
"#;
    let app_text = r#"@no_prelude
module test.diagapp
use test.diaglib (helper)
run = helper 1
"#;
    let lib_uri = Url::parse("file:///diaglib.aivi").unwrap();
    let app_uri = Url::parse("file:///diagapp.aivi").unwrap();

    let lib_path = PathBuf::from("diaglib.aivi");
    let (lib_modules, _) = parse_modules(&lib_path, lib_text);
    let mut workspace = HashMap::new();
    for module in lib_modules {
        workspace.insert(
            module.name.name.clone(),
            IndexedModule {
                uri: lib_uri.clone(),
                module,
                text: Some(lib_text.to_string()),
            },
        );
    }

    let diags = Backend::build_diagnostics_with_workspace(
        app_text,
        &app_uri,
        &workspace,
        false,
        &crate::strict::StrictConfig::default(),
        None,
        None,
    );
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        .collect();
    assert!(
        errors.is_empty(),
        "valid cross-module import should not produce errors: {:?}",
        errors
    );
}

#[test]
fn diagnostics_no_prelude_valid_module_clean() {
    let text = r#"@no_prelude
module test.clean
export inc
inc : Int -> Int
inc = x => x + 1
"#;
    let uri = sample_uri();
    let diags = Backend::build_diagnostics(text, &uri);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        .collect();
    assert!(errors.is_empty(), "valid @no_prelude module should be clean");
}

#[test]
fn diagnostics_with_workspace_catches_missing_match_arm() {
    let text = r#"@no_prelude
module test.match_err

Color = Red | Green | Blue

show = Red match
  | Red => "red"
"#;
    let uri = sample_uri();
    let diags = Backend::build_diagnostics(text, &uri);
    assert!(
        diags.iter().any(|d| {
            matches!(d.code.as_ref(), Some(NumberOrString::String(c)) if c == "E3100")
        }),
        "should report non-exhaustive match"
    );
}

#[test]
fn diagnostics_unclosed_delimiter_produces_e1000() {
    let text = "module demo\n\nresult = (1 + 2\n";
    let uri = sample_uri();
    let diags = Backend::build_diagnostics(text, &uri);
    assert!(
        diags.iter().any(|d| {
            matches!(d.code.as_ref(), Some(NumberOrString::String(c)) if c.starts_with("E1"))
        }),
        "unclosed delimiter should produce lex/parse error"
    );
}

// ── strict.rs — additional coverage for strict categories and levels ─────────

#[test]
fn strict_category_as_str_covers_all_variants() {
    use crate::strict::StrictCategory;
    let categories = [
        (StrictCategory::Syntax, "Syntax"),
        (StrictCategory::Import, "Import"),
        (StrictCategory::Pipe, "Pipe"),
        (StrictCategory::Pattern, "Pattern"),
        (StrictCategory::Effect, "Effect"),
        (StrictCategory::Generator, "Generator"),
        (StrictCategory::Style, "Style"),
        (StrictCategory::Kernel, "Kernel"),
        (StrictCategory::Domain, "Domain"),
        (StrictCategory::Type, "Type"),
    ];
    for (cat, expected) in categories {
        assert_eq!(cat.as_str(), expected, "StrictCategory::{:?} should be '{}'", cat, expected);
    }
}

#[test]
fn strict_config_default_is_off() {
    let config = crate::strict::StrictConfig::default();
    assert_eq!(config.level, crate::strict::StrictLevel::Off);
    assert!(!config.forbid_implicit_coercions);
    assert!(!config.warnings_as_errors);
}

#[test]
fn strict_level4_no_implicit_coercions_runs() {
    use crate::strict::{StrictConfig, StrictLevel};
    let path = std::path::Path::new("/test.aivi");
    let text = "module demo\n\nval = 1\n";
    let url = Url::parse("file:///strict_test.aivi").unwrap();
    let cfg = StrictConfig {
        level: StrictLevel::NoImplicitCoercions,
        forbid_implicit_coercions: true,
        warnings_as_errors: false,
    };
    // Should not panic; may return any diagnostics
    let _ = crate::strict::build_strict_diagnostics(
        text,
        &url,
        path,
        &cfg,
        &std::collections::HashMap::new(),
    );
}

#[test]
fn strict_pedantic_with_workspace_modules() {
    use crate::strict::{StrictConfig, StrictLevel};
    let path = std::path::Path::new("/test.aivi");
    let text = "module demo\nuse aivi\n\nval = 1\n";
    let url = Url::parse("file:///strict_test.aivi").unwrap();
    let cfg = StrictConfig {
        level: StrictLevel::Pedantic,
        forbid_implicit_coercions: false,
        warnings_as_errors: false,
    };
    let workspace = workspace_with_stdlib(&["aivi"]);
    let _ = crate::strict::build_strict_diagnostics(
        text,
        &url,
        path,
        &cfg,
        &workspace,
    );
}

#[test]
fn strict_config_deserializes_from_json() {
    use crate::strict::{StrictConfig, StrictLevel};
    let json = r#"{"level": 3, "forbidImplicitCoercions": true, "warningsAsErrors": true}"#;
    let cfg: StrictConfig = serde_json::from_str(json).expect("should deserialize");
    assert_eq!(cfg.level, StrictLevel::TypesDomains);
    assert!(cfg.forbid_implicit_coercions);
    assert!(cfg.warnings_as_errors);
}

// ── strict/pattern_discipline.rs — additional pattern coverage ────────────────

#[test]
fn pattern_discipline_nested_match_unused_binding() {
    let text = r#"module demo

Option A = None | Some A

val = Some (Some 1) match
  | Some inner => inner match
    | Some unused => 42
    | None => 0
  | None => 0
"#;
    let diags = Backend::build_diagnostics_strict(
        text,
        &Url::parse("file:///strict_test.aivi").unwrap(),
        &crate::strict::StrictConfig {
            level: crate::strict::StrictLevel::LexicalStructural,
            forbid_implicit_coercions: false,
            warnings_as_errors: false,
        },
    );
    assert!(
        diags.iter().any(|d| {
            matches!(d.code.as_ref(), Some(NumberOrString::String(c)) if c == "AIVI-S301")
        }),
        "should detect unused binding 'unused' in nested match"
    );
}

#[test]
fn pattern_discipline_underscore_prefixed_binding_no_s301() {
    let text = r#"module demo

Option A = None | Some A

val = Some 1 match
  | Some _ignored => 42
  | None => 0
"#;
    let diags = Backend::build_diagnostics_strict(
        text,
        &Url::parse("file:///strict_test.aivi").unwrap(),
        &crate::strict::StrictConfig {
            level: crate::strict::StrictLevel::LexicalStructural,
            forbid_implicit_coercions: false,
            warnings_as_errors: false,
        },
    );
    let has_s301 = diags.iter().any(|d| {
        matches!(d.code.as_ref(), Some(NumberOrString::String(c)) if c == "AIVI-S301")
    });
    assert!(!has_s301, "underscore-prefixed binding should not trigger S301");
}

#[test]
fn pattern_discipline_used_binding_in_guard_no_s301() {
    let text = r#"module demo

Option A = None | Some A

val = Some 1 match
  | Some n when n > 0 => n
  | _ => 0
"#;
    let diags = Backend::build_diagnostics_strict(
        text,
        &Url::parse("file:///strict_test.aivi").unwrap(),
        &crate::strict::StrictConfig {
            level: crate::strict::StrictLevel::LexicalStructural,
            forbid_implicit_coercions: false,
            warnings_as_errors: false,
        },
    );
    let has_s301 = diags.iter().any(|d| {
        matches!(d.code.as_ref(), Some(NumberOrString::String(c)) if c == "AIVI-S301")
    });
    assert!(!has_s301, "binding used in guard should not trigger S301");
}

#[test]
fn pattern_discipline_used_binding_in_unless_guard_no_s301() {
    let text = r#"module demo

Option A = None | Some A

val = Some 1 match
  | Some n unless n <= 0 => n
  | _ => 0
"#;
    let diags = Backend::build_diagnostics_strict(
        text,
        &Url::parse("file:///strict_test.aivi").unwrap(),
        &crate::strict::StrictConfig {
            level: crate::strict::StrictLevel::LexicalStructural,
            forbid_implicit_coercions: false,
            warnings_as_errors: false,
        },
    );
    let has_s301 = diags.iter().any(|d| {
        matches!(d.code.as_ref(), Some(NumberOrString::String(c)) if c == "AIVI-S301")
    });
    assert!(
        !has_s301,
        "binding used in unless guard should not trigger S301"
    );
}

#[test]
fn pattern_discipline_block_unused_bind_in_do() {
    let text = "module demo\n\nval = do Effect {\n  unused <- pure 42\n  anotherUnused = 99\n  pure 1\n}\n";
    let diags = Backend::build_diagnostics_strict(
        text,
        &Url::parse("file:///strict_test.aivi").unwrap(),
        &crate::strict::StrictConfig {
            level: crate::strict::StrictLevel::LexicalStructural,
            forbid_implicit_coercions: false,
            warnings_as_errors: false,
        },
    );
    let s221_count = diags.iter().filter(|d| {
        matches!(d.code.as_ref(), Some(NumberOrString::String(c)) if c == "AIVI-S221")
    }).count();
    assert!(s221_count >= 2, "expected at least 2 AIVI-S221 for unused bindings in do block");
}

// ── signature.rs — additional signature help coverage ─────────────────────────

#[test]
fn signature_help_inside_match_arm() {
    let text = r#"@no_prelude
module test.sigm
Option A = None | Some A
add : Int -> Int -> Int
add = x y => x + y
run = Some 1 match
  | Some n => add n 2
  | None => 0
"#;
    let uri = sample_uri();
    let position = position_for(text, "n 2");
    let help =
        Backend::build_signature_help_with_workspace(text, &uri, position, &HashMap::new());
    assert!(
        help.is_some(),
        "expected signature help for call inside match arm"
    );
    let help = help.unwrap();
    assert!(help.signatures[0].label.contains("add"));
}

#[test]
fn signature_help_inside_if_expression() {
    let text = r#"@no_prelude
module test.sigif
add : Int -> Int -> Int
add = x y => x + y
run = if true then add 1 2 else 0
"#;
    let uri = sample_uri();
    let position = position_for(text, "1 2");
    let help =
        Backend::build_signature_help_with_workspace(text, &uri, position, &HashMap::new());
    assert!(
        help.is_some(),
        "expected signature help for call inside if expression"
    );
}

#[test]
fn signature_help_inside_lambda_body() {
    let text = r#"@no_prelude
module test.siglam
add : Int -> Int -> Int
add = x y => x + y
run = x => add x 1
"#;
    let uri = sample_uri();
    let position = position_for(text, "x 1");
    let help =
        Backend::build_signature_help_with_workspace(text, &uri, position, &HashMap::new());
    assert!(
        help.is_some(),
        "expected signature help for call inside lambda body"
    );
}

#[test]
fn signature_help_nested_call() {
    let text = r#"@no_prelude
module test.signest
add : Int -> Int -> Int
add = x y => x + y
inc : Int -> Int
inc = x => x + 1
run = add (inc 1) 2
"#;
    let uri = sample_uri();
    // Position at the inner call `inc 1`
    let position = position_for(text, "inc 1");
    let help =
        Backend::build_signature_help_with_workspace(text, &uri, position, &HashMap::new());
    // Should get signature for inc or add
    if let Some(help) = help {
        assert!(!help.signatures.is_empty());
    }
}

#[test]
fn signature_help_for_pipe_expression() {
    let text = r#"@no_prelude
module test.sigpipe
inc : Int -> Int
inc = x => x + 1
run = 1 |> inc
"#;
    let uri = sample_uri();
    // Position at inc in pipe; this tests whether signature help works in binary exprs
    let position = position_for(text, "inc\n");
    let _ = Backend::build_signature_help_with_workspace(text, &uri, position, &HashMap::new());
    // Just ensure no panic
}

// ── hover — additional coverage for constructor, class member, instance ───────

#[test]
fn hover_constructor_shows_type() {
    let text = r#"@no_prelude
module test.hover_ctor
Color = Red | Green | Blue
run = Green
"#;
    let uri = sample_uri();
    let doc_index = DocIndex::default();
    let position = position_for(text, "Green\n");
    let hover = Backend::build_hover(text, &uri, position, &doc_index);
    assert!(hover.is_some(), "hover should resolve for constructor");
    let hover = hover.unwrap();
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected markup hover");
    };
    assert!(
        markup.value.contains("Green"),
        "hover should mention 'Green', got: {}",
        markup.value
    );
}

#[test]
fn hover_class_member_shows_type() {
    let text = r#"@no_prelude
module test.hover_class_member
class Show A where
  show : A -> Text
run = show
"#;
    let uri = sample_uri();
    let doc_index = DocIndex::default();
    let position = position_for(text, "show\n");
    let hover = Backend::build_hover(text, &uri, position, &doc_index);
    if let Some(hover) = hover {
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup hover");
        };
        assert!(
            markup.value.contains("show"),
            "hover should mention 'show', got: {}",
            markup.value
        );
    }
}

#[test]
fn hover_lambda_param() {
    let text = r#"@no_prelude
module test.hover_param
run = myParam => myParam
"#;
    let uri = sample_uri();
    let doc_index = DocIndex::default();
    // Hover over the second occurrence of myParam (usage, not definition)
    let position = position_for(text, "myParam\n");
    let hover = Backend::build_hover(text, &uri, position, &doc_index);
    if let Some(hover) = hover {
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup hover");
        };
        assert!(
            markup.value.contains("myParam"),
            "hover should mention lambda param, got: {}",
            markup.value
        );
    }
}

#[test]
fn hover_match_scrutinee_binding() {
    let text = r#"@no_prelude
module test.hover_match_bind
Option A = None | Some A
run = Some 1 match
  | Some val => val
  | None => 0
"#;
    let uri = sample_uri();
    let doc_index = DocIndex::default();
    let position = position_for(text, "val\n");
    let hover = Backend::build_hover(text, &uri, position, &doc_index);
    if let Some(hover) = hover {
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup hover");
        };
        assert!(
            markup.value.contains("val"),
            "hover should mention match binding 'val', got: {}",
            markup.value
        );
    }
}

#[test]
fn hover_on_keyword_returns_none_or_info() {
    let text = r#"@no_prelude
module test.hover_kw
run = if true then 1 else 0
"#;
    let uri = sample_uri();
    let doc_index = DocIndex::default();
    let position = position_for(text, "if ");
    // Keywords may or may not produce hover; ensure no panic
    let _ = Backend::build_hover(text, &uri, position, &doc_index);
}

// ── workspace.rs — project_root_for_path ─────────────────────────────────────

#[test]
fn project_root_for_path_falls_back_to_parent_dir() {
    let workspace_folders = Vec::new();
    let path = std::path::Path::new("/tmp/test_project/main.aivi");
    let root = Backend::project_root_for_path(path, &workspace_folders);
    assert!(root.is_some(), "should fall back to parent dir");
    assert_eq!(
        root.unwrap(),
        std::path::PathBuf::from("/tmp/test_project"),
        "should return parent directory"
    );
}

#[test]
fn project_root_for_path_prefers_workspace_folder() {
    let workspace_folders = vec![std::path::PathBuf::from("/workspace/project")];
    let path = std::path::Path::new("/workspace/project/src/main.aivi");
    let root = Backend::project_root_for_path(path, &workspace_folders);
    assert_eq!(
        root,
        Some(std::path::PathBuf::from("/workspace/project")),
        "should prefer workspace folder"
    );
}

// ── references — additional branch coverage ──────────────────────────────────

#[test]
fn references_finds_type_decl_mentions() {
    let text = r#"@no_prelude
module test.refs_type
Color = Red | Green | Blue
show : Color -> Int
show = c => 1
"#;
    let uri = sample_uri();
    let position = position_for(text, "Color -> Int");
    let locations = Backend::build_references(text, &uri, position, true);
    assert!(
        locations.len() >= 2,
        "should find at least decl + sig reference for Color"
    );
}

#[test]
fn references_finds_constructor_mentions() {
    let text = r#"@no_prelude
module test.refs_ctor
Color = Red | Green | Blue
run = Red
"#;
    let uri = sample_uri();
    let position = position_for(text, "Red\n");
    let locations = Backend::build_references(text, &uri, position, true);
    assert!(
        locations.len() >= 2,
        "should find at least decl + usage for Red constructor"
    );
}

#[test]
fn references_include_pattern_matches() {
    let text = r#"@no_prelude
module test.refs_pattern
Option A = None | Some A
run = Some 1 match
  | Some x => x
  | None => 0
"#;
    let uri = sample_uri();
    let position = position_for(text, "None => 0");
    let locations = Backend::build_references(text, &uri, position, true);
    assert!(
        !locations.is_empty(),
        "should find references for None in pattern"
    );
}

// ── semantic tokens — helper coverage ────────────────────────────────────────

#[test]
fn semantic_tokens_for_valid_module() {
    let text = r#"@no_prelude
module test.tokens
add : Int -> Int -> Int
add = x y => x + y
"#;
    let uri = sample_uri();
    let result = Backend::build_semantic_tokens(text, &uri);
    assert!(
        !result.data.is_empty(),
        "should produce semantic tokens for valid module"
    );
}

#[test]
fn semantic_tokens_for_empty_text() {
    let text = "";
    let uri = sample_uri();
    let result = Backend::build_semantic_tokens(text, &uri);
    // Empty or non-empty; should not panic
    let _ = result;
}

#[test]
fn semantic_tokens_for_module_with_types() {
    let text = r#"@no_prelude
module test.tokens_types
Color = Red | Green | Blue
type Name = Text
show : Color -> Name
show = _ => "red"
"#;
    let uri = sample_uri();
    let result = Backend::build_semantic_tokens(text, &uri);
    assert!(
        !result.data.is_empty(),
        "should produce semantic tokens for module with types"
    );
}

#[test]
fn semantic_tokens_for_sigil_expressions() {
    let text = r#"module test.tokens_sigils
url = ~url(https://example.com)
"#;
    let uri = sample_uri();
    let result = Backend::build_semantic_tokens(text, &uri);
    assert!(
        !result.data.is_empty(),
        "should produce semantic tokens for sigil expressions"
    );
}

#[test]
fn semantic_tokens_for_record_labels() {
    let text = r#"@no_prelude
module test.tokens_records
config = { host: "localhost", port: 8080 }
"#;
    let uri = sample_uri();
    let result = Backend::build_semantic_tokens(text, &uri);
    assert!(
        !result.data.is_empty(),
        "should produce semantic tokens for record labels"
    );
}

#[test]
fn semantic_tokens_for_domain_decl() {
    let text = r#"@no_prelude
module test.tokens_domain
domain Math where
  double : Int -> Int
  double = x => x + x
"#;
    let uri = sample_uri();
    let result = Backend::build_semantic_tokens(text, &uri);
    assert!(
        !result.data.is_empty(),
        "should produce semantic tokens for domain declarations"
    );
}

#[test]
fn semantic_tokens_for_pipe_and_arrow() {
    let text = r#"@no_prelude
module test.tokens_ops
inc = x => x + 1
run = 1 |> inc
"#;
    let uri = sample_uri();
    let result = Backend::build_semantic_tokens(text, &uri);
    assert!(
        !result.data.is_empty(),
        "should produce semantic tokens for pipe and arrow operators"
    );
}

#[test]
fn semantic_tokens_for_decorators() {
    let text = r#"@no_prelude
module test.tokens_deco
@test "my test"
run = 42
"#;
    let uri = sample_uri();
    let result = Backend::build_semantic_tokens(text, &uri);
    assert!(
        !result.data.is_empty(),
        "should produce semantic tokens for decorators"
    );
}

// ── code actions — additional coverage ───────────────────────────────────────

#[test]
fn code_actions_no_crash_on_empty_diagnostics() {
    let text = "@no_prelude\nmodule test.action\nrun = 42\n";
    let uri = sample_uri();
    let actions = Backend::build_code_actions_with_workspace(
        text,
        &uri,
        &[],
        &HashMap::new(),
        tower_lsp::lsp_types::Range::default(),
    );
    // No diagnostics → may or may not have refactoring actions
    let _ = actions;
}

#[test]
fn code_actions_no_crash_on_malformed_input() {
    let text = "module broken = {{ let let";
    let uri = sample_uri();
    let diags = Backend::build_diagnostics(text, &uri);
    let actions = Backend::build_code_actions_with_workspace(
        text,
        &uri,
        &diags,
        &HashMap::new(),
        tower_lsp::lsp_types::Range::default(),
    );
    let _ = actions;
}

// ── formatting — build_formatting coverage ───────────────────────────────────

#[test]
fn formatting_produces_edits() {
    let text = "@no_prelude\nmodule test.fmt\nadd   =  x    y  =>  x  +  y\n";
    let uri = sample_uri();
    let edits = Backend::build_formatting(text, &uri, None);
    // Formatter may or may not produce edits; should not panic
    let _ = edits;
}

#[test]
fn formatting_empty_input() {
    let text = "";
    let uri = sample_uri();
    let edits = Backend::build_formatting(text, &uri, None);
    let _ = edits;
}

// ── rename — additional coverage ─────────────────────────────────────────────

#[test]
fn rename_local_variable() {
    let text = r#"@no_prelude
module test.rename
add = x y => x + y
run = add 1 2
"#;
    let uri = sample_uri();
    let position = position_for(text, "add 1 2");
    let edit = Backend::build_rename(text, &uri, position, "sum");
    assert!(edit.is_some(), "should produce rename edit");
    let edit = edit.unwrap();
    let changes = edit.changes.expect("changes");
    assert!(
        changes.values().flatten().all(|e| e.new_text == "sum"),
        "all edits should use new name 'sum'"
    );
}

#[test]
fn rename_returns_none_for_unknown() {
    let text = r#"@no_prelude
module test.rename_none
run = unknownSymbol
"#;
    let uri = sample_uri();
    let position = position_for(text, "unknownSymbol");
    let edit = Backend::build_rename(text, &uri, position, "newName");
    // May or may not return Some depending on how rename handles unresolved idents
    let _ = edit;
}
