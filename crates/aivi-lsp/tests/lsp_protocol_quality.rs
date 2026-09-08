use std::{path::PathBuf, sync::Arc};

use aivi_lsp::{
    document_highlights::document_highlights,
    documents::{change_document, close_document, open_document},
    folding_ranges::folding_ranges,
    semantic_tokens::{semantic_tokens_full, semantic_tokens_full_delta, semantic_tokens_range},
    signature_help::signature_help,
    state::ServerState,
};
use tower_lsp::lsp_types::{
    DocumentHighlightParams, FoldingRangeKind, FoldingRangeParams, PartialResultParams, Position,
    Range, SemanticToken, SemanticTokensDeltaParams, SemanticTokensFullDeltaResult,
    SemanticTokensParams, SemanticTokensRangeParams, SemanticTokensRangeResult,
    SemanticTokensResult, SignatureHelpParams, TextDocumentContentChangeEvent,
    TextDocumentIdentifier, TextDocumentPositionParams, Url, WorkDoneProgressParams,
};

fn uri(name: &str) -> Url {
    Url::from_file_path(PathBuf::from("/protocol-quality").join(name))
        .expect("test URI should be valid")
}

fn open(name: &str, text: &str) -> (Arc<ServerState>, Url) {
    let state = Arc::new(ServerState::new());
    let uri = uri(name);
    open_document(&state, &uri, 1, text.to_owned());
    (state, uri)
}

fn position_of_nth(text: &str, needle: &str, occurrence: usize) -> Position {
    let mut search_start = 0;
    for index in 0..=occurrence {
        let relative = text[search_start..]
            .find(needle)
            .unwrap_or_else(|| panic!("missing occurrence {occurrence} of `{needle}`"));
        let byte = search_start + relative;
        if index == occurrence {
            let prefix = &text[..byte];
            let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
            let line_start = prefix.rfind('\n').map_or(0, |newline| newline + 1);
            return Position {
                line,
                character: text[line_start..byte].encode_utf16().count() as u32,
            };
        }
        search_start = byte + needle.len();
    }
    unreachable!("occurrence loop always returns")
}

#[test]
fn signature_help_resolves_the_callable_and_active_argument() {
    let text = "type Int -> Int -> Int\nfunc add = left right =>\n    left + right\n\nvalue total = add 1 2\n";
    let (state, uri) = open("signature.aivi", text);
    let help = signature_help(
        SignatureHelpParams {
            context: None,
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: position_of_nth(text, "2", 0),
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        },
        Arc::clone(&state),
    )
    .expect("signature help should resolve a direct function application");

    assert_eq!(help.active_signature, Some(0));
    assert_eq!(help.active_parameter, Some(1));
    assert!(help.signatures[0].label.starts_with("add("));
    let parameters = help.signatures[0]
        .parameters
        .as_ref()
        .expect("function parameters should be described");
    assert_eq!(parameters.len(), 2);

    assert!(
        signature_help(
            SignatureHelpParams {
                context: None,
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri },
                    position: Position {
                        line: 99,
                        character: 0,
                    },
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
            },
            state,
        )
        .is_none()
    );
}

#[test]
fn document_highlights_return_only_same_document_semantic_occurrences() {
    let text = "type Int -> Int\nfunc id = value =>\n    value\n";
    let (state, uri) = open("highlights.aivi", text);
    let highlights = document_highlights(
        DocumentHighlightParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: position_of_nth(text, "value", 0),
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        },
        Arc::clone(&state),
    )
    .expect("declaration and use should be highlighted");

    assert_eq!(highlights.len(), 2);
    assert_eq!(highlights[0].range.start, position_of_nth(text, "value", 0));
    assert_eq!(highlights[1].range.start, position_of_nth(text, "value", 1));

    assert!(
        document_highlights(
            DocumentHighlightParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri },
                    position: Position {
                        line: 99,
                        character: 0,
                    },
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: PartialResultParams::default(),
            },
            state,
        )
        .is_none()
    );
}

#[test]
fn folding_ranges_cover_multiline_symbols_and_comments() {
    let text = "/* first\n   second */\ntype Int -> Int\nfunc id = value =>\n    value\n";
    let (state, uri) = open("folding.aivi", text);
    let ranges = folding_ranges(
        FoldingRangeParams {
            text_document: TextDocumentIdentifier { uri },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        },
        state,
    )
    .expect("multiline constructs should produce folding ranges");

    assert!(
        ranges
            .iter()
            .any(|range| range.kind == Some(FoldingRangeKind::Comment))
    );
    assert!(
        ranges
            .iter()
            .any(|range| range.start_line == 3 && range.end_line == 4)
    );
}

#[test]
fn semantic_token_range_and_delta_follow_result_ids_and_edits() {
    let text = "value first = 1\nvalue second = \"two\"\n";
    let (state, uri) = open("semantic.aivi", text);
    let full_params = SemanticTokensParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };
    let SemanticTokensResult::Tokens(full) =
        semantic_tokens_full(full_params.clone(), Arc::clone(&state))
            .expect("full tokens should be returned")
    else {
        panic!("full request should not return a partial result");
    };
    let first_result_id = full
        .result_id
        .clone()
        .expect("full tokens must carry a delta base result id");

    let unchanged = semantic_tokens_full_delta(
        SemanticTokensDeltaParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            previous_result_id: first_result_id.clone(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        },
        Arc::clone(&state),
    )
    .expect("unchanged delta should be returned");
    let SemanticTokensFullDeltaResult::TokensDelta(unchanged) = unchanged else {
        panic!("known result id should produce a delta");
    };
    assert!(unchanged.edits.is_empty());

    change_document(
        &state,
        &uri,
        2,
        &[TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "value first = 1\nvalue second = 2\n".to_owned(),
        }],
    )
    .expect("full replacement should succeed");
    let changed = semantic_tokens_full_delta(
        SemanticTokensDeltaParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            previous_result_id: first_result_id.clone(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        },
        Arc::clone(&state),
    )
    .expect("changed delta should be returned");
    let SemanticTokensFullDeltaResult::TokensDelta(changed) = changed else {
        panic!("retained result id should produce a changed delta");
    };
    assert_eq!(changed.edits.len(), 1);
    assert_eq!(changed.edits[0].start % 5, 0);
    assert_eq!(changed.edits[0].delete_count % 5, 0);

    let unknown = semantic_tokens_full_delta(
        SemanticTokensDeltaParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            previous_result_id: "unknown-result".to_owned(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        },
        Arc::clone(&state),
    )
    .expect("unknown result id should fall back to full tokens");
    assert!(matches!(unknown, SemanticTokensFullDeltaResult::Tokens(_)));

    let range = semantic_tokens_range(
        SemanticTokensRangeParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range {
                start: Position {
                    line: 1,
                    character: 0,
                },
                end: Position {
                    line: 1,
                    character: 16,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        },
        Arc::clone(&state),
    )
    .expect("valid range should return tokens");
    let SemanticTokensRangeResult::Tokens(range) = range else {
        panic!("range request should return complete tokens");
    };
    assert!(!range.data.is_empty());
    assert!(decoded_lines(&range.data).iter().all(|line| *line == 1));

    assert!(
        semantic_tokens_range(
            SemanticTokensRangeParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                range: Range {
                    start: Position {
                        line: 99,
                        character: 0,
                    },
                    end: Position {
                        line: 99,
                        character: 1,
                    },
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: PartialResultParams::default(),
            },
            Arc::clone(&state),
        )
        .is_none()
    );

    close_document(&state, &uri).expect("document should close");
    open_document(
        &state,
        &uri,
        3,
        "value first = 1\nvalue second = 2\n".to_owned(),
    );
    let after_reopen = semantic_tokens_full_delta(
        SemanticTokensDeltaParams {
            text_document: TextDocumentIdentifier { uri },
            previous_result_id: first_result_id,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        },
        state,
    )
    .expect("reopened document should return tokens");
    assert!(
        matches!(after_reopen, SemanticTokensFullDeltaResult::Tokens(_)),
        "closing a document must discard its previous semantic-token result IDs",
    );
}

fn decoded_lines(tokens: &[SemanticToken]) -> Vec<u32> {
    let mut line = 0;
    tokens
        .iter()
        .map(|token| {
            line += token.delta_line;
            line
        })
        .collect()
}
