use std::collections::HashMap;
use std::sync::Arc;

use super::util::{
    builtin, decode_error_list_from_value, make_source_decode_error, value_type_name,
};
use crate::runtime::{EffectValue, RuntimeError, SourceValue, Value};

pub(super) fn build_source_record() -> Value {
    let mut schema_fields = HashMap::new();
    schema_fields.insert(
        "derive".to_string(),
        Value::Constructor {
            name: "SourceSchemaDerive".to_string(),
            args: Vec::new(),
        },
    );

    let mut fields = HashMap::new();
    fields.insert("schema".to_string(), Value::Record(Arc::new(schema_fields)));
    fields.insert(
        "transform".to_string(),
        builtin("source.transform", 2, |mut args, _| {
            let source_value = args.pop().unwrap();
            let transform = args.pop().unwrap();
            let Value::Source(source) = source_value else {
                return Err(RuntimeError::TypeError {
                    context: "source.transform".to_string(),
                    expected: "Source".to_string(),
                    got: value_type_name(&source_value).to_string(),
                });
            };
            let inner_effect = source.effect.clone();
            let kind = source.kind.clone();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let value = runtime.run_effect_value(Value::Effect(inner_effect.clone()))?;
                    runtime.apply(transform.clone(), value)
                }),
            };
            let mut wrapped = SourceValue::new(kind, Arc::new(effect));
            wrapped.schema = source.schema.clone();
            wrapped.raw_text = source.raw_text.clone();
            Ok(Value::Source(Arc::new(wrapped)))
        }),
    );
    fields.insert(
        "validate".to_string(),
        builtin("source.validate", 2, |mut args, _| {
            let source_value = args.pop().unwrap();
            let validate = args.pop().unwrap();
            let Value::Source(source) = source_value else {
                return Err(RuntimeError::TypeError {
                    context: "source.validate".to_string(),
                    expected: "Source".to_string(),
                    got: value_type_name(&source_value).to_string(),
                });
            };
            let inner_effect = source.effect.clone();
            let kind = source.kind.clone();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let value = runtime.run_effect_value(Value::Effect(inner_effect.clone()))?;
                    let validation = runtime.apply(validate.clone(), value)?;
                    match validation {
                        Value::Constructor { name, args } if name == "Valid" && args.len() == 1 => {
                            Ok(args[0].clone())
                        }
                        Value::Constructor { name, args }
                            if name == "Invalid" && args.len() == 1 =>
                        {
                            let errors = decode_error_list_from_value(
                                &args[0],
                                "source.validate expects Invalid (List DecodeError)",
                            )?;
                            Err(RuntimeError::Error(make_source_decode_error(errors)))
                        }
                        other => Err(RuntimeError::Message(format!(
                            "source.validate expects Validation result, got {}",
                            value_type_name(&other)
                        ))),
                    }
                }),
            };
            let mut wrapped = SourceValue::new(kind, Arc::new(effect));
            wrapped.schema = source.schema.clone();
            wrapped.raw_text = source.raw_text.clone();
            Ok(Value::Source(Arc::new(wrapped)))
        }),
    );
    fields.insert(
        "decodeErrors".to_string(),
        builtin("source.decodeErrors", 1, |mut args, _| {
            let err = args.pop().unwrap();
            match err {
                Value::Constructor { name, args } if name == "DecodeError" && args.len() == 1 => {
                    Ok(args[0].clone())
                }
                Value::Constructor { name, args } if name == "IOError" && args.len() == 1 => {
                    Ok(Value::List(Arc::new(Vec::new())))
                }
                other => Err(RuntimeError::TypeError {
                    context: "source.decodeErrors".to_string(),
                    expected: "SourceError".to_string(),
                    got: value_type_name(&other).to_string(),
                }),
            }
        }),
    );
    Value::Record(Arc::new(fields))
}
