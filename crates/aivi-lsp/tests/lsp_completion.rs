use aivi_lsp::{documents::open_document, state::ServerState};
use std::sync::Arc;
use tower_lsp::lsp_types::*;

fn fields_after(text: &str, selector: &str) -> Vec<CompletionItem> {
    let state = Arc::new(ServerState::new());
    let uri = Url::parse("file:///transparent-alias-completion/main.aivi").unwrap();
    open_document(&state, &uri, 1, text.to_owned());
    let offset = text.find(selector).unwrap() + selector.len();
    let prefix = &text[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let character = prefix.rsplit('\n').next().unwrap().encode_utf16().count() as u32;
    let response = aivi_lsp::completion::completion(
        CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            context: None,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
        state,
    );
    let Some(CompletionResponse::Array(items)) = response else {
        panic!("expected field completion items");
    };
    items
}

fn assert_entry_fields(items: &[CompletionItem]) {
    assert_eq!(items.len(), 2, "{items:?}");
    for (label, detail) in [("key", "Text"), ("payload", "Int")] {
        let item = items
            .iter()
            .find(|item| item.label == label)
            .expect("entry field");
        assert_eq!(item.kind, Some(CompletionItemKind::FIELD));
        assert_eq!(item.detail.as_deref(), Some(detail));
    }
}

#[test]
fn completion_expands_generic_record_alias_parameters_and_fixed_arguments() {
    let text = "type Entry K A = { key: K, payload: A }\ntype Entry Text Int -> Int\nfunc read = entry => entry.payload\n";
    assert_entry_fields(&fields_after(text, "entry."));
}

#[test]
fn completion_expands_aliases_at_each_nested_record_projection() {
    let text = "type Entry K A = { key: K, payload: A }\ntype Envelope A = { inner: Entry Text A }\ntype Envelope Int -> Int\nfunc read = envelope => envelope.inner.payload\n";
    assert_entry_fields(&fields_after(text, "envelope.inner."));
}
