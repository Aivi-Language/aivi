use std::collections::HashMap;
use std::sync::Arc;

use super::util::{
    builtin, builtin_constructor, expect_list, expect_record, expect_text, list_value,
};
use crate::{EffectValue, Runtime, RuntimeError, Value};

fn table_parts(value: Value, ctx: &str) -> Result<(String, Value, Arc<Vec<Value>>), RuntimeError> {
    let fields = expect_record(value, ctx)?;
    let name = fields
        .get("name")
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Table.name")))?;
    let name = expect_text(name.clone(), ctx)?;
    let columns = fields
        .get("columns")
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Table.columns")))?;
    let rows = fields
        .get("rows")
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects Table.rows")))?;
    let rows = expect_list(rows.clone(), ctx)?;
    Ok((name, columns.clone(), rows))
}

fn make_table(name: String, columns: Value, rows: Vec<Value>) -> Value {
    let mut fields = HashMap::new();
    fields.insert("name".to_string(), Value::Text(name));
    fields.insert("columns".to_string(), columns);
    fields.insert("rows".to_string(), list_value(rows));
    Value::Record(Arc::new(fields))
}

fn apply_delta(table: Value, delta: Value, runtime: &mut Runtime) -> Result<Value, RuntimeError> {
    let (name, columns, rows) = table_parts(table, "database.applyDelta")?;
    let mut out = Vec::with_capacity(rows.len());
    match delta {
        Value::Constructor { name: tag, args } => match tag.as_str() {
            "Insert" => {
                if args.len() != 1 {
                    return Err(RuntimeError::Message(
                        "database.applyDelta expects Insert value".to_string(),
                    ));
                }
                out.extend(rows.iter().cloned());
                out.push(args[0].clone());
            }
            "Update" => {
                if args.len() != 2 {
                    return Err(RuntimeError::Message(
                        "database.applyDelta expects Update predicate and patch".to_string(),
                    ));
                }
                let pred = args[0].clone();
                let patch = args[1].clone();
                for row in rows.iter() {
                    let keep = runtime.apply(pred.clone(), row.clone())?;
                    let keep = match keep {
                        Value::Bool(value) => value,
                        other => {
                            return Err(RuntimeError::Message(format!(
                                "database.applyDelta Update predicate expects Bool, got {}",
                                crate::format_value(&other)
                            )))
                        }
                    };
                    if keep {
                        let updated = runtime.apply(patch.clone(), row.clone())?;
                        out.push(updated);
                    } else {
                        out.push(row.clone());
                    }
                }
            }
            "Delete" => {
                if args.len() != 1 {
                    return Err(RuntimeError::Message(
                        "database.applyDelta expects Delete predicate".to_string(),
                    ));
                }
                let pred = args[0].clone();
                for row in rows.iter() {
                    let matches = runtime.apply(pred.clone(), row.clone())?;
                    let matches = match matches {
                        Value::Bool(value) => value,
                        other => {
                            return Err(RuntimeError::Message(format!(
                                "database.applyDelta Delete predicate expects Bool, got {}",
                                crate::format_value(&other)
                            )))
                        }
                    };
                    if !matches {
                        out.push(row.clone());
                    }
                }
            }
            _ => {
                return Err(RuntimeError::Message(
                    "database.applyDelta expects Delta".to_string(),
                ))
            }
        },
        _ => {
            return Err(RuntimeError::Message(
                "database.applyDelta expects Delta".to_string(),
            ))
        }
    }
    Ok(make_table(name, columns, out))
}

pub(super) fn build_database_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "table".to_string(),
        builtin("database.table", 2, |mut args, _| {
            let columns = args.pop().unwrap();
            let name = expect_text(args.pop().unwrap(), "database.table")?;
            Ok(make_table(name, columns, Vec::new()))
        }),
    );
    fields.insert(
        "load".to_string(),
        builtin("database.load", 1, |mut args, _| {
            let table = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let (_, _, rows) = table_parts(table.clone(), "database.load")?;
                    Ok(list_value(rows.iter().cloned().collect()))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "applyDelta".to_string(),
        builtin("database.applyDelta", 2, |mut args, _| {
            let delta = args.pop().unwrap();
            let table = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| apply_delta(table.clone(), delta.clone(), runtime)),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert(
        "runMigrations".to_string(),
        builtin("database.runMigrations", 1, |mut args, _| {
            let _tables = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| Ok(Value::Unit)),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );
    fields.insert("ins".to_string(), builtin_constructor("Insert", 1));
    fields.insert("upd".to_string(), builtin_constructor("Update", 2));
    fields.insert("del".to_string(), builtin_constructor("Delete", 1));
    Value::Record(Arc::new(fields))
}
