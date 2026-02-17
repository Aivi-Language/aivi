use std::collections::HashMap;
use std::sync::Arc;

use getrandom;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::util::{builtin, expect_int, expect_text};
use crate::runtime::{EffectValue, RuntimeError, Value};

pub(super) fn build_crypto_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "sha256".to_string(),
        builtin("crypto.sha256", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "crypto.sha256")?;
            let digest = Sha256::digest(text.as_bytes());
            Ok(Value::Text(format!("{:x}", digest)))
        }),
    );
    fields.insert(
        "randomUuid".to_string(),
        builtin("crypto.randomUuid", 1, |_, _| {
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let mut bytes = [0u8; 16];
                    getrandom::getrandom(&mut bytes).map_err(|err| {
                        RuntimeError::Message(format!("crypto.randomUuid failed: {err}"))
                    })?;
                    bytes[6] = (bytes[6] & 0x0f) | 0x40;
                    bytes[8] = (bytes[8] & 0x3f) | 0x80;
                    let uuid = Uuid::from_bytes(bytes);
                    Ok(Value::Text(uuid.to_string()))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "randomBytes".to_string(),
        builtin("crypto.randomBytes", 1, |mut args, _| {
            let count = expect_int(args.pop().unwrap(), "crypto.randomBytes")?;
            if count < 0 {
                return Err(RuntimeError::Message(
                    "crypto.randomBytes expects non-negative length".to_string(),
                ));
            }
            let count = usize::try_from(count).map_err(|_| {
                RuntimeError::Message("crypto.randomBytes length overflow".to_string())
            })?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let mut buffer = vec![0u8; count];
                    if count > 0 {
                        getrandom::getrandom(&mut buffer).map_err(|err| {
                            RuntimeError::Message(format!("crypto.randomBytes failed: {err}"))
                        })?;
                    }
                    Ok(Value::Bytes(Arc::new(buffer)))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    Value::Record(Arc::new(fields))
}
