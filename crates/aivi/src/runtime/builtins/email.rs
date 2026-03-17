use std::collections::HashMap;
use std::sync::Arc;

use aivi_email::{EmailAuth, ImapConfig, ImapSession, SmtpConfig};

use super::util::{builtin, expect_int, expect_record, expect_text, make_none};
use crate::runtime::{EffectValue, RuntimeError, SourceValue, Value};

pub(super) fn build_email_record() -> Value {
    let mut fields = HashMap::new();

    // --- One-shot source ---
    fields.insert(
        "imap".to_string(),
        builtin("email.imap", 1, |mut args, _| {
            let cfg = expect_record(args.remove(0), "email.imap")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let imap_cfg = build_imap_config(cfg.as_ref())?;
                    let messages = aivi_email::load_imap_messages(imap_cfg)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.message)))?;
                    Ok(Value::List(Arc::new(messages_to_values(messages))))
                }),
            };
            Ok(Value::Source(Arc::new(SourceValue::new(
                "Imap".to_string(),
                Arc::new(effect),
            ))))
        }),
    );

    // --- SMTP ---
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

    // --- MIME ---
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

    // --- Session-based IMAP ---
    fields.insert(
        "imapOpen".to_string(),
        builtin("email.imapOpen", 1, |mut args, _| {
            let cfg = expect_record(args.remove(0), "email.imapOpen")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let imap_cfg = build_imap_config(cfg.as_ref())?;
                    let session = aivi_email::imap_open(&imap_cfg)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.message)))?;
                    Ok(Value::ImapSession(session))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    fields.insert(
        "imapClose".to_string(),
        builtin("email.imapClose", 1, |mut args, _| {
            let session = expect_imap_session(args.remove(0), "email.imapClose")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    aivi_email::imap_close(&session)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.message)))?;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    fields.insert(
        "imapSelect".to_string(),
        builtin("email.imapSelect", 2, |mut args, _| {
            let mailbox = expect_text(args.remove(0), "email.imapSelect")?;
            let session = expect_imap_session(args.remove(0), "email.imapSelect")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let info = aivi_email::imap_select(&mailbox, &session)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.message)))?;
                    Ok(mailbox_info_to_value(info))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    fields.insert(
        "imapExamine".to_string(),
        builtin("email.imapExamine", 2, |mut args, _| {
            let mailbox = expect_text(args.remove(0), "email.imapExamine")?;
            let session = expect_imap_session(args.remove(0), "email.imapExamine")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let info = aivi_email::imap_examine(&mailbox, &session)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.message)))?;
                    Ok(mailbox_info_to_value(info))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    fields.insert(
        "imapSearch".to_string(),
        builtin("email.imapSearch", 2, |mut args, _| {
            let query = expect_text(args.remove(0), "email.imapSearch")?;
            let session = expect_imap_session(args.remove(0), "email.imapSearch")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let uids = aivi_email::imap_search(&query, &session)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.message)))?;
                    Ok(Value::List(Arc::new(
                        uids.into_iter().map(|u| Value::Int(u as i64)).collect(),
                    )))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    fields.insert(
        "imapFetch".to_string(),
        builtin("email.imapFetch", 2, |mut args, _| {
            let uids = expect_int_list(args.remove(0), "email.imapFetch")?;
            let session = expect_imap_session(args.remove(0), "email.imapFetch")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let uid_u32: Vec<u32> = uids.iter().map(|&u| u as u32).collect();
                    let messages = aivi_email::imap_fetch(&uid_u32, &session)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.message)))?;
                    Ok(Value::List(Arc::new(messages_to_values(messages))))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    fields.insert(
        "imapSetFlags".to_string(),
        builtin("email.imapSetFlags", 3, |mut args, _| {
            let uids = expect_int_list(args.remove(0), "email.imapSetFlags")?;
            let flags = expect_text_list(args.remove(0), "email.imapSetFlags")?;
            let session = expect_imap_session(args.remove(0), "email.imapSetFlags")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let uid_u32: Vec<u32> = uids.iter().map(|&u| u as u32).collect();
                    aivi_email::imap_set_flags(&uid_u32, &flags, &session)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.message)))?;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    fields.insert(
        "imapAddFlags".to_string(),
        builtin("email.imapAddFlags", 3, |mut args, _| {
            let uids = expect_int_list(args.remove(0), "email.imapAddFlags")?;
            let flags = expect_text_list(args.remove(0), "email.imapAddFlags")?;
            let session = expect_imap_session(args.remove(0), "email.imapAddFlags")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let uid_u32: Vec<u32> = uids.iter().map(|&u| u as u32).collect();
                    aivi_email::imap_add_flags(&uid_u32, &flags, &session)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.message)))?;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    fields.insert(
        "imapRemoveFlags".to_string(),
        builtin("email.imapRemoveFlags", 3, |mut args, _| {
            let uids = expect_int_list(args.remove(0), "email.imapRemoveFlags")?;
            let flags = expect_text_list(args.remove(0), "email.imapRemoveFlags")?;
            let session = expect_imap_session(args.remove(0), "email.imapRemoveFlags")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let uid_u32: Vec<u32> = uids.iter().map(|&u| u as u32).collect();
                    aivi_email::imap_remove_flags(&uid_u32, &flags, &session)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.message)))?;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    fields.insert(
        "imapExpunge".to_string(),
        builtin("email.imapExpunge", 1, |mut args, _| {
            let session = expect_imap_session(args.remove(0), "email.imapExpunge")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    aivi_email::imap_expunge(&session)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.message)))?;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    fields.insert(
        "imapCopy".to_string(),
        builtin("email.imapCopy", 3, |mut args, _| {
            let uids = expect_int_list(args.remove(0), "email.imapCopy")?;
            let mailbox = expect_text(args.remove(0), "email.imapCopy")?;
            let session = expect_imap_session(args.remove(0), "email.imapCopy")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let uid_u32: Vec<u32> = uids.iter().map(|&u| u as u32).collect();
                    aivi_email::imap_copy(&uid_u32, &mailbox, &session)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.message)))?;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    fields.insert(
        "imapMove".to_string(),
        builtin("email.imapMove", 3, |mut args, _| {
            let uids = expect_int_list(args.remove(0), "email.imapMove")?;
            let mailbox = expect_text(args.remove(0), "email.imapMove")?;
            let session = expect_imap_session(args.remove(0), "email.imapMove")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let uid_u32: Vec<u32> = uids.iter().map(|&u| u as u32).collect();
                    aivi_email::imap_move(&uid_u32, &mailbox, &session)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.message)))?;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    fields.insert(
        "imapListMailboxes".to_string(),
        builtin("email.imapListMailboxes", 1, |mut args, _| {
            let session = expect_imap_session(args.remove(0), "email.imapListMailboxes")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let mailboxes = aivi_email::imap_list_mailboxes(&session)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.message)))?;
                    Ok(Value::List(Arc::new(
                        mailboxes.into_iter().map(mailbox_info_to_value).collect(),
                    )))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    fields.insert(
        "imapCreateMailbox".to_string(),
        builtin("email.imapCreateMailbox", 2, |mut args, _| {
            let name = expect_text(args.remove(0), "email.imapCreateMailbox")?;
            let session = expect_imap_session(args.remove(0), "email.imapCreateMailbox")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    aivi_email::imap_create_mailbox(&name, &session)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.message)))?;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    fields.insert(
        "imapDeleteMailbox".to_string(),
        builtin("email.imapDeleteMailbox", 2, |mut args, _| {
            let name = expect_text(args.remove(0), "email.imapDeleteMailbox")?;
            let session = expect_imap_session(args.remove(0), "email.imapDeleteMailbox")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    aivi_email::imap_delete_mailbox(&name, &session)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.message)))?;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    fields.insert(
        "imapRenameMailbox".to_string(),
        builtin("email.imapRenameMailbox", 3, |mut args, _| {
            let from = expect_text(args.remove(0), "email.imapRenameMailbox")?;
            let to = expect_text(args.remove(0), "email.imapRenameMailbox")?;
            let session = expect_imap_session(args.remove(0), "email.imapRenameMailbox")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    aivi_email::imap_rename_mailbox(&from, &to, &session)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.message)))?;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    fields.insert(
        "imapAppend".to_string(),
        builtin("email.imapAppend", 3, |mut args, _| {
            let mailbox = expect_text(args.remove(0), "email.imapAppend")?;
            let content = expect_text(args.remove(0), "email.imapAppend")?;
            let session = expect_imap_session(args.remove(0), "email.imapAppend")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    aivi_email::imap_append(&mailbox, &content, &session)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.message)))?;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    fields.insert(
        "imapIdle".to_string(),
        builtin("email.imapIdle", 2, |mut args, _| {
            let timeout = expect_int(args.remove(0), "email.imapIdle")?;
            let session = expect_imap_session(args.remove(0), "email.imapIdle")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let result = aivi_email::imap_idle(timeout as u64, &session)
                        .map_err(|err| RuntimeError::Error(Value::Text(err.message)))?;
                    match result {
                        aivi_email::IdleResult::TimedOut => Ok(Value::Constructor {
                            name: "TimedOut".to_string(),
                            args: vec![],
                        }),
                        aivi_email::IdleResult::MailboxChanged => Ok(Value::Constructor {
                            name: "MailboxChanged".to_string(),
                            args: vec![],
                        }),
                    }
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    Value::Record(Arc::new(fields))
}

fn build_imap_config(record: &HashMap<String, Value>) -> Result<ImapConfig, RuntimeError> {
    Ok(ImapConfig {
        host: required_text(record, "host", "email.imap")?,
        user: required_text(record, "user", "email.imap")?,
        auth: extract_auth(record, "email.imap")?,
        mailbox: optional_text(record, "mailbox", "INBOX", "email.imap")?,
        filter: optional_text(record, "filter", "ALL", "email.imap")?,
        limit: optional_int(record, "limit", 50, "email.imap")?,
        port: optional_int(record, "port", 993, "email.imap")?,
        starttls: optional_bool(record, "starttls", false, "email.imap")?,
    })
}

fn build_smtp_config(record: &HashMap<String, Value>) -> Result<SmtpConfig, RuntimeError> {
    Ok(SmtpConfig {
        host: required_text(record, "host", "email.smtpSend")?,
        user: required_text(record, "user", "email.smtpSend")?,
        auth: extract_auth(record, "email.smtpSend")?,
        from: required_text(record, "from", "email.smtpSend")?,
        to: required_text_list(record, "to", "email.smtpSend")?,
        cc: optional_text_list(record, "cc", "email.smtpSend")?,
        bcc: optional_text_list(record, "bcc", "email.smtpSend")?,
        subject: required_text(record, "subject", "email.smtpSend")?,
        body: required_text(record, "body", "email.smtpSend")?,
        port: optional_int(record, "port", 465, "email.smtpSend")?,
        starttls: optional_bool(record, "starttls", false, "email.smtpSend")?,
    })
}

fn extract_auth(record: &HashMap<String, Value>, ctx: &str) -> Result<EmailAuth, RuntimeError> {
    let value = record
        .get("auth")
        .cloned()
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects `auth`")))?;
    match value {
        Value::Constructor { name, mut args } if name == "Password" && args.len() == 1 => {
            let pw = expect_text(args.remove(0), ctx)?;
            Ok(EmailAuth::Password(pw))
        }
        Value::Constructor { name, mut args } if name == "OAuth2" && args.len() == 1 => {
            let token = expect_text(args.remove(0), ctx)?;
            Ok(EmailAuth::OAuth2(token))
        }
        other => Err(RuntimeError::Message(format!(
            "{ctx} expected `auth` as EmailAuth (Password Text | OAuth2 Text), got {}",
            crate::runtime::format_value(&other)
        ))),
    }
}

fn expect_imap_session(value: Value, ctx: &str) -> Result<ImapSession, RuntimeError> {
    match value {
        Value::ImapSession(session) => Ok(session),
        other => Err(RuntimeError::Message(format!(
            "{ctx} expected ImapSession, got {}",
            crate::runtime::format_value(&other)
        ))),
    }
}

fn expect_int_list(value: Value, ctx: &str) -> Result<Vec<i64>, RuntimeError> {
    match value {
        Value::List(items) => items.iter().map(|v| expect_int(v.clone(), ctx)).collect(),
        other => Err(RuntimeError::Message(format!(
            "{ctx} expected List Int, got {}",
            crate::runtime::format_value(&other)
        ))),
    }
}

fn expect_text_list(value: Value, ctx: &str) -> Result<Vec<String>, RuntimeError> {
    match value {
        Value::List(items) => items.iter().map(|v| expect_text(v.clone(), ctx)).collect(),
        other => Err(RuntimeError::Message(format!(
            "{ctx} expected List Text, got {}",
            crate::runtime::format_value(&other)
        ))),
    }
}

fn messages_to_values(messages: Vec<aivi_email::EmailMessage>) -> Vec<Value> {
    messages
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
                "cc".to_string(),
                msg.cc.map(Value::Text).unwrap_or_else(make_none),
            );
            rec.insert(
                "bcc".to_string(),
                msg.bcc.map(Value::Text).unwrap_or_else(make_none),
            );
            rec.insert(
                "date".to_string(),
                msg.date.map(Value::Text).unwrap_or_else(make_none),
            );
            rec.insert(
                "messageId".to_string(),
                msg.message_id.map(Value::Text).unwrap_or_else(make_none),
            );
            rec.insert(
                "inReplyTo".to_string(),
                msg.in_reply_to.map(Value::Text).unwrap_or_else(make_none),
            );
            rec.insert(
                "references".to_string(),
                Value::List(Arc::new(msg.references.into_iter().map(Value::Text).collect())),
            );
            rec.insert(
                "textBody".to_string(),
                msg.text_body.map(Value::Text).unwrap_or_else(make_none),
            );
            rec.insert(
                "htmlBody".to_string(),
                msg.html_body.map(Value::Text).unwrap_or_else(make_none),
            );
            rec.insert("body".to_string(), Value::Text(msg.body));
            rec.insert("rawRfc822".to_string(), Value::Text(msg.raw_rfc822));
            Value::Record(Arc::new(rec))
        })
        .collect()
}

fn mailbox_info_to_value(info: aivi_email::MailboxInfo) -> Value {
    let mut rec = HashMap::new();
    rec.insert("name".to_string(), Value::Text(info.name));
    rec.insert(
        "separator".to_string(),
        info.separator.map(Value::Text).unwrap_or_else(make_none),
    );
    rec.insert(
        "attributes".to_string(),
        Value::List(Arc::new(
            info.attributes.into_iter().map(Value::Text).collect(),
        )),
    );
    Value::Record(Arc::new(rec))
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

fn required_text_list(
    record: &HashMap<String, Value>,
    field: &str,
    ctx: &str,
) -> Result<Vec<String>, RuntimeError> {
    let value = record
        .get(field)
        .cloned()
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects `{field}`")))?;
    expect_text_list(value, ctx)
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

fn optional_text_list(
    record: &HashMap<String, Value>,
    field: &str,
    ctx: &str,
) -> Result<Vec<String>, RuntimeError> {
    let Some(value) = record.get(field).cloned() else {
        return Ok(Vec::new());
    };
    match value {
        Value::List(items) => items.iter().map(|v| expect_text(v.clone(), ctx)).collect(),
        Value::Constructor { name, mut args } if name == "Some" && args.len() == 1 => {
            expect_text_list(args.remove(0), ctx)
        }
        Value::Constructor { name, args } if name == "None" && args.is_empty() => Ok(Vec::new()),
        other => Err(RuntimeError::Message(format!(
            "{ctx} expected `{field}` as Option (List Text), got {}",
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

fn optional_bool(
    record: &HashMap<String, Value>,
    field: &str,
    default: bool,
    ctx: &str,
) -> Result<bool, RuntimeError> {
    let Some(value) = record.get(field).cloned() else {
        return Ok(default);
    };
    match value {
        Value::Bool(b) => Ok(b),
        Value::Constructor { name, args } if name == "Some" && args.len() == 1 => match &args[0] {
            Value::Bool(b) => Ok(*b),
            other => Err(RuntimeError::Message(format!(
                "{ctx} expected `{field}` as Bool, got {}",
                crate::runtime::format_value(other)
            ))),
        },
        Value::Constructor { name, args } if name == "None" && args.is_empty() => Ok(default),
        other => Err(RuntimeError::Message(format!(
            "{ctx} expected `{field}` as Bool/Option Bool, got {}",
            crate::runtime::format_value(&other)
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn some(value: Value) -> Value {
        Value::Constructor {
            name: "Some".to_string(),
            args: vec![value],
        }
    }

    fn password(value: &str) -> Value {
        Value::Constructor {
            name: "Password".to_string(),
            args: vec![Value::Text(value.to_string())],
        }
    }

    fn text(value: &str) -> Value {
        Value::Text(value.to_string())
    }

    #[test]
    fn build_imap_config_applies_spec_defaults() {
        let record = HashMap::from([
            ("host".to_string(), text("imap.example.com")),
            ("user".to_string(), text("user@example.com")),
            ("auth".to_string(), password("secret")),
        ]);

        let config = match build_imap_config(&record) {
            Ok(config) => config,
            Err(_) => panic!("imap config"),
        };
        assert_eq!(config.host, "imap.example.com");
        assert_eq!(config.user, "user@example.com");
        assert_eq!(config.mailbox, "INBOX");
        assert_eq!(config.filter, "ALL");
        assert_eq!(config.limit, 50);
        assert_eq!(config.port, 993);
        assert!(!config.starttls);
    }

    #[test]
    fn build_smtp_config_unwraps_optional_recipient_lists() {
        let record = HashMap::from([
            ("host".to_string(), text("smtp.example.com")),
            ("user".to_string(), text("user@example.com")),
            ("auth".to_string(), password("secret")),
            ("from".to_string(), text("from@example.com")),
            (
                "to".to_string(),
                Value::List(Arc::new(vec![text("to@example.com")])),
            ),
            (
                "cc".to_string(),
                some(Value::List(Arc::new(vec![text("cc@example.com")]))),
            ),
            (
                "bcc".to_string(),
                Value::Constructor {
                    name: "None".to_string(),
                    args: vec![],
                },
            ),
            ("subject".to_string(), text("hello")),
            ("body".to_string(), text("world")),
            ("port".to_string(), some(Value::Int(2525))),
            ("starttls".to_string(), some(Value::Bool(true))),
        ]);

        let config = match build_smtp_config(&record) {
            Ok(config) => config,
            Err(_) => panic!("smtp config"),
        };
        assert_eq!(config.to, vec!["to@example.com"]);
        assert_eq!(config.cc, vec!["cc@example.com"]);
        assert!(config.bcc.is_empty());
        assert_eq!(config.port, 2525);
        assert!(config.starttls);
    }
}
