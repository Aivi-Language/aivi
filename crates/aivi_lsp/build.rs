use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum QuickInfoKind {
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
    #[serde(flatten)]
    _extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QuickInfoEntry {
    kind: QuickInfoKind,
    name: String,
    module: Option<String>,
    content: String,
    signature: Option<String>,
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let specs_dir = manifest_dir.join("../../specs");

    // Keep rebuilds correct when specs change.
    println!("cargo:rerun-if-changed={}", specs_dir.display());

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let out_path = out_dir.join("doc_index.json");

    let entries = build_entries_from_specs(&specs_dir).unwrap_or_else(|err| {
        eprintln!("warning: doc index build failed: {err}");
        Vec::new()
    });

    let json = serde_json::to_string_pretty(&entries).expect("serialize doc index");
    fs::write(&out_path, json).expect("write doc_index.json");
}

fn build_entries_from_specs(specs_dir: &Path) -> std::io::Result<Vec<QuickInfoEntry>> {
    let mut entries = Vec::new();
    let mut stack = Vec::new();
    for md_path in list_markdown_files(specs_dir)? {
        let text = fs::read_to_string(md_path)?;
        entries.extend(extract_entries_from_markers(&text, &mut stack));
        stack.clear();
    }
    Ok(entries)
}

fn list_markdown_files(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    fn visit(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.file_name().and_then(|n| n.to_str()) == Some("node_modules") {
                continue;
            }
            if path.is_dir() {
                visit(&path, out)?;
            } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                out.push(path);
            }
        }
        Ok(())
    }

    let mut out = Vec::new();
    visit(root, &mut out)?;
    Ok(out)
}

#[derive(Debug)]
struct OpenMarker {
    metadata: MarkerMetadata,
    content_start: usize,
}

fn extract_entries_from_markers(markdown: &str, stack: &mut Vec<OpenMarker>) -> Vec<QuickInfoEntry> {
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
                    let signature = open
                        .metadata
                        .signature
                        .clone()
                        .or_else(|| extract_signature(&content, open.metadata.extract_signature));
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
        i += 1;
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
        let Some(line_end) = after.find('\n') else {
            return None;
        };
        let info = after[..line_end].trim();
        let code_start = open_at + fence.len() + line_end + 1;
        if info != lang {
            i = code_start;
            continue;
        }
        let rest = &content[code_start..];
        let Some(close_rel) = rest.find("\n```") else {
            return None;
        };
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

