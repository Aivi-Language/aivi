use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;

use crate::values::{values_equal, EffectValue};
use crate::{format_value, RuntimeError, Value};

use super::calendar::build_calendar_record;
use super::collections::build_collections_record;
use super::color::build_color_record;
use super::concurrency::{build_channel_record, build_concurrent_record};
use super::crypto::build_crypto_record;
use super::database::build_database_record;
use super::graph::build_graph_record;
use super::http_server::build_http_server_record;
use super::i18n::build_i18n_record;
use super::instant::build_instant_record;
use super::linalg::build_linalg_record;
use super::log::build_log_record;
use super::math::build_math_record;
use super::mutable_map::build_mutable_map_record;
use super::number::{build_bigint_record, build_decimal_record, build_rational_record};
use super::regex::build_regex_record;
use super::signal::build_signal_record;
use super::sockets::build_sockets_record;
use super::streams::build_streams_record;
use super::system::{
    build_clock_record, build_console_record, build_env_source_record, build_file_record,
    build_random_record, build_system_record,
};
use super::text::build_text_record;
use super::ui::build_ui_record;
use super::url_http::{build_http_client_record, build_url_record, HttpClientMode};
use super::util::{builtin, builtin_constructor};

pub(super) fn register_builtins(env: &mut HashMap<String, Value>) {
    env.insert("Unit".to_string(), Value::Unit);
    env.insert("True".to_string(), Value::Bool(true));
    env.insert("False".to_string(), Value::Bool(false));
    env.insert(
        "None".to_string(),
        Value::Constructor {
            name: "None".to_string(),
            args: Vec::new(),
        },
    );
    env.insert("Some".to_string(), builtin_constructor("Some", 1));
    env.insert("Ok".to_string(), builtin_constructor("Ok", 1));
    env.insert("Err".to_string(), builtin_constructor("Err", 1));
    env.insert(
        "Closed".to_string(),
        Value::Constructor {
            name: "Closed".to_string(),
            args: Vec::new(),
        },
    );

    env.insert(
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

    env.insert(
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

    env.insert(
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

    env.insert(
        "assertEq".to_string(),
        builtin("assertEq", 2, |mut args, _| {
            let right = args.pop().unwrap();
            let left = args.pop().unwrap();
            let ok = values_equal(&left, &right);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
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
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    env.insert(
        "pure".to_string(),
        builtin("pure", 1, |mut args, _| {
            let value = args.remove(0);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| Ok(value.clone())),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    env.insert(
        "fail".to_string(),
        builtin("fail", 1, |mut args, _| {
            let value = args.remove(0);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| Err(RuntimeError::Error(value.clone()))),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    env.insert(
        "bind".to_string(),
        builtin("bind", 2, |mut args, _| {
            let func = args.pop().unwrap();
            let effect = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let value = runtime.run_effect_value(effect.clone())?;
                    let applied = runtime.apply(func.clone(), value)?;
                    runtime.run_effect_value(applied)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    env.insert(
        "attempt".to_string(),
        builtin("attempt", 1, |mut args, _| {
            let effect = args.remove(0);
            let effect = EffectValue::Thunk {
                func: Arc::new(
                    move |runtime| match runtime.run_effect_value(effect.clone()) {
                        Ok(value) => Ok(Value::Constructor {
                            name: "Ok".to_string(),
                            args: vec![value],
                        }),
                        Err(RuntimeError::Error(value)) => Ok(Value::Constructor {
                            name: "Err".to_string(),
                            args: vec![value],
                        }),
                        Err(err) => Err(err),
                    },
                ),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    env.insert(
        "print".to_string(),
        builtin("print", 1, |mut args, _| {
            let value = args.remove(0);
            let text = format_value(&value);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    print!("{text}");
                    let mut out = std::io::stdout();
                    let _ = out.flush();
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    env.insert(
        "println".to_string(),
        builtin("println", 1, |mut args, _| {
            let value = args.remove(0);
            let text = format_value(&value);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    println!("{text}");
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    env.insert(
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

    env.insert("file".to_string(), build_file_record());
    env.insert("env".to_string(), build_env_source_record());
    env.insert("system".to_string(), build_system_record());
    env.insert("clock".to_string(), build_clock_record());
    env.insert("random".to_string(), build_random_record());
    env.insert("channel".to_string(), build_channel_record());
    env.insert("concurrent".to_string(), build_concurrent_record());
    env.insert("httpServer".to_string(), build_http_server_record());
    env.insert("ui".to_string(), build_ui_record());
    env.insert("text".to_string(), build_text_record());
    env.insert("regex".to_string(), build_regex_record());
    env.insert("math".to_string(), build_math_record());
    env.insert("calendar".to_string(), build_calendar_record());
    env.insert("instant".to_string(), build_instant_record());
    env.insert("color".to_string(), build_color_record());
    env.insert("linalg".to_string(), build_linalg_record());
    env.insert("signal".to_string(), build_signal_record());
    env.insert("graph".to_string(), build_graph_record());
    env.insert("bigint".to_string(), build_bigint_record());
    env.insert("rational".to_string(), build_rational_record());
    env.insert("decimal".to_string(), build_decimal_record());
    env.insert("url".to_string(), build_url_record());
    env.insert(
        "http".to_string(),
        build_http_client_record(HttpClientMode::Http),
    );
    env.insert(
        "https".to_string(),
        build_http_client_record(HttpClientMode::Https),
    );
    env.insert("sockets".to_string(), build_sockets_record());
    env.insert("streams".to_string(), build_streams_record());
    let collections = build_collections_record();
    if let Value::Record(fields) = &collections {
        if let Some(map) = fields.get("map") {
            env.insert("Map".to_string(), map.clone());
        }
        if let Some(set) = fields.get("set") {
            env.insert("Set".to_string(), set.clone());
        }
        if let Some(queue) = fields.get("queue") {
            env.insert("Queue".to_string(), queue.clone());
        }
        if let Some(deque) = fields.get("deque") {
            env.insert("Deque".to_string(), deque.clone());
        }
        if let Some(heap) = fields.get("heap") {
            env.insert("Heap".to_string(), heap.clone());
        }
    }
    env.insert("collections".to_string(), collections);
    env.insert("MutableMap".to_string(), build_mutable_map_record());
    env.insert("console".to_string(), build_console_record());
    env.insert("crypto".to_string(), build_crypto_record());
    env.insert("logger".to_string(), build_log_record());
    env.insert("database".to_string(), build_database_record());
    env.insert("i18n".to_string(), build_i18n_record());
}
