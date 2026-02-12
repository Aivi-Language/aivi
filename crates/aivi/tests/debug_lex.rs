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
