use std::io::Write;
use std::sync::{Arc, Mutex};

use super::calendar::build_calendar_record;
use super::collections::build_collections_record;
use super::color::build_color_record;
use super::concurrency::build_concurrent_record;
use super::crypto::build_crypto_record;
use super::email::build_email_record;
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
use super::json::build_json_record;
use super::text::build_text_record;
use super::timezone::build_timezone_record;
use super::ui::build_ui_record;
use super::url_http::{
    build_http_client_record, build_rest_api_record, build_url_record, HttpClientMode,
};
use super::util::{builtin, builtin_constructor};
use super::{database::build_database_record, log::build_log_record};
use crate::runtime::http::build_http_server_record;
use crate::runtime::{format_value, BuiltinImpl, BuiltinValue, EffectValue, Env, Runtime, RuntimeError, Value};

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
                        Ok(value) => {
                            // Check for JIT errors that couldn't propagate through
                            // native code boundaries (e.g. `fail` inside JIT code).
                            if let Some(err) = runtime.jit_pending_error.take() {
                                match err {
                                    RuntimeError::Error(v) => Ok(Value::Constructor {
                                        name: "Err".to_string(),
                                        args: vec![v],
                                    }),
                                    other => Err(other),
                                }
                            } else {
                                Ok(Value::Constructor {
                                    name: "Ok".to_string(),
                                    args: vec![value],
                                })
                            }
                        }
                        Err(RuntimeError::Error(value)) => {
                            Ok(Value::Constructor {
                                name: "Err".to_string(),
                                args: vec![value],
                            })
                        }
                        Err(err) => {
                            Err(err)
                        }
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
        builtin("load", 1, |mut args, _runtime| {
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
    env.set("json".to_string(), build_json_record());
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
    env.set("gtk4".to_string(), build_gtk4_record());
    env.set("secrets".to_string(), build_secrets_record());
    env.set("i18n".to_string(), build_i18n_record());

    // assertSnapshot : Text -> A -> Effect Text Unit
    env.set(
        "__assertSnapshot".to_string(),
        builtin("__assertSnapshot", 2, |mut args, _runtime| {
            let value = args.pop().unwrap();
            let name_val = args.pop().unwrap();
            let name = match &name_val {
                Value::Text(t) => t.clone(),
                other => {
                    return Err(RuntimeError::Message(format!(
                        "assertSnapshot: name must be Text, got {}",
                        format_value(other)
                    )))
                }
            };

            let effect = EffectValue::Thunk {
                func: std::sync::Arc::new(move |runtime| {
                    use crate::runtime::snapshot::{
                        snapshot_file, value_to_snapshot_json,
                    };

                    let test_name = runtime
                        .current_test_name
                        .as_deref()
                        .ok_or_else(|| {
                            RuntimeError::Message(
                                "assertSnapshot: not inside a @test".to_string(),
                            )
                        })?;
                    let root = runtime
                        .project_root
                        .as_deref()
                        .ok_or_else(|| {
                            RuntimeError::Message(
                                "assertSnapshot: no project root configured".to_string(),
                            )
                        })?;

                    let json = value_to_snapshot_json(&value)?;
                    let pretty =
                        serde_json::to_string_pretty(&json).map_err(|e| {
                            RuntimeError::Message(format!(
                                "assertSnapshot: JSON serialization failed: {e}"
                            ))
                        })?;

                    let path = snapshot_file(root, test_name, &name);

                    if runtime.update_snapshots {
                        if let Some(parent) = path.parent() {
                            std::fs::create_dir_all(parent).map_err(|e| {
                                RuntimeError::Message(format!(
                                    "assertSnapshot: cannot create directory: {e}"
                                ))
                            })?;
                        }
                        std::fs::write(&path, &pretty).map_err(|e| {
                            RuntimeError::Message(format!(
                                "assertSnapshot: cannot write snapshot: {e}"
                            ))
                        })?;
                        Ok(Value::Unit)
                    } else {
                        let existing = std::fs::read_to_string(&path).map_err(|_| {
                            RuntimeError::Error(Value::Text(format!(
                                "snapshot file not found: {}; run with --update-snapshots",
                                path.display()
                            )))
                        })?;
                        if existing.trim() == pretty.trim() {
                            Ok(Value::Unit)
                        } else {
                            let msg = format!(
                                "snapshot mismatch for \"{name}\":\n--- expected (snapshot)\n{existing}\n--- actual\n{pretty}"
                            );
                            // Store the mismatch on the runtime so the JIT test
                            // runner can detect it even when Effect error
                            // propagation is not available.
                            runtime.snapshot_failure = Some(msg.clone());
                            Err(RuntimeError::Error(Value::Text(msg)))
                        }
                    }
                }),
            };
            Ok(Value::Effect(std::sync::Arc::new(effect)))
        }),
    );

    // __asGenerator : a -> Generator a
    // If the value is a List, converts it to a generator (fold function).
    // Otherwise, assumes it's already a generator and passes it through.
    env.set(
        "__asGenerator".to_string(),
        builtin("__asGenerator", 1, |mut args, _runtime| {
            let val = args.pop().unwrap();
            match val {
                Value::Unit => {
                    // Unit → empty generator: \k -> \z -> z
                    Ok(builtin("__asGenerator.empty", 2, |mut args, _rt| {
                        let z = args.pop().unwrap();
                        let _k = args.pop().unwrap();
                        Ok(z)
                    }))
                }
                Value::List(items) => {
                    // Return a generator: \k -> \z -> foldl k z items
                    let items = Arc::clone(&items);
                    Ok(builtin("__asGenerator.gen", 2, move |mut args, _rt| {
                        let z = args.pop().unwrap();
                        let k = args.pop().unwrap();
                        let mut acc = z;
                        for item in items.iter() {
                            let k_applied = match &k {
                                Value::Builtin(b) => b.apply(acc.clone(), _rt)?,
                                _ => _rt.apply(k.clone(), acc.clone())?,
                            };
                            acc = match &k_applied {
                                Value::Builtin(b) => b.apply(item.clone(), _rt)?,
                                _ => _rt.apply(k_applied, item.clone())?,
                            };
                        }
                        Ok(acc)
                    }))
                }
                // Already a generator (function) — pass through
                other => Ok(other),
            }
        }),
    );

    // __makeResource : (Unit -> Effect a) -> (Unit -> Effect Unit) -> Resource a
    // Creates a Resource value from an acquire closure and a cleanup closure.
    env.set(
        "__makeResource".to_string(),
        builtin("__makeResource", 2, |mut args, _runtime| {
            let cleanup_fn = args.pop().unwrap();
            let acquire_fn = args.pop().unwrap();
            let resource = crate::runtime::values::ResourceValue {
                acquire: Arc::new(move |runtime: &mut crate::runtime::Runtime| {
                    let result = runtime.apply(acquire_fn.clone(), Value::Unit)?;
                    runtime.run_effect_value(result)
                }),
                cleanup: Arc::new(move |runtime: &mut crate::runtime::Runtime| {
                    let result = runtime.apply(cleanup_fn.clone(), Value::Unit)?;
                    runtime.run_effect_value(result)
                }),
            };
            Ok(Value::Resource(Arc::new(resource)))
        }),
    );

    // __withResourceScope : Effect E A -> Effect E A
    // Wraps an effect in a resource scope: push scope, run effect, pop scope
    // (running all resource cleanups registered during the effect, LIFO).
    env.set(
        "__withResourceScope".to_string(),
        builtin("__withResourceScope", 1, |mut args, _runtime| {
            let effect = args.pop().unwrap();
            Ok(Value::Effect(Arc::new(
                crate::runtime::values::EffectValue::Thunk {
                    func: Arc::new(move |runtime: &mut crate::runtime::Runtime| {
                        runtime.push_resource_scope();
                        let result = runtime.run_effect_value(effect.clone());
                        runtime.pop_resource_scope();
                        result
                    }),
                },
            )))
        }),
    );

    // __fix : (A -> A) -> A
    // Fixpoint combinator for recursive let-bindings (e.g. __loop* in generate blocks).
    // `__fix f` returns a value `v` such that `v = f v`.
    env.set(
        "__fix".to_string(),
        builtin("__fix", 1, |mut args, runtime| {
            let f = args.pop().unwrap();
            let cell: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
            let cell_clone = cell.clone();
            // Proxy closure that forwards calls to the fixpoint value
            let proxy = Value::Builtin(BuiltinValue {
                imp: Arc::new(BuiltinImpl {
                    name: "__fix.proxy".to_string(),
                    arity: 1,
                    func: Arc::new(move |mut args: Vec<Value>, runtime: &mut Runtime| {
                        let arg = args.pop().unwrap();
                        let inner = cell_clone.lock().unwrap().clone().unwrap_or(Value::Unit);
                        runtime.apply(inner, arg)
                    }),
                }),
                args: Vec::new(),
                tagged_args: Some(Vec::new()),
            });
            let result = runtime.apply(f, proxy)?;
            *cell.lock().unwrap() = Some(result.clone());
            Ok(result)
        }),
    );
}
