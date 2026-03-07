#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawTextEmbeddedLanguage {
    pub tag: &'static str,
    pub grammar_scope: &'static str,
    pub language_id: &'static str,
    pub embedded_scope: &'static str,
}

pub const RAW_TEXT_EMBEDDED_LANGUAGES: &[RawTextEmbeddedLanguage] = &[
    RawTextEmbeddedLanguage {
        tag: "css",
        grammar_scope: "source.css",
        language_id: "css",
        embedded_scope: "meta.embedded.inline.css.aivi",
    },
    RawTextEmbeddedLanguage {
        tag: "html",
        grammar_scope: "text.html.basic",
        language_id: "html",
        embedded_scope: "meta.embedded.inline.html.aivi",
    },
    RawTextEmbeddedLanguage {
        tag: "xml",
        grammar_scope: "text.xml",
        language_id: "xml",
        embedded_scope: "meta.embedded.inline.xml.aivi",
    },
    RawTextEmbeddedLanguage {
        tag: "json",
        grammar_scope: "source.json",
        language_id: "json",
        embedded_scope: "meta.embedded.inline.json.aivi",
    },
    RawTextEmbeddedLanguage {
        tag: "sql",
        grammar_scope: "source.sql",
        language_id: "sql",
        embedded_scope: "meta.embedded.inline.sql.aivi",
    },
    RawTextEmbeddedLanguage {
        tag: "javascript",
        grammar_scope: "source.js",
        language_id: "javascript",
        embedded_scope: "meta.embedded.inline.javascript.aivi",
    },
    RawTextEmbeddedLanguage {
        tag: "js",
        grammar_scope: "source.js",
        language_id: "javascript",
        embedded_scope: "meta.embedded.inline.javascript.aivi",
    },
    RawTextEmbeddedLanguage {
        tag: "typescript",
        grammar_scope: "source.ts",
        language_id: "typescript",
        embedded_scope: "meta.embedded.inline.typescript.aivi",
    },
    RawTextEmbeddedLanguage {
        tag: "ts",
        grammar_scope: "source.ts",
        language_id: "typescript",
        embedded_scope: "meta.embedded.inline.typescript.aivi",
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRawTextSigil {
    pub language: Option<&'static RawTextEmbeddedLanguage>,
    pub body: String,
    pub margin_stripped: bool,
}

pub fn raw_text_embedded_language(tag: &str) -> Option<&'static RawTextEmbeddedLanguage> {
    RAW_TEXT_EMBEDDED_LANGUAGES
        .iter()
        .find(|language| language.tag == tag)
}

pub fn raw_text_language_line(first_line: &str) -> Option<&'static RawTextEmbeddedLanguage> {
    if first_line.trim_start_matches([' ', '\t']) != first_line {
        return None;
    }

    let trimmed = first_line.trim_end_matches([' ', '\t', '\r']);
    if trimmed.is_empty() {
        return None;
    }

    raw_text_embedded_language(trimmed)
}

pub fn parse_raw_text_sigil(text: &str) -> Option<ParsedRawTextSigil> {
    let inner = text.strip_prefix("~`")?.strip_suffix('`')?;
    let (language, body_source) = split_language_tag(inner);
    let (body, margin_stripped) = strip_margin(body_source);
    Some(ParsedRawTextSigil {
        language,
        body,
        margin_stripped,
    })
}

fn split_language_tag(inner: &str) -> (Option<&'static RawTextEmbeddedLanguage>, &str) {
    let Some(newline_index) = inner.find('\n') else {
        return (None, inner);
    };

    let first_line = &inner[..newline_index];
    let Some(language) = raw_text_language_line(first_line) else {
        return (None, inner);
    };

    (Some(language), &inner[newline_index + 1..])
}

fn strip_margin(body_source: &str) -> (String, bool) {
    let mut candidate = body_source;
    if let Some(rest) = candidate.strip_prefix('\n') {
        candidate = rest;
    }
    if let Some(rest) = candidate.strip_suffix('\n') {
        candidate = rest;
    }

    let lines: Vec<&str> = candidate.split('\n').collect();
    let non_empty: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|line| !line.trim().is_empty())
        .collect();
    if non_empty.is_empty() {
        return (body_source.to_string(), false);
    }

    if !non_empty
        .iter()
        .all(|line| split_margin_line(line).is_some())
    {
        return (body_source.to_string(), false);
    }

    let body = lines
        .into_iter()
        .map(|line| {
            if line.trim().is_empty() {
                ""
            } else {
                split_margin_line(line).unwrap_or(line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    (body, true)
}

fn split_margin_line(line: &str) -> Option<&str> {
    let indent_bytes = line
        .bytes()
        .take_while(|byte| matches!(byte, b' ' | b'\t'))
        .count();
    let after_indent = &line[indent_bytes..];
    let after_pipe = after_indent.strip_prefix('|')?;
    Some(
        after_pipe
            .strip_prefix(' ')
            .or_else(|| after_pipe.strip_prefix('\t'))
            .unwrap_or(after_pipe),
    )
}

#[cfg(test)]
mod tests {
    use super::parse_raw_text_sigil;

    #[test]
    fn keeps_plain_raw_text_verbatim() {
        let parsed = parse_raw_text_sigil("~`line one\nline two`").expect("raw text");
        assert_eq!(parsed.language.map(|lang| lang.tag), None);
        assert_eq!(parsed.body, "line one\nline two");
        assert!(!parsed.margin_stripped);
    }

    #[test]
    fn strips_supported_language_header() {
        let parsed = parse_raw_text_sigil("~`css\nbody`").expect("raw text");
        assert_eq!(parsed.language.map(|lang| lang.tag), Some("css"));
        assert_eq!(parsed.body, "body");
    }

    #[test]
    fn keeps_unknown_first_line_as_text() {
        let parsed = parse_raw_text_sigil("~`hello\nworld`").expect("raw text");
        assert_eq!(parsed.language.map(|lang| lang.tag), None);
        assert_eq!(parsed.body, "hello\nworld");
    }

    #[test]
    fn strips_pipe_margin_and_outer_newlines() {
        let parsed = parse_raw_text_sigil("~`\n    | Hallo\n    | Andreas\n`").expect("raw text");
        assert_eq!(parsed.body, "Hallo\nAndreas");
        assert!(parsed.margin_stripped);
    }
}
