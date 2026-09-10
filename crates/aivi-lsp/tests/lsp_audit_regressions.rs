use aivi_lsp::{documents::open_document, state::ServerState};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tower_lsp::lsp_types::*;

fn position(uri: &Url, line: u32, character: u32) -> TextDocumentPositionParams {
    TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
        position: Position { line, character },
    }
}
fn memory(text: &str) -> (Arc<ServerState>, Url) {
    let state = Arc::new(ServerState::new());
    let uri = Url::parse("file:///lsp-audit/main.aivi").unwrap();
    open_document(&state, &uri, 7, text.to_owned());
    (state, uri)
}
fn rename(state: &Arc<ServerState>, uri: &Url, name: &str) -> Option<WorkspaceEdit> {
    aivi_lsp::rename::rename(
        RenameParams {
            text_document_position: position(uri, 0, 8),
            new_name: name.to_owned(),
            work_done_progress_params: Default::default(),
        },
        Arc::clone(state),
    )
}

#[test]
fn rename_rejects_invalid_identifiers_and_capture_and_versions_valid_edits() {
    let (state, uri) = memory("value answer = 42\nvalue other = answer\n");
    for name in ["bad name", "value", "other", "", "42"] {
        assert!(rename(&state, &uri, name).is_none(), "accepted {name:?}");
    }
    let edit = rename(&state, &uri, "renamed").expect("fresh identifier");
    let Some(DocumentChanges::Edits(edits)) = edit.document_changes else {
        panic!("versioned edits required")
    };
    assert_eq!(edits[0].text_document.version, Some(7));
    assert_eq!(edits[0].edits.len(), 2);
}

#[test]
fn hover_uses_reference_range_and_main_is_not_unused() {
    let (state, uri) = memory("value answer = 42\nvalue main = answer\n");
    let hover = aivi_lsp::hover::hover(
        HoverParams {
            text_document_position_params: position(&uri, 1, 15),
            work_done_progress_params: Default::default(),
        },
        Arc::clone(&state),
    )
    .unwrap();
    assert_eq!(hover.range.unwrap().start.line, 1);
    let hir = aivi_query::hir_module(&state.db, state.file(&uri).unwrap());
    assert!(aivi_lsp::unused::collect_unused_diagnostics(hir.module(), hir.source()).is_empty());
}

#[test]
fn unused_action_removes_whole_multiline_function_and_signature() {
    let text = "type Int -> Int\nfunc unused = input =>\n    input + 1\n\nvalue main = 42\n";
    let (state, uri) = memory(text);
    let actions = aivi_lsp::code_actions::code_actions(
        CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range {
                start: Position::new(1, 5),
                end: Position::new(1, 11),
            },
            context: CodeActionContext {
                diagnostics: vec![],
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
        Arc::clone(&state),
    )
    .unwrap();
    let action = actions
        .into_iter()
        .find_map(|action| match action {
            CodeActionOrCommand::CodeAction(action) if action.title == "Remove unused symbol" => {
                Some(action)
            }
            _ => None,
        })
        .unwrap();
    let edit = &action.edit.unwrap().changes.unwrap()[&uri][0];
    let file = state.file(&uri).unwrap().source(&state.db);
    let start = file
        .lsp_position_to_offset(aivi_base::LspPosition {
            line: edit.range.start.line,
            character: edit.range.start.character,
        })
        .unwrap()
        .as_usize();
    let end = file
        .lsp_position_to_offset(aivi_base::LspPosition {
            line: edit.range.end.line,
            character: edit.range.end.character,
        })
        .unwrap()
        .as_usize();
    let result = format!("{}{}{}", &text[..start], edit.new_text, &text[end..]);
    assert!(!result.contains("input"));
    assert!(!result.contains("type Int"));
    assert!(result.contains("value main"));
    let mut db = aivi_base::SourceDatabase::new();
    let id = db.add_file("edited.aivi", result);
    assert!(!aivi_syntax::parse_module(&db[id]).has_errors());
}

fn completions(state: &Arc<ServerState>, uri: &Url, line: u32, character: u32) -> Vec<String> {
    let response = aivi_lsp::completion::completion(
        CompletionParams {
            text_document_position: position(uri, line, character),
            context: None,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
        Arc::clone(state),
    );
    match response {
        Some(CompletionResponse::Array(items)) => {
            items.into_iter().map(|item| item.label).collect()
        }
        _ => vec![],
    }
}

#[test]
fn completion_respects_local_parameters_and_does_not_leak_other_files() {
    let (state, uri) = memory("type Int -> Int\nfunc identity = input =>\n    input\n");
    let other = Url::parse("file:///lsp-audit/other.aivi").unwrap();
    open_document(&state, &other, 1, "value privateName = 1\n".to_owned());
    let labels = completions(&state, &uri, 2, 6);
    assert!(labels.contains(&"input".to_owned()));
    assert!(!labels.contains(&"privateName".to_owned()));
}

#[test]
fn completion_offers_only_known_record_fields_after_dot() {
    let (state, uri) = memory("value user = { name: \"Ada\", age: 36 }\nvalue main = user.name\n");
    let labels = completions(&state, &uri, 1, 18);
    assert!(labels.contains(&"name".to_owned()), "{labels:?}");
    assert!(labels.contains(&"age".to_owned()));
    assert!(!labels.contains(&"main".to_owned()));
}

struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "aivi-lsp-audit-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("aivi.toml"), "").unwrap();
        Self(path)
    }
    fn write(&self, path: &str, text: &str) -> Url {
        let path = self.0.join(path);
        std::fs::write(&path, text).unwrap();
        Url::from_file_path(path).unwrap()
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn rename_includes_unopened_caller_and_disk_refresh_preserves_buffers() {
    let project = Project::new();
    let uri = project.write("shared.aivi", "value answer = 42\nexport answer\n");
    let caller = project.write("main.aivi", "use shared (answer)\nvalue main = answer\n");
    let state = Arc::new(ServerState::new());
    state.set_workspace_roots(vec![project.0.clone()]);
    open_document(
        &state,
        &uri,
        3,
        "value answer = 43\nexport answer\n".to_owned(),
    );
    state.refresh_workspace_files();
    assert_eq!(
        state.file(&uri).unwrap().text(&state.db),
        "value answer = 43\nexport answer\n"
    );
    let edit = rename(&state, &uri, "renamed").unwrap();
    let Some(DocumentChanges::Edits(edits)) = edit.document_changes else {
        panic!()
    };
    assert!(
        edits
            .iter()
            .any(|edit| edit.text_document.uri == caller && edit.text_document.version.is_none())
    );
    project.write("main.aivi", "value main = 99\n");
    state.refresh_workspace_files();
    let file = state
        .project_files()
        .into_iter()
        .find(|(uri, _)| *uri == caller)
        .unwrap()
        .1;
    assert_eq!(file.text(&state.db), "value main = 99\n");
    std::fs::remove_file(project.0.join("main.aivi")).unwrap();
    state.refresh_workspace_files();
    assert!(!state.project_files().iter().any(|(uri, _)| *uri == caller));
}

#[test]
fn shipped_snippets_parse_after_default_expansion() {
    let snippets: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tooling/packages/vscode-aivi/snippets/aivi.json"
    ))
    .unwrap();
    for (name, snippet) in snippets.as_object().unwrap() {
        let mut text = snippet["body"]
            .as_array()
            .unwrap()
            .iter()
            .map(|line| line.as_str().unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        while let Some(start) = text.find("${") {
            let end = start + text[start..].find('}').unwrap();
            let contents = &text[start + 2..end];
            let replacement = contents
                .split_once(':')
                .map(|(_, default)| default)
                .or_else(|| {
                    contents.split_once('|').map(|(_, choices)| {
                        choices.split(',').next().unwrap().trim_end_matches('|')
                    })
                })
                .unwrap_or("")
                .to_owned();
            text.replace_range(start..=end, &replacement);
        }
        if matches!(name.as_str(), "Applicative cluster" | "Case split pipe") {
            text = format!("value demo = subject\n{text}");
        }
        if name.ends_with("markup") {
            text = format!("value demo =\n{text}");
        }
        let mut db = aivi_base::SourceDatabase::new();
        let id = db.add_file("snippet.aivi", text.clone());
        let parsed = aivi_syntax::parse_module(&db[id]);
        assert!(
            !parsed.has_errors(),
            "{name}: {text}\n{:?}",
            parsed.all_diagnostics().collect::<Vec<_>>()
        );
    }
}

#[test]
fn semantic_tokens_classify_resolved_function_references() {
    let (state, uri) =
        memory("type Int -> Int\nfunc identity = input => input\nvalue main = identity 3\n");
    let result = aivi_lsp::semantic_tokens::semantic_tokens_full(
        SemanticTokensParams {
            text_document: TextDocumentIdentifier { uri },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
        state,
    )
    .unwrap();
    let SemanticTokensResult::Tokens(tokens) = result else {
        panic!()
    };
    let mut line = 0;
    let mut column = 0;
    let mut found = false;
    for token in tokens.data {
        line += token.delta_line;
        column = if token.delta_line == 0 {
            column + token.delta_start
        } else {
            token.delta_start
        };
        if line == 2 && column == 13 {
            assert_eq!(
                aivi_lsp::semantic_tokens::TOKEN_TYPES[token.token_type as usize],
                SemanticTokenType::FUNCTION
            );
            found = true;
        }
    }
    assert!(
        found,
        "call-site identifier must have a semantic function token"
    );
}

#[test]
fn pattern_completion_stays_inside_its_case_arm() {
    let (state, uri) = memory(
        "type Option Int -> Int\nfunc pick = choice => choice\n ||> Some bound -> bound\n ||> None -> 0\n",
    );
    assert!(completions(&state, &uri, 2, 21).contains(&"bound".to_owned()));
    assert!(!completions(&state, &uri, 3, 13).contains(&"bound".to_owned()));
}

#[tokio::test]
async fn disk_changes_and_closing_buffers_republish_importer_diagnostics() {
    use futures::StreamExt;
    use tower::{Service, ServiceExt};
    use tower_lsp::{LspService, jsonrpc::Request};
    let project = Project::new();
    let shared = project.write("shared.aivi", "value answer = 42\nexport answer\n");
    let main_text = "use shared (answer)\nvalue main = answer\n";
    let main = project.write("main.aivi", main_text);
    let (mut service, mut messages) = LspService::new(aivi_lsp::server::Backend::new);
    service.ready().await.unwrap().call(Request::build("initialize").params(serde_json::json!({
        "capabilities": {}, "workspaceFolders": [{"uri": Url::from_file_path(&project.0).unwrap(), "name":"audit"}]
    })).id(1).finish()).await.unwrap();
    service
        .ready()
        .await
        .unwrap()
        .call(
            Request::build("textDocument/didOpen")
                .params(serde_json::json!({
                    "textDocument": {"uri":main, "languageId":"aivi", "version":1, "text":main_text}
                }))
                .finish(),
        )
        .await
        .unwrap();
    let initial = tokio::time::timeout(std::time::Duration::from_secs(10), messages.next())
        .await
        .unwrap()
        .unwrap();
    let initial: PublishDiagnosticsParams =
        serde_json::from_value(initial.params().unwrap().clone()).unwrap();
    assert_eq!(initial.uri, main);
    assert!(
        !initial
            .diagnostics
            .iter()
            .any(|d| d.severity == Some(DiagnosticSeverity::ERROR))
    );

    for (method, params, expect_error) in [
        (
            "workspace/didChangeWatchedFiles",
            {
                project.write("shared.aivi", "value changed = 42\nexport changed\n");
                serde_json::json!({"changes":[{"uri":shared,"type":2}]})
            },
            true,
        ),
        (
            "textDocument/didOpen",
            serde_json::json!({"textDocument":{"uri":shared,"languageId":"aivi","version":1,"text":"value answer = 99\nexport answer\n"}}),
            false,
        ),
        (
            "textDocument/didClose",
            serde_json::json!({"textDocument":{"uri":shared}}),
            true,
        ),
    ] {
        service
            .ready()
            .await
            .unwrap()
            .call(Request::build(method).params(params).finish())
            .await
            .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let notification = messages.next().await.unwrap();
                if notification.method() != "textDocument/publishDiagnostics" {
                    continue;
                }
                let published: PublishDiagnosticsParams =
                    serde_json::from_value(notification.params().unwrap().clone()).unwrap();
                if published.uri == main {
                    assert_eq!(published.version, Some(1));
                    assert_eq!(
                        published
                            .diagnostics
                            .iter()
                            .any(|d| d.severity == Some(DiagnosticSeverity::ERROR)),
                        expect_error,
                        "{method}: {:?}",
                        published.diagnostics
                    );
                    break;
                }
            }
        })
        .await
        .unwrap();
    }
}
