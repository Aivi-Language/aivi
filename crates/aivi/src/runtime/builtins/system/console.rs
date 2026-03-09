use std::collections::HashMap;
use std::io::IsTerminal;
use std::sync::Arc;

#[cfg(unix)]
use std::os::fd::AsRawFd;

use super::super::util::{builtin, expect_record, expect_text, make_err, make_ok};
use crate::runtime::{format_value, write_stderr, write_stdout, EffectValue, RuntimeError, Value};

pub(in crate::runtime::builtins) fn build_console_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "log".to_string(),
        builtin("console.log", 1, |mut args, _| {
            let value = args.remove(0);
            let text = format_value(&value);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    write_stdout(runtime, &text, true);
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "println".to_string(),
        builtin("console.println", 1, |mut args, _| {
            let value = args.remove(0);
            let text = format_value(&value);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    write_stdout(runtime, &text, true);
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "print".to_string(),
        builtin("console.print", 1, |mut args, _| {
            let value = args.remove(0);
            let text = format_value(&value);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    write_stdout(runtime, &text, false);
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "error".to_string(),
        builtin("console.error", 1, |mut args, _| {
            let value = args.remove(0);
            let text = format_value(&value);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    write_stderr(runtime, &text, true);
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "readLine".to_string(),
        builtin("console.readLine", 1, |_, _| {
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let stdin = std::io::stdin();
                    if !stdin.is_terminal() && !stdin_is_ready(&stdin) {
                        return Ok(make_err(Value::Text("stdin is not ready".to_string())));
                    }
                    let mut buffer = String::new();
                    match stdin.read_line(&mut buffer) {
                        Ok(0) => Ok(make_err(Value::Text("end of input".to_string()))),
                        Ok(_) => Ok(make_ok(Value::Text(
                            buffer.trim_end_matches(&['\n', '\r'][..]).to_string(),
                        ))),
                        Err(err) => Ok(make_err(Value::Text(err.to_string()))),
                    }
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "color".to_string(),
        builtin("console.color", 2, |mut args, _| {
            let value = expect_text(args.pop().unwrap(), "console.color")?;
            let color = ansi_color(args.pop().unwrap(), false, "console.color")?;
            Ok(Value::Text(apply_ansi(&[color], &value)))
        }),
    );
    fields.insert(
        "bgColor".to_string(),
        builtin("console.bgColor", 2, |mut args, _| {
            let value = expect_text(args.pop().unwrap(), "console.bgColor")?;
            let color = ansi_color(args.pop().unwrap(), true, "console.bgColor")?;
            Ok(Value::Text(apply_ansi(&[color], &value)))
        }),
    );
    fields.insert(
        "style".to_string(),
        builtin("console.style", 2, |mut args, _| {
            let value = expect_text(args.pop().unwrap(), "console.style")?;
            let style = args.pop().unwrap();
            let codes = style_codes(style, "console.style")?;
            Ok(Value::Text(apply_ansi(&codes, &value)))
        }),
    );
    fields.insert(
        "strip".to_string(),
        builtin("console.strip", 1, |mut args, _| {
            let value = expect_text(args.pop().unwrap(), "console.strip")?;
            Ok(Value::Text(strip_ansi(&value)))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn stdin_is_ready(stdin: &std::io::Stdin) -> bool {
    #[cfg(unix)]
    {
        let fd = stdin.as_raw_fd();
        let mut poll_fd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        unsafe { libc::poll(&mut poll_fd, 1, 0) > 0 && (poll_fd.revents & (libc::POLLIN | libc::POLLHUP)) != 0 }
    }

    #[cfg(not(unix))]
    {
        let _ = stdin;
        true
    }
}

fn ansi_color(value: Value, is_bg: bool, ctx: &str) -> Result<i64, RuntimeError> {
    let name = match value {
        Value::Constructor { name, args } if args.is_empty() => name,
        _ => return Err(RuntimeError::Message(format!("{ctx} expects AnsiColor"))),
    };
    let base = if is_bg { 40 } else { 30 };
    let code = match name.as_str() {
        "Black" => base,
        "Red" => base + 1,
        "Green" => base + 2,
        "Yellow" => base + 3,
        "Blue" => base + 4,
        "Magenta" => base + 5,
        "Cyan" => base + 6,
        "White" => base + 7,
        "Default" => {
            if is_bg {
                49
            } else {
                39
            }
        }
        _ => return Err(RuntimeError::Message(format!("{ctx} expects AnsiColor"))),
    };
    Ok(code)
}

fn style_codes(value: Value, ctx: &str) -> Result<Vec<i64>, RuntimeError> {
    let fields = expect_record(value, ctx)?;
    let fg = fields
        .get("fg")
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects fg")))?;
    let bg = fields
        .get("bg")
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects bg")))?;
    let mut codes = Vec::new();
    if let Some(code) = option_color(fg.clone(), false, ctx)? {
        codes.push(code);
    }
    if let Some(code) = option_color(bg.clone(), true, ctx)? {
        codes.push(code);
    }
    let flags = [
        ("bold", 1),
        ("dim", 2),
        ("italic", 3),
        ("underline", 4),
        ("blink", 5),
        ("inverse", 7),
        ("hidden", 8),
        ("strike", 9),
    ];
    for (field, code) in flags {
        let value = fields
            .get(field)
            .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects {field}")))?;
        if expect_bool(value.clone(), ctx)? {
            codes.push(code);
        }
    }
    Ok(codes)
}

fn option_color(value: Value, is_bg: bool, ctx: &str) -> Result<Option<i64>, RuntimeError> {
    match value {
        Value::Constructor { name, args } if name == "Some" && args.len() == 1 => {
            Ok(Some(ansi_color(args[0].clone(), is_bg, ctx)?))
        }
        Value::Constructor { name, args } if name == "None" && args.is_empty() => Ok(None),
        other => Err(RuntimeError::Message(format!(
            "{ctx} expects Option AnsiColor, got {}",
            format_value(&other)
        ))),
    }
}

fn expect_bool(value: Value, ctx: &str) -> Result<bool, RuntimeError> {
    match value {
        Value::Bool(value) => Ok(value),
        other => Err(RuntimeError::Message(format!(
            "{ctx} expects Bool, got {}",
            format_value(&other)
        ))),
    }
}

fn apply_ansi(codes: &[i64], value: &str) -> String {
    if codes.is_empty() {
        return value.to_string();
    }
    let joined = codes
        .iter()
        .map(|code| code.to_string())
        .collect::<Vec<_>>()
        .join(";");
    format!("\x1b[{joined}m{value}\x1b[0m")
}

fn strip_ansi(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for code in chars.by_ref() {
                if code == 'm' {
                    break;
                }
            }
            continue;
        }
        out.push(ch);
    }
    out
}
