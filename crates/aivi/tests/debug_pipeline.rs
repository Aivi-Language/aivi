use aivi::lexer::{filter_tokens, lex};

#[test]
#[ignore = "debug-only: prints pipeline output, no assertions"]
fn debug_full_pipeline() {
    let content = "base = ~map{\n    \"a\" => 1\n  }";
    let (cst_tokens, _lex_diags) = lex(content);
    println!("CST TOKENS:");
    for t in &cst_tokens {
        println!("  kind={}, text={:?}, span={:?}", t.kind, t.text, t.span);
    }

    let tokens = filter_tokens(&cst_tokens);
    println!("\nFILTERED TOKENS:");
    for t in &tokens {
        println!("  kind={:?}, text={:?}, span={:?}", t.kind, t.text, t.span);
    }
}

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
