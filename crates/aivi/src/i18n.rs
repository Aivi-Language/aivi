use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleParts {
    pub language: String,
    pub region: Option<String>,
    pub variants: Vec<String>,
    /// Normalized tag (e.g. `en-US`).
    pub tag: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessagePart {
    Lit(String),
    Hole { name: String, ty: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMessage {
    pub parts: Vec<MessagePart>,
}

pub fn validate_key_text(text: &str) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("key is empty".to_string());
    }
    if text.starts_with('.') || text.ends_with('.') || text.contains("..") {
        return Err("key must not have empty segments".to_string());
    }
    for segment in text.split('.') {
        validate_key_segment(segment)?;
    }
    Ok(())
}

fn validate_key_segment(segment: &str) -> Result<(), String> {
    let mut chars = segment.chars();
    let Some(first) = chars.next() else {
        return Err("key segment is empty".to_string());
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(format!(
            "invalid key segment '{segment}': must start with ASCII letter or '_'"
        ));
    }
    for ch in chars {
        if !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-') {
            return Err(format!(
                "invalid key segment '{segment}': unexpected char '{ch}'"
            ));
        }
    }
    Ok(())
}

pub fn parse_locale_tag(tag: &str) -> Result<LocaleParts, String> {
    let raw = tag.trim();
    if raw.is_empty() {
        return Err("locale tag is empty".to_string());
    }
    let subtags: Vec<&str> = raw.split(['-', '_']).filter(|s| !s.is_empty()).collect();
    if subtags.is_empty() {
        return Err("locale tag is empty".to_string());
    }

    let language_raw = subtags[0];
    if !language_raw.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err("language subtag must be ASCII letters".to_string());
    }
    if !(2..=8).contains(&language_raw.len()) {
        return Err("language subtag must be 2..=8 letters".to_string());
    }
    let language = language_raw.to_ascii_lowercase();

    let mut region = None;
    let mut variants = Vec::new();

    let mut i = 1;
    if i < subtags.len() {
        let s = subtags[i];
        let is_region = (s.len() == 2 && s.chars().all(|c| c.is_ascii_alphabetic()))
            || (s.len() == 3 && s.chars().all(|c| c.is_ascii_digit()));
        if is_region {
            region = Some(s.to_ascii_uppercase());
            i += 1;
        }
    }

    while i < subtags.len() {
        let s = subtags[i];
        if !s.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(format!("invalid locale subtag '{s}'"));
        }
        if !(4..=8).contains(&s.len()) {
            return Err(format!(
                "invalid locale subtag '{s}': expected 4..=8 alphanumeric"
            ));
        }
        variants.push(s.to_string());
        i += 1;
    }

    let mut normalized = language.clone();
    if let Some(r) = &region {
        normalized.push('-');
        normalized.push_str(r);
    }
    for v in &variants {
        normalized.push('-');
        normalized.push_str(v);
    }

    Ok(LocaleParts {
        language,
        region,
        variants,
        tag: normalized,
    })
}

pub fn parse_message_template(text: &str) -> Result<ParsedMessage, String> {
    // Template grammar:
    // - Literal text.
    // - Escapes: '{{' -> '{', '}}' -> '}'.
    // - Placeholder: '{name}' or '{name:Type}' where name is identifier-ish.
    let mut parts = Vec::new();
    let mut buf = String::new();

    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' => {
                if matches!(chars.peek(), Some('{')) {
                    chars.next();
                    buf.push('{');
                    continue;
                }
                if !buf.is_empty() {
                    parts.push(MessagePart::Lit(std::mem::take(&mut buf)));
                }
                let mut inner = String::new();
                let mut closed = false;
                for next in chars.by_ref() {
                    if next == '}' {
                        closed = true;
                        break;
                    }
                    inner.push(next);
                }
                if !closed {
                    return Err("unclosed '{' in message template".to_string());
                }
                let inner = inner.trim();
                if inner.is_empty() {
                    return Err("empty placeholder '{}'".to_string());
                }
                let (name, ty) = match inner.split_once(':') {
                    Some((n, t)) => (n.trim(), Some(t.trim())),
                    None => (inner, None),
                };
                validate_placeholder_name(name)?;
                let ty = ty
                    .filter(|t| !t.is_empty())
                    .map(|t| validate_placeholder_type(t).map(|_| t.to_string()))
                    .transpose()?;
                parts.push(MessagePart::Hole {
                    name: name.to_string(),
                    ty,
                });
            }
            '}' => {
                if matches!(chars.peek(), Some('}')) {
                    chars.next();
                    buf.push('}');
                    continue;
                }
                return Err("unexpected '}' (use '}}' to escape)".to_string());
            }
            other => buf.push(other),
        }
    }
    if !buf.is_empty() {
        parts.push(MessagePart::Lit(buf));
    }

    Ok(ParsedMessage { parts })
}

fn validate_placeholder_name(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err("placeholder name is empty".to_string());
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(format!(
            "invalid placeholder name '{name}': must start with ASCII letter or '_'"
        ));
    }
    for ch in chars {
        if !(ch.is_ascii_alphanumeric() || ch == '_') {
            return Err(format!(
                "invalid placeholder name '{name}': unexpected char '{ch}'"
            ));
        }
    }
    Ok(())
}

fn validate_placeholder_type(ty: &str) -> Result<(), String> {
    match ty {
        "Text" | "Int" | "Float" | "Bool" | "Decimal" | "DateTime" => Ok(()),
        other => Err(format!("unsupported placeholder type '{other}'")),
    }
}

pub fn escape_sigil_string_body(text: &str) -> Cow<'_, str> {
    // `~tag"..."` uses the sigil string body verbatim with backslash escapes preserved.
    // Keep the escaping minimal and ASCII-friendly.
    if !text.contains('\\') && !text.contains('"') && !text.contains('\n') && !text.contains('\r') {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len() + 8);
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(ch),
        }
    }
    Cow::Owned(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn key_validation_accepts_dotted() {
        validate_key_text("app.welcome").unwrap();
        validate_key_text("a_b.c-d").unwrap();
    }

    #[test]
    fn key_validation_rejects_empty_segment() {
        assert!(validate_key_text("app..welcome").is_err());
        assert!(validate_key_text(".app").is_err());
        assert!(validate_key_text("app.").is_err());
    }

    #[test]
    fn locale_parse_normalizes() {
        let loc = parse_locale_tag("EN_us").unwrap();
        assert_eq!(loc.language, "en");
        assert_eq!(loc.region.as_deref(), Some("US"));
        assert_eq!(loc.tag, "en-US");
    }

    #[test]
    fn message_parses_placeholders_and_escapes() {
        let msg = parse_message_template("Hello {{ {name:Text} }}!").unwrap();
        assert_eq!(
            msg.parts,
            vec![
                MessagePart::Lit("Hello { ".to_string()),
                MessagePart::Hole {
                    name: "name".to_string(),
                    ty: Some("Text".to_string())
                },
                MessagePart::Lit(" }!".to_string())
            ]
        );
    }
}
