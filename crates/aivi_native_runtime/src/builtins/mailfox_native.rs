use std::collections::HashMap;
use std::sync::Arc;

use super::util::{builtin, expect_text};
use crate::{EffectValue, RuntimeError, Value};

fn effect<F>(f: F) -> Value
where
    F: Fn(&mut crate::Runtime) -> Result<Value, RuntimeError> + Send + Sync + 'static,
{
    Value::Effect(Arc::new(EffectValue::Thunk { func: Arc::new(f) }))
}

pub(super) fn build_mailfox_native_record() -> Value {
    let mut fields = HashMap::new();

    fields.insert(
        "accountUpsert".to_string(),
        builtin("mailfoxNative.accountUpsert", 5, |mut args, _| {
            let _sync_since = expect_text(args.remove(4), "mailfoxNative.accountUpsert")?;
            let _provider = expect_text(args.remove(3), "mailfoxNative.accountUpsert")?;
            let _email = expect_text(args.remove(2), "mailfoxNative.accountUpsert")?;
            let _display_name = expect_text(args.remove(1), "mailfoxNative.accountUpsert")?;
            let _account_id = expect_text(args.remove(0), "mailfoxNative.accountUpsert")?;
            Ok(effect(|_| Ok(Value::Unit)))
        }),
    );

    fields.insert(
        "accountSoftDelete".to_string(),
        builtin("mailfoxNative.accountSoftDelete", 1, |mut args, _| {
            let _account_id = expect_text(args.remove(0), "mailfoxNative.accountSoftDelete")?;
            Ok(effect(|_| Ok(Value::Unit)))
        }),
    );

    fields.insert(
        "accountReAdd".to_string(),
        builtin("mailfoxNative.accountReAdd", 1, |mut args, _| {
            let _account_id = expect_text(args.remove(0), "mailfoxNative.accountReAdd")?;
            Ok(effect(|_| Ok(Value::Unit)))
        }),
    );

    fields.insert(
        "commandEnqueue".to_string(),
        builtin("mailfoxNative.commandEnqueue", 3, |mut args, _| {
            let _dedupe_key = expect_text(args.remove(2), "mailfoxNative.commandEnqueue")?;
            let _payload = expect_text(args.remove(1), "mailfoxNative.commandEnqueue")?;
            let _kind = expect_text(args.remove(0), "mailfoxNative.commandEnqueue")?;
            Ok(effect(|_| Ok(Value::Unit)))
        }),
    );

    fields.insert(
        "heartbeatWrite".to_string(),
        builtin("mailfoxNative.heartbeatWrite", 2, |mut args, _| {
            let _status = expect_text(args.remove(1), "mailfoxNative.heartbeatWrite")?;
            let _owner_id = expect_text(args.remove(0), "mailfoxNative.heartbeatWrite")?;
            Ok(effect(|_| Ok(Value::Unit)))
        }),
    );

    Value::Record(Arc::new(fields))
}
