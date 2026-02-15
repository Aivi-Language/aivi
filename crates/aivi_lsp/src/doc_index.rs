use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub const DOC_INDEX_JSON: &str = include_str!(concat!(env!("OUT_DIR"), "/doc_index.json"));

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QuickInfoKind {
    Module,
    Function,
    Type,
    Class,
    Domain,
    Operator,
    ClassMember,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickInfoEntry {
    pub kind: QuickInfoKind,
    pub name: String,
    pub module: Option<String>,
    pub content: String,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocIndex {
    entries: Vec<QuickInfoEntry>,
    #[serde(skip)]
    by_name: HashMap<String, Vec<usize>>,
}

impl DocIndex {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let entries: Vec<QuickInfoEntry> = serde_json::from_str(json)?;
        let mut index = DocIndex {
            entries,
            ..Default::default()
        };
        index.rebuild_maps();
        Ok(index)
    }

    pub fn lookup_best(&self, name: &str, module: Option<&str>) -> Option<&QuickInfoEntry> {
        let candidates = self.by_name.get(name)?;
        if let Some(module) = module {
            for i in candidates {
                let entry = self.entries.get(*i)?;
                if entry.module.as_deref() == Some(module) {
                    return Some(entry);
                }
            }
        }
        // Avoid incorrect docs when multiple modules export the same name.
        if candidates.len() == 1 {
            candidates.first().and_then(|i| self.entries.get(*i))
        } else {
            None
        }
    }

    fn rebuild_maps(&mut self) {
        self.by_name.clear();
        for (i, entry) in self.entries.iter().enumerate() {
            self.by_name.entry(entry.name.clone()).or_default().push(i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct MarkerMetadata {
        kind: QuickInfoKind,
        name: String,
        #[serde(default)]
        module: Option<String>,
        #[serde(default)]
        signature: Option<String>,
        #[serde(default)]
        extract_signature: Option<bool>,
        // Allow future keys without breaking parsing.
        #[serde(flatten)]
        _extra: HashMap<String, serde_json::Value>,
    }

    #[derive(Debug)]
    struct OpenMarker {
        metadata: MarkerMetadata,
        content_start: usize,
    }

    fn extract_entries_from_markers(
        markdown: &str,
        stack: &mut Vec<OpenMarker>,
    ) -> Vec<QuickInfoEntry> {
        const OPEN: &str = "<!-- quick-info:";
        const CLOSE: &str = "<!-- /quick-info -->";

        let mut entries = Vec::new();
        let mut i = 0usize;
        while i < markdown.len() {
            let rest = &markdown[i..];
            if rest.starts_with(OPEN) {
                if let Some(end) = rest.find("-->") {
                    let header = &rest[..end];
                    let json = header.strip_prefix(OPEN).unwrap_or("").trim();
                    if let Ok(metadata) = serde_json::from_str::<MarkerMetadata>(json) {
                        let content_start = i + end + "-->".len();
                        stack.push(OpenMarker {
                            metadata,
                            content_start,
                        });
                    }
                    i += end + "-->".len();
                    continue;
                }
            } else if rest.starts_with(CLOSE) {
                if let Some(open) = stack.pop() {
                    let raw = markdown[open.content_start..i].trim();
                    let content = strip_marker_comments(raw).trim().to_string();
                    if !content.is_empty() {
                        let signature = open.metadata.signature.clone().or_else(|| {
                            extract_signature(&content, open.metadata.extract_signature)
                        });
                        entries.push(QuickInfoEntry {
                            kind: open.metadata.kind,
                            name: open.metadata.name,
                            module: open.metadata.module,
                            content,
                            signature,
                        });
                    }
                }
                i += CLOSE.len();
                continue;
            }

            let ch = rest.chars().next().unwrap();
            i += ch.len_utf8();
        }
        entries
    }

    fn strip_marker_comments(input: &str) -> String {
        const OPEN: &str = "<!-- quick-info:";
        const CLOSE: &str = "<!-- /quick-info -->";

        let mut out = String::with_capacity(input.len());
        let mut i = 0usize;
        while i < input.len() {
            let rest = &input[i..];
            if rest.starts_with(OPEN) {
                if let Some(end) = rest.find("-->") {
                    i += end + "-->".len();
                    continue;
                }
            }
            if rest.starts_with(CLOSE) {
                i += CLOSE.len();
                continue;
            }
            let ch = rest.chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
        out
    }

    fn extract_signature(content: &str, extract_signature: Option<bool>) -> Option<String> {
        if extract_signature == Some(false) {
            return None;
        }

        if let Some(block) = extract_fenced_block(content, "aivi") {
            let block = block.trim();
            if !block.is_empty() {
                return Some(block.to_string());
            }
        }

        for span in extract_inline_code_spans(content) {
            let span = span.trim();
            if span.contains("->") || span.contains(':') {
                return Some(span.to_string());
            }
        }

        None
    }

    fn extract_fenced_block(content: &str, lang: &str) -> Option<String> {
        let fence = "```";
        let mut i = 0usize;
        while let Some(open_at) = content[i..].find(fence) {
            let open_at = i + open_at;
            let after = &content[open_at + fence.len()..];
            let line_end = after.find('\n')?;
            let info = after[..line_end].trim();
            let code_start = open_at + fence.len() + line_end + 1;
            if info != lang {
                i = code_start;
                continue;
            }
            let rest = &content[code_start..];
            let close_rel = rest.find("\n```")?;
            return Some(rest[..close_rel].to_string());
        }
        None
    }

    fn extract_inline_code_spans(content: &str) -> Vec<String> {
        let mut spans = Vec::new();
        let mut i = 0usize;
        while let Some(open) = content[i..].find('`') {
            let open = i + open;
            let rest = &content[open + 1..];
            let Some(close) = rest.find('`') else {
                break;
            };
            spans.push(rest[..close].to_string());
            i = open + 1 + close + 1;
        }
        spans
    }

    #[test]
    fn extract_wrapping_marker_simple() {
        let md = r#"
<!-- quick-info: {"kind":"module","name":"aivi.text"} -->
Core string utilities.
<!-- /quick-info -->
"#;
        let mut stack = Vec::new();
        let entries = extract_entries_from_markers(md, &mut stack);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "aivi.text");
        assert_eq!(entries[0].content, "Core string utilities.");
    }

    #[test]
    fn extract_signature_from_code_block() {
        let content = r#"
```aivi
length : Text -> Int
```
Returns string length.
"#;
        let sig = extract_signature(content, None);
        assert_eq!(sig, Some("length : Text -> Int".to_string()));
    }

    #[test]
    fn extract_table_cell_marker_content() {
        let md = r#"
| `isAlnum` | <!-- quick-info: {"kind":"function","name":"isAlnum"} -->Returns whether char is alphanumeric.<!-- /quick-info --> |
"#;
        let mut stack = Vec::new();
        let entries = extract_entries_from_markers(md, &mut stack);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].content, "Returns whether char is alphanumeric.");
    }

    #[test]
    fn nested_markers_keep_wrapped_text_but_drop_comments() {
        let md = r#"
<!-- quick-info: {"kind":"class","name":"Functor","module":"aivi.logic"} -->
Functors support mapping.
<!-- quick-info: {"kind":"class-member","name":"map","module":"aivi.logic"} -->
Map a function.
<!-- /quick-info -->
Done.
<!-- /quick-info -->
"#;
        let mut stack = Vec::new();
        let entries = extract_entries_from_markers(md, &mut stack);
        assert_eq!(entries.len(), 2);

        let outer = entries.iter().find(|e| e.name == "Functor").unwrap();
        assert!(outer.content.contains("Functors support mapping."));
        assert!(outer.content.contains("Map a function."));
        assert!(outer.content.contains("Done."));
        assert!(!outer.content.contains("quick-info:"));
    }

    #[test]
    fn built_doc_index_includes_match_and_pipe_quick_info() {
        // Regression guard: core syntax operators should be indexed for hover/quick-info.
        let index = DocIndex::from_json(DOC_INDEX_JSON).expect("doc index JSON parses");
        assert!(
            index.lookup_best("|>", None).is_some(),
            "expected `|>` quick-info entry"
        );
        assert!(
            index.lookup_best("?", None).is_some(),
            "expected `?` quick-info entry"
        );
    }
}
