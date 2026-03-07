#[derive(Debug, Clone)]
struct MarkupSigil {
    open: &'static str,
    close: &'static str,
    body: String,
}

fn parse_markup_sigil(text: &str) -> Option<MarkupSigil> {
    for (open, close) in [("~<html>", "</html>"), ("~<gtk>", "</gtk>")] {
        if text.starts_with(open) && text.ends_with(close) {
            let body_start = open.chars().count();
            let body_end = text.chars().count().saturating_sub(close.chars().count());
            let body: String = text
                .chars()
                .skip(body_start)
                .take(body_end.saturating_sub(body_start))
                .collect();
            return Some(MarkupSigil { open, close, body });
        }
    }
    None
}

fn parse_open_markup_tag(tag_text: &str) -> Option<(String, Vec<String>, bool)> {
    fn scan_markup_part_end(chars: &[char], start: usize) -> usize {
        let mut i = start;
        let mut brace_depth = 0isize;
        let mut paren_depth = 0isize;
        let mut bracket_depth = 0isize;
        let mut in_quote: Option<char> = None;

        while i < chars.len() {
            let ch = chars[i];
            if let Some(quote) = in_quote {
                if quote != '`' && ch == '\\' && i + 1 < chars.len() {
                    i += 2;
                    continue;
                }
                if ch == quote {
                    in_quote = None;
                }
                i += 1;
                continue;
            }

            match ch {
                '"' | '\'' | '`' => in_quote = Some(ch),
                '{' => brace_depth += 1,
                '}' => {
                    if brace_depth == 0 {
                        break;
                    }
                    brace_depth -= 1;
                }
                '(' => paren_depth += 1,
                ')' => {
                    if paren_depth == 0 {
                        break;
                    }
                    paren_depth -= 1;
                }
                '[' => bracket_depth += 1,
                ']' => {
                    if bracket_depth == 0 {
                        break;
                    }
                    bracket_depth -= 1;
                }
                '>'
                    if brace_depth == 0 && paren_depth == 0 && bracket_depth == 0 =>
                {
                    break;
                }
                '/'
                    if brace_depth == 0
                        && paren_depth == 0
                        && bracket_depth == 0
                        && i + 1 < chars.len()
                        && chars[i + 1] == '>' =>
                {
                    break;
                }
                _ if ch.is_whitespace()
                    && brace_depth == 0
                    && paren_depth == 0
                    && bracket_depth == 0 =>
                {
                    break;
                }
                _ => {}
            }

            i += 1;
        }

        i
    }

    let raw = tag_text.trim();
    if !raw.starts_with('<') || raw.starts_with("</") || !raw.ends_with('>') {
        return None;
    }
    let mut inner = raw
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim()
        .to_string();
    let self_close = inner.ends_with('/');
    if self_close {
        inner = inner.trim_end_matches('/').trim_end().to_string();
    }
    if inner.is_empty() {
        return None;
    }

    let chars: Vec<char> = inner.chars().collect();
    let mut i = 0usize;
    while i < chars.len() && !chars[i].is_whitespace() {
        i += 1;
    }
    let tag_name: String = chars[0..i].iter().collect();
    if tag_name.is_empty() {
        return None;
    }

    let mut attrs = Vec::new();
    while i < chars.len() {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }

        let part_start = i;
        let mut name_end = i;
        if chars[i].is_ascii_alphabetic() || chars[i] == '_' || chars[i] == ':' {
            name_end += 1;
            while name_end < chars.len()
                && (chars[name_end].is_ascii_alphanumeric()
                    || matches!(chars[name_end], '-' | ':' | '_' | '.'))
            {
                name_end += 1;
            }
            let mut look = name_end;
            while look < chars.len() && chars[look].is_whitespace() {
                look += 1;
            }
            if look < chars.len() && chars[look] == '=' {
                let name: String = chars[part_start..name_end].iter().collect();
                i = look + 1;
                while i < chars.len() && chars[i].is_whitespace() {
                    i += 1;
                }
                if i >= chars.len() {
                    attrs.push(name);
                    break;
                }
                let value_start = i;
                if chars[i] == '"' || chars[i] == '\'' {
                    let quote = chars[i];
                    i += 1;
                    while i < chars.len() {
                        if chars[i] == '\\' && i + 1 < chars.len() {
                            i += 2;
                            continue;
                        }
                        if chars[i] == quote {
                            i += 1;
                            break;
                        }
                        i += 1;
                    }
                } else if chars[i] == '{' {
                    let mut depth = 1isize;
                    i += 1;
                    let mut in_quote: Option<char> = None;
                    while i < chars.len() && depth > 0 {
                        let ch = chars[i];
                        if let Some(q) = in_quote {
                            if q != '`' && ch == '\\' && i + 1 < chars.len() {
                                i += 2;
                                continue;
                            }
                            if ch == q {
                                in_quote = None;
                            }
                            i += 1;
                            continue;
                        }
                        match ch {
                            '"' | '\'' | '`' => in_quote = Some(ch),
                            '{' => depth += 1,
                            '}' => depth -= 1,
                            _ => {}
                        }
                        i += 1;
                    }
                } else {
                    i = scan_markup_part_end(&chars, value_start);
                }
                let value: String = chars[value_start..i.min(chars.len())].iter().collect();
                attrs.push(format!("{name}={value}"));
                continue;
            }
        }

        i = scan_markup_part_end(&chars, part_start);
        let part: String = chars[part_start..i].iter().collect();
        if !part.trim().is_empty() {
            attrs.push(part.trim().to_string());
        }
    }

    Some((tag_name, attrs, self_close))
}

fn normalize_top_level_colon_spacing(field: &str) -> String {
    let mut brace_depth = 0isize;
    let mut paren_depth = 0isize;
    let mut bracket_depth = 0isize;
    let mut in_quote: Option<char> = None;
    let mut escaped = false;
    let mut colon_idx: Option<usize> = None;

    for (idx, ch) in field.char_indices() {
        if let Some(q) = in_quote {
            if q != '`' && !escaped && ch == '\\' {
                escaped = true;
                continue;
            }
            if escaped {
                escaped = false;
                continue;
            }
            if ch == q {
                in_quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' | '`' => in_quote = Some(ch),
            '{' => brace_depth += 1,
            '}' => {
                if brace_depth > 0 {
                    brace_depth -= 1;
                }
            }
            '(' => paren_depth += 1,
            ')' => {
                if paren_depth > 0 {
                    paren_depth -= 1;
                }
            }
            '[' => bracket_depth += 1,
            ']' => {
                if bracket_depth > 0 {
                    bracket_depth -= 1;
                }
            }
            ':' if brace_depth == 0 && paren_depth == 0 && bracket_depth == 0 => {
                colon_idx = Some(idx);
                break;
            }
            _ => {}
        }
    }

    if let Some(idx) = colon_idx {
        let lhs = field[..idx].trim();
        let rhs = field[idx + 1..].trim();
        format!("{lhs}: {rhs}")
    } else {
        field.trim().to_string()
    }
}

fn split_top_level_commas(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut brace_depth = 0isize;
    let mut paren_depth = 0isize;
    let mut bracket_depth = 0isize;
    let mut in_quote: Option<char> = None;
    let mut escaped = false;

    for (idx, ch) in text.char_indices() {
        if let Some(q) = in_quote {
            if q != '`' && !escaped && ch == '\\' {
                escaped = true;
                continue;
            }
            if escaped {
                escaped = false;
                continue;
            }
            if ch == q {
                in_quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' | '`' => in_quote = Some(ch),
            '{' => brace_depth += 1,
            '}' => {
                if brace_depth > 0 {
                    brace_depth -= 1;
                }
            }
            '(' => paren_depth += 1,
            ')' => {
                if paren_depth > 0 {
                    paren_depth -= 1;
                }
            }
            '[' => bracket_depth += 1,
            ']' => {
                if bracket_depth > 0 {
                    bracket_depth -= 1;
                }
            }
            ',' if brace_depth == 0 && paren_depth == 0 && bracket_depth == 0 => {
                let item = text[start..idx].trim();
                if !item.is_empty() {
                    out.push(normalize_top_level_colon_spacing(item));
                }
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }

    let tail = text[start..].trim();
    if !tail.is_empty() {
        out.push(normalize_top_level_colon_spacing(tail));
    }
    out
}

fn parse_wrapped_record_attr(attr: &str) -> Option<(String, Vec<String>)> {
    let (name, value) = attr.split_once('=')?;
    let value = value.trim();
    let outer = value.strip_prefix('{')?.strip_suffix('}')?.trim();
    let inner = outer.strip_prefix('{')?.strip_suffix('}')?.trim();
    if inner.is_empty() || !inner.contains(':') {
        return None;
    }
    let fields = split_top_level_commas(inner);
    if fields.is_empty() {
        return None;
    }
    Some((name.trim().to_string(), fields))
}

fn format_wrapped_record_attr_inline(name: &str, fields: &[String]) -> String {
    format!("{name}={{{{ {} }}}}", fields.join(", "))
}

fn format_markup_sigil(text: &str) -> Option<Vec<String>> {
    let MarkupSigil { open, close, body } = parse_markup_sigil(text)?;
    let body_chars: Vec<char> = body.chars().collect();
    let mut lines = vec![open.to_string()];
    let mut depth = 1usize;
    let mut i = 0usize;

    while i < body_chars.len() {
        let ch = body_chars[i];
        if ch.is_whitespace() {
            i += 1;
            continue;
        }

        if ch == '<' {
            if i + 3 < body_chars.len()
                && body_chars[i + 1] == '!'
                && body_chars[i + 2] == '-'
                && body_chars[i + 3] == '-'
            {
                let start = i;
                i += 4;
                while i + 2 < body_chars.len() {
                    if body_chars[i] == '-'
                        && body_chars[i + 1] == '-'
                        && body_chars[i + 2] == '>'
                    {
                        i += 3;
                        break;
                    }
                    i += 1;
                }
                let comment: String = body_chars[start..i.min(body_chars.len())].iter().collect();
                lines.push(format!("{}{}", " ".repeat(depth * 2), comment.trim()));
                continue;
            }

            let start = i;
            i += 1;
            let mut in_quote: Option<char> = None;
            let mut brace_depth = 0isize;
            while i < body_chars.len() {
                let c = body_chars[i];
                if let Some(q) = in_quote {
                    if q != '`' && c == '\\' && i + 1 < body_chars.len() {
                        i += 2;
                        continue;
                    }
                    if c == q {
                        in_quote = None;
                    }
                    i += 1;
                    continue;
                }
                match c {
                    '"' | '\'' | '`' => in_quote = Some(c),
                    '{' => brace_depth += 1,
                    '}' => {
                        if brace_depth > 0 {
                            brace_depth -= 1;
                        }
                    }
                    '>' if brace_depth == 0 => {
                        i += 1;
                        break;
                    }
                    _ => {}
                }
                i += 1;
            }
            if i > body_chars.len() {
                return None;
            }
            let tag_text: String = body_chars[start..i.min(body_chars.len())].iter().collect();
            let trimmed = tag_text.trim();
            if trimmed.starts_with("</") {
                let name = trimmed
                    .trim_start_matches("</")
                    .trim_end_matches('>')
                    .trim();
                depth = depth.saturating_sub(1);
                lines.push(format!("{}</{}>", " ".repeat(depth * 2), name));
                continue;
            }
            let (tag, attrs, self_close) = parse_open_markup_tag(trimmed)?;
            let indent = " ".repeat(depth * 2);
            // If the source tag spans multiple lines, break all attributes onto
            // their own lines.  Otherwise keep them inline (just fix spacing).
            let attrs_on_new_lines = !attrs.is_empty() && tag_text.contains('\n');
            if !attrs_on_new_lines {
                let wrapped = attrs
                    .iter()
                    .enumerate()
                    .find_map(|(idx, attr)| parse_wrapped_record_attr(attr).map(|v| (idx, v)));

                if let Some((wrapped_idx, (wrapped_name, wrapped_fields))) = wrapped {
                    if wrapped_fields.len() <= 3 {
                        let mut line = format!("{indent}<{tag}");
                        for attr in attrs.iter().take(wrapped_idx) {
                            line.push(' ');
                            line.push_str(attr);
                        }
                        line.push(' ');
                        line.push_str(&format_wrapped_record_attr_inline(
                            &wrapped_name,
                            &wrapped_fields,
                        ));
                        for attr in attrs.iter().skip(wrapped_idx + 1) {
                            line.push(' ');
                            line.push_str(attr);
                        }
                        if self_close {
                            line.push_str(" />");
                        } else {
                            line.push('>');
                        }
                        lines.push(line);
                    } else {
                        let mut open_line = format!("{indent}<{tag}");
                        for attr in attrs.iter().take(wrapped_idx) {
                            open_line.push(' ');
                            open_line.push_str(attr);
                        }
                        open_line.push(' ');
                        open_line.push_str(&format!("{wrapped_name}={{{{"));
                        lines.push(open_line);

                        let field_indent = format!("{indent}  ");
                        for (idx, field) in wrapped_fields.iter().enumerate() {
                            if idx + 1 == wrapped_fields.len() {
                                lines.push(format!("{field_indent}{field}"));
                            } else {
                                lines.push(format!("{field_indent}{field},"));
                            }
                        }

                        let mut close_line = format!("{indent}}}}}");
                        for attr in attrs.iter().skip(wrapped_idx + 1) {
                            close_line.push(' ');
                            close_line.push_str(attr);
                        }
                        if self_close {
                            close_line.push_str(" />");
                        } else {
                            close_line.push('>');
                        }
                        lines.push(close_line);
                    }
                } else {
                    let mut line = format!("{indent}<{tag}");
                    for attr in attrs {
                        line.push(' ');
                        line.push_str(&attr);
                    }
                    if self_close {
                        line.push_str(" />");
                    } else {
                        line.push('>');
                    }
                    lines.push(line);
                }
            } else {
                lines.push(format!("{indent}<{tag}"));
                let attr_indent = format!("{indent}  ");
                for (idx, attr) in attrs.iter().enumerate() {
                    if idx + 1 == attrs.len() {
                        if self_close {
                            lines.push(format!("{attr_indent}{attr} />"));
                        } else {
                            lines.push(format!("{attr_indent}{attr}>"));
                        }
                    } else {
                        lines.push(format!("{attr_indent}{attr}"));
                    }
                }
            }
            if !self_close {
                depth += 1;
            }
            continue;
        }

        if ch == '{' {
            let start = i;
            i += 1;
            let mut brace_depth = 1isize;
            let mut in_quote: Option<char> = None;
            while i < body_chars.len() && brace_depth > 0 {
                let c = body_chars[i];
                if let Some(q) = in_quote {
                    if q != '`' && c == '\\' && i + 1 < body_chars.len() {
                        i += 2;
                        continue;
                    }
                    if c == q {
                        in_quote = None;
                    }
                    i += 1;
                    continue;
                }
                match c {
                    '"' | '\'' | '`' => in_quote = Some(c),
                    '{' => brace_depth += 1,
                    '}' => brace_depth -= 1,
                    _ => {}
                }
                i += 1;
            }
            let end = i.min(body_chars.len());
            let inner: String = body_chars[start + 1..end.saturating_sub(1)].iter().collect();
            let inner = inner.trim();
            if !inner.is_empty() {
                lines.push(format!("{}{{ {inner} }}", " ".repeat(depth * 2)));
            }
            continue;
        }

        let start = i;
        while i < body_chars.len() && body_chars[i] != '<' && body_chars[i] != '{' {
            i += 1;
        }
        let text: String = body_chars[start..i].iter().collect();
        let text = text.trim();
        if !text.is_empty() {
            lines.push(format!("{}{}", " ".repeat(depth * 2), text));
        }
    }

    lines.push(close.to_string());
    Some(lines)
}
