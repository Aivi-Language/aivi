use std::io::Write;
use std::sync::Arc;

use super::calendar::build_calendar_record;
use super::collections::build_collections_record;
use super::color::build_color_record;
use super::concurrency::build_concurrent_record;
use super::crypto::build_crypto_record;
use super::email::build_email_record;
use super::goa::build_goa_record;
use super::graph::build_graph_record;
use super::gtk4::build_gtk4_record;
use super::i18n::build_i18n_record;
use super::instant::build_instant_record;
use super::linalg::build_linalg_record;
use super::list::build_list_record;
use super::math::build_math_record;
use super::number::{build_bigint_record, build_decimal_record, build_rational_record};
use super::regex::build_regex_record;
use super::secrets::build_secrets_record;
use super::signal::build_signal_record;
use super::system::{
    build_clock_record, build_console_record, build_env_source_record, build_file_record,
    build_random_record, build_system_record,
};
use super::text::build_text_record;
use super::timezone::build_timezone_record;
use super::ui::build_ui_record;
use super::url_http::{
    build_http_client_record, build_rest_api_record, build_url_record, HttpClientMode,
};
use super::util::{builtin, builtin_constructor};
use super::{database::build_database_record, log::build_log_record};
use crate::runtime::http::build_http_server_record;
use crate::runtime::{format_value, EffectValue, Env, RuntimeError, Value};

pub(crate) fn register_builtins(env: &Env) {
    env.set("Unit".to_string(), Value::Unit);
    env.set("True".to_string(), Value::Bool(true));
    env.set("False".to_string(), Value::Bool(false));
    env.set(
        "None".to_string(),
        Value::Constructor {
            name: "None".to_string(),
            args: Vec::new(),
        },
    );
    env.set("Some".to_string(), builtin_constructor("Some", 1));
    env.set("Ok".to_string(), builtin_constructor("Ok", 1));
    env.set("Err".to_string(), builtin_constructor("Err", 1));
    env.set(
        "Closed".to_string(),
        Value::Constructor {
            name: "Closed".to_string(),
            args: Vec::new(),
        },
    );

    env.set(
        "foldGen".to_string(),
        builtin("foldGen", 3, |mut args, runtime| {
            let init = args.pop().unwrap();
            let step = args.pop().unwrap();
            let gen = args.pop().unwrap();
            let with_step = runtime.apply(gen, step)?;
            let result = runtime.apply(with_step, init)?;
            Ok(result)
        }),
    );

    env.set(
        "constructorName".to_string(),
        builtin("constructorName", 1, |mut args, _| {
            let value = args.pop().unwrap();
            match value {
                Value::Constructor { name, .. } => Ok(Value::Text(name)),
                other => Err(RuntimeError::Message(format!(
                    "constructorName expects an ADT constructor value, got {}",
                    format_value(&other)
                ))),
            }
        }),
    );

    env.set(
        "constructorOrdinal".to_string(),
        builtin("constructorOrdinal", 1, |mut args, runtime| {
            let value = args.pop().unwrap();
            let Value::Constructor { name, .. } = value else {
                return Err(RuntimeError::Message(
                    "constructorOrdinal expects an ADT constructor value".to_string(),
                ));
            };
            match runtime.ctx.constructor_ordinal(&name) {
                Some(Some(ordinal)) => Ok(Value::Int(ordinal as i64)),
                Some(None) => Err(RuntimeError::Message(format!(
                    "constructorOrdinal is ambiguous for constructor {name}"
                ))),
                None => Err(RuntimeError::Message(format!(
                    "constructorOrdinal does not know constructor {name}"
                ))),
            }
        }),
    );

    env.set(
        "map".to_string(),
        builtin("map", 2, |mut args, runtime| {
            let container = args.pop().unwrap();
            let func = args.pop().unwrap();
            match container {
                Value::List(items) => {
                    let mut out = Vec::with_capacity(items.len());
                    for item in items.iter().cloned() {
                        out.push(runtime.apply(func.clone(), item)?);
                    }
                    Ok(Value::List(Arc::new(out)))
                }
                Value::Constructor { name, args } if name == "None" && args.is_empty() => {
                    Ok(Value::Constructor {
                        name: "None".to_string(),
                        args: Vec::new(),
                    })
                }
                Value::Constructor { name, args } if name == "Some" && args.len() == 1 => {
                    let mapped = runtime.apply(func, args[0].clone())?;
                    Ok(Value::Constructor {
                        name: "Some".to_string(),
                        args: vec![mapped],
                    })
                }
                Value::Constructor { name, args } if name == "Ok" && args.len() == 1 => {
                    let mapped = runtime.apply(func, args[0].clone())?;
                    Ok(Value::Constructor {
                        name: "Ok".to_string(),
                        args: vec![mapped],
                    })
                }
                Value::Constructor { name, args } if name == "Err" && args.len() == 1 => {
                    Ok(Value::Constructor {
                        name: "Err".to_string(),
                        args,
                    })
                }
                other => Err(RuntimeError::Message(format!(
                    "map expects List/Option/Result, got {}",
                    format_value(&other)
                ))),
            }
        }),
    );

    env.set(
        "chain".to_string(),
        builtin("chain", 2, |mut args, runtime| {
            let container = args.pop().unwrap();
            let func = args.pop().unwrap();
            match container {
                Value::List(items) => {
                    let mut out = Vec::new();
                    for item in items.iter().cloned() {
                        let value = runtime.apply(func.clone(), item)?;
                        match value {
                            Value::List(inner) => out.extend(inner.iter().cloned()),
                            other => {
                                return Err(RuntimeError::Message(format!(
                                    "chain on List expects f : A -> List B, got {}",
                                    format_value(&other)
                                )))
                            }
                        }
                    }
                    Ok(Value::List(Arc::new(out)))
                }
                Value::Constructor { name, args } if name == "None" && args.is_empty() => {
                    Ok(Value::Constructor {
                        name: "None".to_string(),
                        args: Vec::new(),
                    })
                }
                Value::Constructor { name, args } if name == "Some" && args.len() == 1 => {
                    runtime.apply(func, args[0].clone())
                }
                Value::Constructor { name, args } if name == "Ok" && args.len() == 1 => {
                    runtime.apply(func, args[0].clone())
                }
                Value::Constructor { name, args } if name == "Err" && args.len() == 1 => {
                    Ok(Value::Constructor {
                        name: "Err".to_string(),
                        args,
                    })
                }
                other => Err(RuntimeError::Message(format!(
                    "chain expects List/Option/Result, got {}",
                    format_value(&other)
                ))),
            }
        }),
    );

    env.set(
        "assertEq".to_string(),
        builtin("assertEq", 2, |mut args, _| {
            let right = args.pop().unwrap();
            let left = args.pop().unwrap();
            let ok = super::super::values_equal(&left, &right);
            let effect = EffectValue::Thunk {
                func: std::sync::Arc::new(move |_| {
                    if ok {
                        Ok(Value::Unit)
                    } else {
                        Err(RuntimeError::Error(Value::Text(format!(
                            "assertEq failed: left={}, right={}",
                            format_value(&left),
                            format_value(&right)
                        ))))
                    }
                }),
            };
            Ok(Value::Effect(std::sync::Arc::new(effect)))
        }),
    );

    env.set(
        "pure".to_string(),
        builtin("pure", 1, |mut args, _| {
            let value = args.remove(0);
            let effect = EffectValue::Thunk {
                func: std::sync::Arc::new(move |_| Ok(value.clone())),
            };
            Ok(Value::Effect(std::sync::Arc::new(effect)))
        }),
    );

    env.set(
        "fail".to_string(),
        builtin("fail", 1, |mut args, _| {
            let value = args.remove(0);
            let effect = EffectValue::Thunk {
                func: std::sync::Arc::new(move |_| Err(RuntimeError::Error(value.clone()))),
            };
            Ok(Value::Effect(std::sync::Arc::new(effect)))
        }),
    );

    env.set(
        "bind".to_string(),
        builtin("bind", 2, |mut args, _| {
            let func = args.pop().unwrap();
            let effect = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: std::sync::Arc::new(move |runtime| {
                    let value = runtime.run_effect_value(effect.clone())?;
                    let applied = runtime.apply(func.clone(), value)?;
                    runtime.run_effect_value(applied)
                }),
            };
            Ok(Value::Effect(std::sync::Arc::new(effect)))
        }),
    );

    env.set(
        "attempt".to_string(),
        builtin("attempt", 1, |mut args, _| {
            let effect = args.remove(0);
            let effect = EffectValue::Thunk {
                func: std::sync::Arc::new(move |runtime| {
                    match runtime.run_effect_value(effect.clone()) {
                        Ok(value) => Ok(Value::Constructor {
                            name: "Ok".to_string(),
                            args: vec![value],
                        }),
                        Err(RuntimeError::Error(value)) => Ok(Value::Constructor {
                            name: "Err".to_string(),
                            args: vec![value],
                        }),
                        Err(err) => Err(err),
                    }
                }),
            };
            Ok(Value::Effect(std::sync::Arc::new(effect)))
        }),
    );

    env.set(
        "print".to_string(),
        builtin("print", 1, |mut args, _| {
            let value = args.remove(0);
            let text = format_value(&value);
            let effect = EffectValue::Thunk {
                func: std::sync::Arc::new(move |_| {
                    print!("{text}");
                    let mut out = std::io::stdout();
                    let _ = out.flush();
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(std::sync::Arc::new(effect)))
        }),
    );

    env.set(
        "println".to_string(),
        builtin("println", 1, |mut args, _| {
            let value = args.remove(0);
            let text = format_value(&value);
            let effect = EffectValue::Thunk {
                func: std::sync::Arc::new(move |_| {
                    println!("{text}");
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(std::sync::Arc::new(effect)))
        }),
    );

    env.set(
        "load".to_string(),
        builtin("load", 1, |mut args, _| {
            let value = args.remove(0);
            match value {
                Value::Source(source) => Ok(Value::Effect(source.effect.clone())),
                // Back-compat: older code treated `load` as an `Effect`-identity.
                Value::Effect(_) => Ok(value),
                _ => Err(RuntimeError::Message("load expects a Source".to_string())),
            }
        }),
    );

    env.set("file".to_string(), build_file_record());
    env.set("env".to_string(), build_env_source_record());
    env.set("system".to_string(), build_system_record());
    env.set("clock".to_string(), build_clock_record());
    env.set("random".to_string(), build_random_record());
    env.set(
        "channel".to_string(),
        super::concurrency::build_channel_record(),
    );
    env.set("concurrent".to_string(), build_concurrent_record());
    env.set("httpServer".to_string(), build_http_server_record());
    env.set("ui".to_string(), build_ui_record());
    env.set("text".to_string(), build_text_record());
    env.set("regex".to_string(), build_regex_record());
    env.set("math".to_string(), build_math_record());
    env.set("calendar".to_string(), build_calendar_record());
    env.set("instant".to_string(), build_instant_record());
    env.set("timezone".to_string(), build_timezone_record());
    env.set("color".to_string(), build_color_record());
    env.set("linalg".to_string(), build_linalg_record());
    env.set("signal".to_string(), build_signal_record());
    env.set("graph".to_string(), build_graph_record());
    env.set("bigint".to_string(), build_bigint_record());
    env.set("rational".to_string(), build_rational_record());
    env.set("decimal".to_string(), build_decimal_record());
    env.set("url".to_string(), build_url_record());
    env.set(
        "http".to_string(),
        build_http_client_record(HttpClientMode::Http),
    );
    env.set(
        "https".to_string(),
        build_http_client_record(HttpClientMode::Https),
    );
    env.set("rest".to_string(), build_rest_api_record());
    env.set("email".to_string(), build_email_record());
    env.set(
        "sockets".to_string(),
        super::sockets::build_sockets_record(),
    );
    env.set(
        "streams".to_string(),
        super::streams::build_streams_record(),
    );
    env.set("List".to_string(), build_list_record());
    let collections = build_collections_record();
    if let Value::Record(fields) = &collections {
        if let Some(map) = fields.get("map") {
            env.set("Map".to_string(), map.clone());
        }
        if let Some(set) = fields.get("set") {
            env.set("Set".to_string(), set.clone());
        }
        if let Some(queue) = fields.get("queue") {
            env.set("Queue".to_string(), queue.clone());
        }
        if let Some(deque) = fields.get("deque") {
            env.set("Deque".to_string(), deque.clone());
        }
        if let Some(heap) = fields.get("heap") {
            env.set("Heap".to_string(), heap.clone());
        }
    }
    env.set("collections".to_string(), collections);
    env.set("console".to_string(), build_console_record());
    env.set("crypto".to_string(), build_crypto_record());
    env.set("logger".to_string(), build_log_record());
    env.set("database".to_string(), build_database_record());
    env.set("goa".to_string(), build_goa_record());
    env.set("gtk4".to_string(), build_gtk4_record());
    env.set("secrets".to_string(), build_secrets_record());
    env.set("i18n".to_string(), build_i18n_record());
}
