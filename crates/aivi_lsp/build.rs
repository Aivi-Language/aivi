use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::events::Event;
use quick_xml::reader::Reader;
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

    // --- GTK index from GIR files ---
    let gir_dir = manifest_dir.join("../../assets/gir");
    let gtk_index_committed = manifest_dir.join("../../assets/gtk_index.json");
    let gtk_out_path = out_dir.join("gtk_index.json");

    println!("cargo:rerun-if-changed={}", gir_dir.display());
    println!("cargo:rerun-if-changed={}", gtk_index_committed.display());

    if gtk_index_committed.exists() {
        // Use the committed pre-built index.
        fs::copy(&gtk_index_committed, &gtk_out_path).expect("copy gtk_index.json");
    } else if gir_dir.exists() {
        let widgets = build_gtk_index(&gir_dir);
        let json = serde_json::to_string_pretty(&widgets).expect("serialize gtk index");
        fs::write(&gtk_out_path, json).expect("write gtk_index.json");
    } else {
        // No GIR files and no committed index — emit empty index.
        fs::write(&gtk_out_path, "[]").expect("write empty gtk_index.json");
    }
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
                    let signature =
                        open.metadata.signature.clone().or_else(|| {
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

// ---------------------------------------------------------------------------
// GIR → GTK index
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GtkWidgetInfo {
    name: String,
    parent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    doc: Option<String>,
    properties: Vec<GtkPropertyInfo>,
    signals: Vec<GtkSignalInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GtkPropertyInfo {
    name: String,
    #[serde(rename = "type")]
    prop_type: String,
    writable: bool,
    construct_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    doc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GtkSignalInfo {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    doc: Option<String>,
}

fn build_gtk_index(gir_dir: &Path) -> Vec<GtkWidgetInfo> {
    let mut all_widgets: Vec<GtkWidgetInfo> = Vec::new();

    for entry in fs::read_dir(gir_dir).expect("read gir dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("gir") {
            continue;
        }
        let xml = fs::read_to_string(&path).unwrap_or_else(|e| {
            eprintln!("warning: cannot read {}: {e}", path.display());
            String::new()
        });
        if xml.is_empty() {
            continue;
        }

        let ns_prefix = detect_namespace(&xml);
        let widgets = parse_gir_classes(&xml, &ns_prefix);
        all_widgets.extend(widgets);
    }

    // Resolve inheritance: flatten parent properties/signals into children.
    resolve_inheritance(&mut all_widgets);

    // Sort for deterministic output.
    all_widgets.sort_by(|a, b| a.name.cmp(&b.name));
    all_widgets
}

fn detect_namespace(xml: &str) -> String {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"namespace" => {
                let name = e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .find(|a| a.key.as_ref() == b"name")
                    .map(|a| String::from_utf8_lossy(&a.value).to_string())
                    .unwrap_or_default();
                return name;
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    String::new()
}

fn parse_gir_classes(xml: &str, ns_prefix: &str) -> Vec<GtkWidgetInfo> {
    let mut widgets = Vec::new();
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"class" => {
                if let Some(widget) = parse_class(&mut reader, e, ns_prefix) {
                    widgets.push(widget);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                eprintln!("warning: GIR parse error: {e}");
                break;
            }
            _ => {}
        }
        buf.clear();
    }
    widgets
}

fn parse_class(
    reader: &mut Reader<&[u8]>,
    start: &quick_xml::events::BytesStart,
    ns_prefix: &str,
) -> Option<GtkWidgetInfo> {
    let mut class_name = String::new();
    let mut parent = None;

    for attr in start.attributes().filter_map(|a| a.ok()) {
        match attr.key.as_ref() {
            b"name" => class_name = String::from_utf8_lossy(&attr.value).to_string(),
            b"parent" => parent = Some(String::from_utf8_lossy(&attr.value).to_string()),
            _ => {}
        }
    }

    if class_name.is_empty() {
        return None;
    }

    // Prefix with namespace (e.g. "Button" → "GtkButton", "ActionRow" → "AdwActionRow")
    let full_name = format!("{ns_prefix}{class_name}");

    // Also store parent with prefix if it doesn't contain a dot (same namespace).
    let full_parent = parent.map(|p| {
        if p.contains('.') {
            // Cross-namespace like "Gtk.Widget" → "GtkWidget"
            p.replace('.', "")
        } else {
            format!("{ns_prefix}{p}")
        }
    });

    let mut properties = Vec::new();
    let mut signals = Vec::new();
    let mut doc = None;
    let mut buf = Vec::new();
    let mut depth = 1u32;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                depth += 1;
                let tag = e.name();
                match tag.as_ref() {
                    b"property" if depth == 2 => {
                        if let Some(prop) = parse_property(reader, e) {
                            properties.push(prop);
                            // parse_property consumed up to </property>
                            depth -= 1;
                        }
                    }
                    b"glib:signal" if depth == 2 => {
                        if let Some(sig) = parse_signal(reader, e) {
                            signals.push(sig);
                            depth -= 1;
                        }
                    }
                    b"doc" if depth == 2 && doc.is_none() => {
                        doc = read_doc_text(reader);
                        depth -= 1;
                    }
                    _ => {}
                }
            }
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Ok(Event::Empty(ref e)) => {
                let tag = e.name();
                if tag.as_ref() == b"property" && depth == 1 {
                    if let Some(prop) = parse_property_from_empty(e) {
                        properties.push(prop);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    // Truncate docs to first sentence for index compactness.
    let doc = doc.map(|d| truncate_doc(&d));

    Some(GtkWidgetInfo {
        name: full_name,
        parent: full_parent,
        doc,
        properties,
        signals,
    })
}

fn parse_property(
    reader: &mut Reader<&[u8]>,
    start: &quick_xml::events::BytesStart,
) -> Option<GtkPropertyInfo> {
    let mut name = String::new();
    let mut writable = false;
    let mut construct_only = false;
    let mut default_value = None;

    for attr in start.attributes().filter_map(|a| a.ok()) {
        match attr.key.as_ref() {
            b"name" => name = String::from_utf8_lossy(&attr.value).to_string(),
            b"writable" => writable = attr.value.as_ref() == b"1",
            b"construct-only" => construct_only = attr.value.as_ref() == b"1",
            b"default-value" => {
                default_value = Some(String::from_utf8_lossy(&attr.value).to_string())
            }
            _ => {}
        }
    }

    if name.is_empty() {
        skip_to_end(reader, b"property");
        return None;
    }

    let mut prop_type = String::from("unknown");
    let mut doc = None;
    let mut buf = Vec::new();
    let mut depth = 1u32;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                depth += 1;
                match e.name().as_ref() {
                    b"type" if depth == 2 => {
                        for attr in e.attributes().filter_map(|a| a.ok()) {
                            if attr.key.as_ref() == b"name" {
                                prop_type =
                                    String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    }
                    b"doc" if depth == 2 && doc.is_none() => {
                        doc = read_doc_text(reader);
                        depth -= 1;
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) if e.name().as_ref() == b"type" && depth == 1 => {
                for attr in e.attributes().filter_map(|a| a.ok()) {
                    if attr.key.as_ref() == b"name" {
                        prop_type = String::from_utf8_lossy(&attr.value).to_string();
                    }
                }
            }
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    let doc = doc.map(|d| truncate_doc(&d));

    Some(GtkPropertyInfo {
        name,
        prop_type,
        writable,
        construct_only,
        default_value,
        doc,
    })
}

fn parse_property_from_empty(e: &quick_xml::events::BytesStart) -> Option<GtkPropertyInfo> {
    let mut name = String::new();
    let mut writable = false;
    let mut construct_only = false;
    let mut default_value = None;
    let mut prop_type = String::from("unknown");

    for attr in e.attributes().filter_map(|a| a.ok()) {
        match attr.key.as_ref() {
            b"name" => name = String::from_utf8_lossy(&attr.value).to_string(),
            b"writable" => writable = attr.value.as_ref() == b"1",
            b"construct-only" => construct_only = attr.value.as_ref() == b"1",
            b"default-value" => {
                default_value = Some(String::from_utf8_lossy(&attr.value).to_string())
            }
            _ => {}
        }
    }

    if name.is_empty() {
        return None;
    }

    // Empty property elements don't have child <type>, but sometimes have c:type
    for attr in e.attributes().filter_map(|a| a.ok()) {
        if attr.key.as_ref() == b"c:type" {
            prop_type = String::from_utf8_lossy(&attr.value).to_string();
        }
    }

    Some(GtkPropertyInfo {
        name,
        prop_type,
        writable,
        construct_only,
        default_value,
        doc: None,
    })
}

fn parse_signal(
    reader: &mut Reader<&[u8]>,
    start: &quick_xml::events::BytesStart,
) -> Option<GtkSignalInfo> {
    let mut name = String::new();

    for attr in start.attributes().filter_map(|a| a.ok()) {
        if attr.key.as_ref() == b"name" {
            name = String::from_utf8_lossy(&attr.value).to_string();
        }
    }

    let mut doc = None;
    let mut buf = Vec::new();
    let mut depth = 1u32;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                depth += 1;
                if e.name().as_ref() == b"doc" && depth == 2 && doc.is_none() {
                    doc = read_doc_text(reader);
                    depth -= 1;
                }
            }
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    if name.is_empty() {
        return None;
    }

    let doc = doc.map(|d| truncate_doc(&d));
    Some(GtkSignalInfo { name, doc })
}

fn read_doc_text(reader: &mut Reader<&[u8]>) -> Option<String> {
    let mut text = String::new();
    let mut buf = Vec::new();
    let mut depth = 1u32;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(ref e)) => {
                text.push_str(&e.unescape().unwrap_or_default());
            }
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn skip_to_end(reader: &mut Reader<&[u8]>, tag: &[u8]) {
    let mut buf = Vec::new();
    let mut depth = 1u32;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if e.name().as_ref() == tag => depth += 1,
            Ok(Event::End(ref e)) if e.name().as_ref() == tag => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
}

fn truncate_doc(doc: &str) -> String {
    // Take first sentence (up to first period followed by space/newline, or first newline).
    let trimmed = doc.trim();
    if let Some(pos) = trimmed.find(". ") {
        return trimmed[..pos + 1].to_string();
    }
    if let Some(pos) = trimmed.find(".\n") {
        return trimmed[..pos + 1].to_string();
    }
    // Limit to ~200 chars.
    if trimmed.len() > 200 {
        let end = trimmed[..200].rfind(' ').unwrap_or(200);
        return format!("{}…", &trimmed[..end]);
    }
    trimmed.to_string()
}

fn resolve_inheritance(widgets: &mut Vec<GtkWidgetInfo>) {
    // Build a name→index map.
    let index: HashMap<String, usize> = widgets
        .iter()
        .enumerate()
        .map(|(i, w)| (w.name.clone(), i))
        .collect();

    // For each widget, collect ancestor property/signal names, then merge missing ones.
    // We iterate in multiple passes to handle multi-level inheritance.
    for _ in 0..10 {
        let mut changed = false;
        // Clone current state for reading while we mutate.
        let snapshot: Vec<GtkWidgetInfo> = widgets.clone();
        for widget in widgets.iter_mut() {
            if let Some(ref parent_name) = widget.parent {
                if let Some(&pi) = index.get(parent_name) {
                    let parent = &snapshot[pi];
                    let existing_props: std::collections::HashSet<String> =
                        widget.properties.iter().map(|p| p.name.clone()).collect();
                    for prop in &parent.properties {
                        if !existing_props.contains(&prop.name) {
                            widget.properties.push(prop.clone());
                            changed = true;
                        }
                    }
                    let existing_sigs: std::collections::HashSet<String> =
                        widget.signals.iter().map(|s| s.name.clone()).collect();
                    for sig in &parent.signals {
                        if !existing_sigs.contains(&sig.name) {
                            widget.signals.push(sig.clone());
                            changed = true;
                        }
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
}
