
fn is_match_failure_error(err: &RuntimeError) -> bool {
    matches!(err, RuntimeError::Message(message) if message == "non-exhaustive match")
}

fn seed_rng_state() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    nanos ^ 0x9E3779B97F4A7C15
}

pub fn values_equal(left: &Value, right: &Value) -> bool {
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
        Value::Bool(v) => serde_json::Value::String(v.to_string()),
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
        _ => debug_summary_json(value),
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

pub fn format_value(value: &Value) -> String {
    match value {
        Value::Unit => "Unit".to_string(),
        Value::Bool(v) => v.to_string(),
        Value::Int(v) => v.to_string(),
        Value::Float(v) => v.to_string(),
        Value::Text(v) => v.clone(),
        Value::DateTime(v) => v.clone(),
        Value::Bytes(v) => format!("{:?}", v.as_slice()),
        Value::Regex(v) => v.as_str().to_string(),
        Value::BigInt(v) => v.to_string(),
        Value::Rational(v) => v.to_string(),
        Value::Decimal(v) => v.to_string(),
        Value::Map(map) => format!("<map:{}>", map.len()),
        Value::Set(set) => format!("<set:{}>", set.len()),
        Value::Queue(queue) => format!("<queue:{}>", queue.len()),
        Value::Deque(deque) => format!("<deque:{}>", deque.len()),
        Value::Heap(heap) => format!("<heap:{}>", heap.len()),
        Value::List(items) => {
            let inner = items
                .iter()
                .map(format_value)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{}]", inner)
        }
        Value::Tuple(items) => {
            let inner = items
                .iter()
                .map(format_value)
                .collect::<Vec<_>>()
                .join(", ");
            format!("({})", inner)
        }
        Value::Record(map) => {
            let mut keys = map.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            let inner = keys
                .into_iter()
                .map(|k| format!("{}: {}", k, format_value(&map[&k])))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{{}}}", inner)
        }
        Value::Constructor { name, args } => {
            if args.is_empty() {
                name.clone()
            } else {
                let inner = args.iter().map(format_value).collect::<Vec<_>>().join(", ");
                format!("{name}({inner})")
            }
        }
        Value::Closure(_) => "<closure>".to_string(),
        Value::Builtin(b) => format!("<builtin {}>", b.imp.name),
        Value::Effect(_) => "<effect>".to_string(),
        Value::Source(source) => format!("<source:{}>", source.kind),
        Value::Resource(_) => "<resource>".to_string(),
        Value::Thunk(_) => "<thunk>".to_string(),
        Value::MultiClause(_) => "<multiclause>".to_string(),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Builtin {
    Unit,
    True,
    False,
    None,
    Some,
    Ok,
    Err,
    Closed,
    Pure,
    Fail,
    Attempt,
    Load,
    Bind,
    Print,
    Println,
    Map,
    Chain,
    AssertEq,
    File,
    System,
    Clock,
    Random,
    Channel,
    Concurrent,
    HttpServer,
    Text,
    Regex,
    Math,
    Calendar,
    Color,
    Linalg,
    Signal,
    Graph,
    Bigint,
    Rational,
    Decimal,
    Url,
    Http,
    Https,
    Sockets,
    Streams,
    Collections,
    Console,
    Crypto,
    Logger,
    Database,
    MapType,
    SetType,
    QueueType,
    DequeType,
    HeapType,
}
