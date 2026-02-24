use aivi::lex_cst;

#[test]
fn debug_tokenization() {
    let content = "\"=>\"";
    let (tokens, diags) = lex_cst(content);
    println!("CONTENT: {}", content);
    for token in tokens {
        println!(
            "TOKEN: kind={}, text={:?}, span={:?}",
            token.kind, token.text, token.span
        );
    }
    for diag in diags {
        println!("DIAG: {} - {}", diag.code, diag.message);
    }

    let content2 = "=>";
    let (tokens2, diags2) = lex_cst(content2);
    println!("\nCONTENT: {}", content2);
    for token in tokens2 {
        println!(
            "TOKEN: kind={}, text={:?}, span={:?}",
            token.kind, token.text, token.span
        );
    }
    for diag in diags2 {
        println!("DIAG: {} - {}", diag.code, diag.message);
    }
}

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
