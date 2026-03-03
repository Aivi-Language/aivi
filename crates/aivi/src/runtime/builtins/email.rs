use std::collections::HashMap;
use std::sync::Arc;

use aivi_email::{ImapConfig, SmtpConfig};

use super::util::{builtin, expect_int, expect_record, expect_text, make_none};
use crate::runtime::{EffectValue, RuntimeError, SourceValue, Value};

pub(super) fn build_email_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "imap".to_string(),
        builtin("email.imap", 1, |mut args, _| {
            let cfg = expect_record(args.remove(0), "email.imap")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let imap_cfg = build_imap_config(cfg.as_ref())?;
                    let messages = aivi_email::load_imap_messages(imap_cfg)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.message)))?;
                    let out: Vec<Value> = messages
                        .into_iter()
                        .map(|msg| {
                            let mut rec = HashMap::new();
                            rec.insert(
                                "uid".to_string(),
                                msg.uid
                                    .map(|uid| Value::Int(uid as i64))
                                    .unwrap_or_else(make_none),
                            );
                            rec.insert(
                                "subject".to_string(),
                                msg.subject.map(Value::Text).unwrap_or_else(make_none),
                            );
                            rec.insert(
                                "from".to_string(),
                                msg.from.map(Value::Text).unwrap_or_else(make_none),
                            );
                            rec.insert(
                                "to".to_string(),
                                msg.to.map(Value::Text).unwrap_or_else(make_none),
                            );
                            rec.insert(
                                "date".to_string(),
                                msg.date.map(Value::Text).unwrap_or_else(make_none),
                            );
                            rec.insert("body".to_string(), Value::Text(msg.body));
                            Value::Record(Arc::new(rec))
                        })
                        .collect();
                    Ok(Value::List(Arc::new(out)))
                }),
            };
            Ok(Value::Source(Arc::new(SourceValue {
                kind: "Imap".to_string(),
                effect: Arc::new(effect),
            })))
        }),
    );
    fields.insert(
        "smtpSend".to_string(),
        builtin("email.smtpSend", 1, |mut args, _| {
            let cfg = expect_record(args.remove(0), "email.smtpSend")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let smtp_cfg = build_smtp_config(cfg.as_ref())?;
                    aivi_email::send_smtp_message(smtp_cfg)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.message)))?;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "mimeParts".to_string(),
        builtin("email.mimeParts", 1, |mut args, _| {
            let raw = expect_text(args.remove(0), "email.mimeParts")?;
            let parts = aivi_email::parse_mime_parts(&raw)
                .map_err(|err| RuntimeError::Error(Value::Text(err.message)))?;
            let values: Vec<Value> = parts
                .into_iter()
                .map(|part| {
                    let mut fields = HashMap::new();
                    fields.insert("contentType".to_string(), Value::Text(part.content_type));
                    fields.insert("body".to_string(), Value::Text(part.body));
                    Value::Record(Arc::new(fields))
                })
                .collect();
            Ok(Value::List(Arc::new(values)))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn build_imap_config(record: &HashMap<String, Value>) -> Result<ImapConfig, RuntimeError> {
    Ok(ImapConfig {
        host: required_text(record, "host", "email.imap")?,
        user: required_text(record, "user", "email.imap")?,
        password: required_text(record, "password", "email.imap")?,
        mailbox: optional_text(record, "mailbox", "INBOX", "email.imap")?,
        filter: optional_text(record, "filter", "ALL", "email.imap")?,
        limit: optional_int(record, "limit", 50, "email.imap")?,
        port: optional_int(record, "port", 993, "email.imap")?,
    })
}

fn build_smtp_config(record: &HashMap<String, Value>) -> Result<SmtpConfig, RuntimeError> {
    Ok(SmtpConfig {
        host: required_text(record, "host", "email.smtpSend")?,
        user: required_text(record, "user", "email.smtpSend")?,
        password: required_text(record, "password", "email.smtpSend")?,
        from: required_text(record, "from", "email.smtpSend")?,
        to: required_text(record, "to", "email.smtpSend")?,
        subject: required_text(record, "subject", "email.smtpSend")?,
        body: required_text(record, "body", "email.smtpSend")?,
    })
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
            crate::runtime::format_value(&other)
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
            crate::runtime::format_value(&other)
        ))),
    }
}
