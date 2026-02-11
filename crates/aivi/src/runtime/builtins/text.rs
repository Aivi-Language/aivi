fn build_text_record() -> Value {
    let mut fields = HashMap::new();
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
            Ok(Value::Bool(text.is_empty()))
        }),
    );
    fields.insert(
        "isDigit".to_string(),
        builtin("text.isDigit", 1, |mut args, _| {
            let ch = expect_char(args.remove(0), "text.isDigit")?;
            Ok(Value::Bool(ch.is_numeric()))
        }),
    );
    fields.insert(
        "isAlpha".to_string(),
        builtin("text.isAlpha", 1, |mut args, _| {
            let ch = expect_char(args.remove(0), "text.isAlpha")?;
            Ok(Value::Bool(ch.is_alphabetic()))
        }),
    );
    fields.insert(
        "isAlnum".to_string(),
        builtin("text.isAlnum", 1, |mut args, _| {
            let ch = expect_char(args.remove(0), "text.isAlnum")?;
            Ok(Value::Bool(ch.is_alphanumeric()))
        }),
    );
    fields.insert(
        "isSpace".to_string(),
        builtin("text.isSpace", 1, |mut args, _| {
            let ch = expect_char(args.remove(0), "text.isSpace")?;
            Ok(Value::Bool(ch.is_whitespace()))
        }),
    );
    fields.insert(
        "isUpper".to_string(),
        builtin("text.isUpper", 1, |mut args, _| {
            let ch = expect_char(args.remove(0), "text.isUpper")?;
            Ok(Value::Bool(ch.is_uppercase()))
        }),
    );
    fields.insert(
        "isLower".to_string(),
        builtin("text.isLower", 1, |mut args, _| {
            let ch = expect_char(args.remove(0), "text.isLower")?;
            Ok(Value::Bool(ch.is_lowercase()))
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
            let text = expect_text(args.pop().unwrap(), "text.startsWith")?;
            Ok(Value::Bool(text.starts_with(&prefix)))
        }),
    );
    fields.insert(
        "endsWith".to_string(),
        builtin("text.endsWith", 2, |mut args, _| {
            let suffix = expect_text(args.pop().unwrap(), "text.endsWith")?;
            let text = expect_text(args.pop().unwrap(), "text.endsWith")?;
            Ok(Value::Bool(text.ends_with(&suffix)))
        }),
    );
    fields.insert(
        "indexOf".to_string(),
        builtin("text.indexOf", 2, |mut args, _| {
            let needle = expect_text(args.pop().unwrap(), "text.indexOf")?;
            let haystack = expect_text(args.pop().unwrap(), "text.indexOf")?;
            match haystack.find(&needle) {
                Some(idx) => Ok(make_some(Value::Int(
                    haystack[..idx].chars().count() as i64,
                ))),
                None => Ok(make_none()),
            }
        }),
    );
    fields.insert(
        "lastIndexOf".to_string(),
        builtin("text.lastIndexOf", 2, |mut args, _| {
            let needle = expect_text(args.pop().unwrap(), "text.lastIndexOf")?;
            let haystack = expect_text(args.pop().unwrap(), "text.lastIndexOf")?;
            match haystack.rfind(&needle) {
                Some(idx) => Ok(make_some(Value::Int(
                    haystack[..idx].chars().count() as i64,
                ))),
                None => Ok(make_none()),
            }
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
            let ord = left.cmp(&right);
            let value = match ord {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            };
            Ok(Value::Int(value))
        }),
    );
    fields.insert(
        "slice".to_string(),
        builtin("text.slice", 3, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.slice")?;
            let end = expect_int(args.pop().unwrap(), "text.slice")?;
            let start = expect_int(args.pop().unwrap(), "text.slice")?;
            Ok(Value::Text(slice_chars(&text, start, end)))
        }),
    );
    fields.insert(
        "split".to_string(),
        builtin("text.split", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.split")?;
            let sep = expect_text(args.pop().unwrap(), "text.split")?;
            let parts = text
                .split(&sep)
                .map(|part| Value::Text(part.to_string()))
                .collect::<Vec<_>>();
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
                .collect::<Vec<_>>();
            Ok(list_value(parts))
        }),
    );
    fields.insert(
        "chunk".to_string(),
        builtin("text.chunk", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.chunk")?;
            let size = expect_int(args.pop().unwrap(), "text.chunk")?;
            if size <= 0 {
                return Ok(list_value(Vec::new()));
            }
            let size = size as usize;
            let mut items = Vec::new();
            let mut iter = text.chars().peekable();
            while iter.peek().is_some() {
                let chunk: String = iter.by_ref().take(size).collect();
                items.push(Value::Text(chunk));
            }
            Ok(list_value(items))
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
            let text = expect_text(args.pop().unwrap(), "text.padStart")?;
            let fill = expect_text(args.pop().unwrap(), "text.padStart")?;
            let width = expect_int(args.pop().unwrap(), "text.padStart")?;
            Ok(Value::Text(pad_text(&text, width, &fill, true)))
        }),
    );
    fields.insert(
        "padEnd".to_string(),
        builtin("text.padEnd", 3, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.padEnd")?;
            let fill = expect_text(args.pop().unwrap(), "text.padEnd")?;
            let width = expect_int(args.pop().unwrap(), "text.padEnd")?;
            Ok(Value::Text(pad_text(&text, width, &fill, false)))
        }),
    );
    fields.insert(
        "replace".to_string(),
        builtin("text.replace", 3, |mut args, _| {
            let replacement = expect_text(args.pop().unwrap(), "text.replace")?;
            let needle = expect_text(args.pop().unwrap(), "text.replace")?;
            let text = expect_text(args.pop().unwrap(), "text.replace")?;
            Ok(Value::Text(text.replacen(&needle, &replacement, 1)))
        }),
    );
    fields.insert(
        "replaceAll".to_string(),
        builtin("text.replaceAll", 3, |mut args, _| {
            let replacement = expect_text(args.pop().unwrap(), "text.replaceAll")?;
            let needle = expect_text(args.pop().unwrap(), "text.replaceAll")?;
            let text = expect_text(args.pop().unwrap(), "text.replaceAll")?;
            Ok(Value::Text(text.replace(&needle, &replacement)))
        }),
    );
    fields.insert(
        "remove".to_string(),
        builtin("text.remove", 2, |mut args, _| {
            let needle = expect_text(args.pop().unwrap(), "text.remove")?;
            let text = expect_text(args.pop().unwrap(), "text.remove")?;
            Ok(Value::Text(text.replace(&needle, "")))
        }),
    );
    fields.insert(
        "repeat".to_string(),
        builtin("text.repeat", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.repeat")?;
            let count = expect_int(args.pop().unwrap(), "text.repeat")?;
            let count = if count < 0 { 0 } else { count as usize };
            Ok(Value::Text(text.repeat(count)))
        }),
    );
    fields.insert(
        "reverse".to_string(),
        builtin("text.reverse", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.reverse")?;
            let reversed = UnicodeSegmentation::graphemes(text.as_str(), true)
                .rev()
                .collect::<String>();
            Ok(Value::Text(reversed))
        }),
    );
    fields.insert(
        "concat".to_string(),
        builtin("text.concat", 1, |mut args, _| {
            let list = expect_list(args.pop().unwrap(), "text.concat")?;
            let mut out = String::new();
            for item in list.iter() {
                match item {
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
            let text = expect_text(args.pop().unwrap(), "text.toLower")?;
            Ok(Value::Text(text.to_lowercase()))
        }),
    );
    fields.insert(
        "toUpper".to_string(),
        builtin("text.toUpper", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.toUpper")?;
            Ok(Value::Text(text.to_uppercase()))
        }),
    );
    fields.insert(
        "capitalize".to_string(),
        builtin("text.capitalize", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.capitalize")?;
            Ok(Value::Text(capitalize_segment(&text)))
        }),
    );
    fields.insert(
        "titleCase".to_string(),
        builtin("text.titleCase", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.titleCase")?;
            let mut out = String::new();
            for segment in UnicodeSegmentation::split_word_bounds(text.as_str()) {
                if segment.chars().any(|ch| ch.is_alphabetic()) {
                    out.push_str(&capitalize_segment(segment));
                } else {
                    out.push_str(segment);
                }
            }
            Ok(Value::Text(out))
        }),
    );
    fields.insert(
        "caseFold".to_string(),
        builtin("text.caseFold", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.caseFold")?;
            Ok(Value::Text(text.to_lowercase()))
        }),
    );
    fields.insert(
        "normalizeNFC".to_string(),
        builtin("text.normalizeNFC", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.normalizeNFC")?;
            Ok(Value::Text(text.nfc().collect()))
        }),
    );
    fields.insert(
        "normalizeNFD".to_string(),
        builtin("text.normalizeNFD", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.normalizeNFD")?;
            Ok(Value::Text(text.nfd().collect()))
        }),
    );
    fields.insert(
        "normalizeNFKC".to_string(),
        builtin("text.normalizeNFKC", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.normalizeNFKC")?;
            Ok(Value::Text(text.nfkc().collect()))
        }),
    );
    fields.insert(
        "normalizeNFKD".to_string(),
        builtin("text.normalizeNFKD", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.normalizeNFKD")?;
            Ok(Value::Text(text.nfkd().collect()))
        }),
    );
    fields.insert(
        "toBytes".to_string(),
        builtin("text.toBytes", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.toBytes")?;
            let encoding_value = args.pop().unwrap();
            let encoding = encoding_kind(&encoding_value).ok_or_else(|| {
                RuntimeError::Message("text.toBytes expects Encoding".to_string())
            })?;
            Ok(Value::Bytes(Arc::new(encode_text(encoding, &text))))
        }),
    );
    fields.insert(
        "fromBytes".to_string(),
        builtin("text.fromBytes", 2, |mut args, _| {
            let bytes = expect_bytes(args.pop().unwrap(), "text.fromBytes")?;
            let encoding_value = args.pop().unwrap();
            let encoding = encoding_kind(&encoding_value).ok_or_else(|| {
                RuntimeError::Message("text.fromBytes expects Encoding".to_string())
            })?;
            match decode_bytes(encoding, &bytes) {
                Ok(text) => Ok(make_ok(Value::Text(text))),
                Err(()) => Ok(make_err(Value::Constructor {
                    name: "InvalidEncoding".to_string(),
                    args: vec![encoding_value],
                })),
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
            let text = expect_text(args.pop().unwrap(), "text.parseInt")?;
            match text.trim().parse::<i64>() {
                Ok(value) => Ok(make_some(Value::Int(value))),
                Err(_) => Ok(make_none()),
            }
        }),
    );
    fields.insert(
        "parseFloat".to_string(),
        builtin("text.parseFloat", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "text.parseFloat")?;
            match text.trim().parse::<f64>() {
                Ok(value) => Ok(make_some(Value::Float(value))),
                Err(_) => Ok(make_none()),
            }
        }),
    );
    Value::Record(Arc::new(fields))
}

