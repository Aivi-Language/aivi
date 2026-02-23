
fn apply_delta_rows(
    rows: &[Value],
    delta: Value,
    runtime: &mut Runtime,
) -> Result<Vec<Value>, RuntimeError> {
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
                                crate::runtime::format_value(&other)
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
                                crate::runtime::format_value(&other)
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
    Ok(out)
}

fn apply_delta(table: Value, delta: Value, runtime: &mut Runtime) -> Result<Value, RuntimeError> {
    let (name, columns, rows) = table_parts(table, "database.applyDelta")?;
    let out = apply_delta_rows(rows.as_ref(), delta, runtime)?;
    Ok(make_table(name, columns, out))
}

fn parse_driver(value: Value) -> Result<Driver, RuntimeError> {
    match value {
        Value::Constructor { name, args } if args.is_empty() => match name.as_str() {
            "Sqlite" => Ok(Driver::Sqlite),
            "Postgresql" => Ok(Driver::Postgresql),
            "Mysql" => Ok(Driver::Mysql),
            _ => Err(RuntimeError::Message(format!(
                "database.configure expects Driver (Sqlite|Postgresql|Mysql), got {name}"
            ))),
        },
        other => Err(RuntimeError::Message(format!(
            "database.configure expects Driver, got {}",
            crate::runtime::format_value(&other)
        ))),
    }
}

fn require_sql_identifier(value: Value, ctx: &str, field: &str) -> Result<String, RuntimeError> {
    let name = expect_text(value, ctx)?;
    if name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        && !name.is_empty()
    {
        Ok(name)
    } else {
        Err(RuntimeError::Message(format!(
            "{ctx} expects {field} as SQL identifier [A-Za-z0-9_]+"
        )))
    }
}

pub(super) fn build_database_record() -> Value {
    let state = Arc::new(DatabaseState::new());

    let mut fields = HashMap::new();

    fields.insert(
        "table".to_string(),
        builtin("database.table", 2, |mut args, _| {
            let columns = args.pop().unwrap();
            let name = expect_text(args.pop().unwrap(), "database.table")?;
            Ok(make_table(name, columns, Vec::new()))
        }),
    );

    {
        let state = state.clone();
        fields.insert(
            "configure".to_string(),
            builtin("database.configure", 1, move |mut args, _| {
                let config = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |_| {
                            let config_fields =
                                expect_record(config.clone(), "database.configure")?;
                            let driver = config_fields
                                .get("driver")
                                .ok_or_else(|| {
                                    RuntimeError::Message(
                                        "database.configure expects DbConfig.driver".to_string(),
                                    )
                                })?
                                .clone();
                            let url = config_fields
                                .get("url")
                                .ok_or_else(|| {
                                    RuntimeError::Message(
                                        "database.configure expects DbConfig.url".to_string(),
                                    )
                                })?
                                .clone();

                            let driver = parse_driver(driver)?;
                            let url = expect_text(url, "database.configure")?;

                            state
                                .handle
                                .request(|resp| DbRequest::Configure { driver, url, resp })
                                .map_err(RuntimeError::Message)?;
                            state.configured.store(true, Ordering::SeqCst);
                            Ok(Value::Unit)
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    {
        let state = state.clone();
        fields.insert(
            "load".to_string(),
            builtin("database.load", 1, move |mut args, _| {
                let table = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |_| {
                            if !state.is_configured() {
                                let (_, _, rows) = table_parts(table.clone(), "database.load")?;
                                return Ok(list_value(rows.iter().cloned().collect()));
                            }

                            let (name, _columns, _rows) =
                                table_parts(table.clone(), "database.load")?;
                            let entry = state
                                .handle
                                .request(|resp| DbRequest::LoadTable {
                                    name: name.clone(),
                                    resp,
                                })
                                .map_err(RuntimeError::Message)?;
                            let rows_json = match entry {
                                Some((_rev, _cols, rows_json)) => rows_json,
                                None => EMPTY_ROWS_JSON.to_string(),
                            };
                            let rows_value = decode_json(&rows_json)?;
                            let Value::List(rows) = rows_value else {
                                return Err(RuntimeError::Message(
                                    "database: invalid persisted rows (expected List)".to_string(),
                                ));
                            };
                            Ok(Value::List(rows))
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    {
        let state = state.clone();
        fields.insert(
            "applyDelta".to_string(),
            builtin("database.applyDelta", 2, move |mut args, _| {
                let delta = args.pop().unwrap();
                let table = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |runtime| {
                            if !state.is_configured() {
                                return apply_delta(table.clone(), delta.clone(), runtime);
                            }

                            let (name, columns, _rows) =
                                table_parts(table.clone(), "database.applyDelta")?;
                            let columns_json = encode_json(&columns)?;

                            // Retry a few times if another writer updated the same table.
                            for _attempt in 0..3 {
                                let entry = state
                                    .handle
                                    .request(|resp| DbRequest::LoadTable {
                                        name: name.clone(),
                                        resp,
                                    })
                                    .map_err(RuntimeError::Message)?;

                                if entry.is_none() {
                                    state
                                        .handle
                                        .request(|resp| DbRequest::MigrateTable {
                                            name: name.clone(),
                                            columns_json: columns_json.clone(),
                                            resp,
                                        })
                                        .map_err(RuntimeError::Message)?;
                                    continue;
                                }

                                let (rev, _stored_cols, rows_json) = entry.unwrap();
                                let rows_value = decode_json(&rows_json)?;
                                let Value::List(rows_list) = rows_value else {
                                    return Err(RuntimeError::Message(
                                        "database: invalid persisted rows (expected List)"
                                            .to_string(),
                                    ));
                                };
                                let current_rows: Vec<Value> = rows_list.iter().cloned().collect();

                                let new_rows =
                                    apply_delta_rows(&current_rows, delta.clone(), runtime)?;
                                let rows_json = encode_json(&list_value(new_rows.clone()))?;

                                let saved =
                                    state.handle.request(|resp| DbRequest::CompareAndSwapRows {
                                        name: name.clone(),
                                        expected_rev: rev,
                                        columns_json: columns_json.clone(),
                                        rows_json,
                                        resp,
                                    });
                                match saved {
                                    Ok(_new_rev) => {
                                        return Ok(make_table(
                                            name.clone(),
                                            columns.clone(),
                                            new_rows,
                                        ))
                                    }
                                    Err(err) => {
                                        if err.contains("retry") {
                                            continue;
                                        }
                                        return Err(RuntimeError::Message(err));
                                    }
                                }
                            }

                            Err(RuntimeError::Message(
                                "database.applyDelta failed due to concurrent writes; retry"
                                    .to_string(),
                            ))
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    {
        let state = state.clone();
        fields.insert(
            "runMigrations".to_string(),
            builtin("database.runMigrations", 1, move |mut args, _| {
                let tables = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |_| {
                            if !state.is_configured() {
                                return Ok(Value::Unit);
                            }
                            let tables = expect_list(tables.clone(), "database.runMigrations")?;

                            state
                                .handle
                                .request(|resp| DbRequest::EnsureSchema { resp })
                                .map_err(RuntimeError::Message)?;

                            for table in tables.iter() {
                                let (name, columns, _rows) =
                                    table_parts(table.clone(), "database.runMigrations")?;
                                let columns_json = encode_json(&columns)?;
                                state
                                    .handle
                                    .request(|resp| DbRequest::MigrateTable {
                                        name,
                                        columns_json,
                                        resp,
                                    })
                                    .map_err(RuntimeError::Message)?;
                            }
                            Ok(Value::Unit)
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    {
        let state = state.clone();
        fields.insert(
            "configureSqlite".to_string(),
            builtin("database.configureSqlite", 1, move |mut args, _| {
                let config = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |_| {
                            let config_fields =
                                expect_record(config.clone(), "database.configureSqlite")?;
                            let wal = config_fields
                                .get("wal")
                                .cloned()
                                .unwrap_or(Value::Bool(true));
                            let busy_timeout = config_fields
                                .get("busyTimeoutMs")
                                .cloned()
                                .unwrap_or(Value::Int(5000));
                            let wal = match wal {
                                Value::Bool(v) => v,
                                other => {
                                    return Err(RuntimeError::Message(format!(
                                        "database.configureSqlite expects wal Bool, got {}",
                                        crate::runtime::format_value(&other)
                                    )))
                                }
                            };
                            let busy_timeout_ms =
                                expect_int(busy_timeout, "database.configureSqlite")?;
                            state
                                .handle
                                .request(|resp| DbRequest::SqliteConfigure {
                                    wal,
                                    busy_timeout_ms,
                                    resp,
                                })
                                .map_err(RuntimeError::Message)?;
                            Ok(Value::Unit)
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    for name in ["beginTx", "commitTx", "rollbackTx"] {
        let state = state.clone();
        fields.insert(
            name.to_string(),
            builtin(&format!("database.{name}"), 0, move |_, _| {
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |_| {
                            match name {
                                "beginTx" => state
                                    .handle
                                    .request(|resp| DbRequest::BeginTransaction { resp })
                                    .map_err(RuntimeError::Message)?,
                                "commitTx" => state
                                    .handle
                                    .request(|resp| DbRequest::CommitTransaction { resp })
                                    .map_err(RuntimeError::Message)?,
                                _ => state
                                    .handle
                                    .request(|resp| DbRequest::RollbackTransaction { resp })
                                    .map_err(RuntimeError::Message)?,
                            };
                            Ok(Value::Unit)
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    {
        let state = state.clone();
        fields.insert(
            "savepoint".to_string(),
            builtin("database.savepoint", 1, move |mut args, _| {
                let raw_name = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |_| {
                            let name = require_sql_identifier(
                                raw_name.clone(),
                                "database.savepoint",
                                "name",
                            )?;
                            state
                                .handle
                                .request(|resp| DbRequest::Savepoint { name, resp })
                                .map_err(RuntimeError::Message)?;
                            Ok(Value::Unit)
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    {
        let state = state.clone();
        fields.insert(
            "releaseSavepoint".to_string(),
            builtin("database.releaseSavepoint", 1, move |mut args, _| {
                let raw_name = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |_| {
                            let name = require_sql_identifier(
                                raw_name.clone(),
                                "database.releaseSavepoint",
                                "name",
                            )?;
                            state
                                .handle
                                .request(|resp| DbRequest::ReleaseSavepoint { name, resp })
                                .map_err(RuntimeError::Message)?;
                            Ok(Value::Unit)
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    {
        let state = state.clone();
        fields.insert(
            "rollbackToSavepoint".to_string(),
            builtin("database.rollbackToSavepoint", 1, move |mut args, _| {
                let raw_name = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |_| {
                            let name = require_sql_identifier(
                                raw_name.clone(),
                                "database.rollbackToSavepoint",
                                "name",
                            )?;
                            state
                                .handle
                                .request(|resp| DbRequest::RollbackToSavepoint { name, resp })
                                .map_err(RuntimeError::Message)?;
                            Ok(Value::Unit)
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    {
        let state = state.clone();
        fields.insert(
            "runMigrationSql".to_string(),
            builtin("database.runMigrationSql", 1, move |mut args, _| {
                let statements_value = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |_| {
                            let list = expect_list(
                                statements_value.clone(),
                                "database.runMigrationSql",
                            )?;
                            let mut statements = Vec::with_capacity(list.len());
                            for item in list.iter() {
                                statements
                                    .push(expect_text(item.clone(), "database.runMigrationSql")?);
                            }
                            state
                                .handle
                                .request(|resp| DbRequest::RunMigrationSql {
                                    statements: statements.clone(),
                                    resp,
                                })
                                .map_err(RuntimeError::Message)?;
                            Ok(Value::Unit)
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    fields.insert("ins".to_string(), builtin_constructor("Insert", 1));
    fields.insert("upd".to_string(), builtin_constructor("Update", 2));
    fields.insert("del".to_string(), builtin_constructor("Delete", 1));
    fields.insert("pool".to_string(), build_database_pool_record());
    Value::Record(Arc::new(fields))
}
