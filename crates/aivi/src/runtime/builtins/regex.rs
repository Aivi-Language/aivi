use std::sync::Arc;

use regex::Regex;

use super::util::{
    builtin, expect_text, list_value, make_err, make_none, make_ok, make_some, expect_regex,
};
use crate::runtime::Value;

pub(super) fn build_regex_record() -> Value {
    let mut fields = std::collections::HashMap::new();
    fields.insert(
        "compile".to_string(),
        builtin("regex.compile", 1, |mut args, _| {
            let pattern = expect_text(args.pop().unwrap(), "regex.compile")?;
            match Regex::new(&pattern) {
                Ok(regex) => Ok(make_ok(Value::Regex(Arc::new(regex)))),
                Err(err) => Ok(make_err(Value::Constructor {
                    name: "InvalidPattern".to_string(),
                    args: vec![Value::Text(err.to_string())],
                })),
            }
        }),
    );
    fields.insert(
        "test".to_string(),
        builtin("regex.test", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "regex.test")?;
            let regex = expect_regex(args.pop().unwrap(), "regex.test")?;
            Ok(Value::Bool(regex.is_match(&text)))
        }),
    );
    fields.insert(
        "match".to_string(),
        builtin("regex.match", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "regex.match")?;
            let regex = expect_regex(args.pop().unwrap(), "regex.match")?;
            match regex.captures(&text) {
                Some(captures) => {
                    let full = captures.get(0).map(|m| m.as_str()).unwrap_or("");
                    let mut groups = Vec::new();
                    for idx in 1..captures.len() {
                        if let Some(matched) = captures.get(idx) {
                            groups.push(make_some(Value::Text(matched.as_str().to_string())));
                        } else {
                            groups.push(make_none());
                        }
                    }
                    let mut record = std::collections::HashMap::new();
                    let (start, end) = captures.get(0).map(|m| (m.start(), m.end())).unwrap_or((0, 0));
                    record.insert("full".to_string(), Value::Text(full.to_string()));
                    record.insert("groups".to_string(), list_value(groups));
                    record.insert("start".to_string(), Value::Int(start as i64));
                    record.insert("end".to_string(), Value::Int(end as i64));
                    Ok(make_some(Value::Record(Arc::new(record))))
                }
                None => Ok(make_none()),
            }
        }),
    );
    fields.insert(
        "matches".to_string(),
        builtin("regex.matches", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "regex.matches")?;
            let regex = expect_regex(args.pop().unwrap(), "regex.matches")?;
            let mut matches_out = Vec::new();
            for captures in regex.captures_iter(&text) {
                let full = captures.get(0).map(|m| m.as_str()).unwrap_or("");
                let mut groups = Vec::new();
                for idx in 1..captures.len() {
                    if let Some(matched) = captures.get(idx) {
                        groups.push(make_some(Value::Text(matched.as_str().to_string())));
                    } else {
                        groups.push(make_none());
                    }
                }
                let (start, end) = captures.get(0).map(|m| (m.start(), m.end())).unwrap_or((0, 0));
                let mut record = std::collections::HashMap::new();
                record.insert("full".to_string(), Value::Text(full.to_string()));
                record.insert("groups".to_string(), list_value(groups));
                record.insert("start".to_string(), Value::Int(start as i64));
                record.insert("end".to_string(), Value::Int(end as i64));
                matches_out.push(Value::Record(Arc::new(record)));
            }
            Ok(list_value(matches_out))
        }),
    );
    fields.insert(
        "find".to_string(),
        builtin("regex.find", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "regex.find")?;
            let regex = expect_regex(args.pop().unwrap(), "regex.find")?;
            match regex.find(&text) {
                Some(found) => Ok(make_some(Value::Tuple(vec![
                    Value::Int(found.start() as i64),
                    Value::Int(found.end() as i64),
                ]))),
                None => Ok(make_none()),
            }
        }),
    );
    fields.insert(
        "findAll".to_string(),
        builtin("regex.findAll", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "regex.findAll")?;
            let regex = expect_regex(args.pop().unwrap(), "regex.findAll")?;
            let mut out = Vec::new();
            for found in regex.find_iter(&text) {
                out.push(Value::Tuple(vec![
                    Value::Int(found.start() as i64),
                    Value::Int(found.end() as i64),
                ]));
            }
            Ok(list_value(out))
        }),
    );
    fields.insert(
        "split".to_string(),
        builtin("regex.split", 2, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "regex.split")?;
            let regex = expect_regex(args.pop().unwrap(), "regex.split")?;
            let parts = regex
                .split(&text)
                .map(|part| Value::Text(part.to_string()))
                .collect::<Vec<_>>();
            Ok(list_value(parts))
        }),
    );
    fields.insert(
        "replace".to_string(),
        builtin("regex.replace", 3, |mut args, _| {
            let replacement = expect_text(args.pop().unwrap(), "regex.replace")?;
            let text = expect_text(args.pop().unwrap(), "regex.replace")?;
            let regex = expect_regex(args.pop().unwrap(), "regex.replace")?;
            Ok(Value::Text(regex.replace(&text, replacement).to_string()))
        }),
    );
    fields.insert(
        "replaceAll".to_string(),
        builtin("regex.replaceAll", 3, |mut args, _| {
            let replacement = expect_text(args.pop().unwrap(), "regex.replaceAll")?;
            let text = expect_text(args.pop().unwrap(), "regex.replaceAll")?;
            let regex = expect_regex(args.pop().unwrap(), "regex.replaceAll")?;
            Ok(Value::Text(regex.replace_all(&text, replacement).to_string()))
        }),
    );
    Value::Record(Arc::new(fields))
}
