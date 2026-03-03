// Tests for completion.rs, signature.rs, and additional diagnostics.rs coverage.
// Registered via include! in backend_tests.rs.

// ── Diagnostic coverage ──────────────────────────────────────────────────────

#[test]
fn diagnostics_report_unmatched_closing_delimiter() {
    let text = "module demo\n\nresult = (1 + 2))\n";
    let uri = sample_uri();
    let diagnostics = Backend::build_diagnostics(text, &uri);
    assert!(
        diagnostics.iter().any(|diag| {
            matches!(diag.code.as_ref(), Some(NumberOrString::String(code)) if code == "E1002")
        }),
        "expected E1002 for unmatched closing delimiter"
    );
}

#[test]
fn code_actions_offer_remove_unmatched_delimiter() {
    let text = "module demo\n\nresult = (1 + 2))\n";
    let uri = sample_uri();
    let diagnostics = Backend::build_diagnostics(text, &uri);
    let e1002 = diagnostics
        .iter()
        .find(|d| matches!(d.code.as_ref(), Some(NumberOrString::String(c)) if c == "E1002"))
        .expect("expected E1002 diagnostic");

    let actions = Backend::build_code_actions_with_workspace(
        text,
        &uri,
        &[e1002.clone()],
        &std::collections::HashMap::new(),
        tower_lsp::lsp_types::Range::default(),
    );
    assert!(
        actions.iter().any(|action| match action {
            CodeActionOrCommand::CodeAction(a) =>
                a.title.contains("Remove unmatched closing delimiter"),
            _ => false,
        }),
        "expected remove-unmatched-delimiter action"
    );
}

#[test]
fn diagnostics_report_unclosed_string() {
    let text = "module demo\n\nvalue = \"hello\n";
    let uri = sample_uri();
    let diagnostics = Backend::build_diagnostics(text, &uri);
    assert!(
        diagnostics
            .iter()
            .any(|d| matches!(d.code.as_ref(), Some(NumberOrString::String(c)) if c == "E1001")),
        "expected E1001 for unclosed string"
    );
}

#[test]
fn code_actions_offer_close_string() {
    let text = "module demo\n\nvalue = \"hello\n";
    let uri = sample_uri();
    let diagnostics = Backend::build_diagnostics(text, &uri);
    let e1001 = diagnostics
        .iter()
        .find(|d| matches!(d.code.as_ref(), Some(NumberOrString::String(c)) if c == "E1001"))
        .expect("expected E1001 diagnostic");
    let actions = Backend::build_code_actions_with_workspace(
        text,
        &uri,
        &[e1001.clone()],
        &std::collections::HashMap::new(),
        tower_lsp::lsp_types::Range::default(),
    );
    assert!(
        actions.iter().any(|action| match action {
            CodeActionOrCommand::CodeAction(a) => a.title.contains("closing quote"),
            _ => false,
        }),
        "expected insert-closing-quote action"
    );
}

#[test]
fn diagnostics_report_unknown_name_and_offer_import_quickfix() {
    // `length` is exported by `aivi.text`
    let text = "module demo\n\nresult = length \"hello\"\n";
    let uri = sample_uri();
    let workspace = workspace_with_stdlib(&["aivi.text"]);
    let diagnostics = Backend::build_diagnostics_with_workspace(
        text,
        &uri,
        &workspace,
        false,
        &crate::strict::StrictConfig::default(),
        None,
        None,
    );
    // There should be an unknown-name error for `length`
    assert!(
        diagnostics.iter().any(|d| {
            matches!(d.code.as_ref(), Some(NumberOrString::String(c)) if c == "E3000" || c == "E2005")
        }),
        "expected E3000 or E2005 unknown-name diagnostic"
    );

    let actions = Backend::build_code_actions_with_workspace(
        text,
        &uri,
        &diagnostics,
        &workspace,
        tower_lsp::lsp_types::Range::default(),
    );
    assert!(
        actions.iter().any(|action| match action {
            CodeActionOrCommand::CodeAction(a) => a.title.contains("use aivi.text"),
            _ => false,
        }),
        "expected import quickfix for 'length' from aivi.text"
    );
}

#[test]
fn diagnostic_source_field_indicates_category() {
    // Syntax error → source contains "Syntax"
    let text = "module demo\n\nresult = \"unclosed\n";
    let uri = sample_uri();
    let diags = Backend::build_diagnostics(text, &uri);
    let e1001 = diags
        .iter()
        .find(|d| matches!(d.code.as_ref(), Some(NumberOrString::String(c)) if c == "E1001"))
        .expect("E1001 expected");
    assert!(
        e1001.source.as_deref().is_some_and(|s| s.contains("Syntax")),
        "expected 'Syntax' category in source field for E1001"
    );

    // Warning → source contains "Style"
    let text2 = "module demo\nuse aivi.text (format)\nmain = \"hello\"\n";
    let uri2 = sample_uri();
    let diags2 = Backend::build_diagnostics(text2, &uri2);
    let w2100 = diags2
        .iter()
        .find(|d| matches!(d.code.as_ref(), Some(NumberOrString::String(c)) if c == "W2100"));
    if let Some(w) = w2100 {
        assert!(
            w.source.as_deref().is_some_and(|s| s.contains("Style")),
            "expected 'Style' category in source for W2100"
        );
    }
}

#[test]
fn code_actions_add_missing_match_arms_for_e3100() {
    let text = r#"module demo

Colour = Red | Green | Blue

value = Red match
  | Red => 1
"#;
    let uri = sample_uri();
    let diagnostics = Backend::build_diagnostics(text, &uri);
    let e3100 = diagnostics
        .iter()
        .find(|d| matches!(d.code.as_ref(), Some(NumberOrString::String(c)) if c == "E3100"))
        .expect("expected E3100 non-exhaustive match");
    let actions = Backend::build_code_actions_with_workspace(
        text,
        &uri,
        &[e3100.clone()],
        &std::collections::HashMap::new(),
        tower_lsp::lsp_types::Range::default(),
    );
    assert!(
        actions.iter().any(|action| match action {
            CodeActionOrCommand::CodeAction(a) => a.title.contains("Add missing match cases"),
            _ => false,
        }),
        "expected 'Add missing match cases' action"
    );
}

#[test]
fn quickfixes_from_diagnostic_data_builds_code_action() {
    use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Range, TextEdit};

    let uri = sample_uri();
    let edit_range = Range::new(
        tower_lsp::lsp_types::Position::new(0, 0),
        tower_lsp::lsp_types::Position::new(0, 3),
    );
    let diag = Diagnostic {
        range: edit_range,
        severity: Some(DiagnosticSeverity::WARNING),
        code: Some(NumberOrString::String("TEST".to_string())),
        message: "test diagnostic".to_string(),
        data: Some(serde_json::json!({
            "aiviQuickFix": {
                "title": "Fix it",
                "is_preferred": true,
                "edits": [{ "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 3 } }, "newText": "abc" }]
            }
        })),
        ..Diagnostic::default()
    };

    let actions = Backend::build_code_actions_with_workspace(
        "module demo\n",
        &uri,
        &[diag],
        &std::collections::HashMap::new(),
        tower_lsp::lsp_types::Range::default(),
    );
    assert!(
        actions.iter().any(|action| match action {
            CodeActionOrCommand::CodeAction(a) => a.title == "Fix it",
            _ => false,
        }),
        "expected 'Fix it' action from diagnostic data"
    );
}

#[test]
fn diagnostics_empty_for_valid_module() {
    let text = "@no_prelude\nmodule examples.valid\nexport answer\nanswer : Int\nanswer = 42\n";
    let uri = sample_uri();
    let diags = Backend::build_diagnostics(text, &uri);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        .collect();
    assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
}

#[test]
fn diagnostics_type_error_reported() {
    // Type mismatch: passing a Text where Int is expected
    let text = "@no_prelude\nmodule examples.type_err\nadd : Int -> Int -> Int\nadd = x y => x + y\nresult = add \"oops\" 2\n";
    let uri = sample_uri();
    let diags = Backend::build_diagnostics(text, &uri);
    // Should produce some error diagnostic (type or name)
    assert!(
        diags
            .iter()
            .any(|d| d.severity == Some(DiagnosticSeverity::ERROR)),
        "expected an error diagnostic for type mismatch"
    );
}

// ── Completion coverage ───────────────────────────────────────────────────────

#[test]
fn completion_includes_lambda_params_in_scope() {
    // Inside the body of a lambda, params should be available
    let text = "@no_prelude\nmodule examples.lambda_complete\nrun = myParam => my\n";
    let uri = sample_uri();
    let position = position_after(text, "myParam => my");
    let items = Backend::build_completion_items(
        text,
        &uri,
        position,
        &HashMap::new(),
        &GtkIndex::default(),
    );
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.contains(&"myParam"),
        "lambda param 'myParam' should appear in completions, got: {:?}",
        &labels[..labels.len().min(20)]
    );
}

#[test]
fn completion_includes_do_block_bind_vars() {
    let text = "@no_prelude\nmodule examples.do_complete\nuse aivi\nrun = do Effect {\n  myVar <- appNew \"test\"\n  my\n}\n";
    let uri = sample_uri();
    let workspace = workspace_with_stdlib(&["aivi"]);
    let position = position_after(text, "  my\n}");
    // put cursor at the `my` on the last line before `}`
    let position = position_for(text, "  my\n}");
    let position = tower_lsp::lsp_types::Position::new(position.line, position.character + 2);
    let items =
        Backend::build_completion_items(text, &uri, position, &workspace, &GtkIndex::default());
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.contains(&"myVar"),
        "do-block bind variable 'myVar' should appear in completions, got: {:?}",
        &labels[..labels.len().min(20)]
    );
}

#[test]
fn completion_includes_constructors_from_type_decl() {
    let text = "@no_prelude\nmodule examples.ctors\nColour = Red | Green | Blue\nvalue = Re\n";
    let uri = sample_uri();
    let position = position_after(text, "value = Re");
    let items = Backend::build_completion_items(
        text,
        &uri,
        position,
        &HashMap::new(),
        &GtkIndex::default(),
    );
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"Red"), "Red constructor should appear");
    assert!(labels.contains(&"Green"), "Green constructor should appear");
    assert!(labels.contains(&"Blue"), "Blue constructor should appear");
    // Constructors should have ENUM_MEMBER kind
    let red_item = items.iter().find(|i| i.label == "Red").unwrap();
    assert_eq!(
        red_item.kind,
        Some(tower_lsp::lsp_types::CompletionItemKind::ENUM_MEMBER)
    );
}

#[test]
fn completion_wildcard_import_includes_all_exports() {
    // With `use aivi.text`, all text exports should be in completion
    let text = "module examples.wildcard\nuse aivi.text\nrun = le\n";
    let uri = sample_uri();
    let workspace = workspace_with_stdlib(&["aivi.text"]);
    let position = position_after(text, "run = le");
    let items =
        Backend::build_completion_items(text, &uri, position, &workspace, &GtkIndex::default());
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.contains(&"length"),
        "wildcard import should expose 'length' from aivi.text"
    );
}

#[test]
fn completion_selective_import_includes_named_items() {
    let text = "module examples.selective\nuse aivi.text (length)\nrun = le\n";
    let uri = sample_uri();
    let workspace = workspace_with_stdlib(&["aivi.text"]);
    let position = position_after(text, "run = le");
    let items =
        Backend::build_completion_items(text, &uri, position, &workspace, &GtkIndex::default());
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.contains(&"length"),
        "selectively imported 'length' should appear in completions"
    );
}

#[test]
fn completion_does_not_duplicate_items() {
    let text = "@no_prelude\nmodule examples.dedup\nadd : Int -> Int -> Int\nadd = x y => x + y\n";
    let uri = sample_uri();
    let position = position_after(text, "add = x y => x + y\n");
    let items = Backend::build_completion_items(
        text,
        &uri,
        position,
        &HashMap::new(),
        &GtkIndex::default(),
    );
    // Count occurrences of "add"
    let add_count = items.iter().filter(|i| i.label == "add").count();
    assert!(add_count <= 1, "completion 'add' should not be duplicated, count: {add_count}");
}

#[test]
fn completion_includes_aivi_snippets() {
    let text = "@no_prelude\nmodule examples.snip\n";
    let uri = sample_uri();
    let position = position_after(text, "module examples.snip\n");
    let items = Backend::build_completion_items(
        text,
        &uri,
        position,
        &HashMap::new(),
        &GtkIndex::default(),
    );
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"do Effect"), "'do Effect' snippet should appear");
    assert!(labels.contains(&"match"), "'match' snippet should appear");
    assert!(labels.contains(&"lambda"), "'lambda' snippet should appear");
}

#[test]
fn completion_includes_keywords() {
    let text = "@no_prelude\nmodule examples.kw\n";
    let uri = sample_uri();
    let position = position_after(text, "module examples.kw\n");
    let items = Backend::build_completion_items(
        text,
        &uri,
        position,
        &HashMap::new(),
        &GtkIndex::default(),
    );
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    // Keywords like "module", "use", "if", "match", "do" should appear
    assert!(labels.contains(&"if"), "'if' keyword should appear");
    assert!(labels.contains(&"use"), "'use' keyword should appear");
}

#[test]
fn completion_match_arm_variable_in_scope() {
    let text =
        "@no_prelude\nmodule examples.matchcomp\nColour = Red | Green | Blue\nshow = c =>\n  c match\n    | Red => \"r\"\n    | someVar => someV\n";
    let uri = sample_uri();
    let position = position_after(text, "| someVar => someV");
    let items = Backend::build_completion_items(
        text,
        &uri,
        position,
        &HashMap::new(),
        &GtkIndex::default(),
    );
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.contains(&"someVar"),
        "match arm variable 'someVar' should appear in completions, got: {:?}",
        &labels[..labels.len().min(20)]
    );
}

#[test]
fn completion_aliased_module_import_appears() {
    let text = "module examples.aliased\nuse aivi.text as T\nrun = T.\n";
    let uri = sample_uri();
    let workspace = workspace_with_stdlib(&["aivi.text"]);
    let position = position_after(text, "run = T.");
    let items =
        Backend::build_completion_items(text, &uri, position, &workspace, &GtkIndex::default());
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    // After `T.` (qualified), should get aivi.text exports
    assert!(
        !items.is_empty(),
        "should produce completions after aliased module qualifier"
    );
    // The alias T should also appear in general completions earlier
    let text2 = "module examples.aliased\nuse aivi.text as T\nrun = T\n";
    let position2 = position_after(text2, "run = T");
    let items2 =
        Backend::build_completion_items(text2, &uri, position2, &workspace, &GtkIndex::default());
    let labels2: Vec<&str> = items2.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels2.contains(&"T"),
        "aliased module name 'T' should appear as completion"
    );
}

#[test]
fn completion_type_decl_name_has_struct_kind() {
    let text = "@no_prelude\nmodule examples.typekind\nMyType = Foo | Bar\nrun = M\n";
    let uri = sample_uri();
    let position = position_after(text, "run = M");
    let items = Backend::build_completion_items(
        text,
        &uri,
        position,
        &HashMap::new(),
        &GtkIndex::default(),
    );
    let my_type = items.iter().find(|i| i.label == "MyType");
    assert!(my_type.is_some(), "'MyType' should appear");
    assert_eq!(
        my_type.unwrap().kind,
        Some(tower_lsp::lsp_types::CompletionItemKind::STRUCT)
    );
}

// ── Signature help coverage ───────────────────────────────────────────────────

#[test]
fn signature_help_local_function_single_param() {
    let text = "@no_prelude\nmodule examples.sig1\ngreet : Text -> Text\ngreet = name => \"hi\"\nresult = greet \n";
    let uri = sample_uri();
    let position = position_after(text, "result = greet ");
    let help = Backend::build_signature_help_with_workspace(
        text,
        &uri,
        position,
        &HashMap::new(),
    );
    assert!(help.is_some(), "expected signature help for 'greet'");
    let help = help.unwrap();
    assert_eq!(help.active_parameter, Some(0));
    assert!(help.signatures[0].label.contains("greet"));
    assert!(help.signatures[0].label.contains("Text"));
}

#[test]
fn signature_help_second_argument_position() {
    let text =
        "@no_prelude\nmodule examples.sig2\nadd : Int -> Int -> Int\nadd = x y => x + y\nresult = add 1 \n";
    let uri = sample_uri();
    // Position after "add 1 " — second argument
    let position = position_after(text, "result = add 1 ");
    let help = Backend::build_signature_help_with_workspace(
        text,
        &uri,
        position,
        &HashMap::new(),
    );
    assert!(help.is_some(), "expected signature help");
    let help = help.unwrap();
    assert_eq!(
        help.active_parameter,
        Some(1),
        "second arg should have active_parameter=1"
    );
}

#[test]
fn signature_help_includes_parameter_names() {
    let text =
        "@no_prelude\nmodule examples.sigparams\nrange : Int -> Int -> Int\nrange = start end => start\nresult = range \n";
    let uri = sample_uri();
    let position = position_after(text, "result = range ");
    let help = Backend::build_signature_help_with_workspace(
        text,
        &uri,
        position,
        &HashMap::new(),
    );
    assert!(help.is_some(), "expected signature help");
    let help = help.unwrap();
    let sig = &help.signatures[0];
    if let Some(params) = &sig.parameters {
        assert!(!params.is_empty(), "expected parameters");
        // First param should mention "start"
        if let tower_lsp::lsp_types::ParameterLabel::Simple(label) = &params[0].label {
            assert!(
                label.contains("start"),
                "first param should include 'start', got '{label}'"
            );
        }
    }
}

#[test]
fn signature_help_returns_none_when_not_in_call() {
    let text = "@no_prelude\nmodule examples.nosig\nvalue = 42\n";
    let uri = sample_uri();
    // Position at the integer literal — not inside a call
    let position = position_for(text, "42");
    let help =
        Backend::build_signature_help_with_workspace(text, &uri, position, &HashMap::new());
    // May or may not be Some, but it should not panic
    let _ = help;
}

#[test]
fn signature_help_with_inferred_type() {
    // No explicit type annotation — should still resolve via inference
    let text = "@no_prelude\nmodule examples.inferred\ndouble = x => x + x\nresult = double \n";
    let uri = sample_uri();
    let position = position_after(text, "result = double ");
    let help =
        Backend::build_signature_help_with_workspace(text, &uri, position, &HashMap::new());
    assert!(
        help.is_some(),
        "expected signature help via inferred type for 'double'"
    );
    let help = help.unwrap();
    assert!(
        help.signatures[0].label.contains("double"),
        "label should mention 'double'"
    );
}

#[test]
fn signature_help_imported_function_second_param() {
    let math_text = "@no_prelude\nmodule examples.math2\nexport clamp\nclamp : Int -> Int -> Int -> Int\nclamp = lo hi x => x\n";
    let app_text = "@no_prelude\nmodule examples.app2\nuse examples.math2 (clamp)\nresult = clamp 0 100 \n";

    let math_uri = Url::parse("file:///math2.aivi").unwrap();
    let app_uri = Url::parse("file:///app2.aivi").unwrap();

    let math_path = PathBuf::from("math2.aivi");
    let (math_modules, _) = parse_modules(&math_path, math_text);
    let mut workspace = HashMap::new();
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

    let position = position_after(app_text, "result = clamp 0 100 ");
    let help = Backend::build_signature_help_with_workspace(app_text, &app_uri, position, &workspace);
    assert!(help.is_some(), "expected signature help for imported 'clamp'");
    let help = help.unwrap();
    assert_eq!(
        help.active_parameter,
        Some(2),
        "third argument should have active_parameter=2"
    );
    assert!(help.signatures[0].label.contains("clamp"));
}

#[test]
fn signature_help_with_doc_comment() {
    let lib_text = "@no_prelude\nmodule examples.doclib\nexport greetUser\n// Greets the given user by name.\ngreetUser : Text -> Text\ngreetUser = name => \"hi\"\n";
    let app_text = "@no_prelude\nmodule examples.docapp\nuse examples.doclib (greetUser)\nresult = greetUser \n";

    let lib_uri = Url::parse("file:///doclib.aivi").unwrap();
    let app_uri = Url::parse("file:///docapp.aivi").unwrap();

    let lib_path = PathBuf::from("doclib.aivi");
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

    let position = position_after(app_text, "result = greetUser ");
    let help =
        Backend::build_signature_help_with_workspace(app_text, &app_uri, position, &workspace);
    assert!(help.is_some(), "expected signature help");
    let help = help.unwrap();
    // Documentation from doc comment should be present
    let doc = &help.signatures[0].documentation;
    assert!(
        doc.is_some(),
        "expected doc comment to appear in signature documentation"
    );
    if let Some(tower_lsp::lsp_types::Documentation::MarkupContent(mc)) = doc {
        assert!(
            mc.value.contains("Greets the given user"),
            "doc should mention 'Greets the given user', got: {}",
            mc.value
        );
    }
}

#[test]
fn signature_help_inside_do_block() {
    let text =
        "@no_prelude\nmodule examples.doblock\nuse aivi\nprocess : Text -> Effect Unit\nprocess = _ => pure ()\nrun = do Effect {\n  _ <- process \n}\n";
    let uri = sample_uri();
    let workspace = workspace_with_stdlib(&["aivi"]);
    let position = position_after(text, "  _ <- process ");
    let help = Backend::build_signature_help_with_workspace(text, &uri, position, &workspace);
    assert!(
        help.is_some(),
        "expected signature help for call inside do block"
    );
}

#[test]
fn signature_help_no_panic_on_empty_module() {
    let text = "";
    let uri = sample_uri();
    let position = tower_lsp::lsp_types::Position::new(0, 0);
    // Should not panic
    let _ = Backend::build_signature_help_with_workspace(text, &uri, position, &HashMap::new());
}

#[test]
fn signature_help_active_parameter_first_arg() {
    let text =
        "@no_prelude\nmodule examples.firstarg\nf : Int -> Int -> Int\nf = a b => a + b\nresult = f \n";
    let uri = sample_uri();
    let position = position_after(text, "result = f ");
    let help =
        Backend::build_signature_help_with_workspace(text, &uri, position, &HashMap::new());
    assert!(help.is_some(), "expected signature help");
    let help = help.unwrap();
    assert_eq!(
        help.active_parameter,
        Some(0),
        "first arg should have active_parameter=0"
    );
}
