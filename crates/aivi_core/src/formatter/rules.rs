{
    let indent_size = options.indent_size.clamp(1, 16);
    let max_blank_lines = options.max_blank_lines.min(10);
    let (tokens, _) = lex(content);

    let mut raw_lines: Vec<&str> = content.split('\n').collect();
    let mut tokens_by_line: Vec<Vec<&crate::cst::CstToken>> = vec![Vec::new(); raw_lines.len()];
    for token in &tokens {
        if token.kind == "whitespace" {
            continue;
        }
        let start_line = token.span.start.line;
        if start_line == 0 {
            continue;
        }
        if let Some(bucket) = tokens_by_line.get_mut(start_line - 1) {
            bucket.push(token);
        }
    }


    include!("rules/indentation.rs")
}
