use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::util::{builtin, expect_record, expect_text};
use crate::runtime::{EffectValue, RuntimeError, Value};

pub(super) fn build_secrets_record() -> Value {
    let store = Arc::new(Mutex::new(HashMap::<String, Value>::new()));
    let mut fields = HashMap::new();

    {
        let store = store.clone();
        fields.insert(
            "put".to_string(),
            builtin("secrets.put", 2, move |mut args, _| {
                let value = args.pop().unwrap();
                let key = expect_text(args.pop().unwrap(), "secrets.put")?;
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let store = store.clone();
                        move |_| {
                            store
                                .lock()
                                .map_err(|_| RuntimeError::Message("secrets store poisoned".to_string()))?
                                .insert(key.clone(), value.clone());
                            Ok(Value::Unit)
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    {
        let store = store.clone();
        fields.insert(
            "get".to_string(),
            builtin("secrets.get", 1, move |mut args, _| {
                let key = expect_text(args.pop().unwrap(), "secrets.get")?;
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let store = store.clone();
                        move |_| {
                            let value = store
                                .lock()
                                .map_err(|_| RuntimeError::Message("secrets store poisoned".to_string()))?
                                .get(&key)
                                .cloned();
                            Ok(match value {
                                Some(value) => Value::Constructor {
                                    name: "Some".to_string(),
                                    args: vec![value],
                                },
                                None => Value::Constructor {
                                    name: "None".to_string(),
                                    args: Vec::new(),
                                },
                            })
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    {
        let store = store.clone();
        fields.insert(
            "delete".to_string(),
            builtin("secrets.delete", 1, move |mut args, _| {
                let key = expect_text(args.pop().unwrap(), "secrets.delete")?;
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let store = store.clone();
                        move |_| {
                            store
                                .lock()
                                .map_err(|_| RuntimeError::Message("secrets store poisoned".to_string()))?
                                .remove(&key);
                            Ok(Value::Unit)
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    fields.insert(
        "makeBlob".to_string(),
        builtin("secrets.makeBlob", 3, |mut args, _| {
            let ciphertext = args.pop().unwrap();
            let algorithm = expect_text(args.pop().unwrap(), "secrets.makeBlob")?;
            let key_id = expect_text(args.pop().unwrap(), "secrets.makeBlob")?;
            match ciphertext {
                Value::Bytes(_) => {
                    let mut blob = HashMap::new();
                    blob.insert("keyId".to_string(), Value::Text(key_id));
                    blob.insert("algorithm".to_string(), Value::Text(algorithm));
                    blob.insert("ciphertext".to_string(), ciphertext);
                    Ok(Value::Record(Arc::new(blob)))
                }
                other => Err(RuntimeError::Message(format!(
                    "secrets.makeBlob expects Bytes ciphertext, got {}",
                    crate::runtime::format_value(&other)
                ))),
            }
        }),
    );

    fields.insert(
        "validateBlob".to_string(),
        builtin("secrets.validateBlob", 1, |mut args, _| {
            let record = expect_record(args.pop().unwrap(), "secrets.validateBlob")?;
            let has_key_id = record.contains_key("keyId");
            let has_algorithm = record.contains_key("algorithm");
            let has_ciphertext = record
                .get("ciphertext")
                .map(|value| matches!(value, Value::Bytes(_)))
                .unwrap_or(false);
            Ok(Value::Bool(has_key_id && has_algorithm && has_ciphertext))
        }),
    );

    Value::Record(Arc::new(fields))
}
