use serde_json::Value;
use tower_lsp::lsp_types::{CompletionResponse, TextDocumentItem};

#[test]
fn hover_handler_serializes_to_json() {
    let text = sample_text();
    let uri = sample_uri();
    let item = TextDocumentItem {
        uri: uri.clone(),
        language_id: "aivi".to_string(),
        version: 1,
        text: text.to_string(),
    };

    let position = position_for(&item.text, "add 1 2");
    let doc_index = DocIndex::default();
    let hover = Backend::build_hover(&item.text, &item.uri, position, &doc_index)
        .expect("hover");

    let json = serde_json::to_value(&hover).expect("hover json");
    assert!(json.get("contents").is_some());

    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected markup hover");
    };
    assert!(markup.value.contains("`add`"));
}

#[test]
fn definition_handler_serializes_to_json() {
    let text = sample_text();
    let uri = sample_uri();
    let item = TextDocumentItem {
        uri: uri.clone(),
        language_id: "aivi".to_string(),
        version: 1,
        text: text.to_string(),
    };

    let position = position_for(&item.text, "add 1 2");
    let location = Backend::build_definition(&item.text, &item.uri, position)
        .expect("definition");

    let json = serde_json::to_value(&location).expect("definition json");
    assert_eq!(json.get("uri").and_then(Value::as_str), Some(uri.as_str()));
    assert!(json.get("range").is_some());
}

#[test]
fn completion_handler_serializes_to_json() {
    let text = "module examples.app\nrun = add ";
    let uri = sample_uri();
    let item = TextDocumentItem {
        uri: uri.clone(),
        language_id: "aivi".to_string(),
        version: 1,
        text: text.to_string(),
    };

    let position = position_after(&item.text, "add ");
    let items = Backend::build_completion_items(&item.text, &item.uri, position, &HashMap::new());
    let response = CompletionResponse::Array(items);

    let json = serde_json::to_value(&response).expect("completion json");
    let array = json.as_array().expect("completion array");
    assert!(array.iter().any(|item| {
        item.get("label")
            .and_then(Value::as_str)
            .is_some_and(|label| label == "add")
    }));
}

#[test]
fn diagnostics_handler_serializes_to_json() {
    let text = "module demo = {";
    let uri = Url::parse("file:///diag.aivi").expect("uri");
    let item = TextDocumentItem {
        uri: uri.clone(),
        language_id: "aivi".to_string(),
        version: 1,
        text: text.to_string(),
    };

    let diagnostics = Backend::build_diagnostics(&item.text, &item.uri);
    assert!(!diagnostics.is_empty());

    let json = serde_json::to_value(&diagnostics).expect("diagnostics json");
    let array = json.as_array().expect("diagnostics array");
    assert!(array.iter().any(|diag| diag.get("severity").is_some()));
}
