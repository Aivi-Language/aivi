fn build_bigint_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "fromInt".to_string(),
        builtin("bigint.fromInt", 1, |mut args, _| {
            let value = expect_int(args.pop().unwrap(), "bigint.fromInt")?;
            Ok(Value::BigInt(Arc::new(BigInt::from(value))))
        }),
    );
    fields.insert(
        "toInt".to_string(),
        builtin("bigint.toInt", 1, |mut args, _| {
            let value = expect_bigint(args.pop().unwrap(), "bigint.toInt")?;
            let out = value.to_i64().ok_or_else(|| {
                RuntimeError::Message("bigint.toInt overflow".to_string())
            })?;
            Ok(Value::Int(out))
        }),
    );
    fields.insert(
        "add".to_string(),
        builtin("bigint.add", 2, |mut args, _| {
            let right = expect_bigint(args.pop().unwrap(), "bigint.add")?;
            let left = expect_bigint(args.pop().unwrap(), "bigint.add")?;
            Ok(Value::BigInt(Arc::new(&*left + &*right)))
        }),
    );
    fields.insert(
        "sub".to_string(),
        builtin("bigint.sub", 2, |mut args, _| {
            let right = expect_bigint(args.pop().unwrap(), "bigint.sub")?;
            let left = expect_bigint(args.pop().unwrap(), "bigint.sub")?;
            Ok(Value::BigInt(Arc::new(&*left - &*right)))
        }),
    );
    fields.insert(
        "mul".to_string(),
        builtin("bigint.mul", 2, |mut args, _| {
            let right = expect_bigint(args.pop().unwrap(), "bigint.mul")?;
            let left = expect_bigint(args.pop().unwrap(), "bigint.mul")?;
            Ok(Value::BigInt(Arc::new(&*left * &*right)))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn build_rational_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "fromBigInts".to_string(),
        builtin("rational.fromBigInts", 2, |mut args, _| {
            let denom = expect_bigint(args.pop().unwrap(), "rational.fromBigInts")?;
            let numer = expect_bigint(args.pop().unwrap(), "rational.fromBigInts")?;
            if denom.is_zero() {
                return Err(RuntimeError::Message(
                    "rational.fromBigInts expects non-zero denominator".to_string(),
                ));
            }
            Ok(Value::Rational(Arc::new(BigRational::new(
                (*numer).clone(),
                (*denom).clone(),
            ))))
        }),
    );
    fields.insert(
        "normalize".to_string(),
        builtin("rational.normalize", 1, |mut args, _| {
            let value = expect_rational(args.pop().unwrap(), "rational.normalize")?;
            Ok(Value::Rational(Arc::new((*value).clone())))
        }),
    );
    fields.insert(
        "numerator".to_string(),
        builtin("rational.numerator", 1, |mut args, _| {
            let value = expect_rational(args.pop().unwrap(), "rational.numerator")?;
            Ok(Value::BigInt(Arc::new(value.numer().clone())))
        }),
    );
    fields.insert(
        "denominator".to_string(),
        builtin("rational.denominator", 1, |mut args, _| {
            let value = expect_rational(args.pop().unwrap(), "rational.denominator")?;
            Ok(Value::BigInt(Arc::new(value.denom().clone())))
        }),
    );
    fields.insert(
        "add".to_string(),
        builtin("rational.add", 2, |mut args, _| {
            let right = expect_rational(args.pop().unwrap(), "rational.add")?;
            let left = expect_rational(args.pop().unwrap(), "rational.add")?;
            Ok(Value::Rational(Arc::new(&*left + &*right)))
        }),
    );
    fields.insert(
        "sub".to_string(),
        builtin("rational.sub", 2, |mut args, _| {
            let right = expect_rational(args.pop().unwrap(), "rational.sub")?;
            let left = expect_rational(args.pop().unwrap(), "rational.sub")?;
            Ok(Value::Rational(Arc::new(&*left - &*right)))
        }),
    );
    fields.insert(
        "mul".to_string(),
        builtin("rational.mul", 2, |mut args, _| {
            let right = expect_rational(args.pop().unwrap(), "rational.mul")?;
            let left = expect_rational(args.pop().unwrap(), "rational.mul")?;
            Ok(Value::Rational(Arc::new(&*left * &*right)))
        }),
    );
    fields.insert(
        "div".to_string(),
        builtin("rational.div", 2, |mut args, _| {
            let right = expect_rational(args.pop().unwrap(), "rational.div")?;
            let left = expect_rational(args.pop().unwrap(), "rational.div")?;
            Ok(Value::Rational(Arc::new(&*left / &*right)))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn build_decimal_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "fromFloat".to_string(),
        builtin("decimal.fromFloat", 1, |mut args, _| {
            let value = expect_float(args.pop().unwrap(), "decimal.fromFloat")?;
            let decimal = Decimal::from_f64(value).ok_or_else(|| {
                RuntimeError::Message("decimal.fromFloat expects finite Float".to_string())
            })?;
            Ok(Value::Decimal(decimal))
        }),
    );
    fields.insert(
        "toFloat".to_string(),
        builtin("decimal.toFloat", 1, |mut args, _| {
            let value = expect_decimal(args.pop().unwrap(), "decimal.toFloat")?;
            let out = value.to_f64().ok_or_else(|| {
                RuntimeError::Message("decimal.toFloat overflow".to_string())
            })?;
            Ok(Value::Float(out))
        }),
    );
    fields.insert(
        "round".to_string(),
        builtin("decimal.round", 2, |mut args, _| {
            let places = expect_int(args.pop().unwrap(), "decimal.round")?;
            let value = expect_decimal(args.pop().unwrap(), "decimal.round")?;
            let places = places.max(0) as u32;
            Ok(Value::Decimal(value.round_dp(places)))
        }),
    );
    fields.insert(
        "add".to_string(),
        builtin("decimal.add", 2, |mut args, _| {
            let right = expect_decimal(args.pop().unwrap(), "decimal.add")?;
            let left = expect_decimal(args.pop().unwrap(), "decimal.add")?;
            Ok(Value::Decimal(left + right))
        }),
    );
    fields.insert(
        "sub".to_string(),
        builtin("decimal.sub", 2, |mut args, _| {
            let right = expect_decimal(args.pop().unwrap(), "decimal.sub")?;
            let left = expect_decimal(args.pop().unwrap(), "decimal.sub")?;
            Ok(Value::Decimal(left - right))
        }),
    );
    fields.insert(
        "mul".to_string(),
        builtin("decimal.mul", 2, |mut args, _| {
            let right = expect_decimal(args.pop().unwrap(), "decimal.mul")?;
            let left = expect_decimal(args.pop().unwrap(), "decimal.mul")?;
            Ok(Value::Decimal(left * right))
        }),
    );
    fields.insert(
        "div".to_string(),
        builtin("decimal.div", 2, |mut args, _| {
            let right = expect_decimal(args.pop().unwrap(), "decimal.div")?;
            let left = expect_decimal(args.pop().unwrap(), "decimal.div")?;
            Ok(Value::Decimal(left / right))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn url_to_record(url: &Url) -> HashMap<String, Value> {
    let mut map = HashMap::new();
    map.insert("protocol".to_string(), Value::Text(url.scheme().to_string()));
    map.insert(
        "host".to_string(),
        Value::Text(url.host_str().unwrap_or("").to_string()),
    );
    let port = match url.port() {
        Some(port) => make_some(Value::Int(port as i64)),
        None => make_none(),
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
    map.insert("query".to_string(), list_value(query_items));
    let hash = match url.fragment() {
        Some(fragment) => make_some(Value::Text(fragment.to_string())),
        None => make_none(),
    };
    map.insert("hash".to_string(), hash);
    map
}

fn url_from_value(value: Value, ctx: &str) -> Result<Url, RuntimeError> {
    let Value::Record(fields) = value else {
        return Err(RuntimeError::Message(format!("{ctx} expects Url")));
    };
    let protocol = expect_text(
        fields
            .get("protocol")
            .cloned()
            .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Url.protocol")))?,
        ctx,
    )?;
    let host = expect_text(
        fields
            .get("host")
            .cloned()
            .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Url.host")))?,
        ctx,
    )?;
    let base = format!("{protocol}://{host}");
    let mut url = Url::parse(&base).map_err(|err| {
        RuntimeError::Message(format!("{ctx} invalid Url base: {err}"))
    })?;
    if let Some(port) = fields.get("port") {
        match port {
            Value::Constructor { name, args } if name == "Some" && args.len() == 1 => {
                let port = expect_int(args[0].clone(), ctx)? as u16;
                url.set_port(Some(port)).map_err(|_| {
                    RuntimeError::Message(format!("{ctx} invalid Url port"))
                })?;
            }
            Value::Constructor { name, args } if name == "None" && args.is_empty() => {}
            _ => {
                return Err(RuntimeError::Message(format!(
                    "{ctx} expects Url.port Option"
                )))
            }
        }
    }
    if let Some(path) = fields.get("path") {
        let path = expect_text(path.clone(), ctx)?;
        url.set_path(&path);
    }
    if let Some(query) = fields.get("query") {
        let list = expect_list(query.clone(), ctx)?;
        let mut pairs = url.query_pairs_mut();
        pairs.clear();
        for item in list.iter() {
            if let Value::Tuple(items) = item {
                if items.len() == 2 {
                    let key = expect_text(items[0].clone(), ctx)?;
                    let value = expect_text(items[1].clone(), ctx)?;
                    pairs.append_pair(&key, &value);
                    continue;
                }
            }
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Url.query entries"
            )));
        }
        drop(pairs);
    }
    if let Some(hash) = fields.get("hash") {
        match hash {
            Value::Constructor { name, args } if name == "Some" && args.len() == 1 => {
                let value = expect_text(args[0].clone(), ctx)?;
                url.set_fragment(Some(&value));
            }
            Value::Constructor { name, args } if name == "None" && args.is_empty() => {
                url.set_fragment(None);
            }
            _ => {
                return Err(RuntimeError::Message(format!(
                    "{ctx} expects Url.hash Option"
                )))
            }
        }
    }
    Ok(url)
}

fn build_url_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "parse".to_string(),
        builtin("url.parse", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "url.parse")?;
            match Url::parse(&text) {
                Ok(url) => Ok(make_ok(Value::Record(Arc::new(url_to_record(&url))))),
                Err(err) => Ok(make_err(Value::Text(err.to_string()))),
            }
        }),
    );
    fields.insert(
        "toString".to_string(),
        builtin("url.toString", 1, |mut args, _| {
            let url = url_from_value(args.pop().unwrap(), "url.toString")?;
            Ok(Value::Text(url.to_string()))
        }),
    );
    Value::Record(Arc::new(fields))
}

#[derive(Copy, Clone)]
enum HttpClientMode {
    Http,
    Https,
}

fn build_http_client_record(mode: HttpClientMode) -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "get".to_string(),
        builtin("http.get", 1, move |mut args, _| {
            let url = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let url = url_from_value(url.clone(), "http.get")?;
                    ensure_http_scheme(&url, mode, "http.get")?;
                    http_request("GET", &url, Vec::new(), None)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "post".to_string(),
        builtin("http.post", 2, move |mut args, _| {
            let body = args.pop().unwrap();
            let url = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let url = url_from_value(url.clone(), "http.post")?;
                    ensure_http_scheme(&url, mode, "http.post")?;
                    let body = expect_text(body.clone(), "http.post")?;
                    http_request("POST", &url, Vec::new(), Some(body))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "fetch".to_string(),
        builtin("http.fetch", 1, move |mut args, _| {
            let request = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let record = expect_record(request.clone(), "http.fetch expects Request")?;
                    let method = match record.get("method") {
                        Some(Value::Text(text)) => text.clone(),
                        _ => {
                            return Err(RuntimeError::Message(
                                "http.fetch expects Request.method Text".to_string(),
                            ))
                        }
                    };
                    let url_value = record.get("url").cloned().ok_or_else(|| {
                        RuntimeError::Message("http.fetch expects Request.url".to_string())
                    })?;
                    let url = url_from_value(url_value, "http.fetch")?;
                    ensure_http_scheme(&url, mode, "http.fetch")?;
                    let headers = match record.get("headers") {
                        Some(value) => headers_from_value(value, "http.fetch")?,
                        None => Vec::new(),
                    };
                    let body = match record.get("body") {
                        Some(value) => text_option_from_value(value.clone(), "http.fetch")?,
                        None => None,
                    };
                    http_request(&method, &url, headers, body)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn ensure_http_scheme(url: &Url, mode: HttpClientMode, ctx: &str) -> Result<(), RuntimeError> {
    match mode {
        HttpClientMode::Http => Ok(()),
        HttpClientMode::Https => {
            if url.scheme() == "https" {
                Ok(())
            } else {
                Err(RuntimeError::Message(format!(
                    "{ctx} expects an https URL"
                )))
            }
        }
    }
}

fn http_request(
    method: &str,
    url: &Url,
    headers: Vec<(String, String)>,
    body: Option<String>,
) -> Result<Value, RuntimeError> {
    let url_text = url.to_string();
    let mut request = ureq::request(method, &url_text);
    for (name, value) in headers {
        request = request.set(&name, &value);
    }
    let response = match body {
        Some(text) => request.send_string(&text),
        None => request.call(),
    };
    match response {
        Ok(resp) => Ok(make_ok(http_response_to_value(resp)?)),
        Err(err) => Ok(make_err(http_error_record(http_error_message(err)?))),
    }
}

fn http_error_message(err: UreqError) -> Result<String, RuntimeError> {
    match err {
        UreqError::Status(code, response) => {
            let body = response
                .into_string()
                .unwrap_or_else(|_| String::new());
            if body.is_empty() {
                Ok(format!("http status {code}"))
            } else {
                Ok(format!("http status {code}: {body}"))
            }
        }
        UreqError::Transport(err) => Ok(err.to_string()),
    }
}

fn http_response_to_value(resp: ureq::Response) -> Result<Value, RuntimeError> {
    let status = resp.status() as i64;
    let headers = headers_to_value(
        resp.headers_names()
            .into_iter()
            .filter_map(|name| {
                resp.header(&name)
                    .map(|value| (name.to_string(), value.to_string()))
            })
            .collect(),
    );
    let body = resp
        .into_string()
        .map_err(|err| RuntimeError::Error(Value::Text(err.to_string())))?;
    let mut fields = HashMap::new();
    fields.insert("status".to_string(), Value::Int(status));
    fields.insert("headers".to_string(), headers);
    fields.insert("body".to_string(), Value::Text(body));
    Ok(Value::Record(Arc::new(fields)))
}

fn headers_from_value(value: &Value, ctx: &str) -> Result<Vec<(String, String)>, RuntimeError> {
    let list = match value {
        Value::List(items) => items,
        _ => {
            return Err(RuntimeError::Message(format!(
                "{ctx} expects Request.headers List"
            )))
        }
    };
    let mut headers = Vec::with_capacity(list.len());
    for item in list.iter() {
        let record = match item {
            Value::Record(fields) => fields,
            _ => {
                return Err(RuntimeError::Message(format!(
                    "{ctx} expects header records"
                )))
            }
        };
        let name = match record.get("name") {
            Some(Value::Text(text)) => text.clone(),
            _ => {
                return Err(RuntimeError::Message(format!(
                    "{ctx} expects header.name Text"
                )))
            }
        };
        let value = match record.get("value") {
            Some(Value::Text(text)) => text.clone(),
            _ => {
                return Err(RuntimeError::Message(format!(
                    "{ctx} expects header.value Text"
                )))
            }
        };
        headers.push((name, value));
    }
    Ok(headers)
}

fn headers_to_value(entries: Vec<(String, String)>) -> Value {
    let mut list = Vec::with_capacity(entries.len());
    for (name, value) in entries {
        let mut fields = HashMap::new();
        fields.insert("name".to_string(), Value::Text(name));
        fields.insert("value".to_string(), Value::Text(value));
        list.push(Value::Record(Arc::new(fields)));
    }
    Value::List(Arc::new(list))
}

fn text_option_from_value(value: Value, ctx: &str) -> Result<Option<String>, RuntimeError> {
    match value {
        Value::Constructor { name, args } if name == "Some" && args.len() == 1 => {
            Ok(Some(expect_text(args[0].clone(), ctx)?))
        }
        Value::Constructor { name, args } if name == "None" && args.is_empty() => Ok(None),
        _ => Err(RuntimeError::Message(format!(
            "{ctx} expects Option Text"
        ))),
    }
}

fn http_error_record(message: String) -> Value {
    let mut fields = HashMap::new();
    fields.insert("message".to_string(), Value::Text(message));
    Value::Record(Arc::new(fields))
}

fn build_collections_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert("map".to_string(), build_map_record());
    fields.insert("set".to_string(), build_set_record());
    fields.insert("queue".to_string(), build_queue_record());
    fields.insert("deque".to_string(), build_deque_record());
    fields.insert("heap".to_string(), build_heap_record());
    Value::Record(Arc::new(fields))
}

fn build_map_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "empty".to_string(),
        Value::Map(Arc::new(ImHashMap::new())),
    );
    fields.insert(
        "size".to_string(),
        builtin("map.size", 1, |mut args, _| {
            let map = expect_map(args.pop().unwrap(), "map.size")?;
            Ok(Value::Int(map.len() as i64))
        }),
    );
    fields.insert(
        "has".to_string(),
        builtin("map.has", 2, |mut args, _| {
            let map = expect_map(args.pop().unwrap(), "map.has")?;
            let key = key_from_value(&args.pop().unwrap(), "map.has")?;
            Ok(Value::Bool(map.contains_key(&key)))
        }),
    );
    fields.insert(
        "get".to_string(),
        builtin("map.get", 2, |mut args, _| {
            let map = expect_map(args.pop().unwrap(), "map.get")?;
            let key = key_from_value(&args.pop().unwrap(), "map.get")?;
            Ok(match map.get(&key) {
                Some(value) => make_some(value.clone()),
                None => make_none(),
            })
        }),
    );
    fields.insert(
        "insert".to_string(),
        builtin("map.insert", 3, |mut args, _| {
            let map = expect_map(args.pop().unwrap(), "map.insert")?;
            let value = args.pop().unwrap();
            let key = key_from_value(&args.pop().unwrap(), "map.insert")?;
            let mut out = (*map).clone();
            out.insert(key, value);
            Ok(Value::Map(Arc::new(out)))
        }),
    );
    fields.insert(
        "update".to_string(),
        builtin("map.update", 3, |mut args, runtime| {
            let map = expect_map(args.pop().unwrap(), "map.update")?;
            let func = args.pop().unwrap();
            let key = key_from_value(&args.pop().unwrap(), "map.update")?;
            if let Some(current) = map.get(&key) {
                let updated = runtime.apply(func, current.clone())?;
                let mut out = (*map).clone();
                out.insert(key, updated);
                Ok(Value::Map(Arc::new(out)))
            } else {
                Ok(Value::Map(map))
            }
        }),
    );
    fields.insert(
        "remove".to_string(),
        builtin("map.remove", 2, |mut args, _| {
            let map = expect_map(args.pop().unwrap(), "map.remove")?;
            let key = key_from_value(&args.pop().unwrap(), "map.remove")?;
            let mut out = (*map).clone();
            out.remove(&key);
            Ok(Value::Map(Arc::new(out)))
        }),
    );
    fields.insert(
        "map".to_string(),
        builtin("map.map", 2, |mut args, runtime| {
            let map = expect_map(args.pop().unwrap(), "map.map")?;
            let func = args.pop().unwrap();
            let mut out = ImHashMap::new();
            for (key, value) in map.iter() {
                let updated = runtime.apply(func.clone(), value.clone())?;
                out.insert(key.clone(), updated);
            }
            Ok(Value::Map(Arc::new(out)))
        }),
    );
    fields.insert(
        "mapWithKey".to_string(),
        builtin("map.mapWithKey", 2, |mut args, runtime| {
            let map = expect_map(args.pop().unwrap(), "map.mapWithKey")?;
            let func = args.pop().unwrap();
            let mut out = ImHashMap::new();
            for (key, value) in map.iter() {
                let applied = runtime.apply(func.clone(), key.to_value())?;
                let updated = runtime.apply(applied, value.clone())?;
                out.insert(key.clone(), updated);
            }
            Ok(Value::Map(Arc::new(out)))
        }),
    );
    fields.insert(
        "keys".to_string(),
        builtin("map.keys", 1, |mut args, _| {
            let map = expect_map(args.pop().unwrap(), "map.keys")?;
            let items = map.iter().map(|(key, _)| key.to_value()).collect();
            Ok(list_value(items))
        }),
    );
    fields.insert(
        "values".to_string(),
        builtin("map.values", 1, |mut args, _| {
            let map = expect_map(args.pop().unwrap(), "map.values")?;
            let items = map.iter().map(|(_, value)| value.clone()).collect();
            Ok(list_value(items))
        }),
    );
    fields.insert(
        "entries".to_string(),
        builtin("map.entries", 1, |mut args, _| {
            let map = expect_map(args.pop().unwrap(), "map.entries")?;
            let items = map
                .iter()
                .map(|(key, value)| Value::Tuple(vec![key.to_value(), value.clone()]))
                .collect();
            Ok(list_value(items))
        }),
    );
    fields.insert(
        "fromList".to_string(),
        builtin("map.fromList", 1, |mut args, _| {
            let items = expect_list(args.pop().unwrap(), "map.fromList")?;
            let mut out = ImHashMap::new();
            for item in items.iter() {
                match item {
                    Value::Tuple(entries) if entries.len() == 2 => {
                        let key = key_from_value(&entries[0], "map.fromList")?;
                        out.insert(key, entries[1].clone());
                    }
                    _ => {
                        return Err(RuntimeError::Message(
                            "map.fromList expects List (k, v)".to_string(),
                        ))
                    }
                }
            }
            Ok(Value::Map(Arc::new(out)))
        }),
    );
    fields.insert(
        "toList".to_string(),
        builtin("map.toList", 1, |mut args, _| {
            let map = expect_map(args.pop().unwrap(), "map.toList")?;
            let items = map
                .iter()
                .map(|(key, value)| Value::Tuple(vec![key.to_value(), value.clone()]))
                .collect();
            Ok(list_value(items))
        }),
    );
    fields.insert(
        "union".to_string(),
        builtin("map.union", 2, |mut args, _| {
            let right = expect_map(args.pop().unwrap(), "map.union")?;
            let left = expect_map(args.pop().unwrap(), "map.union")?;
            let mut out = (*left).clone();
            for (key, value) in right.iter() {
                out.insert(key.clone(), value.clone());
            }
            Ok(Value::Map(Arc::new(out)))
        }),
    );
    Value::Record(Arc::new(fields))
}

