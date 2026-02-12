use aivi::lexer::{filter_tokens, lex};

#[test]
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
