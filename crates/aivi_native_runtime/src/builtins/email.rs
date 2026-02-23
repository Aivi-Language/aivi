use std::collections::HashMap;
use std::sync::Arc;

use native_tls::TlsConnector;

use super::util::{builtin, expect_int, expect_record, expect_text, make_none};
use crate::{RuntimeError, SourceValue, Value};

pub(super) fn build_email_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "imap".to_string(),
        builtin("email.imap", 1, |mut args, _| {
            let cfg = expect_record(args.remove(0), "email.imap")?;
            let effect = crate::EffectValue::Thunk {
                func: Arc::new(move |_| load_imap_messages(cfg.as_ref().clone())),
            };
            Ok(Value::Source(Arc::new(SourceValue {
                kind: "Imap".to_string(),
                effect: Arc::new(effect),
            })))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn load_imap_messages(config: HashMap<String, Value>) -> Result<Value, RuntimeError> {
    let host = required_text(&config, "host", "email.imap")?;
    let user = required_text(&config, "user", "email.imap")?;
    let password = required_text(&config, "password", "email.imap")?;
    let mailbox = optional_text(&config, "mailbox", "INBOX", "email.imap")?;
    let filter = optional_text(&config, "filter", "ALL", "email.imap")?;
    let limit = optional_int(&config, "limit", 50, "email.imap")?;
    let port = optional_int(&config, "port", 993, "email.imap")?;

    let tls = TlsConnector::builder()
        .build()
        .map_err(|err| RuntimeError::Error(Value::Text(format!("email.imap TLS error: {err}"))))?;
    let client = imap::connect((host.as_str(), port as u16), &host, &tls).map_err(|err| {
        RuntimeError::Error(Value::Text(format!("email.imap transport error: {err}")))
    })?;
    let mut session = client.login(user, password).map_err(|(err, _)| {
        RuntimeError::Error(Value::Text(format!("email.imap auth error: {err}")))
    })?;

    session.select(&mailbox).map_err(|err| {
        RuntimeError::Error(Value::Text(format!("email.imap mailbox error: {err}")))
    })?;

    let ids = session.search(filter).map_err(|err| {
        RuntimeError::Error(Value::Text(format!("email.imap search error: {err}")))
    })?;
    if ids.is_empty() {
        let _ = session.logout();
        return Ok(Value::List(Arc::new(Vec::new())));
    }

    let mut selected = ids.into_iter().collect::<Vec<_>>();
    selected.sort_unstable();
    selected.reverse();
    selected.truncate(limit as usize);
    selected.sort_unstable();
    let sequence_set = selected
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let fetches = session.fetch(sequence_set, "UID RFC822").map_err(|err| {
        RuntimeError::Error(Value::Text(format!("email.imap fetch error: {err}")))
    })?;

    let mut out = Vec::new();
    for msg in fetches.iter() {
        let Some(raw) = msg.body() else {
            continue;
        };
        let parsed = mailparse::parse_mail(raw).map_err(|err| {
            RuntimeError::Error(Value::Text(format!("email.imap decode error: {err}")))
        })?;
        let mut rec = HashMap::new();
        rec.insert(
            "uid".to_string(),
            msg.uid
                .map(|uid| Value::Int(uid as i64))
                .unwrap_or_else(make_none),
        );
        rec.insert(
            "subject".to_string(),
            header_or_none(&parsed, "Subject")
                .map(Value::Text)
                .unwrap_or_else(make_none),
        );
        rec.insert(
            "from".to_string(),
            header_or_none(&parsed, "From")
                .map(Value::Text)
                .unwrap_or_else(make_none),
        );
        rec.insert(
            "to".to_string(),
            header_or_none(&parsed, "To")
                .map(Value::Text)
                .unwrap_or_else(make_none),
        );
        rec.insert(
            "date".to_string(),
            header_or_none(&parsed, "Date")
                .map(Value::Text)
                .unwrap_or_else(make_none),
        );
        let body = parsed.get_body().unwrap_or_default();
        rec.insert("body".to_string(), Value::Text(body));
        out.push(Value::Record(Arc::new(rec)));
    }

    let _ = session.logout();
    Ok(Value::List(Arc::new(out)))
}

fn required_text(
    record: &HashMap<String, Value>,
    field: &str,
    ctx: &str,
) -> Result<String, RuntimeError> {
    let value = record
        .get(field)
        .cloned()
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects `{field}`")))?;
    expect_text(value, ctx)
}

fn optional_text(
    record: &HashMap<String, Value>,
    field: &str,
    default: &str,
    ctx: &str,
) -> Result<String, RuntimeError> {
    let Some(value) = record.get(field).cloned() else {
        return Ok(default.to_string());
    };
    match value {
        Value::Text(text) => Ok(text),
        Value::Constructor { name, args } if name == "Some" && args.len() == 1 => {
            expect_text(args[0].clone(), ctx)
        }
        Value::Constructor { name, args } if name == "None" && args.is_empty() => {
            Ok(default.to_string())
        }
        other => Err(RuntimeError::Message(format!(
            "{ctx} expected `{field}` as Text/Option Text, got {}",
            crate::format_value(&other)
        ))),
    }
}

fn optional_int(
    record: &HashMap<String, Value>,
    field: &str,
    default: i64,
    ctx: &str,
) -> Result<i64, RuntimeError> {
    let Some(value) = record.get(field).cloned() else {
        return Ok(default);
    };
    match value {
        Value::Int(i) => Ok(i),
        Value::Constructor { name, args } if name == "Some" && args.len() == 1 => {
            expect_int(args[0].clone(), ctx)
        }
        Value::Constructor { name, args } if name == "None" && args.is_empty() => Ok(default),
        other => Err(RuntimeError::Message(format!(
            "{ctx} expected `{field}` as Int/Option Int, got {}",
            crate::format_value(&other)
        ))),
    }
}

fn header_or_none(parsed: &mailparse::ParsedMail<'_>, name: &str) -> Option<String> {
    parsed
        .headers
        .iter()
        .find(|h| h.get_key_ref().eq_ignore_ascii_case(name))
        .map(|h| h.get_value())
}
