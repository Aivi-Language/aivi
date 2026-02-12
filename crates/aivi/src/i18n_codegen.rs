use std::collections::{BTreeMap, HashSet};

use crate::i18n::{
    escape_sigil_string_body, parse_locale_tag, parse_message_template, validate_key_text,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertiesEntry {
    pub key: String,
    pub message: String,
}

pub fn parse_properties_catalog(text: &str) -> Result<Vec<PropertiesEntry>, String> {
    let mut entries = Vec::new();
    for (idx, raw_line) in text.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (k, v) =
            split_kv(line).ok_or_else(|| format!("line {line_no}: expected 'key = value'"))?;
        let key = k.trim().to_string();
        let message =
            unescape_properties_value(v.trim()).map_err(|msg| format!("line {line_no}: {msg}"))?;

        validate_key_text(&key).map_err(|msg| format!("line {line_no}: {msg}"))?;
        parse_message_template(&message).map_err(|msg| format!("line {line_no}: {msg}"))?;

        entries.push(PropertiesEntry { key, message });
    }
    Ok(entries)
}

pub fn generate_i18n_module_from_properties(
    module_name: &str,
    locale_tag: &str,
    properties_text: &str,
) -> Result<String, String> {
    let locale = parse_locale_tag(locale_tag)?;
    let entries = parse_properties_catalog(properties_text)?;

    // Sort keys for stable output.
    let mut by_key: BTreeMap<String, String> = BTreeMap::new();
    for entry in entries {
        by_key.insert(entry.key, entry.message);
    }

    let mut ctor_names = Vec::new();
    let mut used = HashSet::new();
    let mut key_to_ctor = BTreeMap::new();
    for key in by_key.keys() {
        let base = key_to_ctor_base(key);
        let mut name = base.clone();
        let mut i = 2usize;
        while !used.insert(name.clone()) {
            name = format!("{base}{i}");
            i += 1;
        }
        key_to_ctor.insert(key.clone(), name.clone());
        ctor_names.push(name);
    }

    let mut out = String::new();
    out.push_str("@no_prelude\n");
    out.push_str(&format!("module {module_name}\n"));
    out.push_str("export KeyId\n");
    out.push_str("export keyText, key\n");
    out.push_str("export locale, bundle\n");
    out.push_str("export t\n\n");
    out.push_str("use aivi\n");
    out.push('\n');

    out.push_str("type KeyId = ");
    out.push_str(&ctor_names.join(" | "));
    out.push('\n');
    out.push('\n');

    out.push_str("keyText : KeyId -> Text\n");
    out.push_str("keyText k = k ?\n");
    for (key, ctor) in &key_to_ctor {
        out.push_str(&format!("  | {ctor} => \"{key}\"\n"));
    }
    out.push('\n');

    out.push_str("key : KeyId -> { tag: Text, body: Text, flags: Text }\n");
    out.push_str("key k = { tag: \"k\", body: keyText k, flags: \"\" }\n\n");

    out.push_str(
        "locale : { language: Text, region: Option Text, variants: List Text, tag: Text }\n",
    );
    out.push_str("locale = {\n");
    out.push_str(&format!("  language: \"{}\"\n", locale.language));
    out.push_str("  region: ");
    match &locale.region {
        Some(region) => out.push_str(&format!("Some \"{}\"\n", region)),
        None => out.push_str("None\n"),
    }
    out.push_str("  variants: [");
    if !locale.variants.is_empty() {
        out.push_str(
            &locale
                .variants
                .iter()
                .map(|v| format!("\"{}\"", v))
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    out.push_str("]\n");
    out.push_str(&format!("  tag: \"{}\"\n", locale.tag));
    out.push_str("}\n\n");

    out.push_str("bundle : { locale: { language: Text, region: Option Text, variants: List Text, tag: Text }, entries: Map Text { tag: Text, body: Text, flags: Text } }\n");
    out.push_str("bundle = {\n");
    out.push_str("  locale: locale\n");
    out.push_str("  entries: Map.fromList [\n");
    for (key, message) in &by_key {
        let escaped = escape_sigil_string_body(message);
        out.push_str(&format!("    (\"{key}\", ~m\"{escaped}\")\n"));
    }
    out.push_str("  ]\n");
    out.push_str("}\n\n");

    out.push_str("t : KeyId -> {} -> Text\n");
    out.push_str("t k args =\n");
    out.push_str("  Map.get (keyText k) bundle.entries ?\n");
    out.push_str("    | None => keyText k\n");
    out.push_str("    | Some msg =>\n");
    out.push_str("      (i18n.render msg args) ?\n");
    out.push_str("        | Ok txt => txt\n");
    out.push_str("        | Err _  => keyText k\n");

    Ok(out)
}

fn key_to_ctor_base(key: &str) -> String {
    let mut out = String::new();
    for seg in key.split('.') {
        for chunk in seg.split(|c| c == '-' || c == '_') {
            if chunk.is_empty() {
                continue;
            }
            let mut chars = chunk.chars().filter(|c| c.is_ascii_alphanumeric());
            let Some(first) = chars.next() else { continue };
            out.push(first.to_ascii_uppercase());
            for ch in chars {
                out.push(ch.to_ascii_lowercase());
            }
        }
    }
    if out.is_empty() {
        "Key".to_string()
    } else if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("K{out}")
    } else {
        out
    }
}

fn split_kv(line: &str) -> Option<(&str, &str)> {
    if let Some((k, v)) = line.split_once('=') {
        return Some((k, v));
    }
    line.split_once(':')
}

fn unescape_properties_value(value: &str) -> Result<String, String> {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        let Some(next) = chars.next() else {
            return Err("dangling escape at end of line".to_string());
        };
        match next {
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            other => out.push(other),
        }
    }
    Ok(out)
}
