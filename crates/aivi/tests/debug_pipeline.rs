use aivi::lexer::{filter_tokens, lex};

#[test]
fn filtered_pipeline_retains_map_arrow() {
    let content = "base = ~map{\n  \"a\" => 1\n}";
    let (cst_tokens, lex_diags) = lex(content);
    assert!(
        lex_diags.is_empty(),
        "unexpected diagnostics: {lex_diags:#?}"
    );

    let tokens = filter_tokens(&cst_tokens);
    assert!(
        tokens.iter().any(|t| t.text == "=>"),
        "expected filtered tokens to include =>, got: {tokens:#?}"
    );
}
