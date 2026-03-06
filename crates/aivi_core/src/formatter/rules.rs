{
    let indent_size = options.indent_size.clamp(1, 16);
    let max_blank_lines = options.max_blank_lines.min(10);
    let (tokens, _) = lex(content);

    // Pre-scan: bail out early when brackets are unbalanced.
    // An unclosed opener inflates the delimiter stack and causes downstream lines
    // to receive wrong indentation / context, which manifests as "scrambled"
    // variables and phantom whitespace far from the actual mismatch site.
    {
        let mut bracket_stack: Vec<char> = Vec::new();
        let mut balanced = true;
        for token in &tokens {
            if matches!(token.kind.as_str(), "comment" | "string" | "whitespace" | "sigil") {
                continue;
            }
            if let Some(open) = is_open_sym(&token.text) {
                bracket_stack.push(open);
            } else if let Some(close) = is_close_sym(&token.text) {
                match bracket_stack.pop() {
                    Some(open) if matches_pair(open, close) => {}
                    _ => {
                        balanced = false;
                        break;
                    }
                }
            }
        }
        if !balanced || !bracket_stack.is_empty() {
            return content.to_string();
        }
    }

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
