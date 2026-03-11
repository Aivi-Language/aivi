use aivi::lex_cst;

#[test]
fn lex_string_containing_arrow_emits_no_errors() {
    let (tokens, diags) = lex_cst("\"=>\"");
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:#?}");
    assert!(
        tokens.iter().any(|t| t.text == "\"=>\""),
        "expected string token text to be preserved, got: {tokens:#?}"
    );
}

#[test]
fn lex_unterminated_string_reports_error() {
    let (_tokens, diags) = lex_cst("\"=>");
    assert!(
        !diags.is_empty(),
        "expected diagnostic for unterminated string literal"
    );
}
