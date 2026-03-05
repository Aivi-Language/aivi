use std::collections::HashMap;
use std::sync::Arc;

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256, Sha384, Sha512};
use subtle::ConstantTimeEq;
use uuid::Uuid;

use super::util::{builtin, expect_bytes, expect_int, expect_text};
use crate::runtime::{EffectValue, RuntimeError, Value};

pub(super) fn build_crypto_record() -> Value {
    let mut fields = HashMap::new();

    // -- Hashing --

    fields.insert(
        "sha256".to_string(),
        builtin("crypto.sha256", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "crypto.sha256")?;
            let digest = Sha256::digest(text.as_bytes());
            Ok(Value::Text(format!("{:x}", digest)))
        }),
    );

    fields.insert(
        "sha384".to_string(),
        builtin("crypto.sha384", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "crypto.sha384")?;
            let digest = Sha384::digest(text.as_bytes());
            Ok(Value::Text(format!("{:x}", digest)))
        }),
    );

    fields.insert(
        "sha512".to_string(),
        builtin("crypto.sha512", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "crypto.sha512")?;
            let digest = Sha512::digest(text.as_bytes());
            Ok(Value::Text(format!("{:x}", digest)))
        }),
    );

    // -- HMAC --

    fields.insert(
        "hmacSha256".to_string(),
        builtin("crypto.hmacSha256", 2, |mut args, _| {
            let msg = expect_bytes(args.pop().unwrap(), "crypto.hmacSha256")?;
            let key = expect_bytes(args.pop().unwrap(), "crypto.hmacSha256")?;
            let mut mac = <Hmac<Sha256>>::new_from_slice(&key)
                .map_err(|e| RuntimeError::Message(format!("crypto.hmacSha256: {e}")))?;
            mac.update(&msg);
            let result = mac.finalize().into_bytes();
            Ok(Value::Bytes(Arc::new(result.to_vec())))
        }),
    );

    fields.insert(
        "hmacSha512".to_string(),
        builtin("crypto.hmacSha512", 2, |mut args, _| {
            let msg = expect_bytes(args.pop().unwrap(), "crypto.hmacSha512")?;
            let key = expect_bytes(args.pop().unwrap(), "crypto.hmacSha512")?;
            let mut mac = <Hmac<Sha512>>::new_from_slice(&key)
                .map_err(|e| RuntimeError::Message(format!("crypto.hmacSha512: {e}")))?;
            mac.update(&msg);
            let result = mac.finalize().into_bytes();
            Ok(Value::Bytes(Arc::new(result.to_vec())))
        }),
    );

    fields.insert(
        "hmacVerify".to_string(),
        builtin("crypto.hmacVerify", 3, |mut args, _| {
            let tag = expect_bytes(args.pop().unwrap(), "crypto.hmacVerify")?;
            let msg = expect_bytes(args.pop().unwrap(), "crypto.hmacVerify")?;
            let key = expect_bytes(args.pop().unwrap(), "crypto.hmacVerify")?;
            let mut mac = <Hmac<Sha256>>::new_from_slice(&key)
                .map_err(|e| RuntimeError::Message(format!("crypto.hmacVerify: {e}")))?;
            mac.update(&msg);
            let valid = mac.verify_slice(&tag).is_ok();
            Ok(Value::Bool(valid))
        }),
    );

    // -- Password Hashing --

    fields.insert(
        "hashPassword".to_string(),
        builtin("crypto.hashPassword", 1, |mut args, _| {
            let password = expect_text(args.pop().unwrap(), "crypto.hashPassword")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let hashed = bcrypt::hash(&password, bcrypt::DEFAULT_COST).map_err(|e| {
                        RuntimeError::Message(format!("crypto.hashPassword failed: {e}"))
                    })?;
                    Ok(Value::Text(hashed))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    fields.insert(
        "verifyPassword".to_string(),
        builtin("crypto.verifyPassword", 2, |mut args, _| {
            let hash = expect_text(args.pop().unwrap(), "crypto.verifyPassword")?;
            let password = expect_text(args.pop().unwrap(), "crypto.verifyPassword")?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let valid = bcrypt::verify(&password, &hash).map_err(|e| {
                        RuntimeError::Message(format!("crypto.verifyPassword failed: {e}"))
                    })?;
                    Ok(Value::Bool(valid))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    // -- Random --

    fields.insert(
        "randomUuid".to_string(),
        builtin("crypto.randomUuid", 1, |_, _| {
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let mut bytes = [0u8; 16];
                    getrandom::fill(&mut bytes).map_err(|err| {
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
                RuntimeError::Overflow { context: "crypto.randomBytes".to_string() }
            })?;
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let mut buffer = vec![0u8; count];
                    if count > 0 {
                        getrandom::fill(&mut buffer).map_err(|err| {
                            RuntimeError::Message(format!("crypto.randomBytes failed: {err}"))
                        })?;
                    }
                    Ok(Value::Bytes(Arc::new(buffer)))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    // -- Utilities --

    fields.insert(
        "secureEquals".to_string(),
        builtin("crypto.secureEquals", 2, |mut args, _| {
            let b = expect_bytes(args.pop().unwrap(), "crypto.secureEquals")?;
            let a = expect_bytes(args.pop().unwrap(), "crypto.secureEquals")?;
            let equal = a.len() == b.len() && a.ct_eq(&b).into();
            Ok(Value::Bool(equal))
        }),
    );

    fields.insert(
        "fromHex".to_string(),
        builtin("crypto.fromHex", 1, |mut args, _| {
            let text = expect_text(args.pop().unwrap(), "crypto.fromHex")?;
            match hex::decode(&text) {
                Ok(bytes) => Ok(Value::Constructor {
                    name: "Ok".to_string(),
                    args: vec![Value::Bytes(Arc::new(bytes))],
                }),
                Err(e) => Ok(Value::Constructor {
                    name: "Err".to_string(),
                    args: vec![Value::Text(format!("{e}"))],
                }),
            }
        }),
    );

    fields.insert(
        "toHex".to_string(),
        builtin("crypto.toHex", 1, |mut args, _| {
            let bytes = expect_bytes(args.pop().unwrap(), "crypto.toHex")?;
            Ok(Value::Text(hex::encode(&*bytes)))
        }),
    );

    Value::Record(Arc::new(fields))
}
