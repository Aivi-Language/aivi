use std::sync::Arc;

use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

use super::util::{
    builtin, expect_bytes, expect_char, expect_int, expect_list, expect_text, list_value, make_err,
    make_none, make_ok, make_some,
};
use crate::runtime::{format_value, RuntimeError, Value};

pub(super) fn build_text_record() -> Value {
    let mut fields = std::collections::HashMap::new();
    fields.insert(
        "length".to_string(),
        builtin("text.length", 1, |mut args, _| {
            let text = expect_text(args.remove(0), "text.length")?;
            Ok(Value::Int(char_len(&text) as i64))
        }),
    );
    fields.insert(
        "isEmpty".to_string(),
        builtin("text.isEmpty", 1, |mut args, _| {
            let text = expect_text(args.remove(0), "text.isEmpty")?;
            Ok(Value::Bool(char_len(&text) == 0))
        }),
    );
    fields.insert(
        "isDigit".to_string(),
        builtin("text.isDigit", 1, |mut args, _| {
            let value = expect_char(args.remove(0), "text.isDigit")?;
            Ok(Value::Bool(value.is_ascii_digit()))
        }),
    );
    fields.insert(
        "isAlpha".to_string(),
        builtin("text.isAlpha", 1, |mut args, _| {
            let value = expect_char(args.remove(0), "text.isAlpha")?;
            Ok(Value::Bool(value.is_alphabetic()))
        }),
    );
    fields.insert(
        "isAlnum".to_string(),
        builtin("text.isAlnum", 1, |mut args, _| {
            let value = expect_char(args.remove(0), "text.isAlnum")?;
            Ok(Value::Bool(value.is_alphanumeric()))
        }),
    );
    fields.insert(
        "isSpace".to_string(),
        builtin("text.isSpace", 1, |mut args, _| {
            let value = expect_char(args.remove(0), "text.isSpace")?;
            Ok(Value::Bool(value.is_whitespace()))
        }),
    );
    fields.insert(
        "isUpper".to_string(),
        builtin("text.isUpper", 1, |mut args, _| {
            let value = expect_char(args.remove(0), "text.isUpper")?;
            Ok(Value::Bool(value.is_uppercase()))
        }),
    );
    fields.insert(
        "isLower".to_string(),
        builtin("text.isLower", 1, |mut args, _| {
            let value = expect_char(args.remove(0), "text.isLower")?;
            Ok(Value::Bool(value.is_lowercase()))
        }),
    );
    fields.insert(
        "contains".to_string(),
        builtin("text.contains", 2, |mut args, _| {
            let needle = expect_text(args.pop().unwrap(), "text.contains")?;
            let haystack = expect_text(args.pop().unwrap(), "text.contains")?;
            Ok(Value::Bool(haystack.contains(&needle)))
        }),
    );
    fields.insert(
        "startsWith".to_string(),
        builtin("text.startsWith", 2, |mut args, _| {
            let prefix = expect_text(args.pop().unwrap(), "text.startsWith")?;
            let value = expect_text(args.pop().unwrap(), "text.startsWith")?;
            Ok(Value::Bool(value.starts_with(&prefix)))
        }),
    );
    fields.insert(
        "endsWith".to_string(),
        builtin("text.endsWith", 2, |mut args, _| {
            let suffix = expect_text(args.pop().unwrap(), "text.endsWith")?;
            let value = expect_text(args.pop().unwrap(), "text.endsWith")?;
            Ok(Value::Bool(value.ends_with(&suffix)))
        }),
    );
    fields.insert(
        "indexOf".to_string(),
        builtin("text.indexOf", 2, |mut args, _| {
            let needle = expect_text(args.pop().unwrap(), "text.indexOf")?;
            let haystack = expect_text(args.pop().unwrap(), "text.indexOf")?;
            Ok(match haystack.find(&needle) {
                Some(value) => make_some(Value::Int(value as i64)),
                None => make_none(),
            })
        }),
    );
    fields.insert(
        "lastIndexOf".to_string(),
        builtin("text.lastIndexOf", 2, |mut args, _| {
            let needle = expect_text(args.pop().unwrap(), "text.lastIndexOf")?;
            let haystack = expect_text(args.pop().unwrap(), "text.lastIndexOf")?;
            Ok(match haystack.rfind(&needle) {
                Some(value) => make_some(Value::Int(value as i64)),
                None => make_none(),
            })
        }),
    );
    fields.insert(
        "count".to_string(),
        builtin("text.count", 2, |mut args, _| {
            let needle = expect_text(args.pop().unwrap(), "text.count")?;
            let haystack = expect_text(args.pop().unwrap(), "text.count")?;
            Ok(Value::Int(haystack.matches(&needle).count() as i64))
        }),
    );
    fields.insert(
        "compare".to_string(),
        builtin("text.compare", 2, |mut args, _| {
            let right = expect_text(args.pop().unwrap(), "text.compare")?;
            let left = expect_text(args.pop().unwrap(), "text.compare")?;
            Ok(Value::Int(left.cmp(&right) as i64))
        }),
    );
    fields.insert(
        "slice".to_string(),
        builtin("text.slice", 3, |mut args, _| {
            let end = expect_int(args.pop().unwrap(), "text.slice")?;
            let start = expect_int(args.pop().unwrap(), "text.slice")?;
            let text = expect_text(args.pop().unwrap(), "text.slice")?;
            Ok(Value::Text(slice_chars(&text, start, end)))
        }),
    );
    fields.insert(
        "split".to_string(),
        builtin("text.split", 2, |mut args, _| {
            let sep = expect_text(args.pop().unwrap(), "text.split")?;
            let text = expect_text(args.pop().unwrap(), "text.split")?;
            let parts = text
                .split(&sep)
                .map(|part| Value::Text(part.to_string()))
                .collect();
            Ok(list_value(parts))
        }),
    );
    fields.insert(
        "splitLines".to_string(),
        builtin("text.splitLines", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.splitLines")?;
            let parts = text
                .lines()
                .map(|part| Value::Text(part.to_string()))
                .collect();
            Ok(list_value(parts))
        }),
    );
    fields.insert(
        "chunk".to_string(),
        builtin("text.chunk", 2, |mut args, _| {
            let size = expect_int(args.pop().unwrap(), "text.chunk")? as usize;
            let text = expect_text(args.pop().unwrap(), "text.chunk")?;
            if size == 0 {
                return Ok(list_value(Vec::new()));
            }
            let mut out = Vec::new();
            let mut chunk = String::new();
            for ch in text.chars() {
                chunk.push(ch);
                if chunk.chars().count() == size {
                    out.push(Value::Text(chunk));
                    chunk = String::new();
                }
            }
            if !chunk.is_empty() {
                out.push(Value::Text(chunk));
            }
            Ok(list_value(out))
        }),
    );
    fields.insert(
        "trim".to_string(),
        builtin("text.trim", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.trim")?;
            Ok(Value::Text(text.trim().to_string()))
        }),
    );
    fields.insert(
        "trimStart".to_string(),
        builtin("text.trimStart", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.trimStart")?;
            Ok(Value::Text(text.trim_start().to_string()))
        }),
    );
    fields.insert(
        "trimEnd".to_string(),
        builtin("text.trimEnd", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.trimEnd")?;
            Ok(Value::Text(text.trim_end().to_string()))
        }),
    );
    fields.insert(
        "padStart".to_string(),
        builtin("text.padStart", 3, |mut args, _| {
            let fill = expect_text(args.pop().unwrap(), "text.padStart")?;
            let width = expect_int(args.pop().unwrap(), "text.padStart")? as usize;
            let text = expect_text(args.pop().unwrap(), "text.padStart")?;
            Ok(Value::Text(pad_text(&text, width, &fill, true)))
        }),
    );
    fields.insert(
        "padEnd".to_string(),
        builtin("text.padEnd", 3, |mut args, _| {
            let fill = expect_text(args.pop().unwrap(), "text.padEnd")?;
            let width = expect_int(args.pop().unwrap(), "text.padEnd")? as usize;
            let text = expect_text(args.pop().unwrap(), "text.padEnd")?;
            Ok(Value::Text(pad_text(&text, width, &fill, false)))
        }),
    );
    fields.insert(
        "replace".to_string(),
        builtin("text.replace", 3, |mut args, _| {
            let replacement = expect_text(args.pop().unwrap(), "text.replace")?;
            let needle = expect_text(args.pop().unwrap(), "text.replace")?;
            let value = expect_text(args.pop().unwrap(), "text.replace")?;
            Ok(Value::Text(value.replacen(&needle, &replacement, 1)))
        }),
    );
    fields.insert(
        "replaceAll".to_string(),
        builtin("text.replaceAll", 3, |mut args, _| {
            let replacement = expect_text(args.pop().unwrap(), "text.replaceAll")?;
            let needle = expect_text(args.pop().unwrap(), "text.replaceAll")?;
            let value = expect_text(args.pop().unwrap(), "text.replaceAll")?;
            Ok(Value::Text(value.replace(&needle, &replacement)))
        }),
    );
    fields.insert(
        "remove".to_string(),
        builtin("text.remove", 2, |mut args, _| {
            let needle = expect_text(args.pop().unwrap(), "text.remove")?;
            let value = expect_text(args.pop().unwrap(), "text.remove")?;
            Ok(Value::Text(value.replace(&needle, "")))
        }),
    );
    fields.insert(
        "repeat".to_string(),
        builtin("text.repeat", 2, |mut args, _| {
            let count = expect_int(args.pop().unwrap(), "text.repeat")? as usize;
            let value = expect_text(args.pop().unwrap(), "text.repeat")?;
            Ok(Value::Text(value.repeat(count)))
        }),
    );
    fields.insert(
        "reverse".to_string(),
        builtin("text.reverse", 1, |mut args, _| {
            let value = expect_text(args.pop().unwrap(), "text.reverse")?;
            Ok(Value::Text(value.chars().rev().collect()))
        }),
    );
    fields.insert(
        "concat".to_string(),
        builtin("text.concat", 1, |mut args, _| {
            let list = expect_list(args.pop().unwrap(), "text.concat")?;
            let mut out = String::new();
            for value in list.iter() {
                match value {
                    Value::Text(text) => out.push_str(text),
                    _ => {
                        return Err(RuntimeError::Message(
                            "text.concat expects List Text".to_string(),
                        ))
                    }
                }
            }
            Ok(Value::Text(out))
        }),
    );
    fields.insert(
        "toLower".to_string(),
        builtin("text.toLower", 1, |mut args, _| {
            let value = expect_text(args.pop().unwrap(), "text.toLower")?;
            Ok(Value::Text(value.to_lowercase()))
        }),
    );
    fields.insert(
        "toUpper".to_string(),
        builtin("text.toUpper", 1, |mut args, _| {
            let value = expect_text(args.pop().unwrap(), "text.toUpper")?;
            Ok(Value::Text(value.to_uppercase()))
        }),
    );
    fields.insert(
        "capitalize".to_string(),
        builtin("text.capitalize", 1, |mut args, _| {
            let value = expect_text(args.pop().unwrap(), "text.capitalize")?;
            Ok(Value::Text(capitalize_segment(&value)))
        }),
    );
    fields.insert(
        "titleCase".to_string(),
        builtin("text.titleCase", 1, |mut args, _| {
            let value = expect_text(args.pop().unwrap(), "text.titleCase")?;
            let mut out = String::new();
            for segment in value.split_whitespace() {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(&capitalize_segment(segment));
            }
            Ok(Value::Text(out))
        }),
    );
    fields.insert(
        "caseFold".to_string(),
        builtin("text.caseFold", 1, |mut args, _| {
            let value = expect_text(args.pop().unwrap(), "text.caseFold")?;
            let out: String = value.chars().flat_map(|c| c.to_lowercase()).collect();
            Ok(Value::Text(out))
        }),
    );
    fields.insert(
        "normalizeNFC".to_string(),
        builtin("text.normalizeNFC", 1, |mut args, _| {
            let value = expect_text(args.pop().unwrap(), "text.normalizeNFC")?;
            Ok(Value::Text(value.nfc().collect()))
        }),
    );
    fields.insert(
        "normalizeNFD".to_string(),
        builtin("text.normalizeNFD", 1, |mut args, _| {
            let value = expect_text(args.pop().unwrap(), "text.normalizeNFD")?;
            Ok(Value::Text(value.nfd().collect()))
        }),
    );
    fields.insert(
        "normalizeNFKC".to_string(),
        builtin("text.normalizeNFKC", 1, |mut args, _| {
            let value = expect_text(args.pop().unwrap(), "text.normalizeNFKC")?;
            Ok(Value::Text(value.nfkc().collect()))
        }),
    );
    fields.insert(
        "normalizeNFKD".to_string(),
        builtin("text.normalizeNFKD", 1, |mut args, _| {
            let value = expect_text(args.pop().unwrap(), "text.normalizeNFKD")?;
            Ok(Value::Text(value.nfkd().collect()))
        }),
    );
    fields.insert(
        "toBytes".to_string(),
        builtin("text.toBytes", 2, |mut args, _| {
            let value = expect_text(args.pop().unwrap(), "text.toBytes")?;
            let encoding = args.pop().unwrap();
            let encoding = encoding_kind(&encoding)
                .ok_or_else(|| RuntimeError::Message("text expects Encoding".to_string()))?;
            let bytes = encode_text(encoding, &value);
            Ok(Value::Bytes(Arc::new(bytes)))
        }),
    );
    fields.insert(
        "fromBytes".to_string(),
        builtin("text.fromBytes", 2, |mut args, _| {
            let bytes = expect_bytes(args.pop().unwrap(), "text.fromBytes")?;
            let encoding = args.pop().unwrap();
            let encoding = encoding_kind(&encoding)
                .ok_or_else(|| RuntimeError::Message("text expects Encoding".to_string()))?;
            match decode_bytes(encoding, &bytes) {
                Ok(text) => Ok(make_ok(Value::Text(text))),
                Err(_) => Ok(make_err(Value::Text("InvalidEncoding".to_string()))),
            }
        }),
    );
    fields.insert(
        "toText".to_string(),
        builtin("text.toText", 1, |mut args, _| {
            let value = args.pop().unwrap();
            Ok(Value::Text(format_value(&value)))
        }),
    );
    fields.insert(
        "parseInt".to_string(),
        builtin("text.parseInt", 1, |mut args, _| {
            let value = expect_text(args.pop().unwrap(), "text.parseInt")?;
            match value.parse::<i64>() {
                Ok(int) => Ok(make_some(Value::Int(int))),
                Err(_) => Ok(make_none()),
            }
        }),
    );
    fields.insert(
        "parseFloat".to_string(),
        builtin("text.parseFloat", 1, |mut args, _| {
            let value = expect_text(args.pop().unwrap(), "text.parseFloat")?;
            match value.parse::<f64>() {
                Ok(value) => Ok(make_some(Value::Float(value))),
                Err(_) => Ok(make_none()),
            }
        }),
    );
    Value::Record(Arc::new(fields))
}

#[derive(Clone, Copy)]
enum EncodingKind {
    Utf8,
    Utf16,
    Utf32,
    Latin1,
}

fn encoding_kind(value: &Value) -> Option<EncodingKind> {
    match value {
        Value::Constructor { name, args } if args.is_empty() => match name.as_str() {
            "Utf8" => Some(EncodingKind::Utf8),
            "Utf16" => Some(EncodingKind::Utf16),
            "Utf32" => Some(EncodingKind::Utf32),
            "Latin1" => Some(EncodingKind::Latin1),
            _ => None,
        },
        _ => None,
    }
}

fn encode_text(encoding: EncodingKind, text: &str) -> Vec<u8> {
    match encoding {
        EncodingKind::Utf8 => text.as_bytes().to_vec(),
        EncodingKind::Latin1 => text
            .chars()
            .map(|ch| if (ch as u32) <= 0xFF { ch as u8 } else { b'?' })
            .collect(),
        EncodingKind::Utf16 => text
            .encode_utf16()
            .flat_map(|unit| unit.to_le_bytes())
            .collect(),
        EncodingKind::Utf32 => text
            .chars()
            .flat_map(|ch| (ch as u32).to_le_bytes())
            .collect(),
    }
}

fn decode_bytes(encoding: EncodingKind, bytes: &[u8]) -> Result<String, ()> {
    match encoding {
        EncodingKind::Utf8 => String::from_utf8(bytes.to_vec()).map_err(|_| ()),
        EncodingKind::Latin1 => Ok(bytes.iter().map(|b| char::from(*b)).collect()),
        EncodingKind::Utf16 => {
            if !bytes.len().is_multiple_of(2) {
                return Err(());
            }
            let units = bytes
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect::<Vec<_>>();
            String::from_utf16(&units).map_err(|_| ())
        }
        EncodingKind::Utf32 => {
            if !bytes.len().is_multiple_of(4) {
                return Err(());
            }
            let mut out = String::new();
            for chunk in bytes.chunks_exact(4) {
                let value = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                let ch = char::from_u32(value).ok_or(())?;
                out.push(ch);
            }
            Ok(out)
        }
    }
}

fn slice_chars(text: &str, start: i64, end: i64) -> String {
    let len = char_len(text) as i64;
    let mut start = start;
    let mut end = end;
    if start < 0 {
        start += len;
    }
    if end < 0 {
        end += len;
    }
    if start < 0 {
        start = 0;
    }
    if end > len {
        end = len;
    }
    if start >= end {
        return String::new();
    }
    text.chars()
        .skip(start as usize)
        .take((end - start) as usize)
        .collect()
}

fn pad_text(text: &str, width: usize, fill: &str, start: bool) -> String {
    if char_len(text) >= width || fill.is_empty() {
        return text.to_string();
    }
    let mut out = String::new();
    let mut needed = width - char_len(text);
    if !start {
        out.push_str(text);
    }
    while needed > 0 {
        let mut fill_iter = fill.chars();
        while needed > 0 {
            if let Some(ch) = fill_iter.next() {
                out.push(ch);
                needed -= 1;
            } else {
                break;
            }
        }
    }
    if start {
        out.push_str(text);
    }
    out
}

fn capitalize_segment(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => {
            let mut out = String::new();
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
            out
        }
        None => String::new(),
    }
}

fn char_len(value: &str) -> usize {
    value.graphemes(true).count()
}
