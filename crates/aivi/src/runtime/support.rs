
fn record_get_path<'a>(record: &'a HashMap<String, Value>, path: &[String]) -> Option<&'a Value> {
    let mut current = record;
    let mut value = None;
    for (index, segment) in path.iter().enumerate() {
        let shaped = shape_record(current);
        value = if shaped.has_field(segment) {
            current.get(segment)
        } else {
            None
        };
        if index + 1 == path.len() {
            return value;
        }
        match value {
            Some(Value::Record(map)) => current = map.as_ref(),
            _ => return None,
        }
    }
    value
}

fn insert_record_path(
    record: &mut HashMap<String, Value>,
    path: &[HirPathSegment],
    value: Value,
) -> Result<(), RuntimeError> {
    if path.is_empty() {
        return Err(RuntimeError::Message(
            "record path must contain at least one segment".to_string(),
        ));
    }
    let mut current = record;
    for (index, segment) in path.iter().enumerate() {
        match segment {
            HirPathSegment::Field(name) => {
                if index + 1 == path.len() {
                    current.insert(name.clone(), value);
                    return Ok(());
                }
                let entry = current
                    .entry(name.clone())
                    .or_insert_with(|| Value::Record(Arc::new(HashMap::new())));
                match entry {
                    Value::Record(map) => {
                        current = Arc::make_mut(map);
                    }
                    _ => {
                        return Err(RuntimeError::Message(format!(
                            "record path conflict at {name}"
                        )))
                    }
                }
            }
            HirPathSegment::Index(_) | HirPathSegment::All => {
                return Err(RuntimeError::Message(
                    "record index path requires runtime-evaluated path segments".to_string(),
                ))
            }
        }
    }
    Ok(())
}

#[derive(Clone)]
enum RuntimePathSegment {
    Field(String),
    IndexValue(Value),
    IndexFieldBool(String),
    IndexPredicate(Value),
    IndexAll,
}

#[derive(Clone, Copy)]
enum PathUpdateMode {
    Assign,
    Patch,
}

fn list_or_tuple_index(index: &Value, target: &str) -> Result<usize, RuntimeError> {
    let Value::Int(raw) = index else {
        return Err(RuntimeError::Message(format!("{target} index expects an Int")));
    };
    if *raw < 0 {
        return Err(RuntimeError::Message("index out of bounds".to_string()));
    }
    Ok(*raw as usize)
}

fn map_index_key(index: &Value) -> Result<KeyValue, RuntimeError> {
    KeyValue::try_from_value(index).ok_or_else(|| {
        RuntimeError::Message(format!(
            "map key is not a valid key type: {}",
            format_value(index)
        ))
    })
}

fn read_indexed_value(base_value: Value, index_value: Value) -> Result<Value, RuntimeError> {
    match base_value {
        Value::List(items) => {
            let idx = list_or_tuple_index(&index_value, "list")?;
            items
                .get(idx)
                .cloned()
                .ok_or_else(|| RuntimeError::Message("index out of bounds".to_string()))
        }
        Value::Tuple(items) => {
            let idx = list_or_tuple_index(&index_value, "tuple")?;
            items
                .get(idx)
                .cloned()
                .ok_or_else(|| RuntimeError::Message("index out of bounds".to_string()))
        }
        Value::Map(entries) => {
            let key = map_index_key(&index_value)?;
            entries
                .get(&key)
                .cloned()
                .ok_or_else(|| RuntimeError::Message("missing map key".to_string()))
        }
        other => Err(RuntimeError::Message(format!(
            "expected List/Tuple/Map for indexed access, got {}",
            format_value(&other)
        ))),
    }
}

fn apply_value_path_update(
    runtime: &mut Runtime,
    target: Value,
    path: &[RuntimePathSegment],
    updater: Value,
    mode: PathUpdateMode,
) -> Result<Value, RuntimeError> {
    if path.is_empty() {
        return match mode {
            PathUpdateMode::Assign => Ok(updater),
            PathUpdateMode::Patch if is_callable(&updater) => runtime.apply(updater, target),
            PathUpdateMode::Patch => Ok(updater),
        };
    }

    match &path[0] {
        RuntimePathSegment::Field(name) => match target {
            Value::Record(mut map) => {
                let old = Arc::make_mut(&mut map).remove(name).unwrap_or(Value::Unit);
                let next = apply_value_path_update(runtime, old, &path[1..], updater, mode)?;
                Arc::make_mut(&mut map).insert(name.clone(), next);
                Ok(Value::Record(map))
            }
            Value::Unit => {
                let next = apply_value_path_update(runtime, Value::Unit, &path[1..], updater, mode)?;
                let mut map = HashMap::new();
                map.insert(name.clone(), next);
                Ok(Value::Record(Arc::new(map)))
            }
            other => Err(RuntimeError::Message(format!(
                "expected Record for field path, got {}",
                format_value(&other)
            ))),
        },
        RuntimePathSegment::IndexAll => match target {
            Value::List(mut items) => {
                for item in Arc::make_mut(&mut items).iter_mut() {
                    *item = apply_value_path_update(
                        runtime,
                        item.clone(),
                        &path[1..],
                        updater.clone(),
                        mode,
                    )?;
                }
                Ok(Value::List(items))
            }
            Value::Map(mut entries) => {
                let snapshot = entries.as_ref().clone();
                for (key, value) in snapshot {
                    let next = apply_value_path_update(runtime, value, &path[1..], updater.clone(), mode)?;
                    Arc::make_mut(&mut entries).insert(key, next);
                }
                Ok(Value::Map(entries))
            }
            other => Err(RuntimeError::Message(format!(
                "expected List/Map for index-all path, got {}",
                format_value(&other)
            ))),
        },
        RuntimePathSegment::IndexValue(index) => match target {
            Value::List(items) => {
                let idx = list_or_tuple_index(index, "list")?;
                if idx >= items.len() {
                    return Err(RuntimeError::Message("index out of bounds".to_string()));
                }
                let mut items = items;
                let out = Arc::make_mut(&mut items);
                let next =
                    apply_value_path_update(runtime, out[idx].clone(), &path[1..], updater, mode)?;
                out[idx] = next;
                Ok(Value::List(items))
            }
            Value::Tuple(mut items) => {
                let idx = list_or_tuple_index(index, "tuple")?;
                if idx >= items.len() {
                    return Err(RuntimeError::Message("index out of bounds".to_string()));
                }
                let next =
                    apply_value_path_update(runtime, items[idx].clone(), &path[1..], updater, mode)?;
                items[idx] = next;
                Ok(Value::Tuple(items))
            }
            Value::Map(entries) => {
                let key = map_index_key(index)?;
                let mut entries = entries;
                let old = entries.get(&key).cloned().unwrap_or(Value::Unit);
                let next = apply_value_path_update(runtime, old, &path[1..], updater, mode)?;
                Arc::make_mut(&mut entries).insert(key, next);
                Ok(Value::Map(entries))
            }
            Value::Unit => match index {
                Value::Int(raw) => {
                    if *raw < 0 {
                        return Err(RuntimeError::Message("index out of bounds".to_string()));
                    }
                    let idx = *raw as usize;
                    let mut items = vec![Value::Unit; idx + 1];
                    let next =
                        apply_value_path_update(runtime, Value::Unit, &path[1..], updater, mode)?;
                    items[idx] = next;
                    Ok(Value::List(Arc::new(items)))
                }
                _ => {
                    let key = map_index_key(index)?;
                    let mut entries = im::HashMap::new();
                    let next =
                        apply_value_path_update(runtime, Value::Unit, &path[1..], updater, mode)?;
                    entries.insert(key, next);
                    Ok(Value::Map(Arc::new(entries)))
                }
            },
            other => Err(RuntimeError::Message(format!(
                "expected List/Tuple/Map for indexed path, got {}",
                format_value(&other)
            ))),
        },
        RuntimePathSegment::IndexFieldBool(field) => match target {
            Value::List(mut items) => {
                for item in Arc::make_mut(&mut items).iter_mut() {
                    let should_update = matches!(
                        item,
                        Value::Record(map) if matches!(map.get(field), Some(Value::Bool(true)))
                    );
                    if should_update {
                        *item = apply_value_path_update(
                            runtime,
                            item.clone(),
                            &path[1..],
                            updater.clone(),
                            mode,
                        )?;
                    }
                }
                Ok(Value::List(items))
            }
            other => Err(RuntimeError::Message(format!(
                "expected List for boolean-field index path, got {}",
                format_value(&other)
            ))),
        },
        RuntimePathSegment::IndexPredicate(predicate) => match target {
            Value::List(mut items) => {
                for item in Arc::make_mut(&mut items).iter_mut() {
                    let keep = match runtime.apply(predicate.clone(), item.clone())? {
                        Value::Bool(value) => value,
                        other => {
                            return Err(RuntimeError::Message(format!(
                                "predicate index expects Bool, got {}",
                                format_value(&other)
                            )))
                        }
                    };
                    if keep {
                        *item = apply_value_path_update(
                            runtime,
                            item.clone(),
                            &path[1..],
                            updater.clone(),
                            mode,
                        )?;
                    }
                }
                Ok(Value::List(items))
            }
            Value::Map(mut entries) => {
                let snapshot = entries.as_ref().clone();
                for (key, value) in snapshot {
                    let mut entry = HashMap::new();
                    entry.insert("key".to_string(), key.to_value());
                    entry.insert("value".to_string(), value.clone());
                    let keep = match runtime.apply(
                        predicate.clone(),
                        Value::Record(Arc::new(entry)),
                    )? {
                        Value::Bool(value) => value,
                        other => {
                            return Err(RuntimeError::Message(format!(
                                "predicate index expects Bool, got {}",
                                format_value(&other)
                            )))
                        }
                    };
                    if keep {
                        let next =
                            apply_value_path_update(runtime, value, &path[1..], updater.clone(), mode)?;
                        Arc::make_mut(&mut entries).insert(key, next);
                    }
                }
                Ok(Value::Map(entries))
            }
            other => Err(RuntimeError::Message(format!(
                "expected List/Map for predicate index path, got {}",
                format_value(&other)
            ))),
        },
    }
}

pub(crate) fn eval_binary_builtin(op: &str, left: &Value, right: &Value) -> Option<Value> {
    match (op, left, right) {
        ("..", Value::Int(start), Value::Int(end)) => {
            if start > end {
                return Some(Value::List(Arc::new(Vec::new())));
            }
            let mut out = Vec::new();
            let mut current = *start;
            loop {
                out.push(Value::Int(current));
                if current == *end {
                    break;
                }
                current = current.checked_add(1)?;
            }
            Some(Value::List(Arc::new(out)))
        }
        ("+", Value::Int(a), Value::Int(b)) => Some(Value::Int(a + b)),
        ("-", Value::Int(a), Value::Int(b)) => Some(Value::Int(a - b)),
        ("*", Value::Int(a), Value::Int(b)) => Some(Value::Int(a * b)),
        ("/", Value::Int(a), Value::Int(b)) => Some(Value::Int(a / b)),
        ("%", Value::Int(a), Value::Int(b)) => Some(Value::Int(a % b)),
        ("+", Value::BigInt(a), Value::BigInt(b)) => Some(Value::BigInt(Arc::new(&**a + &**b))),
        ("-", Value::BigInt(a), Value::BigInt(b)) => Some(Value::BigInt(Arc::new(&**a - &**b))),
        ("*", Value::BigInt(a), Value::BigInt(b)) => Some(Value::BigInt(Arc::new(&**a * &**b))),
        ("+", Value::Decimal(a), Value::Decimal(b)) => Some(Value::Decimal(*a + *b)),
        ("-", Value::Decimal(a), Value::Decimal(b)) => Some(Value::Decimal(*a - *b)),
        ("*", Value::Decimal(a), Value::Decimal(b)) => Some(Value::Decimal(*a * *b)),
        ("/", Value::Decimal(a), Value::Decimal(b)) => Some(Value::Decimal(*a / *b)),
        ("+", Value::Float(a), Value::Float(b)) => Some(Value::Float(a + b)),
        ("-", Value::Float(a), Value::Float(b)) => Some(Value::Float(a - b)),
        ("*", Value::Float(a), Value::Float(b)) => Some(Value::Float(a * b)),
        ("/", Value::Float(a), Value::Float(b)) => Some(Value::Float(a / b)),
        ("%", Value::Float(a), Value::Float(b)) => Some(Value::Float(a % b)),
        ("==", a, b) => Some(Value::Bool(values_equal(a, b))),
        ("!=", a, b) => Some(Value::Bool(!values_equal(a, b))),
        ("<", Value::Int(a), Value::Int(b)) => Some(Value::Bool(a < b)),
        ("<=", Value::Int(a), Value::Int(b)) => Some(Value::Bool(a <= b)),
        (">", Value::Int(a), Value::Int(b)) => Some(Value::Bool(a > b)),
        (">=", Value::Int(a), Value::Int(b)) => Some(Value::Bool(a >= b)),
        ("<", Value::Float(a), Value::Float(b)) => Some(Value::Bool(a < b)),
        ("<=", Value::Float(a), Value::Float(b)) => Some(Value::Bool(a <= b)),
        (">", Value::Float(a), Value::Float(b)) => Some(Value::Bool(a > b)),
        (">=", Value::Float(a), Value::Float(b)) => Some(Value::Bool(a >= b)),
        ("<", Value::BigInt(a), Value::BigInt(b)) => Some(Value::Bool(a < b)),
        ("<=", Value::BigInt(a), Value::BigInt(b)) => Some(Value::Bool(a <= b)),
        (">", Value::BigInt(a), Value::BigInt(b)) => Some(Value::Bool(a > b)),
        (">=", Value::BigInt(a), Value::BigInt(b)) => Some(Value::Bool(a >= b)),
        ("<", Value::Decimal(a), Value::Decimal(b)) => Some(Value::Bool(a < b)),
        ("<=", Value::Decimal(a), Value::Decimal(b)) => Some(Value::Bool(a <= b)),
        (">", Value::Decimal(a), Value::Decimal(b)) => Some(Value::Bool(a > b)),
        (">=", Value::Decimal(a), Value::Decimal(b)) => Some(Value::Bool(a >= b)),
        ("&&", Value::Bool(a), Value::Bool(b)) => Some(Value::Bool(*a && *b)),
        ("||", Value::Bool(a), Value::Bool(b)) => Some(Value::Bool(*a || *b)),
        _ => None,
    }
}

pub(crate) fn values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Unit, Value::Unit) => true,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => a == b,
        (Value::Text(a), Value::Text(b)) => a == b,
        (Value::DateTime(a), Value::DateTime(b)) => a == b,
        (Value::Bytes(a), Value::Bytes(b)) => a == b,
        (Value::Regex(a), Value::Regex(b)) => a.as_str() == b.as_str(),
        (Value::BigInt(a), Value::BigInt(b)) => a == b,
        (Value::Rational(a), Value::Rational(b)) => a == b,
        (Value::Decimal(a), Value::Decimal(b)) => a == b,
        (Value::Map(a), Value::Map(b)) => {
            a.len() == b.len()
                && a.iter().all(|(key, value)| {
                    b.get(key)
                        .map(|other| values_equal(value, other))
                        .unwrap_or(false)
                })
        }
        (Value::Set(a), Value::Set(b)) => a.len() == b.len() && a.iter().all(|key| b.contains(key)),
        (Value::Queue(a), Value::Queue(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|(left, right)| values_equal(left, right))
        }
        (Value::Deque(a), Value::Deque(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|(left, right)| values_equal(left, right))
        }
        (Value::List(a), Value::List(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|(left, right)| values_equal(left, right))
        }
        (Value::Tuple(a), Value::Tuple(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|(left, right)| values_equal(left, right))
        }
        (Value::Record(a), Value::Record(b)) => {
            a.len() == b.len()
                && a.iter().all(|(key, value)| {
                    b.get(key)
                        .map(|other| values_equal(value, other))
                        .unwrap_or(false)
                })
        }
        (Value::Heap(a), Value::Heap(b)) => {
            if a.len() != b.len() {
                return false;
            }
            let mut left: Vec<_> = a.iter().cloned().collect();
            let mut right: Vec<_> = b.iter().cloned().collect();
            left.sort();
            right.sort();
            left == right
        }
        (Value::Constructor { name: a, args: aa }, Value::Constructor { name: b, args: bb }) => {
            a == b
                && aa.len() == bb.len()
                && aa.iter().zip(bb.iter()).all(|(x, y)| values_equal(x, y))
        }
        // Sources are effectful and are not meaningfully comparable.
        (Value::Source(_), Value::Source(_)) => false,
        _ => false,
    }
}

fn parse_number_literal(text: &str) -> Option<i64> {
    if text.contains('.') {
        return None;
    }
    if text.chars().any(|ch| !(ch.is_ascii_digit() || ch == '-')) {
        return None;
    }
    text.parse::<i64>().ok()
}

fn parse_number_value(text: &str) -> Option<Value> {
    if let Some(int) = parse_number_literal(text) {
        Some(Value::Int(int))
    } else if let Ok(float) = text.parse::<f64>() {
        Some(Value::Float(float))
    } else {
        None
    }
}

fn constructor_segment(name: &str) -> Option<&str> {
    let seg = name.rsplit('.').next().unwrap_or(name);
    let ok = seg
        .chars()
        .next()
        .map(|ch| ch.is_ascii_uppercase())
        .unwrap_or(false);
    if ok { Some(seg) } else { None }
}

fn is_callable(value: &Value) -> bool {
    matches!(
        value,
        Value::Closure(_) | Value::Builtin(_) | Value::MultiClause(_)
    )
}

fn is_match_failure_message(message: &str) -> bool {
    message == "non-exhaustive match"
}

const DEBUG_MAX_CHARS: usize = 200;
const DEBUG_MAX_DEPTH: usize = 3;
const DEBUG_MAX_LIST_ITEMS: usize = 20;

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|dur| dur.as_millis() as u64)
        .unwrap_or(0)
}

fn emit_debug_event(event: serde_json::Value) {
    // Emit JSONL-friendly structured logs to stderr by default.
    if let Ok(line) = serde_json::to_string(&event) {
        eprintln!("{line}");
    }
}

fn debug_shape_tag(value: &Value) -> Option<String> {
    match value {
        Value::Constructor { name, args } if args.is_empty() => match name.as_str() {
            "None" | "Some" | "Ok" | "Err" => Some(name.clone()),
            _ => None,
        },
        Value::Constructor { name, args } if args.len() == 1 => match name.as_str() {
            "Some" | "Ok" | "Err" => Some(name.clone()),
            _ => None,
        },
        _ => None,
    }
}

fn debug_value_to_json(value: &Value, depth: usize) -> serde_json::Value {
    if let Value::Constructor { name, args } = value {
        if name == "Sensitive" && args.len() == 1 {
            return serde_json::Value::String("<redacted>".to_string());
        }
    }

    if depth >= DEBUG_MAX_DEPTH {
        return debug_summary_json(value);
    }

    match value {
        Value::Unit => serde_json::Value::String("Unit".to_string()),
        Value::Bool(true) => serde_json::Value::String("True".to_string()),
        Value::Bool(false) => serde_json::Value::String("False".to_string()),
        Value::Int(v) => serde_json::Value::String(v.to_string()),
        Value::Float(v) => serde_json::Value::String(v.to_string()),
        Value::Text(t) => serde_json::Value::String(truncate_debug_text(t)),
        Value::DateTime(t) => serde_json::Value::String(truncate_debug_text(t)),
        Value::Bytes(bytes) => serde_json::Value::String(format!("<bytes:{}>", bytes.len())),
        Value::Regex(regex) => serde_json::Value::String(format!("<regex:{}>", regex.as_str())),
        Value::BigInt(v) => serde_json::Value::String(v.to_string()),
        Value::Rational(v) => serde_json::Value::String(v.to_string()),
        Value::Decimal(v) => serde_json::Value::String(v.to_string()),
        Value::Map(entries) => serde_json::Value::Object(
            [
                (
                    "type".to_string(),
                    serde_json::Value::String("Map".to_string()),
                ),
                (
                    "summary".to_string(),
                    serde_json::Value::String("<opaque>".to_string()),
                ),
                (
                    "size".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(entries.len())),
                ),
            ]
            .into_iter()
            .collect(),
        ),
        Value::Set(entries) => serde_json::Value::Object(
            [
                (
                    "type".to_string(),
                    serde_json::Value::String("Set".to_string()),
                ),
                (
                    "summary".to_string(),
                    serde_json::Value::String("<opaque>".to_string()),
                ),
                (
                    "size".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(entries.len())),
                ),
            ]
            .into_iter()
            .collect(),
        ),
        Value::Queue(items) => serde_json::Value::String(format!("<queue:{}>", items.len())),
        Value::Deque(items) => serde_json::Value::String(format!("<deque:{}>", items.len())),
        Value::Heap(items) => serde_json::Value::String(format!("<heap:{}>", items.len())),
        Value::List(items) => {
            let mut parts = Vec::new();
            for item in items.iter().take(DEBUG_MAX_LIST_ITEMS) {
                parts.push(debug_value_to_json(item, depth + 1));
            }
            let mut out = serde_json::Map::new();
            out.insert("type".to_string(), serde_json::Value::String("List".to_string()));
            out.insert(
                "size".to_string(),
                serde_json::Value::Number(serde_json::Number::from(items.len())),
            );
            out.insert("summary".to_string(), serde_json::Value::Array(parts));
            serde_json::Value::Object(out)
        }
        Value::Tuple(items) => {
            let mut parts = Vec::new();
            for item in items.iter().take(DEBUG_MAX_LIST_ITEMS) {
                parts.push(debug_value_to_json(item, depth + 1));
            }
            let mut out = serde_json::Map::new();
            out.insert("type".to_string(), serde_json::Value::String("Tuple".to_string()));
            out.insert(
                "size".to_string(),
                serde_json::Value::Number(serde_json::Number::from(items.len())),
            );
            out.insert("summary".to_string(), serde_json::Value::Array(parts));
            serde_json::Value::Object(out)
        }
        Value::Record(fields) => {
            let mut keys: Vec<&String> = fields.keys().collect();
            keys.sort();
            let mut out_fields = serde_json::Map::new();
            for key in keys.into_iter().take(DEBUG_MAX_LIST_ITEMS) {
                if let Some(val) = fields.get(key) {
                    out_fields.insert(key.clone(), debug_value_to_json(val, depth + 1));
                }
            }
            let mut out = serde_json::Map::new();
            out.insert(
                "type".to_string(),
                serde_json::Value::String("Record".to_string()),
            );
            out.insert(
                "size".to_string(),
                serde_json::Value::Number(serde_json::Number::from(fields.len())),
            );
            out.insert("summary".to_string(), serde_json::Value::Object(out_fields));
            serde_json::Value::Object(out)
        }
        Value::Constructor { name, args } => {
            if args.is_empty() {
                serde_json::Value::String(name.clone())
            } else {
                let mut out = serde_json::Map::new();
                out.insert("type".to_string(), serde_json::Value::String(name.clone()));
                out.insert(
                    "size".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(args.len())),
                );
                out.insert(
                    "summary".to_string(),
                    serde_json::Value::Array(
                        args.iter()
                            .take(DEBUG_MAX_LIST_ITEMS)
                            .map(|arg| debug_value_to_json(arg, depth + 1))
                            .collect(),
                    ),
                );
                serde_json::Value::Object(out)
            }
        }
        Value::Closure(_) => debug_summary_json(value),
        Value::Builtin(builtin) => serde_json::Value::Object(
            [
                (
                    "type".to_string(),
                    serde_json::Value::String("Builtin".to_string()),
                ),
                (
                    "summary".to_string(),
                    serde_json::Value::String(format!("<builtin:{}>", builtin.imp.name)),
                ),
            ]
            .into_iter()
            .collect(),
        ),
        Value::Effect(_) => debug_summary_json(value),
        Value::Source(_) => debug_summary_json(value),
        Value::Resource(_) => debug_summary_json(value),
        Value::Thunk(_) => debug_summary_json(value),
        Value::MultiClause(_) => debug_summary_json(value),
        Value::ChannelSend(_) => debug_summary_json(value),
        Value::ChannelRecv(_) => debug_summary_json(value),
        Value::FileHandle(_) => debug_summary_json(value),
        Value::Listener(_) => debug_summary_json(value),
        Value::Connection(_) => debug_summary_json(value),
        Value::Stream(_) => debug_summary_json(value),
        Value::HttpServer(_) => debug_summary_json(value),
        Value::WebSocket(_) => debug_summary_json(value),
    }
}

fn truncate_debug_text(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars().take(DEBUG_MAX_CHARS) {
        out.push(ch);
    }
    if text.chars().count() > DEBUG_MAX_CHARS {
        out.push_str("...");
    }
    out
}

fn debug_summary_json(value: &Value) -> serde_json::Value {
    let (ty, size) = match value {
        Value::Unit => ("Unit", None),
        Value::Bool(_) => ("Bool", None),
        Value::Int(_) => ("Int", None),
        Value::Float(_) => ("Float", None),
        Value::Text(_) => ("Text", None),
        Value::DateTime(_) => ("DateTime", None),
        Value::Bytes(bytes) => ("Bytes", Some(bytes.len())),
        Value::Regex(_) => ("Regex", None),
        Value::BigInt(_) => ("BigInt", None),
        Value::Rational(_) => ("Rational", None),
        Value::Decimal(_) => ("Decimal", None),
        Value::Map(entries) => ("Map", Some(entries.len())),
        Value::Set(entries) => ("Set", Some(entries.len())),
        Value::Queue(items) => ("Queue", Some(items.len())),
        Value::Deque(items) => ("Deque", Some(items.len())),
        Value::Heap(items) => ("Heap", Some(items.len())),
        Value::List(items) => ("List", Some(items.len())),
        Value::Tuple(items) => ("Tuple", Some(items.len())),
        Value::Record(fields) => ("Record", Some(fields.len())),
        Value::Constructor { name, args } => (name.as_str(), Some(args.len())),
        Value::Closure(_) => ("Closure", None),
        Value::Builtin(_) => ("Builtin", None),
        Value::Effect(_) => ("Effect", None),
        Value::Source(_) => ("Source", None),
        Value::Resource(_) => ("Resource", None),
        Value::Thunk(_) => ("Thunk", None),
        Value::MultiClause(_) => ("MultiClause", None),
        Value::ChannelSend(_) => ("Send", None),
        Value::ChannelRecv(_) => ("Recv", None),
        Value::FileHandle(_) => ("File", None),
        Value::Listener(_) => ("Listener", None),
        Value::Connection(_) => ("Connection", None),
        Value::Stream(_) => ("Stream", None),
        Value::HttpServer(_) => ("HttpServer", None),
        Value::WebSocket(_) => ("WebSocket", None),
    };

    let mut out = serde_json::Map::new();
    out.insert("type".to_string(), serde_json::Value::String(ty.to_string()));
    out.insert(
        "summary".to_string(),
        serde_json::Value::String("<opaque>".to_string()),
    );
    if let Some(size) = size {
        out.insert(
            "size".to_string(),
            serde_json::Value::Number(serde_json::Number::from(size)),
        );
    }
    serde_json::Value::Object(out)
}

pub(crate) fn format_value(value: &Value) -> String {
    match value {
        Value::Unit => "Unit".to_string(),
        Value::Bool(value) => {
            if *value {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        Value::Int(value) => value.to_string(),
        Value::Float(value) => value.to_string(),
        Value::Text(value) => value.clone(),
        Value::DateTime(value) => value.clone(),
        Value::Bytes(bytes) => format!("<bytes:{}>", bytes.len()),
        Value::Regex(regex) => format!("<regex:{}>", regex.as_str()),
        Value::BigInt(value) => value.to_string(),
        Value::Rational(value) => value.to_string(),
        Value::Decimal(value) => value.to_string(),
        Value::Map(entries) => format!("<map:{}>", entries.len()),
        Value::Set(entries) => format!("<set:{}>", entries.len()),
        Value::Queue(items) => format!("<queue:{}>", items.len()),
        Value::Deque(items) => format!("<deque:{}>", items.len()),
        Value::Heap(items) => format!("<heap:{}>", items.len()),
        Value::List(items) => format!(
            "[{}]",
            items
                .iter()
                .map(format_value)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Tuple(items) => format!(
            "({})",
            items
                .iter()
                .map(format_value)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Record(_) => "{...}".to_string(),
        Value::Constructor { name, args } => {
            if args.is_empty() {
                name.clone()
            } else {
                format!(
                    "{} {}",
                    name,
                    args.iter().map(format_value).collect::<Vec<_>>().join(" ")
                )
            }
        }
        Value::Closure(_) => "<closure>".to_string(),
        Value::Builtin(builtin) => format!("<builtin:{}>", builtin.imp.name),
        Value::Effect(_) => "<effect>".to_string(),
        Value::Source(source) => format!("<source:{}>", source.kind),
        Value::Resource(_) => "<resource>".to_string(),
        Value::Thunk(_) => "<thunk>".to_string(),
        Value::MultiClause(_) => "<multi-clause>".to_string(),
        Value::ChannelSend(_) => "<send>".to_string(),
        Value::ChannelRecv(_) => "<recv>".to_string(),
        Value::FileHandle(_) => "<file>".to_string(),
        Value::Listener(_) => "<listener>".to_string(),
        Value::Connection(_) => "<connection>".to_string(),
        Value::Stream(_) => "<stream>".to_string(),
        Value::HttpServer(_) => "<http-server>".to_string(),
        Value::WebSocket(_) => "<websocket>".to_string(),
    }
}

fn date_to_record(date: NaiveDate) -> HashMap<String, Value> {
    let mut map = HashMap::new();
    map.insert("year".to_string(), Value::Int(date.year() as i64));
    map.insert("month".to_string(), Value::Int(date.month() as i64));
    map.insert("day".to_string(), Value::Int(date.day() as i64));
    map
}

fn url_to_record(url: &Url) -> HashMap<String, Value> {
    let mut map = HashMap::new();
    map.insert(
        "protocol".to_string(),
        Value::Text(url.scheme().to_string()),
    );
    map.insert(
        "host".to_string(),
        Value::Text(url.host_str().unwrap_or("").to_string()),
    );
    let port = match url.port() {
        Some(port) => Value::Constructor {
            name: "Some".to_string(),
            args: vec![Value::Int(port as i64)],
        },
        None => Value::Constructor {
            name: "None".to_string(),
            args: Vec::new(),
        },
    };
    map.insert("port".to_string(), port);
    map.insert("path".to_string(), Value::Text(url.path().to_string()));
    let mut query_items = Vec::new();
    for (key, value) in url.query_pairs() {
        query_items.push(Value::Tuple(vec![
            Value::Text(key.to_string()),
            Value::Text(value.to_string()),
        ]));
    }
    map.insert("query".to_string(), Value::List(Arc::new(query_items)));
    let hash = match url.fragment() {
        Some(fragment) => Value::Constructor {
            name: "Some".to_string(),
            args: vec![Value::Text(fragment.to_string())],
        },
        None => Value::Constructor {
            name: "None".to_string(),
            args: Vec::new(),
        },
    };
    map.insert("hash".to_string(), hash);
    map
}

fn i18n_message_parts_value(parts: &[MessagePart]) -> Value {
    let mut out = Vec::with_capacity(parts.len());
    for part in parts {
        match part {
            MessagePart::Lit(text) => {
                out.push(Value::Record(Arc::new(HashMap::from([
                    ("kind".to_string(), Value::Text("lit".to_string())),
                    ("text".to_string(), Value::Text(text.clone())),
                ]))));
            }
            MessagePart::Hole { name, ty } => {
                let ty_value = match ty {
                    Some(t) => Value::Constructor {
                        name: "Some".to_string(),
                        args: vec![Value::Text(t.clone())],
                    },
                    None => Value::Constructor {
                        name: "None".to_string(),
                        args: Vec::new(),
                    },
                };
                out.push(Value::Record(Arc::new(HashMap::from([
                    ("kind".to_string(), Value::Text("hole".to_string())),
                    ("name".to_string(), Value::Text(name.clone())),
                    ("ty".to_string(), ty_value),
                ]))));
            }
        }
    }
    Value::List(Arc::new(out))
}

/// Evaluate a sigil literal into its runtime value.
///
/// Extracted as a standalone function so both the interpreter and JIT can use it.
pub(crate) fn eval_sigil_literal(
    tag: &str,
    body: &str,
    flags: &str,
) -> Result<Value, RuntimeError> {
    match tag {
        "r" => {
            let mut builder = RegexBuilder::new(body);
            for flag in flags.chars() {
                match flag {
                    'i' => {
                        builder.case_insensitive(true);
                    }
                    'm' => {
                        builder.multi_line(true);
                    }
                    's' => {
                        builder.dot_matches_new_line(true);
                    }
                    'x' => {
                        builder.ignore_whitespace(true);
                    }
                    _ => {}
                }
            }
            let regex = builder.build().map_err(|err| {
                RuntimeError::Message(format!("invalid regex literal: {err}"))
            })?;
            Ok(Value::Regex(Arc::new(regex)))
        }
        "u" | "url" => {
            let parsed = Url::parse(body).map_err(|err| {
                RuntimeError::Message(format!("invalid url literal: {err}"))
            })?;
            Ok(Value::Record(Arc::new(url_to_record(&parsed))))
        }
        "p" | "path" => {
            let cleaned = body.trim().replace('\\', "/");
            if cleaned.contains('\0') {
                return Err(RuntimeError::Message(
                    "invalid path literal: contains NUL byte".to_string(),
                ));
            }
            let absolute = cleaned.starts_with('/');
            let mut segments: Vec<String> = Vec::new();
            for raw in cleaned.split('/') {
                if raw.is_empty() || raw == "." {
                    continue;
                }
                if raw == ".." {
                    if let Some(last) = segments.last() {
                        if last != ".." {
                            segments.pop();
                            continue;
                        }
                    }
                    if !absolute {
                        segments.push("..".to_string());
                    }
                    continue;
                }
                segments.push(raw.to_string());
            }

            let mut map = HashMap::new();
            map.insert("absolute".to_string(), Value::Bool(absolute));
            map.insert(
                "segments".to_string(),
                Value::List(Arc::new(
                    segments.into_iter().map(Value::Text).collect::<Vec<_>>(),
                )),
            );
            Ok(Value::Record(Arc::new(map)))
        }
        "d" => {
            let date = NaiveDate::parse_from_str(body, "%Y-%m-%d").map_err(|err| {
                RuntimeError::Message(format!("invalid date literal: {err}"))
            })?;
            Ok(Value::Record(Arc::new(date_to_record(date))))
        }
        "t" | "dt" => {
            let _ = chrono::DateTime::parse_from_rfc3339(body).map_err(|err| {
                RuntimeError::Message(format!("invalid datetime literal: {err}"))
            })?;
            Ok(Value::DateTime(body.to_string()))
        }
        "tz" => {
            let zone_id = body.trim();
            let _: chrono_tz::Tz = zone_id.parse().map_err(|_| {
                RuntimeError::Message(format!("invalid timezone id: {zone_id}"))
            })?;
            let mut map = HashMap::new();
            map.insert("id".to_string(), Value::Text(zone_id.to_string()));
            Ok(Value::Record(Arc::new(map)))
        }
        "zdt" => {
            let text = body.trim();
            let (dt_text, zone_id) = parse_zdt_parts(text)?;
            let tz: chrono_tz::Tz = zone_id.parse().map_err(|_| {
                RuntimeError::Message(format!("invalid timezone id: {zone_id}"))
            })?;

            let zdt = if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(dt_text) {
                parsed.with_timezone(&tz)
            } else {
                let naive = parse_naive_datetime(dt_text)?;
                tz.from_local_datetime(&naive)
                    .single()
                    .ok_or_else(|| {
                        RuntimeError::Message("ambiguous or invalid local time".to_string())
                    })?
            };

            let offset_millis =
                i64::from(chrono::offset::Offset::fix(zdt.offset()).local_minus_utc()) * 1000;

            let mut dt_map = HashMap::new();
            dt_map.insert("year".to_string(), Value::Int(zdt.year() as i64));
            dt_map.insert("month".to_string(), Value::Int(zdt.month() as i64));
            dt_map.insert("day".to_string(), Value::Int(zdt.day() as i64));
            dt_map.insert("hour".to_string(), Value::Int(zdt.hour() as i64));
            dt_map.insert("minute".to_string(), Value::Int(zdt.minute() as i64));
            dt_map.insert("second".to_string(), Value::Int(zdt.second() as i64));
            dt_map.insert(
                "millisecond".to_string(),
                Value::Int(zdt.timestamp_subsec_millis() as i64),
            );

            let mut zone_map = HashMap::new();
            zone_map.insert("id".to_string(), Value::Text(zone_id.to_string()));

            let mut offset_map = HashMap::new();
            offset_map.insert("millis".to_string(), Value::Int(offset_millis));

            let mut map = HashMap::new();
            map.insert("dateTime".to_string(), Value::Record(Arc::new(dt_map)));
            map.insert("zone".to_string(), Value::Record(Arc::new(zone_map)));
            map.insert("offset".to_string(), Value::Record(Arc::new(offset_map)));
            Ok(Value::Record(Arc::new(map)))
        }
        "k" => {
            validate_key_text(body).map_err(|msg| {
                RuntimeError::Message(format!("invalid i18n key literal: {msg}"))
            })?;
            let mut map = HashMap::new();
            map.insert("tag".to_string(), Value::Text(tag.to_string()));
            map.insert("body".to_string(), Value::Text(body.trim().to_string()));
            map.insert("flags".to_string(), Value::Text(flags.to_string()));
            Ok(Value::Record(Arc::new(map)))
        }
        "m" => {
            let parsed = parse_message_template(body).map_err(|msg| {
                RuntimeError::Message(format!("invalid i18n message literal: {msg}"))
            })?;
            let mut map = HashMap::new();
            map.insert("tag".to_string(), Value::Text(tag.to_string()));
            map.insert("body".to_string(), Value::Text(body.to_string()));
            map.insert("flags".to_string(), Value::Text(flags.to_string()));
            map.insert("parts".to_string(), i18n_message_parts_value(&parsed.parts));
            Ok(Value::Record(Arc::new(map)))
        }
        _ => {
            let mut map = HashMap::new();
            map.insert("tag".to_string(), Value::Text(tag.to_string()));
            map.insert("body".to_string(), Value::Text(body.to_string()));
            map.insert("flags".to_string(), Value::Text(flags.to_string()));
            Ok(Value::Record(Arc::new(map)))
        }
    }
}
