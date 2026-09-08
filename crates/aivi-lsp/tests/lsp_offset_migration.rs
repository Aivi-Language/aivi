use std::{path::PathBuf, sync::Arc};

use aivi_lsp::{
    completion::completion, definition::definition, documents::open_document, hover::hover,
    state::ServerState,
};
use tower_lsp::lsp_types::{
    CompletionParams, CompletionResponse, GotoDefinitionParams, HoverParams, Position,
    TextDocumentIdentifier, TextDocumentPositionParams, Url,
};

fn test_uri() -> Url {
    Url::from_file_path(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/lsp-offset.aivi"))
        .expect("test file path should convert to a file URL")
}

fn test_state() -> (Arc<ServerState>, Url) {
    let state = Arc::new(ServerState::new());
    let uri = test_uri();
    open_document(&state, &uri, 1, "value answer = 42\n".to_owned());
    (state, uri)
}

fn completion_params(uri: Url, position: Position) -> CompletionParams {
    CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: None,
    }
}

fn hover_params(uri: Url, position: Position) -> HoverParams {
    HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position,
        },
        work_done_progress_params: Default::default(),
    }
}

fn definition_params(uri: Url, position: Position) -> GotoDefinitionParams {
    GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    }
}

#[test]
fn completion_still_works_for_valid_positions() {
    let (state, uri) = test_state();

    let response = completion(
        completion_params(
            uri,
            Position {
                line: 0,
                character: 7,
            },
        ),
        state,
    );

    assert!(matches!(response, Some(CompletionResponse::Array(_))));
}

#[test]
fn hover_still_works_for_valid_positions() {
    let (state, uri) = test_state();

    let response = hover(
        hover_params(
            uri,
            Position {
                line: 0,
                character: 7,
            },
        ),
        state,
    );

    assert!(response.is_some());
}

#[test]
fn definition_still_works_for_valid_positions() {
    let (state, uri) = test_state();

    let response = definition(
        definition_params(
            uri.clone(),
            Position {
                line: 0,
                character: 7,
            },
        ),
        state,
    );

    let location = response.expect("definition should resolve for a valid symbol position");
    let tower_lsp::lsp_types::GotoDefinitionResponse::Scalar(location) = location else {
        panic!("definition should resolve to a single location");
    };
    assert_eq!(location.uri, uri);
}

#[test]
fn completion_returns_none_for_out_of_range_columns() {
    let (state, uri) = test_state();

    let response = completion(
        completion_params(
            uri,
            Position {
                line: 0,
                character: 99,
            },
        ),
        state,
    );

    assert!(response.is_none());
}

#[test]
fn hover_returns_none_for_out_of_range_columns() {
    let (state, uri) = test_state();

    let response = hover(
        hover_params(
            uri,
            Position {
                line: 0,
                character: 99,
            },
        ),
        state,
    );

    assert!(response.is_none());
}

#[test]
fn definition_returns_none_for_out_of_range_columns() {
    let (state, uri) = test_state();

    let response = definition(
        definition_params(
            uri,
            Position {
                line: 0,
                character: 99,
            },
        ),
        state,
    );

    assert!(response.is_none());
}
