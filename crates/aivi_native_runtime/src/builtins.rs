use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, OnceLock};

use crate::values::{
    format_value, values_equal, BuiltinImpl, BuiltinValue, EffectValue, Runtime, RuntimeError,
    Value,
};

fn builtin(
    name: &str,
    arity: usize,
    f: impl Fn(Vec<Value>, &mut Runtime) -> Result<Value, RuntimeError> + Send + Sync + 'static,
) -> Value {
    let imp = Arc::new(BuiltinImpl {
        name: name.to_string(),
        arity,
        func: Arc::new(f),
    });
    Value::Builtin(BuiltinValue {
        imp,
        args: Vec::new(),
    })
}

fn builtin_constructor(name: &str, arity: usize) -> Value {
    let ctor_name = name.to_string();
    builtin(name, arity, move |args, _| {
        Ok(Value::Constructor {
            name: ctor_name.clone(),
            args,
        })
    })
}

pub fn get_builtin(name: &str) -> Option<Value> {
    BUILTINS.get_or_init(build_all).get(name).cloned()
}

static BUILTINS: OnceLock<HashMap<String, Value>> = OnceLock::new();

fn build_all() -> HashMap<String, Value> {
    let mut env = HashMap::new();

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
                func: Arc::new(move |_| Err(format_value(&value))),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    env.insert(
        "attempt".to_string(),
        builtin("attempt", 1, |mut args, _| {
            let effect = args.remove(0);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let result = runtime.run_effect_value(effect.clone());
                    match result {
                        Ok(value) => Ok(Value::Constructor {
                            name: "Ok".to_string(),
                            args: vec![value],
                        }),
                        Err(err) => Ok(Value::Constructor {
                            name: "Err".to_string(),
                            args: vec![Value::Text(err)],
                        }),
                    }
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
                Value::Effect(_) => Ok(value),
                _ => Err("load expects an Effect".to_string()),
            }
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
        "print".to_string(),
        builtin("print", 1, |mut args, _| {
            let value = args.remove(0);
            let text = format_value(&value);
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    print!("{text}");
                    let _ = std::io::stdout().flush();
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
                        Err(format!(
                            "assertEq failed: left={}, right={}",
                            format_value(&left),
                            format_value(&right)
                        ))
                    }
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    // List/Option/Result mapping behavior, used heavily by stdlib.
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
                Value::Constructor { name, args } if name == "None" && args.is_empty() => Ok(
                    Value::Constructor {
                        name: "None".to_string(),
                        args: Vec::new(),
                    },
                ),
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
                Value::Constructor { name, args } if name == "Err" && args.len() == 1 => Ok(
                    Value::Constructor {
                        name: "Err".to_string(),
                        args,
                    },
                ),
                other => Err(format!(
                    "map expects List/Option/Result, got {}",
                    format_value(&other)
                )),
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
                                return Err(format!(
                                    "chain on List expects f : A -> List B, got {}",
                                    format_value(&other)
                                ))
                            }
                        }
                    }
                    Ok(Value::List(Arc::new(out)))
                }
                Value::Constructor { name, args } if name == "None" && args.is_empty() => Ok(
                    Value::Constructor {
                        name: "None".to_string(),
                        args: Vec::new(),
                    },
                ),
                Value::Constructor { name, args } if name == "Some" && args.len() == 1 => {
                    runtime.apply(func, args[0].clone())
                }
                Value::Constructor { name, args } if name == "Ok" && args.len() == 1 => {
                    runtime.apply(func, args[0].clone())
                }
                Value::Constructor { name, args } if name == "Err" && args.len() == 1 => Ok(
                    Value::Constructor {
                        name: "Err".to_string(),
                        args,
                    },
                ),
                other => Err(format!(
                    "chain expects List/Option/Result, got {}",
                    format_value(&other)
                )),
            }
        }),
    );

    // Placeholder records/types for now. These keep stdlib compiling even before
    // we port the full interpreter builtins.
    for (name, value) in placeholder_records() {
        env.insert(name, value);
    }

    env
}

fn placeholder_records() -> Vec<(String, Value)> {
    let mut out = Vec::new();
    for name in [
        "file",
        "system",
        "clock",
        "random",
        "channel",
        "concurrent",
        "httpServer",
        "text",
        "regex",
        "math",
        "calendar",
        "color",
        "linalg",
        "signal",
        "graph",
        "bigint",
        "rational",
        "decimal",
        "url",
        "http",
        "https",
        "sockets",
        "streams",
        "collections",
        "console",
        "crypto",
        "logger",
        "database",
    ] {
        out.push((name.to_string(), Value::Record(Arc::new(HashMap::new()))));
    }
    for name in ["Map", "Set", "Queue", "Deque", "Heap"] {
        out.push((name.to_string(), Value::Record(Arc::new(HashMap::new()))));
    }
    out
}
