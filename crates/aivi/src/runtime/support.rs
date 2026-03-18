fn is_url_option_int(value: &Value) -> bool {
    matches!(
        value,
        Value::Constructor { name, args } if (name == "None" && args.is_empty())
            || (name == "Some" && args.len() == 1 && matches!(args[0], Value::Int(_)))
    )
}

fn is_url_option_text(value: &Value) -> bool {
    matches!(
        value,
        Value::Constructor { name, args } if (name == "None" && args.is_empty())
            || (name == "Some" && args.len() == 1 && matches!(args[0], Value::Text(_)))
    )
}

fn is_url_like_record(fields: &HashMap<String, Value>) -> bool {
    matches!(fields.get("protocol"), Some(Value::Text(_)))
        && matches!(fields.get("host"), Some(Value::Text(_)))
        && matches!(fields.get("path"), Some(Value::Text(_)))
        && matches!(fields.get("query"), Some(Value::List(_)))
        && fields.get("port").is_some_and(is_url_option_int)
        && fields.get("hash").is_some_and(is_url_option_text)
}

fn try_url_query_binary(op: &str, left: &Value, right: &Value) -> Option<Value> {
    match (op, left, right) {
        ("+", Value::Record(fields), Value::Tuple(items)) if items.len() == 2 => {
            if !is_url_like_record(fields) {
                return None;
            }
            let (key, value) = match (&items[0], &items[1]) {
                (Value::Text(key), Value::Text(value)) => (key.clone(), value.clone()),
                _ => return None,
            };
            let query_items = match fields.get("query") {
                Some(Value::List(items)) => items,
                _ => return None,
            };
            let mut new_query = query_items.iter().cloned().collect::<Vec<_>>();
            new_query.push(Value::Tuple(vec![Value::Text(key), Value::Text(value)]));

            let mut new_fields = fields.as_ref().clone();
            new_fields.insert("query".to_string(), Value::List(Arc::new(new_query)));
            Some(Value::Record(Arc::new(new_fields)))
        }
        ("-", Value::Record(fields), Value::Text(key)) => {
            if !is_url_like_record(fields) {
                return None;
            }
            let query_items = match fields.get("query") {
                Some(Value::List(items)) => items,
                _ => return None,
            };
            let mut filtered = Vec::new();
            for item in query_items.iter() {
                let Value::Tuple(parts) = item else {
                    return None;
                };
                if parts.len() != 2 {
                    return None;
                }
                let (Value::Text(existing_key), Value::Text(_)) = (&parts[0], &parts[1]) else {
                    return None;
                };
                if existing_key != key {
                    filtered.push(item.clone());
                }
            }

            let mut new_fields = fields.as_ref().clone();
            new_fields.insert("query".to_string(), Value::List(Arc::new(filtered)));
            Some(Value::Record(Arc::new(new_fields)))
        }
        _ => None,
    }
}

pub(crate) fn eval_binary_builtin(
    op: &str,
    left: &Value,
    right: &Value,
) -> Result<Option<Value>, RuntimeError> {
    if let Some(result) = try_url_query_binary(op, left, right) {
        return Ok(Some(result));
    }

    match (op, left, right) {
        ("..", Value::Int(start), Value::Int(end)) => {
            if start > end {
                return Ok(Some(Value::List(Arc::new(Vec::new()))));
            }
            let mut out = Vec::new();
            let mut current = *start;
            loop {
                out.push(Value::Int(current));
                if current == *end {
                    break;
                }
                current = match current.checked_add(1) {
                    Some(next) => next,
                    None => return Ok(None),
                };
            }
            Ok(Some(Value::List(Arc::new(out))))
        }
        // Use wrapping arithmetic to match Cranelift's iadd/isub/imul semantics.
        ("+", Value::Int(a), Value::Int(b)) => Ok(Some(Value::Int(a.wrapping_add(*b)))),
        ("-", Value::Int(a), Value::Int(b)) => Ok(Some(Value::Int(a.wrapping_sub(*b)))),
        ("*", Value::Int(a), Value::Int(b)) => Ok(Some(Value::Int(a.wrapping_mul(*b)))),
        ("/", Value::Int(a), Value::Int(b)) => {
            if *b == 0 {
                Ok(None)
            } else {
                Ok(Some(Value::Int(a.wrapping_div(*b))))
            }
        }
        ("%", Value::Int(a), Value::Int(b)) => {
            if *b == 0 {
                Ok(None)
            } else {
                Ok(Some(Value::Int(a.wrapping_rem(*b))))
            }
        }
        ("+", Value::BigInt(a), Value::BigInt(b)) => Ok(Some(Value::BigInt(Arc::new(&**a + &**b)))),
        ("-", Value::BigInt(a), Value::BigInt(b)) => Ok(Some(Value::BigInt(Arc::new(&**a - &**b)))),
        ("*", Value::BigInt(a), Value::BigInt(b)) => Ok(Some(Value::BigInt(Arc::new(&**a * &**b)))),
        ("+", Value::Decimal(a), Value::Decimal(b)) => Ok(Some(Value::Decimal(*a + *b))),
        ("-", Value::Decimal(a), Value::Decimal(b)) => Ok(Some(Value::Decimal(*a - *b))),
        ("*", Value::Decimal(a), Value::Decimal(b)) => Ok(Some(Value::Decimal(*a * *b))),
        ("/", Value::Decimal(_), Value::Decimal(b)) if *b == rust_decimal::Decimal::ZERO => {
            Err(RuntimeError::DivisionByZero {
                context: "decimal.div".to_string(),
            })
        }
        ("/", Value::Decimal(a), Value::Decimal(b)) => Ok(Some(Value::Decimal(*a / *b))),
        ("+", Value::Float(a), Value::Float(b)) => Ok(Some(Value::Float(a + b))),
        ("-", Value::Float(a), Value::Float(b)) => Ok(Some(Value::Float(a - b))),
        ("*", Value::Float(a), Value::Float(b)) => Ok(Some(Value::Float(a * b))),
        ("/", Value::Float(a), Value::Float(b)) => Ok(Some(Value::Float(a / b))),
        ("%", Value::Float(a), Value::Float(b)) => Ok(Some(Value::Float(a % b))),
        ("==", a, b) => Ok(Some(Value::Bool(values_equal(a, b)))),
        ("!=", a, b) => Ok(Some(Value::Bool(!values_equal(a, b)))),
        ("<", Value::Int(a), Value::Int(b)) => Ok(Some(Value::Bool(a < b))),
        ("<=", Value::Int(a), Value::Int(b)) => Ok(Some(Value::Bool(a <= b))),
        (">", Value::Int(a), Value::Int(b)) => Ok(Some(Value::Bool(a > b))),
        (">=", Value::Int(a), Value::Int(b)) => Ok(Some(Value::Bool(a >= b))),
        ("<", Value::Float(a), Value::Float(b)) => Ok(Some(Value::Bool(a < b))),
        ("<=", Value::Float(a), Value::Float(b)) => Ok(Some(Value::Bool(a <= b))),
        (">", Value::Float(a), Value::Float(b)) => Ok(Some(Value::Bool(a > b))),
        (">=", Value::Float(a), Value::Float(b)) => Ok(Some(Value::Bool(a >= b))),
        ("<", Value::BigInt(a), Value::BigInt(b)) => Ok(Some(Value::Bool(a < b))),
        ("<=", Value::BigInt(a), Value::BigInt(b)) => Ok(Some(Value::Bool(a <= b))),
        (">", Value::BigInt(a), Value::BigInt(b)) => Ok(Some(Value::Bool(a > b))),
        (">=", Value::BigInt(a), Value::BigInt(b)) => Ok(Some(Value::Bool(a >= b))),
        ("<", Value::Decimal(a), Value::Decimal(b)) => Ok(Some(Value::Bool(a < b))),
        ("<=", Value::Decimal(a), Value::Decimal(b)) => Ok(Some(Value::Bool(a <= b))),
        (">", Value::Decimal(a), Value::Decimal(b)) => Ok(Some(Value::Bool(a > b))),
        (">=", Value::Decimal(a), Value::Decimal(b)) => Ok(Some(Value::Bool(a >= b))),
        ("<", Value::Text(a), Value::Text(b)) => Ok(Some(Value::Bool(a < b))),
        ("<=", Value::Text(a), Value::Text(b)) => Ok(Some(Value::Bool(a <= b))),
        (">", Value::Text(a), Value::Text(b)) => Ok(Some(Value::Bool(a > b))),
        (">=", Value::Text(a), Value::Text(b)) => Ok(Some(Value::Bool(a >= b))),
        ("++", Value::Text(a), Value::Text(b)) => {
            let mut result = a.clone();
            result.push_str(b);
            Ok(Some(Value::Text(result)))
        }
        ("&&", Value::Bool(a), Value::Bool(b)) => Ok(Some(Value::Bool(*a && *b))),
        ("||", Value::Bool(a), Value::Bool(b)) => Ok(Some(Value::Bool(*a || *b))),
        ("??", Value::Constructor { name, args }, _rhs) if name == "Some" && args.len() == 1 => {
            Ok(Some(args[0].clone()))
        }
        ("??", Value::Constructor { name, .. }, rhs) if name == "None" => Ok(Some(rhs.clone())),
        // Handle un-wrapped values from schema-less JSON deserialization.
        // The type checker guarantees `??` is only used on `Option A`, so if
        // the LHS is Unit (absent field) use the default, otherwise the value
        // is present — pass it through.
        ("??", Value::Unit, rhs) => Ok(Some(rhs.clone())),
        ("??", lhs, _rhs) => Ok(Some(lhs.clone())),
        _ => Ok(None),
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
        (Value::Signal(a), Value::Signal(b)) => a.id == b.id,
        // Sources are effectful and are not meaningfully comparable.
        (Value::Source(_), Value::Source(_)) => false,
        _ => false,
    }
}

fn is_callable(value: &Value) -> bool {
    matches!(value, Value::Builtin(_) | Value::MultiClause(_))
}

/// Legacy check for string-based match failure messages. Kept for backward
/// compatibility with any `RuntimeError::Message` sites not yet migrated to
/// `RuntimeError::NonExhaustiveMatch`.
fn is_match_failure_message(message: &str) -> bool {
    message == "non-exhaustive match" || message.starts_with("non-exhaustive match ")
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
        Value::Record(fields) => {
            let mut pairs: Vec<String> = fields
                .iter()
                .map(|(k, v)| format!("{}: {}", k, format_value(v)))
                .collect();
            pairs.sort();
            format!("{{ {} }}", pairs.join(", "))
        }
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
        Value::Builtin(builtin) => format!("<builtin:{}>", builtin.imp.name),
        Value::Effect(_) => "<effect>".to_string(),
        Value::Source(source) => format!("<source:{}>", source.kind),
        Value::Resource(_) => "<resource>".to_string(),
        Value::Thunk(_) => "<thunk>".to_string(),
        Value::MultiClause(_) => "<multi-clause>".to_string(),
        Value::Signal(signal) => format!("<signal:{}>", signal.id),
        Value::ChannelSend(_) => "<send>".to_string(),
        Value::ChannelRecv(_) => "<recv>".to_string(),
        Value::FileHandle(_) => "<file>".to_string(),
        Value::Listener(_) => "<listener>".to_string(),
        Value::Connection(_) => "<connection>".to_string(),
        Value::Stream(_) => "<stream>".to_string(),
        Value::HttpServer(_) => "<http-server>".to_string(),
        Value::WebSocket(_) => "<websocket>".to_string(),
        Value::ImapSession(_) => "<imap-session>".to_string(),
        Value::DbConnection(_) => "<db-connection>".to_string(),
    }
}

pub(crate) fn write_stdout(runtime: &Runtime, text: &str, newline: bool) {
    if runtime.ctx.capture_stdout(text, newline) {
        return;
    }
    if newline {
        println!("{text}");
    } else {
        print!("{text}");
        let mut out = std::io::stdout();
        let _ = std::io::Write::flush(&mut out);
    }
}

pub(crate) fn write_stderr(runtime: &Runtime, text: &str, newline: bool) {
    if runtime.ctx.capture_stderr(text, newline) {
        return;
    }
    if newline {
        eprintln!("{text}");
    } else {
        eprint!("{text}");
        let mut err = std::io::stderr();
        let _ = std::io::Write::flush(&mut err);
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
            let regex = builder.build().map_err(|err| RuntimeError::ParseError {
                context: "regex literal".to_string(),
                input: format!("{err}"),
            })?;
            Ok(Value::Regex(Arc::new(regex)))
        }
        "u" | "url" => {
            let parsed = Url::parse(body).map_err(|err| RuntimeError::ParseError {
                context: "url literal".to_string(),
                input: format!("{err}"),
            })?;
            Ok(Value::Record(Arc::new(url_to_record(&parsed))))
        }
        "p" | "path" => {
            let cleaned = body.trim().replace('\\', "/");
            if cleaned.contains('\0') {
                return Err(RuntimeError::ParseError {
                    context: "path literal".to_string(),
                    input: "contains NUL byte".to_string(),
                });
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
                RuntimeError::ParseError {
                    context: "date literal".to_string(),
                    input: format!("{err}"),
                }
            })?;
            Ok(Value::Record(Arc::new(date_to_record(date))))
        }
        "t" | "dt" => {
            let _ = chrono::DateTime::parse_from_rfc3339(body).map_err(|err| {
                RuntimeError::ParseError {
                    context: "datetime literal".to_string(),
                    input: format!("{err}"),
                }
            })?;
            Ok(Value::DateTime(body.to_string()))
        }
        "tz" => {
            let zone_id = body.trim();
            let _: chrono_tz::Tz = zone_id.parse().map_err(|_| RuntimeError::ParseError {
                context: "timezone literal".to_string(),
                input: zone_id.to_string(),
            })?;
            let mut map = HashMap::new();
            map.insert("id".to_string(), Value::Text(zone_id.to_string()));
            Ok(Value::Record(Arc::new(map)))
        }
        "zdt" => {
            let text = body.trim();
            let (dt_text, zone_id) = parse_zdt_parts(text)?;
            let tz: chrono_tz::Tz = zone_id.parse().map_err(|_| RuntimeError::ParseError {
                context: "timezone literal".to_string(),
                input: zone_id.to_string(),
            })?;

            let zdt = if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(dt_text) {
                parsed.with_timezone(&tz)
            } else {
                let naive = parse_naive_datetime(dt_text)?;
                tz.from_local_datetime(&naive)
                    .single()
                    .ok_or_else(|| RuntimeError::ParseError {
                        context: "zoned datetime literal".to_string(),
                        input: "ambiguous or invalid local time".to_string(),
                    })?
            };

            let offset_millis =
                i64::from(chrono::offset::Offset::fix(zdt.offset()).local_minus_utc()) * 1000;

            let local_naive = zdt.naive_local();
            let dt_str = local_naive
                .and_utc()
                .to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true);

            let mut zone_map = HashMap::new();
            zone_map.insert("id".to_string(), Value::Text(zone_id.to_string()));

            let mut offset_map = HashMap::new();
            offset_map.insert("millis".to_string(), Value::Int(offset_millis));

            let mut map = HashMap::new();
            map.insert("dateTime".to_string(), Value::DateTime(dt_str));
            map.insert("zone".to_string(), Value::Record(Arc::new(zone_map)));
            map.insert("offset".to_string(), Value::Record(Arc::new(offset_map)));
            Ok(Value::Record(Arc::new(map)))
        }
        "k" => {
            validate_key_text(body).map_err(|msg| RuntimeError::ParseError {
                context: "i18n key literal".to_string(),
                input: msg,
            })?;
            let mut map = HashMap::new();
            map.insert("tag".to_string(), Value::Text(tag.to_string()));
            map.insert("body".to_string(), Value::Text(body.trim().to_string()));
            map.insert("flags".to_string(), Value::Text(flags.to_string()));
            Ok(Value::Record(Arc::new(map)))
        }
        "m" => {
            let parsed = parse_message_template(body).map_err(|msg| RuntimeError::ParseError {
                context: "i18n message literal".to_string(),
                input: msg,
            })?;
            let mut map = HashMap::new();
            map.insert("tag".to_string(), Value::Text(tag.to_string()));
            map.insert("body".to_string(), Value::Text(body.to_string()));
            map.insert("flags".to_string(), Value::Text(flags.to_string()));
            map.insert("parts".to_string(), i18n_message_parts_value(&parsed.parts));
            Ok(Value::Record(Arc::new(map)))
        }
        "raw" => Ok(Value::Text(body.to_string())),
        _ => {
            let mut map = HashMap::new();
            map.insert("tag".to_string(), Value::Text(tag.to_string()));
            map.insert("body".to_string(), Value::Text(body.to_string()));
            map.insert("flags".to_string(), Value::Text(flags.to_string()));
            Ok(Value::Record(Arc::new(map)))
        }
    }
}
