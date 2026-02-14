#[test]
fn build_diagnostics_reports_error() {
    let text = "module broken = {";
    let uri = sample_uri();
    let diagnostics = Backend::build_diagnostics(text, &uri);
    assert!(!diagnostics.is_empty());
    assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
    assert!(diagnostics[0]
        .source
        .as_deref()
        .is_some_and(|s| s.starts_with("aivi.")));
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
fn strict_mode_reports_split_arrow_and_offers_fix() {
    let text = "module demo\n\nid = x = > x\n";
    let uri = sample_uri();
    let strict = crate::strict::StrictConfig {
        level: crate::strict::StrictLevel::LexicalStructural,
        forbid_implicit_coercions: false,
        warnings_as_errors: false,
    };
    let diagnostics = Backend::build_diagnostics_strict(text, &uri, &strict);
    let split_arrow = diagnostics.iter().find(|diag| {
        matches!(diag.code.as_ref(), Some(NumberOrString::String(code)) if code == "AIVI-S014")
    });
    assert!(split_arrow.is_some(), "expected split-arrow diagnostic");

    let actions = Backend::build_code_actions(text, &uri, &diagnostics);
    assert!(actions.iter().any(|action| match action {
        CodeActionOrCommand::CodeAction(action) => action.title.contains("Replace with \"=>\""),
        _ => false,
    }));
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
