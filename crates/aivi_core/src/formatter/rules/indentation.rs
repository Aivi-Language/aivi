{
    // Pre-pass: split trailing closers (`}`/`]`/`)`) off a line onto their own
    // line when the line also contains other tokens and the matching opener lives
    // on a different source line.  This prevents the common pattern:
    //
    //   rgb = {
    //     r: 255, g: 0, b: 0 }      ← `}` should be on its own line
    //
    // from causing downstream indentation drift.
    {
        let mut split_raw_lines: Vec<&str> = Vec::with_capacity(raw_lines.len() + 16);
        let mut split_tokens_by_line: Vec<Vec<&crate::cst::CstToken>> =
            Vec::with_capacity(tokens_by_line.len() + 16);

        for (line_index, raw) in raw_lines.iter().enumerate() {
            let line_tokens = tokens_by_line[line_index].clone();
            let has_comment = line_tokens.iter().any(|t| t.kind == "comment");
            if has_comment || line_tokens.len() < 2 {
                split_raw_lines.push(*raw);
                split_tokens_by_line.push(line_tokens);
                continue;
            }
            // Check if the last code token is a closer.
            let last_close = last_code_token_is(&line_tokens, &["}", "]", ")"]);
            if !last_close {
                split_raw_lines.push(*raw);
                split_tokens_by_line.push(line_tokens);
                continue;
            }
            // Check that there are other code tokens besides the closer.
            let first_code = first_code_index(&line_tokens);
            let last_code_idx = line_tokens
                .iter()
                .rposition(|t| t.kind != "comment" && t.text != "\n" && t.text != ";");
            if first_code == last_code_idx {
                // Only token on the line is the closer — nothing to split.
                split_raw_lines.push(*raw);
                split_tokens_by_line.push(line_tokens);
                continue;
            }
            // If the first code token is also a closer, the line contains only
            // closers (e.g. `})` or `})]`).  Keep them together.
            if let Some(fc) = first_code {
                if is_close_sym(line_tokens[fc].text.as_str()).is_some() {
                    split_raw_lines.push(*raw);
                    split_tokens_by_line.push(line_tokens);
                    continue;
                }
            }
            // Find the matching open delimiter for the trailing closer.
            // Walk backwards through all tokens up to this point to find the
            // opener.  If the opener is on the same source line, keep the line
            // intact (e.g. `{ r: 255, g: 0, b: 0 }` all on one source line).
            let closer_tok = line_tokens[last_code_idx.expect("infallible")];
            let _opener_char = match closer_tok.text.as_str() {
                "}" => "{",
                "]" => "[",
                ")" => "(",
                _ => {
                    split_raw_lines.push(*raw);
                    split_tokens_by_line.push(line_tokens);
                    continue;
                }
            };
            // Find matching opener by scanning backward through all token buckets.
            // On the current line, exclude the closer token itself from the scan so we
            // don't count it in the nesting depth.
            let mut depth = 0isize;
            let mut opener_line: Option<usize> = None;
            'outer: for scan_line in (0..=line_index).rev() {
                let scan_tokens = &tokens_by_line[scan_line];
                let end = if scan_line == line_index {
                    last_code_idx.expect("infallible")
                } else {
                    scan_tokens.len()
                };
                for t in scan_tokens[..end].iter().rev() {
                    if t.kind == "comment" || t.kind == "string" {
                        continue;
                    }
                    if is_close_sym(t.text.as_str()).is_some() {
                        depth += 1;
                    } else if is_open_sym(t.text.as_str()).is_some() {
                        if depth == 0 {
                            opener_line = Some(scan_line);
                            break 'outer;
                        }
                        depth -= 1;
                    }
                }
            }
            let same_line = opener_line == Some(line_index);
            if same_line || opener_line.is_none() {
                // Don't split if opener is on the same line, or if there is no
                // matching opener (unmatched closer — splitting would be
                // non-idempotent because the closer wasn't originally alone).
                split_raw_lines.push(*raw);
                split_tokens_by_line.push(line_tokens);
                continue;
            }
            // Split: everything except the closer goes on one line, the closer
            // goes on a new line.
            let mut before = line_tokens.clone();
            let closer = before.pop().expect("closer token");
            split_raw_lines.push(*raw);
            split_tokens_by_line.push(before);
            split_raw_lines.push("");
            split_tokens_by_line.push(vec![closer]);
        }

        raw_lines = split_raw_lines;
        tokens_by_line = split_tokens_by_line;
    }

    // Pre-pass: merge "hanging" openers (`{`/`[`) that appear alone on the next line after
    // `=` / `=>` / `<-` / `->` back onto the previous line, then drop the opener-only line.
    //
    // This is intentionally conservative (no comments on the opener line) to avoid surprising
    // rewrites while still fixing the common formatter artifact in `integration-tests/complex`.
    {
        // Allman brace style: split trailing `{` onto its own line (best-effort).
        if matches!(options.brace_style, BraceStyle::Allman) {
            let mut split_raw_lines: Vec<&str> = Vec::with_capacity(raw_lines.len() + 16);
            let mut split_tokens_by_line: Vec<Vec<&crate::cst::CstToken>> =
                Vec::with_capacity(tokens_by_line.len() + 16);

            for (line_index, raw) in raw_lines.iter().enumerate() {
                let mut line_tokens = tokens_by_line[line_index].clone();
                let has_comment = line_tokens.iter().any(|t| t.kind == "comment");
                let last_is_open = last_code_token_is(&line_tokens, &["{"]);
                if !has_comment && last_is_open && line_tokens.len() >= 2 {
                    // Move the last `{` token to a new line.
                    let brace = line_tokens.pop().expect("brace token");
                    split_raw_lines.push(*raw);
                    split_tokens_by_line.push(line_tokens);
                    split_raw_lines.push("");
                    split_tokens_by_line.push(vec![brace]);
                    continue;
                }
                split_raw_lines.push(*raw);
                split_tokens_by_line.push(line_tokens);
            }

            raw_lines = split_raw_lines;
            tokens_by_line = split_tokens_by_line;
        }

        let mut merged_raw_lines: Vec<&str> = Vec::with_capacity(raw_lines.len());
        let mut merged_tokens_by_line: Vec<Vec<&crate::cst::CstToken>> =
            Vec::with_capacity(tokens_by_line.len());

        for (line_index, raw) in raw_lines.iter().enumerate() {
            let line_tokens = tokens_by_line[line_index].clone();
            let opener_tok = if line_tokens.iter().any(|t| t.kind == "comment") {
                None
            } else if let Some(first_idx) = first_code_index(&line_tokens) {
                if line_tokens.len() == 1
                    && matches!(line_tokens[first_idx].text.as_str(), "{" | "[")
                {
                    Some(line_tokens[first_idx])
                } else {
                    None
                }
            } else {
                None
            };

            if matches!(options.brace_style, BraceStyle::Kr) {
                if let Some(opener_tok) = opener_tok {
                    if let Some(prev_tokens) = merged_tokens_by_line.last_mut() {
                        let merge_for_flow = last_code_token_is(
                            prev_tokens,
                            &["=>", "<-", "->", "then", "else", "?", "match"],
                        );
                        // `class` and `instance` bodies require `{` on the same line as `=`.
                        let merge_for_class_instance = last_code_token_is(prev_tokens, &["="])
                            && first_code_index(prev_tokens).is_some_and(|i| {
                                matches!(
                                    prev_tokens[i].text.as_str(),
                                    "class" | "instance"
                                )
                            });
                        if merge_for_flow || merge_for_class_instance {
                            prev_tokens.push(opener_tok);
                            continue;
                        }
                    }
                }
            }

            merged_raw_lines.push(*raw);
            merged_tokens_by_line.push(line_tokens);
        }

        raw_lines = merged_raw_lines;
        tokens_by_line = merged_tokens_by_line;
    }

    // Pre-pass: merge "hanging" match subjects onto the `=>` line:
    //
    //   name = args =>
    //     subject match
    //       | ...
    //
    // becomes
    //
    //   name = args => subject match
    //     | ...
    //
    // This is intentionally conservative (no comments on either merged line).
    {
        fn starts_with(tokens: &[&crate::cst::CstToken], text: &str) -> bool {
            first_code_index(tokens).is_some_and(|i| tokens[i].text == text)
        }

        let mut merged_raw_lines: Vec<&str> = Vec::with_capacity(raw_lines.len());
        let mut merged_tokens_by_line: Vec<Vec<&crate::cst::CstToken>> =
            Vec::with_capacity(tokens_by_line.len());

        let mut i = 0usize;
        while i < raw_lines.len() {
            let tokens = tokens_by_line[i].clone();
            if tokens.is_empty() {
                merged_raw_lines.push(raw_lines[i]);
                merged_tokens_by_line.push(tokens);
                i += 1;
                continue;
            }

            let can_merge = !tokens.iter().any(|t| t.kind == "comment")
                && last_code_token_is(&tokens, &["=>"]);
            if can_merge && i + 2 < raw_lines.len() {
                let next_tokens = tokens_by_line[i + 1].clone();
                let after_tokens = tokens_by_line[i + 2].clone();
                if !next_tokens.is_empty()
                    && !after_tokens.is_empty()
                    && !next_tokens.iter().any(|t| t.kind == "comment")
                    && last_code_token_is(&next_tokens, &["?", "match"])
                    && !starts_with(&next_tokens, "|")
                    && starts_with(&after_tokens, "|")
                {
                    let mut combined = tokens.clone();
                    combined.extend(next_tokens);
                    merged_raw_lines.push(raw_lines[i]);
                    merged_tokens_by_line.push(combined);
                    i += 2;
                    continue;
                }
            }

            merged_raw_lines.push(raw_lines[i]);
            merged_tokens_by_line.push(tokens);
            i += 1;
        }

        raw_lines = merged_raw_lines;
        tokens_by_line = merged_tokens_by_line;
    }

    // First pass: compute context per line and indentation level.
    let mut stack: Vec<OpenFrame> = Vec::new();
    let mut degraded = false;
    let mut prev_non_comment_text: Option<String> = None;
    let mut prevprev_non_comment_text: Option<String> = None;
    let mut machine_pending = false;

    let mut lines: Vec<LineState<'_>> = Vec::with_capacity(raw_lines.len());

    for line_index in 0..raw_lines.len() {
        let mut line_tokens = tokens_by_line[line_index].clone();
        // Sort by original (line, column) to stay correct even after we merge tokens across lines.
        line_tokens.sort_by_key(|t| (t.span.start.line, t.span.start.column, t.span.end.column));

        let (input_indent, _) = leading_indent(raw_lines[line_index]);

        let mut indent_level = stack
            .iter()
            .filter(|f| matches!(f.sym, '{' | '[' | '('))
            .count();
        if !degraded {
            if let Some(first_idx) = first_code_index(&line_tokens) {
                if is_close_sym(line_tokens[first_idx].text.as_str()).is_some() {
                    indent_level = indent_level.saturating_sub(1);
                }
            }
        }

        let indent = if degraded {
            input_indent
        } else {
            " ".repeat(indent_level * indent_size)
        };
        let indent_len = indent.chars().count();
        let top_context = stack.last().map(|f| f.kind);

        lines.push(LineState {
            tokens: line_tokens,
            indent,
            indent_len,
            top_delim: stack.last().map(|f| f.sym),
            top_context,
            effect_align_lhs: None,
            arm_align_pat: None,
            map_align_key: None,
            machine_align: None,
            degraded,
        });

        if degraded {
            continue;
        }

        // Use the sorted line tokens so delimiter tracking stays stable even after we merge tokens
        // across lines in pre-passes.
        for t in lines
            .last()
            .expect("just pushed current line")
            .tokens
            .iter()
        {
            if t.kind == "comment" {
                continue;
            }
            let text = t.text.as_str();
            if text == "machine" {
                machine_pending = true;
            }
            if let Some(open) = is_open_sym(text) {
                let kind = match (
                    open,
                    prev_non_comment_text.as_deref(),
                    prevprev_non_comment_text.as_deref(),
                ) {
                    ('{', Some(monad), Some("do")) if !is_keyword(monad) => ContextKind::Effect,
                    ('{', Some("effect"), _) => ContextKind::Effect,
                    ('{', Some("generate"), _) => ContextKind::Generate,
                    ('{', Some("resource"), _) => ContextKind::Resource,
                    ('{', Some("map"), Some("~")) => ContextKind::MapSigil,
                    ('[', Some("set"), Some("~")) => ContextKind::SetSigil,
                    ('[', Some("mat"), Some("~")) => ContextKind::MatSigil,
                    _ => {
                        if machine_pending && open == '{' {
                            machine_pending = false;
                            ContextKind::Machine
                        } else {
                            ContextKind::Other
                        }
                    }
                };
                stack.push(OpenFrame { sym: open, kind });
            } else if let Some(close) = is_close_sym(text) {
                let Some(frame) = stack.pop() else {
                    degraded = true;
                    break;
                };
                if !matches_pair(frame.sym, close) {
                    degraded = true;
                    break;
                }
            }

            prevprev_non_comment_text = prev_non_comment_text;
            prev_non_comment_text = Some(text.to_string());
        }
    }

    // Second pass: mark alignment groups.
    let mut i = 0usize;
    while i < lines.len() {
        if lines[i].tokens.is_empty() || lines[i].degraded {
            i += 1;
            continue;
        }

        let first = first_code_index(&lines[i].tokens);
        if let Some(first_idx) = first {
            if lines[i].top_context == Some(ContextKind::Effect) {
                // Effect bind alignment groups: consecutive `<-` lines, unbroken.
                if find_top_level_token(&lines[i].tokens, "<-", first_idx).is_some() {
                    let mut j = i;
                    let mut max_lhs = 0usize;
                    while j < lines.len() {
                        if lines[j].tokens.is_empty() || lines[j].degraded {
                            break;
                        }
                        if lines[j].top_context != Some(ContextKind::Effect) {
                            break;
                        }
                        let first_idx_j = match first_code_index(&lines[j].tokens) {
                            Some(v) => v,
                            None => break,
                        };
                        let Some(arrow_idx) =
                            find_top_level_token(&lines[j].tokens, "<-", first_idx_j)
                        else {
                            break;
                        };
                        let lhs_tokens = &lines[j].tokens[first_idx_j..arrow_idx];
                        let lhs_str =
                            format_tokens_simple(lhs_tokens, lines[j].top_delim).trim().to_string();
                        max_lhs = max_lhs.max(lhs_str.len());
                        j += 1;
                    }
                    if j - i >= 2 {
                        for line in lines.iter_mut().take(j).skip(i) {
                            line.effect_align_lhs = Some(max_lhs);
                        }
                    }
                    i = j;
                    continue;
                }
            }

            // Pattern match arm alignment groups.
            let is_arm = lines[i].tokens[first_idx].text == "|"
                && find_top_level_token(&lines[i].tokens, "=>", first_idx + 1).is_some();
            if is_arm {
                let this_indent = lines[i].indent_len;
                let mut j = i;
                let mut max_pat = 0usize;
                while j < lines.len() {
                    if lines[j].tokens.is_empty()
                        || lines[j].degraded
                        || lines[j].indent_len != this_indent
                    {
                        break;
                    }
                    let Some(first_idx_j) = first_code_index(&lines[j].tokens) else {
                        break;
                    };
                    if lines[j].tokens[first_idx_j].text != "|" {
                        break;
                    }
                    let Some(arrow_idx) =
                        find_top_level_token(&lines[j].tokens, "=>", first_idx_j + 1)
                    else {
                        break;
                    };
                    let pat_tokens = &lines[j].tokens[first_idx_j + 1..arrow_idx];
                    let pat_str =
                        format_tokens_simple(pat_tokens, lines[j].top_delim).trim().to_string();
                    max_pat = max_pat.max(pat_str.len());
                    j += 1;
                }
                if j - i >= 2 {
                    for line in lines.iter_mut().take(j).skip(i) {
                        line.arm_align_pat = Some(max_pat);
                    }
                }
                i = if j == i { i + 1 } else { j };
                continue;
            }

            // Structured map literal entry alignment groups (inside `~map{ ... }`).
            if lines[i].top_context == Some(ContextKind::MapSigil) {
                let Some(_) = find_top_level_token(&lines[i].tokens, "=>", first_idx) else {
                    i += 1;
                    continue;
                };
                let this_indent = lines[i].indent_len;
                let mut j = i;
                let mut max_key = 0usize;
                while j < lines.len() {
                    if lines[j].tokens.is_empty()
                        || lines[j].degraded
                        || lines[j].indent_len != this_indent
                        || lines[j].top_context != Some(ContextKind::MapSigil)
                    {
                        break;
                    }
                    let Some(first_idx_j) = first_code_index(&lines[j].tokens) else {
                        break;
                    };
                    let Some(arrow_idx_j) =
                        find_top_level_token(&lines[j].tokens, "=>", first_idx_j)
                    else {
                        break;
                    };
                    let key_tokens = &lines[j].tokens[first_idx_j..arrow_idx_j];
                    let key_str =
                        format_tokens_simple(key_tokens, lines[j].top_delim).trim().to_string();
                    max_key = max_key.max(key_str.len());
                    j += 1;
                }
                if j - i >= 2 {
                    for line in lines.iter_mut().take(j).skip(i) {
                        line.map_align_key = Some(max_key);
                    }
                }
                i = j;
                continue;
            }

            // Machine transition alignment groups (inside `machine ... = { ... }`).
            if lines[i].top_context == Some(ContextKind::Machine)
                && find_top_level_token(&lines[i].tokens, "->", first_idx).is_some() {
                    let mut j = i;
                    let mut max_source = 0usize;
                    let mut max_target = 0usize;
                    let mut max_event = 0usize;
                    while j < lines.len() {
                        if lines[j].tokens.is_empty() || lines[j].degraded {
                            break;
                        }
                        if lines[j].top_context != Some(ContextKind::Machine) {
                            break;
                        }
                        let first_idx_j = match first_code_index(&lines[j].tokens) {
                            Some(v) => v,
                            None => break,
                        };
                        let Some(arrow_idx) =
                            find_top_level_token(&lines[j].tokens, "->", first_idx_j)
                        else {
                            break;
                        };
                        // source = tokens before `->` (may be empty for initial transitions)
                        let source_tokens = &lines[j].tokens[first_idx_j..arrow_idx];
                        let source_str =
                            format_tokens_simple(source_tokens, lines[j].top_delim).trim().to_string();
                        max_source = max_source.max(source_str.len());
                        // target = tokens between `->` and first top-level `:`
                        let colon_idx = find_top_level_token(&lines[j].tokens, ":", arrow_idx + 1);
                        let target_end = colon_idx.unwrap_or(lines[j].tokens.len());
                        let target_tokens = &lines[j].tokens[arrow_idx + 1..target_end];
                        let target_str =
                            format_tokens_simple(target_tokens, lines[j].top_delim).trim().to_string();
                        max_target = max_target.max(target_str.len());
                        // event = tokens between `:` and first top-level `{` (or end)
                        if let Some(colon_idx) = colon_idx {
                            let brace_idx = find_top_level_token(&lines[j].tokens, "{", colon_idx + 1);
                            let event_end = brace_idx.unwrap_or(lines[j].tokens.len());
                            let event_tokens = &lines[j].tokens[colon_idx + 1..event_end];
                            let event_str =
                                format_tokens_simple(event_tokens, lines[j].top_delim).trim().to_string();
                            max_event = max_event.max(event_str.len());
                        }
                        j += 1;
                    }
                    if j > i {
                        for line in lines.iter_mut().take(j).skip(i) {
                            line.machine_align = Some((max_source, max_target, max_event));
                        }
                    }
                    i = j;
                    continue;
                }
        }

        i += 1;
    }

    // Third pass: render.
    //
    // NOTE: The lexer/parser is not indentation-sensitive per spec, but the current compiler
    // implementation uses newlines + indentation to disambiguate some constructs. To keep the
    // formatter deterministic and robust even when the input indentation is inconsistent, we
    // compute indentation from delimiter nesting (`{[(` / `}])`) plus a small set of newline
    // continuations (`|` arms, `then`/`else`, trailing `=`/`=>`).

    // Lines that are interior to a multi-line token (e.g. sigil body) have no tokens of
    // their own and must be skipped — not treated as blank lines.
    let covered_by_multiline: Vec<bool> = {
        let mut covered = vec![false; lines.len()];
        for (i, state) in lines.iter().enumerate() {
            for t in &state.tokens {
                let span_lines = t.span.end.line.saturating_sub(t.span.start.line);
                if span_lines > 0 {
                    for offset in 1..=span_lines {
                        if i + offset < covered.len() {
                            covered[i + offset] = true;
                        }
                    }
                }
            }
        }
        covered
    };

    let mut rendered_lines: Vec<String> = Vec::new();
    let mut blank_run = 0usize;
    let mut pipe_block_stack: Vec<(usize, isize)> = Vec::new();
    let mut pipeop_block_base_indent: Option<usize> = None;
    let mut pipeop_block_base_depth: Option<isize> = None;
    let mut rhs_next_line_indent: Option<usize> = None;
    let mut rhs_next_line_depth: Option<isize> = None;
    let mut rhs_block_base_indent: Option<usize> = None;
    let mut rhs_block_base_depth: Option<isize> = None;
    let mut rhs_decorator_pending: bool = false;
    let mut arm_rhs_active = false;
    let mut pipeop_seed_indent: Option<usize> = None;
    let mut prev_non_blank_last_token: Option<String> = None;
    // Delimiter groups opened at end-of-line (`{`/`(`/`[`) that should cause a hanging indent
    // until the matching close delimiter starts a line. We also keep the opener line's effective
    // indentation to align the corresponding closer and contents.
    let mut hang_delim_stack: Vec<(char, usize)> = Vec::new();
    let mut open_depth: isize = 0;
    let mut prev_effective_indent_len: usize = 0;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum IfPhase {
        Then,
        Else,
    }

    #[derive(Debug, Clone, Copy)]
    struct IfFrame {
        if_indent: usize,
        phase: IfPhase,
        active_indent: bool,
    }

    // Tracks multiline `if ... then ... else ...` indentation so nested `if`s format correctly.
    let mut if_stack: Vec<IfFrame> = Vec::new();
    // When a line ends with `if <cond>` (no `then` yet), record its effective indentation so the
    // following `then` / `else` lines can be indented one level deeper.
    let mut pending_then_indent: Option<usize> = None;

    fn seeds_rhs_continuation(last: Option<&str>) -> bool {
        matches!(last, Some("=" | "=>" | "<-" | "->"))
    }

    fn last_continuation_token(tokens: &[&crate::cst::CstToken]) -> Option<String> {
        tokens
            .iter()
            .rev()
            .find(|t| {
                if t.kind == "comment" {
                    return false;
                }
                !matches!(t.text.as_str(), "{" | "[" | "(")
            })
            .map(|t| t.text.clone())
    }

    fn last_code_token(tokens: &[&crate::cst::CstToken]) -> Option<String> {
        tokens
            .iter()
            .rev()
            .find(|t| t.kind != "comment")
            .map(|t| t.text.clone())
    }

    fn matches_hang_close(opener: char, first_token_text: &str) -> bool {
        matches!(
            (opener, first_token_text),
            ('{', "}") | ('[', "]") | ('(', ")")
        )
    }

    fn net_open_depth(tokens: &[&crate::cst::CstToken]) -> isize {
        let mut depth = 0isize;
        for t in tokens {
            if matches!(t.kind.as_str(), "comment" | "string") {
                continue;
            }
            let text = t.text.as_str();
            if is_open_sym(text).is_some() {
                depth += 1;
            } else if is_close_sym(text).is_some() {
                depth -= 1;
            }
        }
        depth.max(0)
    }

    fn update_open_depth(open_depth: &mut isize, tokens: &[&crate::cst::CstToken]) {
        for t in tokens {
            if matches!(t.kind.as_str(), "comment" | "string") {
                continue;
            }
            let text = t.text.as_str();
            if is_open_sym(text).is_some() {
                *open_depth += 1;
            } else if is_close_sym(text).is_some() {
                *open_depth -= 1;
            }
        }
        if *open_depth < 0 {
            *open_depth = 0;
        }
    }

    fn looks_like_new_stmt(tokens: &[&crate::cst::CstToken], first_idx: usize) -> bool {
        let first = tokens[first_idx].text.as_str();
        if matches!(
            first,
            "module" | "use" | "export" | "type" | "class" | "instance" | "domain"
        ) {
            return true;
        }
        if tokens[first_idx].kind == "ident" {
            // A definition or type signature at the same indentation likely terminates a `|` block.
            if find_top_level_token(tokens, "=", first_idx + 1).is_some()
                || find_top_level_token(tokens, ":", first_idx + 1).is_some()
            {
                return true;
            }
        }
        false
    }

    fn find_top_level_token_clamped(
        tokens: &[&crate::cst::CstToken],
        needle: &str,
        start: usize,
    ) -> Option<usize> {
        let mut depth = 0isize;
        for (i, t) in tokens.iter().enumerate().skip(start) {
            let text = t.text.as_str();
            if t.kind == "string" || t.kind == "comment" {
                continue;
            }
            if is_open_sym(text).is_some() {
                depth += 1;
                continue;
            }
            if is_close_sym(text).is_some() {
                depth = (depth - 1).max(0);
                continue;
            }
            if depth == 0 && text == needle {
                return Some(i);
            }
        }
        None
    }

    // Precompute per-line `use` metadata so the blank-line handler in the render loop
    // can look up neighbours in O(1) instead of doing an O(n) scan per blank line.
    let line_is_use: Vec<bool> = lines
        .iter()
        .map(|line| first_code_index(&line.tokens).is_some_and(|i| line.tokens[i].text == "use"))
        .collect();
    // First path segment of each `use` line (e.g. "aivi" or "mailfox").  Used to group
    // consecutive `use` declarations and suppress blank lines within the same group.
    let line_use_group: Vec<Option<String>> = lines
        .iter()
        .map(|line| {
            let fi = first_code_index(&line.tokens)?;
            if line.tokens[fi].text != "use" {
                return None;
            }
            line.tokens[fi + 1..]
                .iter()
                .find(|t| t.kind != "whitespace" && t.kind != "comment")
                .map(|t| t.text.to_string())
        })
        .collect();

    for (line_index, state) in lines.iter().enumerate() {
        // Lines covered by a multi-line token (e.g. interior of a sigil) have no tokens
        // of their own — skip them entirely so they aren't treated as blank lines.
        if covered_by_multiline.get(line_index).copied().unwrap_or(false) && state.tokens.is_empty()
        {
            continue;
        }
        if state.tokens.is_empty() {
            // Suppress blank lines that are sandwiched between two consecutive `use` lines
            // belonging to the same first-segment group (e.g. both `aivi.*`).  Blank lines
            // between different groups are preserved so the post-render pass can keep them.
            let between_uses = {
                let prev_idx = lines[..line_index]
                    .iter()
                    .rposition(|l| !l.tokens.is_empty());
                let next_idx = lines[line_index + 1..]
                    .iter()
                    .position(|l| !l.tokens.is_empty())
                    .map(|i| line_index + 1 + i);
                match (prev_idx, next_idx) {
                    (Some(p), Some(n)) if line_is_use[p] && line_is_use[n] => {
                        line_use_group[p] == line_use_group[n]
                    }
                    _ => false,
                }
            };
            if between_uses {
                continue;
            }

            blank_run += 1;
            if blank_run > max_blank_lines {
                continue;
            }
            rendered_lines.push(String::new());
            // Keep continuation state across blank lines so indentation inside continuation blocks
            // and delimiter groups stays stable when the author uses spacing for readability.
            rhs_next_line_indent = None;
            rhs_next_line_depth = None;
            pipeop_seed_indent = None;
            continue;
        }

        let preceded_by_blank = blank_run > 0;
        blank_run = 0;

        let mut out = String::new();

        // One-shot seeds: only apply to the next non-blank line.
        let rhs_seed_indent = rhs_next_line_indent.take();
        let rhs_seed_depth = rhs_next_line_depth.take().unwrap_or(0);
        let pipeop_seed = pipeop_seed_indent.take();

        if state.degraded {
            out.push_str(state.indent.as_str());
            out.push_str(&format_tokens_with_matrix(
                &state.tokens,
                state.top_delim,
                state.indent.as_str(),
            ));
            rendered_lines.push(out);
            pipe_block_stack.clear();
            pipeop_block_base_indent = None;
            pipeop_block_base_depth = None;
            rhs_next_line_indent = None;
            rhs_next_line_depth = None;
            rhs_block_base_indent = None;
            rhs_block_base_depth = None;
            pipeop_seed_indent = None;
            if_stack.clear();
            rhs_decorator_pending = false;
            prev_non_blank_last_token = last_continuation_token(&state.tokens);
            update_open_depth(&mut open_depth, &state.tokens);
            continue;
        }

        let Some(first_idx) = first_code_index(&state.tokens) else {
            out.push_str(state.indent.as_str());
            out.push_str(&format_tokens_with_matrix(
                &state.tokens,
                state.top_delim,
                state.indent.as_str(),
            ));
            rendered_lines.push(out);
            pipe_block_stack.clear();
            pipeop_block_base_indent = None;
            pipeop_block_base_depth = None;
            rhs_next_line_indent = None;
            rhs_next_line_depth = None;
            rhs_block_base_indent = None;
            rhs_block_base_depth = None;
            pipeop_seed_indent = None;
            // Preserve `if_stack` inside delimiter blocks so that `else` after a block-body
            // `then do { ... // comment ... }` still aligns correctly.
            if hang_delim_stack.is_empty() {
                if_stack.clear();
            }
            rhs_decorator_pending = false;
            prev_non_blank_last_token = last_continuation_token(&state.tokens);
            update_open_depth(&mut open_depth, &state.tokens);
            continue;
        };

        let rhs_decorator_pending_for_this_line = rhs_decorator_pending;
        rhs_decorator_pending = false;

        // Canonical base indentation from delimiter nesting (computed in pass 1).
        // This matches `stack`-based delimiter nesting, and avoids drift when heuristics for
        // continuation blocks are active.
        let line_indent_len = state.indent_len;
        let line_depth = (line_indent_len / indent_size) as isize;
        let pipeop_seed_match = pipeop_seed == Some(line_indent_len);
        let should_pop_hang = hang_delim_stack.last().is_some_and(|&(opener, _)| {
            matches_hang_close(opener, state.tokens[first_idx].text.as_str())
        });

        let line_has_top_level_eq =
            find_top_level_token(&state.tokens, "=", first_idx).is_some();

        if let (Some(base_indent), Some(base_depth)) = (rhs_block_base_indent, rhs_block_base_depth)
        {
            let is_decorator_start = state.tokens[first_idx].text == "@";
            if line_depth <= base_depth
                && line_indent_len <= base_indent
                && (looks_like_new_stmt(&state.tokens, first_idx)
                    || (is_decorator_start && preceded_by_blank))
                && !rhs_decorator_pending_for_this_line
            {
                rhs_block_base_indent = None;
                rhs_block_base_depth = None;
            }
        }
        let is_decorator_line = state.tokens[first_idx].text == "@";
        let is_decorator_only_line = is_decorator_line
            && find_top_level_token(&state.tokens, "=", first_idx).is_none()
            && find_top_level_token(&state.tokens, ":", first_idx).is_none();

        // Continuation blocks:
        // - Multi-line `| ...` blocks (multi-clause functions and `match` expressions).
        //   These blocks can contain continuation lines (e.g. multi-line patterns/bodies), so we
        //   keep the block active until we hit a same-indent non-`|` line (or a blank line).
        // - Multi-line `|> ...` pipeline blocks (common after `=`, even when RHS starts on same line).
        // - A single continuation line after a trailing `=` (e.g. `x =\n  expr`).
        let starts_with_pipe = state.tokens[first_idx].text == "|";
        let starts_with_pipeop = state.tokens[first_idx].text == "|>";
        let is_arm_line =
            starts_with_pipe && find_top_level_token(&state.tokens, "=>", first_idx + 1).is_some();
        let should_start_pipe_block =
            starts_with_pipe && matches!(prev_non_blank_last_token.as_deref(), Some("=") | Some("?") | Some("match"));
        let should_start_pipeop_block = starts_with_pipeop
            && (pipeop_seed_match
                || matches!(prev_non_blank_last_token.as_deref(), Some("=") | Some("?")));

        if should_start_pipe_block {
            // Anchor `|` blocks to the subject line's effective indentation so arms align even
            // when the subject is itself indented by other continuation rules.
            pipe_block_stack.push((prev_effective_indent_len, line_depth));
        }
        if should_start_pipeop_block {
            pipeop_block_base_indent = Some(prev_effective_indent_len);
            pipeop_block_base_depth = Some(line_depth);
        }

        // Close any nested `|` blocks we've left by delimiter nesting.
        while pipe_block_stack
            .last()
            .is_some_and(|&(_, base_depth)| line_depth < base_depth)
        {
            pipe_block_stack.pop();
        }
        if pipe_block_stack.is_empty() {
            arm_rhs_active = false;
        }

        // For `|`/`|>` lines, anchor indentation to the subject line's indent (not just delimiter nesting).
        let mut base_indent_len_for_line = line_indent_len;
        let hang_top = hang_delim_stack.last().copied();
        let hang_is_close = hang_top.is_some_and(|(opener, _)| {
            matches_hang_close(opener, state.tokens[first_idx].text.as_str())
        });
        // Suppress extra continuation indentation inside multi-line "hanging" delimiter groups.
        // The hang stack already aligns contents to the opener's *effective* indentation (which
        // includes any continuation indentation on the opener line), so adding continuation
        // levels again would double-indent.
        let inside_hang = hang_top.is_some_and(|(_, opener_indent)| {
            !hang_is_close && (opener_indent + indent_size) > line_indent_len
        });
        if starts_with_pipe {
            if let Some(&(base, _)) = pipe_block_stack.last() {
                // Indent arms one level relative to the match subject.
                base_indent_len_for_line = base + indent_size;
            }
        } else if starts_with_pipeop {
            if let Some(base) = pipeop_block_base_indent {
                base_indent_len_for_line = base + indent_size;
            }
        }
        if let Some((_, opener_indent)) = hang_top {
            if hang_is_close {
                // Closers align with their opener (not with any other continuation blocks).
                base_indent_len_for_line = opener_indent;
            } else {
                base_indent_len_for_line = base_indent_len_for_line.max(opener_indent + indent_size);
            }
        }
        // End a continuation block when we hit a line that clearly starts a new statement at or
        // above the block's base indentation. Avoid ending blocks just because a line starts with
        // a closing delimiter (`}`/`]`/`)`) which naturally decreases the computed indent.
        if let Some(&(_base_indent, base_depth)) = pipe_block_stack.last() {
            if !starts_with_pipe
                && !starts_with_pipeop
                && open_depth == 0
                && line_depth <= base_depth
                && looks_like_new_stmt(&state.tokens, first_idx)
            {
                pipe_block_stack.pop();
                if pipe_block_stack.is_empty() {
                    arm_rhs_active = false;
                }
            }
        }
        if let (Some(_base_indent), Some(base_depth)) = (pipeop_block_base_indent, pipeop_block_base_depth)
        {
            if !starts_with_pipeop
                && open_depth == 0
                && line_depth <= base_depth
                && looks_like_new_stmt(&state.tokens, first_idx)
            {
                pipeop_block_base_indent = None;
                pipeop_block_base_depth = None;
            }
        }

        let in_pipe_block = !pipe_block_stack.is_empty();
        let in_pipeop_block = pipeop_block_base_indent.is_some();
        let in_rhs_block = rhs_block_base_indent.is_some();

        let mut continuation_levels = 0usize;
        // A decorator on its own line conceptually belongs to the following item; it should align
        // with that item's indentation boundary instead of inheriting indentation from a preceding
        // `|`/`|>` continuation block.
        if !inside_hang
            && (in_pipe_block || in_pipeop_block)
            && !starts_with_pipe
            && !starts_with_pipeop
            && !is_decorator_only_line
        {
            continuation_levels += 1;
        }
        if !inside_hang && in_rhs_block && !starts_with_pipe && !starts_with_pipeop {
            continuation_levels += 1;
        }
        // If a line ended with `=`/`=>` and did not open a delimiter group, indent the next line.
        // Avoid double-indenting `|`/`|>` continuation blocks after `=`/`?`.
        let rhs_seed_active = rhs_seed_indent.is_some()
            && !starts_with_pipe
            && !starts_with_pipeop
            && !in_rhs_block
            && (rhs_seed_depth == 0 || prev_non_blank_last_token.as_deref() == Some("=>"));
        if !inside_hang && rhs_seed_active {
            continuation_levels += 1;
        }
        if !inside_hang && arm_rhs_active && !starts_with_pipe && !is_decorator_only_line {
            continuation_levels += 1;
        }
        if hang_is_close {
            // Standalone closers align with their opener and should not inherit continuation
            // indentation. Exceptions like `} } => ...` inside match arms should keep the arm
            // indentation so `=>` stays aligned.
            let has_arrow = find_top_level_token_clamped(&state.tokens, "=>", first_idx).is_some();
            let has_else = find_top_level_token_clamped(&state.tokens, "else", first_idx).is_some();
            if !has_arrow || has_else {
                continuation_levels = 0;
            }
        }

        // Base indentation including continuation blocks, but excluding multiline `if` handling.
        let effective_indent_len_pre_if =
            base_indent_len_for_line + (continuation_levels * indent_size);

        // If we start a new statement at or above an `if`'s indentation, we left that `if`.
        // This is intentionally conservative to avoid popping while still inside branch bodies.
        if looks_like_new_stmt(&state.tokens, first_idx) {
            while if_stack
                .last()
                .is_some_and(|f| effective_indent_len_pre_if <= f.if_indent)
            {
                if_stack.pop();
            }
        }

        let mut effective_indent_len = effective_indent_len_pre_if;

        // Persistent `if`/`then`/`else` indentation (fixes nested ifs).
        //
        // - Body lines are indented one level relative to their `if` header.
        // - `else` header lines align with their matching `if`.
        // - `} else {` is handled by delimiter/hang indentation; we only update stack state.
        if !hang_is_close {
            let first_text = state.tokens[first_idx].text.as_str();
            let is_else_line = first_text == "else";

            if is_else_line {
                // We're starting an `else` header; any completed inner `else` branches end here.
                while if_stack.last().is_some_and(|f| f.phase == IfPhase::Else) {
                    if_stack.pop();
                }

                if let Some(idx) = if_stack.iter().rposition(|f| f.phase == IfPhase::Then) {
                    let outer_body_indent = if_stack
                        .iter()
                        .take(idx)
                        .filter(|f| f.active_indent)
                        .map(|f| f.if_indent + indent_size)
                        .max()
                        .unwrap_or(0);
                    effective_indent_len = outer_body_indent.max(if_stack[idx].if_indent);
                }
            } else if first_text == "then" {
                // `then` starting a line: indent one level deeper than the `if` it belongs to.
                if let Some(if_indent) = pending_then_indent {
                    effective_indent_len = effective_indent_len.max(if_indent + indent_size);
                }
            } else if let Some(min_indent) = if_stack
                .iter()
                .filter(|f| f.active_indent)
                .map(|f| f.if_indent + indent_size)
                .max()
            {
                effective_indent_len = effective_indent_len.max(min_indent);
            }
        }

        let effective_indent = " ".repeat(effective_indent_len);

        if let Some(max_lhs) = state.effect_align_lhs {
            if let Some(arrow_idx) = find_top_level_token(&state.tokens, "<-", first_idx) {
                // `<-` alignment across consecutive effect lines.
                let lhs_tokens = &state.tokens[first_idx..arrow_idx];
                let rhs_tokens = &state.tokens[arrow_idx + 1..];
                let lhs = format_tokens_simple(lhs_tokens, state.top_delim)
                    .trim()
                    .to_string();
                let rhs = format_tokens_simple(rhs_tokens, state.top_delim)
                    .trim()
                    .to_string();
                let spaces = (max_lhs.saturating_sub(lhs.len())) + 1;
                out.push_str(&effective_indent);
                out.push_str(&lhs);
                out.push_str(&" ".repeat(spaces));
                out.push_str("<-");
                if !rhs.is_empty() {
                    out.push(' ');
                    out.push_str(&rhs);
                }
                rendered_lines.push(out);
                prev_effective_indent_len = effective_indent_len;
                prev_non_blank_last_token = last_continuation_token(&state.tokens);
                if should_pop_hang {
                    hang_delim_stack.pop();
                }
                if let Some(last) = last_code_token(&state.tokens) {
                    if let Some(open) = is_open_sym(&last) {
                        hang_delim_stack.push((open, prev_effective_indent_len));
                    }
                }
                if let Some(else_idx) =
                    find_top_level_token_clamped(&state.tokens, "else", first_idx)
                {
                    if let Some(idx) = if_stack.iter().rposition(|f| f.phase == IfPhase::Then) {
                        let else_inline = state.tokens.iter().skip(else_idx + 1).any(|t| {
                            t.kind != "comment" && t.text != "\n"
                        });
                        if_stack[idx].phase = IfPhase::Else;
                        if_stack[idx].active_indent = !else_inline;
                    }
                }
                if prev_non_blank_last_token.as_deref() == Some("then") {
                    if_stack.push(IfFrame {
                        if_indent: prev_effective_indent_len,
                        phase: IfPhase::Then,
                        active_indent: true,
                    });
                }
                if state.tokens.get(first_idx).map(|t| t.text.as_str()) == Some("then")
                    && prev_non_blank_last_token.as_deref() != Some("then")
                {
                    if_stack.push(IfFrame {
                        if_indent: prev_effective_indent_len,
                        phase: IfPhase::Then,
                        active_indent: false,
                    });
                }
                if find_top_level_token(&state.tokens, "if", first_idx).is_some()
                    && find_top_level_token(&state.tokens, "then", first_idx).is_none()
                {
                    pending_then_indent = Some(prev_effective_indent_len);
                } else {
                    pending_then_indent = None;
                }
                if line_has_top_level_eq {
                    pipeop_seed_indent = Some(line_indent_len);
                }
                if seeds_rhs_continuation(prev_non_blank_last_token.as_deref()) {
                    let depth = net_open_depth(&state.tokens);
                    // If the line already opened a delimiter group (e.g. `=> {`), delimiter-based
                    // indentation handles the continuation; avoid a one-shot RHS indent.
                    if depth == 0 {
                        rhs_next_line_indent = Some(line_indent_len);
                        rhs_next_line_depth = Some(depth);
                        rhs_block_base_indent = Some(line_indent_len);
                        rhs_block_base_depth = Some(line_depth);
                    }
                }
                update_open_depth(&mut open_depth, &state.tokens);
                continue;
            }
        }

        if let Some(max_pat) = state.arm_align_pat {
            let arrow_idx = find_top_level_token(&state.tokens, "=>", first_idx + 1);
            if state.tokens[first_idx].text == "|" {
                if let Some(arrow_idx) = arrow_idx {
                    let pat_tokens = &state.tokens[first_idx + 1..arrow_idx];
                    let rhs_tokens = &state.tokens[arrow_idx + 1..];
                    let pat = format_tokens_simple(pat_tokens, state.top_delim)
                        .trim()
                        .to_string();
                    let rhs = format_tokens_simple(rhs_tokens, state.top_delim)
                        .trim()
                        .to_string();
                    let spaces = (max_pat.saturating_sub(pat.len())) + 1;
                    out.push_str(&effective_indent);
                    out.push_str("| ");
                    out.push_str(&pat);
                    out.push_str(&" ".repeat(spaces));
                    out.push_str("=>");
                    if !rhs.is_empty() {
                        out.push(' ');
                        out.push_str(&rhs);
                    }
                    rendered_lines.push(out);
                    prev_effective_indent_len = effective_indent_len;
                    prev_non_blank_last_token = last_continuation_token(&state.tokens);
                    if should_pop_hang {
                        hang_delim_stack.pop();
                    }
                    if let Some(last) = last_code_token(&state.tokens) {
                        if let Some(open) = is_open_sym(&last) {
                            // Arm bodies (match arm `|` lines) need the block content indented
                            // one extra level relative to the arm itself.  Push the hang opener
                            // at arm_indent + indent_size so the body lands at +2*indent_size
                            // and the closing `}` aligns at arm_indent + indent_size.
                            let hang_indent = if open == '{' {
                                prev_effective_indent_len + indent_size
                            } else {
                                prev_effective_indent_len
                            };
                            hang_delim_stack.push((open, hang_indent));
                        }
                    }
                    if let Some(else_idx) =
                        find_top_level_token_clamped(&state.tokens, "else", first_idx)
                    {
                        if let Some(idx) = if_stack.iter().rposition(|f| f.phase == IfPhase::Then) {
                            let else_inline = state.tokens.iter().skip(else_idx + 1).any(|t| {
                                t.kind != "comment" && t.text != "\n"
                            });
                            if_stack[idx].active_indent = !else_inline;
                        }
                    }
                    if prev_non_blank_last_token.as_deref() == Some("then") {
                        if_stack.push(IfFrame {
                            if_indent: prev_effective_indent_len,
                            phase: IfPhase::Then,
                            active_indent: true,
                        });
                    }
                    if state.tokens.get(first_idx).map(|t| t.text.as_str()) == Some("then")
                        && prev_non_blank_last_token.as_deref() != Some("then")
                    {
                        if_stack.push(IfFrame {
                            if_indent: prev_effective_indent_len,
                            phase: IfPhase::Then,
                            active_indent: false,
                        });
                    }
                    if find_top_level_token(&state.tokens, "if", first_idx).is_some()
                        && find_top_level_token(&state.tokens, "then", first_idx).is_none()
                    {
                        pending_then_indent = Some(prev_effective_indent_len);
                    } else {
                        pending_then_indent = None;
                    }
                    if line_has_top_level_eq {
                        pipeop_seed_indent = Some(line_indent_len);
                    }
                    if seeds_rhs_continuation(prev_non_blank_last_token.as_deref()) {
                        let depth = net_open_depth(&state.tokens);
                        if depth == 0 {
                            rhs_next_line_indent = Some(line_indent_len);
                            rhs_next_line_depth = Some(depth);
                            rhs_block_base_indent = Some(line_indent_len);
                            rhs_block_base_depth = Some(line_depth);
                        }
                    }
                    update_open_depth(&mut open_depth, &state.tokens);
                    continue;
                }
            }
        }

        if let Some(max_key) = state.map_align_key {
                let arrow_idx = find_top_level_token(&state.tokens, "=>", first_idx);
                if let Some(arrow_idx) = arrow_idx {
                    let key_tokens = &state.tokens[first_idx..arrow_idx];
                    let rhs_tokens = &state.tokens[arrow_idx + 1..];
                    let key = format_tokens_simple(key_tokens, state.top_delim)
                        .trim()
                        .to_string();
                    let rhs = format_tokens_simple(rhs_tokens, state.top_delim)
                        .trim()
                        .to_string();
                let spaces = (max_key.saturating_sub(key.len())) + 1;
                out.push_str(&effective_indent);
                out.push_str(&key);
                out.push_str(&" ".repeat(spaces));
                out.push_str("=>");
                if !rhs.is_empty() {
                    out.push(' ');
                    out.push_str(&rhs);
                }
                rendered_lines.push(out);
                prev_effective_indent_len = effective_indent_len;
                prev_non_blank_last_token = last_continuation_token(&state.tokens);
                if should_pop_hang {
                    hang_delim_stack.pop();
                }
                if let Some(last) = last_code_token(&state.tokens) {
                    if let Some(open) = is_open_sym(&last) {
                        hang_delim_stack.push((open, prev_effective_indent_len));
                    }
                }
                if let Some(else_idx) =
                    find_top_level_token_clamped(&state.tokens, "else", first_idx)
                {
                    if let Some(idx) = if_stack.iter().rposition(|f| f.phase == IfPhase::Then) {
                        let else_inline = state.tokens.iter().skip(else_idx + 1).any(|t| {
                            t.kind != "comment" && t.text != "\n"
                        });
                        if_stack[idx].phase = IfPhase::Else;
                        if_stack[idx].active_indent = !else_inline;
                    }
                }
                if prev_non_blank_last_token.as_deref() == Some("then") {
                    if_stack.push(IfFrame {
                        if_indent: prev_effective_indent_len,
                        phase: IfPhase::Then,
                        active_indent: true,
                    });
                }
                if state.tokens.get(first_idx).map(|t| t.text.as_str()) == Some("then")
                    && prev_non_blank_last_token.as_deref() != Some("then")
                {
                    if_stack.push(IfFrame {
                        if_indent: prev_effective_indent_len,
                        phase: IfPhase::Then,
                        active_indent: false,
                    });
                }
                if find_top_level_token(&state.tokens, "if", first_idx).is_some()
                    && find_top_level_token(&state.tokens, "then", first_idx).is_none()
                {
                    pending_then_indent = Some(prev_effective_indent_len);
                } else {
                    pending_then_indent = None;
                }
                if line_has_top_level_eq {
                    pipeop_seed_indent = Some(line_indent_len);
                }
                if seeds_rhs_continuation(prev_non_blank_last_token.as_deref()) {
                    let depth = net_open_depth(&state.tokens);
                    if depth == 0 {
                        rhs_next_line_indent = Some(line_indent_len);
                        rhs_next_line_depth = Some(depth);
                        rhs_block_base_indent = Some(line_indent_len);
                        rhs_block_base_depth = Some(line_depth);
                    }
                }
                update_open_depth(&mut open_depth, &state.tokens);
                continue;
            }
        }

        if let Some((max_source, max_target, max_event)) = state.machine_align {
            if let Some(arrow_idx) = find_top_level_token(&state.tokens, "->", first_idx) {
                // source = tokens before `->`
                let source_tokens = &state.tokens[first_idx..arrow_idx];
                let source = format_tokens_simple(source_tokens, state.top_delim)
                    .trim()
                    .to_string();
                // target = tokens between `->` and first top-level `:`
                let colon_idx = find_top_level_token(&state.tokens, ":", arrow_idx + 1);
                let target_end = colon_idx.unwrap_or(state.tokens.len());
                let target_tokens = &state.tokens[arrow_idx + 1..target_end];
                let target = format_tokens_simple(target_tokens, state.top_delim)
                    .trim()
                    .to_string();
                // event = tokens between `:` and first top-level `{`
                let (event, payload) = if let Some(colon_idx) = colon_idx {
                    let brace_idx =
                        find_top_level_token(&state.tokens, "{", colon_idx + 1);
                    let event_end = brace_idx.unwrap_or(state.tokens.len());
                    let event_tokens = &state.tokens[colon_idx + 1..event_end];
                    let event_str = format_tokens_simple(event_tokens, state.top_delim)
                        .trim()
                        .to_string();
                    let payload_str = if let Some(brace_idx) = brace_idx {
                        let payload_tokens = &state.tokens[brace_idx..];
                        format_tokens_simple(payload_tokens, state.top_delim)
                            .trim()
                            .to_string()
                    } else {
                        String::new()
                    };
                    (event_str, payload_str)
                } else {
                    (String::new(), String::new())
                };

                out.push_str(&effective_indent);
                // Source column (left-aligned, padded to max_source width)
                if !source.is_empty() {
                    out.push_str(&source);
                    out.push_str(&" ".repeat(max_source.saturating_sub(source.len())));
                } else {
                    out.push_str(&" ".repeat(max_source));
                }
                if max_source > 0 {
                    out.push(' ');
                }
                out.push_str("->");
                out.push(' ');
                out.push_str(&target);
                // Pad target to max_target width
                let target_pad = max_target.saturating_sub(target.len());
                out.push_str(&" ".repeat(target_pad));
                if !event.is_empty() || !payload.is_empty() {
                    out.push_str(" : ");
                    out.push_str(&event);
                    if !payload.is_empty() {
                        let event_pad = max_event.saturating_sub(event.len());
                        out.push_str(&" ".repeat(event_pad));
                        out.push(' ');
                        out.push_str(&payload);
                    }
                }

                rendered_lines.push(out);
                prev_effective_indent_len = effective_indent_len;
                prev_non_blank_last_token = last_continuation_token(&state.tokens);
                if should_pop_hang {
                    hang_delim_stack.pop();
                }
                if let Some(last) = last_code_token(&state.tokens) {
                    if let Some(open) = is_open_sym(&last) {
                        hang_delim_stack.push((open, prev_effective_indent_len));
                    }
                }
                pending_then_indent = None;
                update_open_depth(&mut open_depth, &state.tokens);
                continue;
            }
        }

        // Type signatures: `name : Type` (only when followed by a matching `name ... =` definition).
        if let Some(colon_idx) = find_top_level_token(&state.tokens, ":", first_idx) {
            if colon_idx > first_idx {
                let name_tokens = &state.tokens[first_idx..colon_idx];
                let rest_tokens = &state.tokens[colon_idx + 1..];
                let name_len = name_tokens.len();

                // If the current line ends with an open bracket (multi-line type body),
                // skip past the matching closing bracket before looking for the definition.
                let search_from = if last_code_token(&state.tokens)
                    .as_deref()
                    .and_then(is_open_sym)
                    .is_some()
                {
                    let mut depth = 1isize;
                    let mut close_line = line_index;
                    'outer: for (j, line) in lines.iter().enumerate().skip(line_index + 1) {
                        for tok in &line.tokens {
                            if is_open_sym(tok.text.as_str()).is_some() {
                                depth += 1;
                            } else if is_close_sym(tok.text.as_str()).is_some() {
                                depth -= 1;
                                if depth == 0 {
                                    close_line = j;
                                    break 'outer;
                                }
                            }
                        }
                    }
                    close_line + 1
                } else {
                    line_index + 1
                };

                let mut next_line = None;
                for (j, line) in lines.iter().enumerate().skip(search_from) {
                    if line.degraded || line.tokens.is_empty() {
                        continue;
                    }
                    next_line = Some(j);
                    break;
                }

                if let Some(j) = next_line {
                    if let Some(next_first) = first_code_index(&lines[j].tokens) {
                        // Skip 'export' on the definition line only when the signature
                        // itself does NOT start with 'export' (e.g. `user : T` matched
                        // against `export user = ...`).
                        let sig_starts_with_export = name_tokens
                            .first()
                            .map(|t| t.text.as_str())
                            == Some("export");
                        let def_first = if !sig_starts_with_export
                            && lines[j]
                                .tokens
                                .get(next_first)
                                .map(|t| t.text.as_str())
                                == Some("export")
                        {
                            next_first + 1
                        } else {
                            next_first
                        };

                        let mut name_matches = true;
                        for k in 0..name_len {
                            let a = name_tokens.get(k).map(|t| t.text.as_str());
                            let b = lines[j].tokens.get(def_first + k).map(|t| t.text.as_str());
                            if a != b {
                                name_matches = false;
                                break;
                            }
                        }

                        if name_matches
                            && find_top_level_token(&lines[j].tokens, "=", def_first + name_len)
                                .is_some()
                        {
                            out.push_str(&effective_indent);
                            out.push_str(format_tokens_simple(name_tokens, state.top_delim).trim());
                            out.push_str(" : ");
                            out.push_str(format_tokens_simple(rest_tokens, state.top_delim).trim());
                            rendered_lines.push(out);
                            prev_effective_indent_len = effective_indent_len;
                            prev_non_blank_last_token = last_continuation_token(&state.tokens);
                            if should_pop_hang {
                                hang_delim_stack.pop();
                            }
                            if let Some(last) = last_code_token(&state.tokens) {
                                if let Some(open) = is_open_sym(&last) {
                                    hang_delim_stack.push((open, prev_effective_indent_len));
                                }
                            }
                            if let Some(else_idx) =
                                find_top_level_token_clamped(&state.tokens, "else", first_idx)
                            {
                                if let Some(idx) =
                                    if_stack.iter().rposition(|f| f.phase == IfPhase::Then)
                                {
                                    let else_inline = state.tokens.iter().skip(else_idx + 1).any(|t| {
                                        t.kind != "comment" && t.text != "\n"
                                    });
                                    if_stack[idx].phase = IfPhase::Else;
                                    if_stack[idx].active_indent = !else_inline;
                                }
                            }
                            if prev_non_blank_last_token.as_deref() == Some("then") {
                                if_stack.push(IfFrame {
                                    if_indent: prev_effective_indent_len,
                                    phase: IfPhase::Then,
                                    active_indent: true,
                                });
                            }
                            if state.tokens.get(first_idx).map(|t| t.text.as_str()) == Some("then")
                                && prev_non_blank_last_token.as_deref() != Some("then")
                            {
                                if_stack.push(IfFrame {
                                    if_indent: prev_effective_indent_len,
                                    phase: IfPhase::Then,
                                    active_indent: false,
                                });
                            }
                            if find_top_level_token(&state.tokens, "if", first_idx).is_some()
                                && find_top_level_token(&state.tokens, "then", first_idx).is_none()
                            {
                                pending_then_indent = Some(prev_effective_indent_len);
                            } else {
                                pending_then_indent = None;
                            }
                            if line_has_top_level_eq {
                                pipeop_seed_indent = Some(line_indent_len);
                            }
                    if seeds_rhs_continuation(prev_non_blank_last_token.as_deref()) {
                        let depth = net_open_depth(&state.tokens);
                        if depth == 0 {
                            rhs_next_line_indent = Some(line_indent_len);
                            rhs_next_line_depth = Some(depth);
                            rhs_block_base_indent = Some(line_indent_len);
                            rhs_block_base_depth = Some(line_depth);
                        }
                    }
                            update_open_depth(&mut open_depth, &state.tokens);
                            continue;
                        }
                    }
                }
            }
        }

        out.push_str(&effective_indent);
        out.push_str(&format_tokens_with_matrix(
            &state.tokens,
            state.top_delim,
            &effective_indent,
        ));
        rendered_lines.push(out);
        prev_effective_indent_len = effective_indent_len;

        prev_non_blank_last_token = last_continuation_token(&state.tokens);
        if should_pop_hang {
            hang_delim_stack.pop();
        }
        if let Some(last) = last_code_token(&state.tokens) {
            if let Some(open) = is_open_sym(&last) {
                // For match arm lines ending with `{`, indent the hang opener one extra level
                // so the block body is at arm_indent + 2*indent_size and `}` at arm_indent + indent_size.
                let hang_indent = if is_arm_line && open == '{' {
                    prev_effective_indent_len + indent_size
                } else {
                    prev_effective_indent_len
                };
                hang_delim_stack.push((open, hang_indent));
            }
        }
        if let Some(else_idx) = find_top_level_token_clamped(&state.tokens, "else", first_idx) {
            if let Some(idx) = if_stack.iter().rposition(|f| f.phase == IfPhase::Then) {
                let else_inline = state.tokens.iter().skip(else_idx + 1).any(|t| {
                    t.kind != "comment" && t.text != "\n"
                });
                if_stack[idx].phase = IfPhase::Else;
                if_stack[idx].active_indent = !else_inline;
            }
        }
        if prev_non_blank_last_token.as_deref() == Some("then") {
            if_stack.push(IfFrame {
                if_indent: prev_effective_indent_len,
                phase: IfPhase::Then,
                active_indent: true,
            });
        }
        if state.tokens.get(first_idx).map(|t| t.text.as_str()) == Some("then")
            && prev_non_blank_last_token.as_deref() != Some("then")
        {
            if_stack.push(IfFrame {
                if_indent: prev_effective_indent_len,
                phase: IfPhase::Then,
                active_indent: false,
            });
        }
        if find_top_level_token(&state.tokens, "if", first_idx).is_some()
            && find_top_level_token(&state.tokens, "then", first_idx).is_none()
        {
            pending_then_indent = Some(prev_effective_indent_len);
        } else {
            pending_then_indent = None;
        }
        if line_has_top_level_eq {
            pipeop_seed_indent = Some(line_indent_len);
        }
        if seeds_rhs_continuation(prev_non_blank_last_token.as_deref()) {
            let depth = net_open_depth(&state.tokens);
            if depth == 0 {
                rhs_next_line_indent = Some(line_indent_len);
                rhs_next_line_depth = Some(depth);
                rhs_block_base_indent = Some(line_indent_len);
                rhs_block_base_depth = Some(line_depth);
            }
        }
        update_open_depth(&mut open_depth, &state.tokens);

        // Decorators on their own line are part of the following definition/type-sig, even in
        // RHS continuation blocks (e.g. `x =\n  @test\n  foo = ...`). Keep the RHS block alive
        // for the next non-blank line so the binding doesn't accidentally dedent.
        if rhs_block_base_indent.is_some() && is_decorator_only_line {
            rhs_decorator_pending = true;
        }

        // After rendering an arm line, keep an extra indentation level for its body until the next
        // arm (or until we leave the surrounding `|` block).
        if is_arm_line {
            arm_rhs_active = true;
        } else if starts_with_pipe {
            // Starting a new arm resets the body indent for this line.
            arm_rhs_active = false;
        }
    }

    // Strip leading blank lines to keep output stable when inputs start with a newline.
    let first_non_blank = rendered_lines
        .iter()
        .position(|line| !line.is_empty())
        .unwrap_or(rendered_lines.len());
    if first_non_blank > 0 {
        rendered_lines.drain(0..first_non_blank);
    }

    // Post-render pass: expand `use path (a, b, c, ...)` lines whose rendered width exceeds
    // `max_width` or that import ≥ 4 names into a one-import-per-line form:
    //
    //   use path (
    //     name1,
    //     name2,
    //   )
    {
        fn try_expand_use(line: &str, max_width: usize) -> Option<Vec<String>> {
            let trimmed = line.trim_start();
            if !trimmed.starts_with("use ") {
                return None;
            }
            let indent_len = line.len() - trimmed.len();
            let indent = &line[..indent_len];
            let rest = &trimmed[4..]; // skip "use "
            let paren_pos = rest.find('(')?;
            let path_part = rest[..paren_pos].trim_end();
            let closing = rest.rfind(')')?;
            let imports_str = rest[paren_pos + 1..closing].trim();
            if imports_str.is_empty() {
                return None;
            }
            let imports: Vec<&str> = imports_str
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();
            // Expand when the rendered line is at least `max_width` characters wide.
            if line.len() < max_width {
                return None;
            }
            if imports.len() < 2 {
                return None;
            }
            let item_indent = format!("{}  ", indent);
            let mut result = vec![format!("{}use {} (", indent, path_part)];
            for import in &imports {
                result.push(format!("{}{},", item_indent, import));
            }
            result.push(format!("{})", indent));
            Some(result)
        }

        let old_lines = std::mem::take(&mut rendered_lines);
        rendered_lines.reserve(old_lines.len() + 32);
        for line in old_lines {
            match try_expand_use(&line, options.max_width) {
                Some(expanded) => rendered_lines.extend(expanded),
                None => rendered_lines.push(line),
            }
        }
    }

    // Post-render pass: manage blank lines between `use` groups.
    //
    // After multiline expansion consecutive `use` blocks may share a first-segment group
    // (e.g. all `mailfox.ui.*`) or belong to different groups (e.g. `aivi.*` vs `mailfox.*`).
    // Rules:
    //   • Same first-segment: no blank between them (remove any stray blanks).
    //   • Different first-segment: exactly one blank between them (add if missing).
    //
    // We detect the "current use block" as either a single `use …` line or a multi-line
    // expansion starting with `use ` and ending with the matching `)`.
    {
        /// Extract the first path segment from a rendered `use …` line, e.g. "aivi" or "mailfox".
        fn use_first_seg(line: &str) -> Option<&str> {
            let trimmed = line.trim_start();
            let rest = trimmed.strip_prefix("use ")?;
            let end = rest
                .find(['.', '(', ' '])
                .unwrap_or(rest.len());
            if end == 0 {
                return None;
            }
            Some(&rest[..end])
        }

        // Build a list of (start_idx, end_idx, group_key) for each contiguous use block.
        // `end_idx` is exclusive (one past the last line of the block).
        let mut use_blocks: Vec<(usize, usize, String)> = Vec::new();
        let mut i = 0usize;
        while i < rendered_lines.len() {
            let line = &rendered_lines[i];
            if let Some(seg) = use_first_seg(line) {
                let group = seg.to_string();
                // A multi-line expansion ends with a line that is exactly `<indent>)`.
                let indent_len = line.len() - line.trim_start().len();
                let close_pat = format!("{})", &line[..indent_len]);
                let trimmed = line.trim_start();
                if trimmed.contains('(') && !trimmed.ends_with(')') {
                    // Multi-line use: scan forward for the closing `)`.
                    let start = i;
                    i += 1;
                    while i < rendered_lines.len() && rendered_lines[i] != close_pat {
                        i += 1;
                    }
                    let end = if i < rendered_lines.len() { i + 1 } else { i };
                    use_blocks.push((start, end, group));
                    i = end;
                } else {
                    use_blocks.push((i, i + 1, group));
                    i += 1;
                }
            } else {
                i += 1;
            }
        }

        // Walk through consecutive use-block pairs and enforce blank-line policy.
        // We process in reverse order so index manipulation doesn't invalidate later positions.
        for pair in use_blocks.windows(2).rev() {
            let (_, end_a, ref group_a) = pair[0];
            let (start_b, _, ref group_b) = pair[1];
            // Lines between the two blocks (end_a..start_b) should contain the blank lines.
            let between_start = end_a;
            let between_end = start_b;
            let between_len = between_end.saturating_sub(between_start);
            let blanks_present = (between_start..between_end)
                .all(|k| rendered_lines[k].trim().is_empty());

            if group_a == group_b {
                // Same group: remove all blank lines between them.
                if between_len > 0 && blanks_present {
                    rendered_lines.drain(between_start..between_end);
                }
            } else {
                // Different group: ensure exactly one blank line between them.
                if between_len == 0 {
                    rendered_lines.insert(between_start, String::new());
                } else if between_len > 1 && blanks_present {
                    rendered_lines.drain(between_start + 1..between_end);
                }
            }
        }
    }

    // Post-render pass: align comment-only lines to the indentation of the next code line.
    // The render pass assigns `state.indent` (delimiter-nesting only) to comment-only lines,
    // but code lines may receive a larger effective indent from continuation/alignment heuristics.
    // When the next non-blank non-comment line is more deeply indented, raise the comment to match.
    {
        let mut i = 0usize;
        while i < rendered_lines.len() {
            let trimmed = rendered_lines[i].trim_start();
            if trimmed.starts_with("//") {
                let current_indent = rendered_lines[i].len() - trimmed.len();
                if let Some(next_indent) = (i + 1..rendered_lines.len())
                    .find(|&j| {
                        let t = rendered_lines[j].trim_start();
                        !t.is_empty() && !t.starts_with("//")
                    })
                    .map(|j| rendered_lines[j].len() - rendered_lines[j].trim_start().len())
                {
                    if next_indent > current_indent {
                        rendered_lines[i] = format!("{}{}", " ".repeat(next_indent), trimmed);
                    }
                }
            }
            i += 1;
        }
    }

    // Align consecutive single-line records with identical field structure inside list literals.
    // E.g. a list of `{ key: "n", modifiers: "ctrl", action: "compose", label: "New email" }`
    // lines gets their corresponding field values aligned to the same column.
    {
        /// Split `s` by top-level commas (respecting `{}`, `()`, `[]`, and `"…"` strings).
        fn split_top_level_commas(s: &str) -> Vec<String> {
            let mut result = Vec::new();
            let mut depth: i32 = 0;
            let mut in_string = false;
            let mut escape = false;
            let mut start = 0usize;
            let bytes = s.as_bytes();
            let mut i = 0usize;
            while i < bytes.len() {
                let b = bytes[i];
                if escape {
                    escape = false;
                    i += 1;
                    continue;
                }
                if in_string {
                    if b == b'\\' {
                        escape = true;
                    } else if b == b'"' {
                        in_string = false;
                    }
                    i += 1;
                    continue;
                }
                match b {
                    b'"' => in_string = true,
                    b'{' | b'(' | b'[' => depth += 1,
                    b'}' | b')' | b']' => depth -= 1,
                    b',' if depth == 0 => {
                        result.push(s[start..i].trim().to_string());
                        start = i + 1;
                    }
                    _ => {}
                }
                i += 1;
            }
            let tail = s[start..].trim();
            if !tail.is_empty() {
                result.push(tail.to_string());
            }
            result
        }

        /// Parse a rendered line as an inline record: returns `(indent, fields)` where each
        /// field is the trimmed `key: value` string (without surrounding comma).
        /// Returns `None` if the line is not a single-line record.
        fn parse_inline_record(line: &str) -> Option<(String, Vec<String>)> {
            let trimmed_end = line.trim_end_matches([' ', '\t']);
            let indent_len = trimmed_end.len() - trimmed_end.trim_start().len();
            let indent = trimmed_end[..indent_len].to_string();
            let inner = trimmed_end.trim_start();
            // Must look like `{ ... }`; no nested `{` allowed at depth 0 in the content.
            if !inner.starts_with('{') || !inner.ends_with('}') {
                return None;
            }
            let content = inner[1..inner.len() - 1].trim();
            if content.is_empty() {
                return None;
            }
            let fields = split_top_level_commas(content);
            // Every field must look like `ident: value` (at least one `:` after an identifier).
            for f in &fields {
                let f = f.trim();
                let colon = f.find(':')?;
                let key = f[..colon].trim();
                if key.is_empty() || key.contains(' ') {
                    return None;
                }
            }
            if fields.is_empty() {
                return None;
            }
            Some((indent, fields))
        }

        /// Extract the field key (before the first `:`).
        fn field_key(field: &str) -> &str {
            field.split(':').next().map(str::trim).unwrap_or("")
        }

        let mut i = 0usize;
        while i < rendered_lines.len() {
            let Some((indent0, fields0)) = parse_inline_record(&rendered_lines[i]) else {
                i += 1;
                continue;
            };
            // Collect the run of consecutive same-structure records at the same indent.
            let mut j = i + 1;
            while j < rendered_lines.len() {
                if let Some((ind, flds)) = parse_inline_record(&rendered_lines[j]) {
                    // Same indent and same field keys in same order.
                    if ind == indent0
                        && flds.len() == fields0.len()
                        && flds
                            .iter()
                            .zip(fields0.iter())
                            .all(|(a, b)| field_key(a) == field_key(b))
                    {
                        j += 1;
                        continue;
                    }
                }
                break;
            }
            if j - i >= 2 {
                // Compute max width per field position (for all but the last field).
                let n_fields = fields0.len();
                let mut max_widths = vec![0usize; n_fields.saturating_sub(1)];
                for line in &rendered_lines[i..j] {
                    if let Some((_, flds)) = parse_inline_record(line) {
                        for (k, w) in max_widths.iter_mut().enumerate() {
                            *w = (*w).max(flds[k].len());
                        }
                    }
                }
                // Re-render each record with padding.
                for line in rendered_lines[i..j].iter_mut() {
                    if let Some((ind, flds)) = parse_inline_record(line) {
                        let mut s = ind;
                        s.push('{');
                        for (k, f) in flds.iter().enumerate() {
                            if k == 0 {
                                s.push(' ');
                            }
                            s.push_str(f);
                            if k + 1 < flds.len() {
                                s.push(',');
                                // Pad after the comma so the next field starts at a fixed column.
                                let pad = max_widths[k].saturating_sub(f.len()) + 1;
                                s.push_str(&" ".repeat(pad));
                            }
                        }
                        s.push_str(" }");
                        *line = s;
                    }
                }
            }
            i = j;
        }
    }

    // Final render via the `Doc` renderer. Today we mostly use hardlines, but this keeps the
    // formatter architecture ready for width-aware grouping in future rules.
    let mut doc_items = Vec::with_capacity(rendered_lines.len().saturating_mul(2));
    for line in rendered_lines.into_iter() {
        // Strip trailing whitespace so formatting is idempotent.
        // Only strip ASCII whitespace (space/tab) — not all Unicode whitespace —
        // to avoid removing unknown tokens (e.g. \x0c form-feed) that the lexer
        // emits as content tokens, which would change the token structure between
        // formatting passes.
        let trimmed = line.trim_end_matches([' ', '\t']).to_string();
        doc_items.push(super::doc::Doc::text(trimmed));
        doc_items.push(super::doc::Doc::hardline());
    }
    let mut result = super::doc::render(super::doc::Doc::concat(doc_items), options.max_width);

    // Ensure exactly one trailing newline.
    while result.ends_with('\n') {
        result.pop();
    }
    result.push('\n');
    result
}
