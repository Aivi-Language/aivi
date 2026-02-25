use std::collections::HashMap;
use std::sync::Arc;

use super::util::{builtin, expect_text};
use crate::runtime::{EffectValue, RuntimeError, Value};

type ManagedObjects = HashMap<
    zbus::zvariant::OwnedObjectPath,
    HashMap<String, HashMap<String, zbus::zvariant::OwnedValue>>,
>;

pub(super) fn build_goa_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "listAccounts".to_string(),
        builtin("goa.listAccounts", 1, |_, _| {
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| list_accounts()),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "getAccessToken".to_string(),
        builtin("goa.getAccessToken", 1, |mut args, _| {
            let key = expect_text(args.pop().unwrap(), "goa.getAccessToken")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| get_access_token(&key)),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    Value::Record(Arc::new(fields))
}

fn list_accounts() -> Result<Value, RuntimeError> {
    let conn = zbus::blocking::Connection::session().map_err(goa_err("session bus"))?;
    let proxy = zbus::blocking::Proxy::new(
        &conn,
        "org.gnome.OnlineAccounts",
        "/org/gnome/OnlineAccounts",
        "org.freedesktop.DBus.ObjectManager",
    )
    .map_err(goa_err("object manager"))?;

    let managed: ManagedObjects = proxy
        .call("GetManagedObjects", &())
        .map_err(goa_err("GetManagedObjects"))?;

    let mut accounts = Vec::new();
    for (path, interfaces) in managed {
        if !interfaces.contains_key("org.gnome.OnlineAccounts.Account") {
            continue;
        }
        let mut fields = HashMap::new();
        fields.insert("key".to_string(), Value::Text(path.to_string()));
        accounts.push(Value::Record(Arc::new(fields)));
    }
    Ok(Value::List(Arc::new(accounts)))
}

fn get_access_token(key: &str) -> Result<Value, RuntimeError> {
    if !key.starts_with('/') {
        return Err(RuntimeError::Message(
            "goa.getAccessToken expects a GOA object path key".to_string(),
        ));
    }
    let conn = zbus::blocking::Connection::session().map_err(goa_err("session bus"))?;
    let proxy = zbus::blocking::Proxy::new(
        &conn,
        "org.gnome.OnlineAccounts",
        key,
        "org.gnome.OnlineAccounts.OAuth2Based",
    )
    .map_err(goa_err("OAuth2Based proxy"))?;

    let (token, expires_unix): (String, i64) = proxy
        .call("GetAccessToken", &())
        .map_err(goa_err("GetAccessToken"))?;

    let mut fields = HashMap::new();
    fields.insert("token".to_string(), Value::Text(token));
    fields.insert("expiresUnix".to_string(), Value::Int(expires_unix));
    Ok(Value::Record(Arc::new(fields)))
}

fn goa_err(ctx: &'static str) -> impl FnOnce(zbus::Error) -> RuntimeError {
    move |err| RuntimeError::Error(Value::Text(format!("goa.{ctx}: {err}")))
}
