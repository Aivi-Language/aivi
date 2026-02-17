use std::collections::HashMap;
use std::sync::Arc;

use url::Url;

use super::util::{
    builtin, expect_int, expect_list, expect_record, expect_text, list_value, make_err, make_none,
    make_ok, make_some,
};
use crate::runtime::{EffectValue, RuntimeError, SourceValue, Value};
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
    let mut url = Url::parse(&base)
        .map_err(|err| RuntimeError::Message(format!("{ctx} invalid Url base: {err}")))?;
    if let Some(port) = fields.get("port") {
        match port {
            Value::Constructor { name, args } if name == "Some" && args.len() == 1 => {
                let port = expect_int(args[0].clone(), ctx)? as u16;
                url.set_port(Some(port))
                    .map_err(|_| RuntimeError::Message(format!("{ctx} invalid Url port")))?;
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

pub(super) fn build_url_record() -> Value {
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

pub(super) enum HttpClientMode {
    Http,
    Https,
}

pub(super) fn build_http_client_record(mode: HttpClientMode) -> Value {
    let kind = match mode {
        HttpClientMode::Http => "Http",
        HttpClientMode::Https => "Https",
    }
    .to_string();
    let mut fields = HashMap::new();
    let kind_get = kind.clone();
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
            Ok(Value::Source(Arc::new(SourceValue {
                kind: kind_get.clone(),
                effect: Arc::new(effect),
            })))
        }),
    );
    let kind_post = kind.clone();
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
            Ok(Value::Source(Arc::new(SourceValue {
                kind: kind_post.clone(),
                effect: Arc::new(effect),
            })))
        }),
    );
    let kind_fetch = kind.clone();
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
            Ok(Value::Source(Arc::new(SourceValue {
                kind: kind_fetch.clone(),
                effect: Arc::new(effect),
            })))
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
                Err(RuntimeError::Message(format!("{ctx} expects an https URL")))
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
    let method = method.to_ascii_uppercase();

    // ureq 3 uses typestates for methods that can/can't have request bodies. Keep it explicit.
    let mut response = if let Some(body_text) = body {
        let builder = match method.as_str() {
            "POST" => ureq::post(&url_text),
            "PUT" => ureq::put(&url_text),
            "PATCH" => ureq::patch(&url_text),
            _ => {
                return Err(RuntimeError::Message(format!(
                    "http.fetch does not support body for method {method}"
                )))
            }
        };
        let builder = headers
            .into_iter()
            .fold(builder, |builder, (name, value)| builder.header(name, value));
        builder
            .config()
            .http_status_as_error(false)
            .build()
            .send(body_text)
    } else {
        match method.as_str() {
            "POST" | "PUT" | "PATCH" => {
                let builder = match method.as_str() {
                    "POST" => ureq::post(&url_text),
                    "PUT" => ureq::put(&url_text),
                    "PATCH" => ureq::patch(&url_text),
                    _ => unreachable!("method matched above"),
                };
                let builder = headers
                    .into_iter()
                    .fold(builder, |builder, (name, value)| builder.header(name, value));
                builder
                    .config()
                    .http_status_as_error(false)
                    .build()
                    .send_empty()
            }
            _ => {
                let builder = match method.as_str() {
                    "GET" => ureq::get(&url_text),
                    "DELETE" => ureq::delete(&url_text),
                    "HEAD" => ureq::head(&url_text),
                    _ => ureq::get(&url_text),
                };
                let builder = headers
                    .into_iter()
                    .fold(builder, |builder, (name, value)| builder.header(name, value));
                builder
                    .config()
                    .http_status_as_error(false)
                    .build()
                    .call()
            }
        }
    };

    match response.as_mut() {
        Ok(resp) => Ok(make_ok(http_response_to_value(resp)?)),
        Err(err) => Ok(make_err(http_error_record(err.to_string()))),
    }
}

fn http_response_to_value(
    resp: &mut ureq::http::Response<ureq::Body>,
) -> Result<Value, RuntimeError> {
    let status = resp.status().as_u16() as i64;
    let headers = headers_to_value(
        resp.headers()
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.to_string(), value.to_string()))
            })
            .collect(),
    );
    let body = resp
        .body_mut()
        .read_to_string()
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
        _ => Err(RuntimeError::Message(format!("{ctx} expects Option Text"))),
    }
}

fn http_error_record(message: String) -> Value {
    let mut fields = HashMap::new();
    fields.insert("message".to_string(), Value::Text(message));
    Value::Record(Arc::new(fields))
}
