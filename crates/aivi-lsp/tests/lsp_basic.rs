use std::path::PathBuf;

use aivi_lsp::{
    diagnostics::collect_lsp_diagnostics,
    documents::{change_document, close_document, open_document},
    state::ServerState,
};
use tower_lsp::lsp_types::{
    DiagnosticSeverity, NumberOrString, TextDocumentContentChangeEvent, Url,
};

fn test_uri(name: &str) -> Url {
    Url::from_file_path(PathBuf::from("/test-documents").join(name))
        .expect("test URI should be valid")
}

#[test]
fn open_change_close_document_lifecycle() {
    let state = ServerState::new();
    let uri = test_uri("lifecycle.aivi");

    open_document(&state, &uri, 1, "value answer = 42\n".to_owned());
    assert!(
        state.contains_document(&uri),
        "document should be tracked after open"
    );

    change_document(
        &state,
        &uri,
        2,
        &[TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "value answer = 43\n".to_owned(),
        }],
    )
    .expect("newer full change should apply");
    assert!(
        state.contains_document(&uri),
        "document should still be tracked after change"
    );

    close_document(&state, &uri);
    assert!(
        !state.contains_document(&uri),
        "document should not be tracked after close"
    );
}

#[test]
fn valid_document_has_no_error_diagnostics() {
    let state = ServerState::new();
    let uri = test_uri("valid.aivi");
    open_document(&state, &uri, 1, "value answer = 42\n".to_owned());
    let file = state.file(&uri).expect("file should be open");

    let diagnostics = collect_lsp_diagnostics(&state.db, file, &uri);
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        .collect();

    assert!(
        errors.is_empty(),
        "a valid document should produce no error diagnostics; got: {errors:#?}"
    );
}

#[test]
fn invalid_document_has_error_diagnostics() {
    let state = ServerState::new();
    let uri = test_uri("invalid.aivi");
    // "val = 42" is not valid AIVI syntax; a valid declaration is "value val = 42"
    open_document(&state, &uri, 1, "val = 42\n".to_owned());
    let file = state.file(&uri).expect("file should be open");

    let diagnostics = collect_lsp_diagnostics(&state.db, file, &uri);
    assert_eq!(
        diagnostics,
        collect_lsp_diagnostics(&state.db, file, &uri),
        "unchanged inputs should produce byte-for-byte stable diagnostic ordering"
    );
    assert!(
        !diagnostics.is_empty(),
        "an invalid document should produce at least one diagnostic"
    );
}

#[test]
fn duplicate_declarations_keep_typed_codes_and_related_locations() {
    let state = ServerState::new();
    let uri = test_uri("duplicate.aivi");
    open_document(
        &state,
        &uri,
        1,
        "value title = \"first\"\nvalue title = \"second\"\n".to_owned(),
    );
    let file = state.file(&uri).expect("file should be open");

    let diagnostics = collect_lsp_diagnostics(&state.db, file, &uri);
    let duplicate = diagnostics
        .iter()
        .find(|diagnostic| {
            matches!(
                diagnostic.code.as_ref(),
                Some(NumberOrString::String(code)) if code.ends_with("duplicate-term-name")
            )
        })
        .expect("duplicate declaration should keep its stable diagnostic code");
    let related = duplicate
        .related_information
        .as_ref()
        .and_then(|information| information.first())
        .expect("duplicate diagnostic should point to the first declaration");

    assert_eq!(duplicate.range.start.line, 1);
    assert_eq!(related.location.uri, uri);
    assert_eq!(related.location.range.start.line, 0);
}
