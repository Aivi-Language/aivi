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
            "Upsert" => {
                if args.len() != 3 {
                    return Err(RuntimeError::Message(
                        "database.applyDelta expects Upsert predicate, value, and patch"
                            .to_string(),
                    ));
                }
                let pred = args[0].clone();
                let value = args[1].clone();
                let patch = args[2].clone();
                let mut matched = false;
                for row in rows.iter() {
                    let keep = runtime.apply(pred.clone(), row.clone())?;
                    let keep = match keep {
                        Value::Bool(value) => value,
                        other => {
                            return Err(RuntimeError::Message(format!(
                                "database.applyDelta Upsert predicate expects Bool, got {}",
                                crate::runtime::format_value(&other)
                            )))
                        }
                    };
                    if keep {
                        matched = true;
                        let updated = runtime.apply(patch.clone(), row.clone())?;
                        out.push(updated);
                    } else {
                        out.push(row.clone());
                    }
                }
                if !matched {
                    out.push(value);
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

fn parse_db_config(config: Value, ctx: &str) -> Result<(Driver, String), RuntimeError> {
    let config_fields = expect_record(config, ctx)?;
    let driver = config_fields
        .get("driver")
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects DbConfig.driver")))?
        .clone();
    let url = config_fields
        .get("url")
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects DbConfig.url")))?
        .clone();
    let driver = parse_driver(driver)?;
    let url = expect_text(url, ctx)?;
    Ok((driver, url))
}

fn expect_db_connection(value: Value, ctx: &str) -> Result<aivi_database::DbConnection, RuntimeError> {
    match value {
        Value::DbConnection(connection) => Ok(connection),
        other => Err(RuntimeError::Message(format!(
            "{ctx} expects DbConnection, got {}",
            crate::runtime::format_value(&other)
        ))),
    }
}

fn default_db_connection(
    state: &DatabaseState,
) -> Result<Option<aivi_database::DbConnection>, RuntimeError> {
    state.default_connection().map_err(RuntimeError::Message)
}

fn require_default_db_connection(
    state: &DatabaseState,
) -> Result<aivi_database::DbConnection, RuntimeError> {
    default_db_connection(state)?
        .ok_or_else(|| RuntimeError::Message("database backend is not configured".to_string()))
}

fn load_rows_from_connection(
    table: Value,
    connection: &aivi_database::DbConnection,
) -> Result<Value, RuntimeError> {
    let (name, _columns, _rows) = table_parts(table, "database.load")?;
    let entry = connection
        .load_table(name)
        .map_err(RuntimeError::Message)?;
    let rows_json = match entry {
        Some((_rev, _cols, rows_json)) => rows_json,
        None => aivi_database::EMPTY_ROWS_JSON.to_string(),
    };
    let rows_value = decode_json(&rows_json)?;
    let Value::List(rows) = rows_value else {
        return Err(RuntimeError::Message(
            "database: invalid persisted rows (expected List)".to_string(),
        ));
    };
    Ok(Value::List(rows))
}

fn apply_delta_on_connection(
    table: Value,
    delta: Value,
    connection: &aivi_database::DbConnection,
    runtime: &mut Runtime,
) -> Result<Value, RuntimeError> {
    let (name, columns, _rows) = table_parts(table, "database.applyDelta")?;
    let columns_json = encode_json(&columns)?;

    for _attempt in 0..3 {
        let entry = connection
            .load_table(name.clone())
            .map_err(RuntimeError::Message)?;

        if entry.is_none() {
            connection
                .migrate_table(name.clone(), columns_json.clone())
                .map_err(RuntimeError::Message)?;
            continue;
        }

        let (rev, _stored_cols, rows_json) = entry.unwrap();
        let rows_value = decode_json(&rows_json)?;
        let Value::List(rows_list) = rows_value else {
            return Err(RuntimeError::Message(
                "database: invalid persisted rows (expected List)".to_string(),
            ));
        };
        let current_rows: Vec<Value> = rows_list.iter().cloned().collect();

        let new_rows = apply_delta_rows(&current_rows, delta.clone(), runtime)?;
        let rows_json = encode_json(&list_value(new_rows.clone()))?;

        let saved = connection.compare_and_swap_rows(
            name.clone(),
            rev,
            columns_json.clone(),
            rows_json,
        );
        match saved {
            Ok(_new_rev) => return Ok(make_table(name.clone(), columns.clone(), new_rows)),
            Err(err) => {
                if err.contains("retry") {
                    continue;
                }
                return Err(RuntimeError::Message(err));
            }
        }
    }

    Err(RuntimeError::Message(
        "database.applyDelta failed due to concurrent writes; retry".to_string(),
    ))
}

fn run_migrations_on_connection(
    tables: Value,
    connection: &aivi_database::DbConnection,
) -> Result<Value, RuntimeError> {
    let tables = expect_list(tables, "database.runMigrations")?;
    connection.ensure_schema().map_err(RuntimeError::Message)?;

    for table in tables.iter() {
        let (name, columns, _rows) = table_parts(table.clone(), "database.runMigrations")?;
        let columns_json = encode_json(&columns)?;
        connection
            .migrate_table(name, columns_json)
            .map_err(RuntimeError::Message)?;
    }
    Ok(Value::Unit)
}

fn configure_sqlite_on_connection(
    config: Value,
    connection: &aivi_database::DbConnection,
) -> Result<Value, RuntimeError> {
    let config_fields = expect_record(config, "database.configureSqlite")?;
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
    let busy_timeout_ms = expect_int(busy_timeout, "database.configureSqlite")?;
    connection
        .sqlite_configure(wal, busy_timeout_ms)
        .map_err(RuntimeError::Message)?;
    Ok(Value::Unit)
}

fn run_migration_sql_on_connection(
    statements_value: Value,
    connection: &aivi_database::DbConnection,
) -> Result<Value, RuntimeError> {
    let list = expect_list(statements_value, "database.runMigrationSql")?;
    let mut statements = Vec::with_capacity(list.len());
    for item in list.iter() {
        statements.push(expect_text(item.clone(), "database.runMigrationSql")?);
    }
    connection
        .run_migration_sql(statements)
        .map_err(RuntimeError::Message)?;
    Ok(Value::Unit)
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
                            let (driver, url) =
                                parse_db_config(config.clone(), "database.configure")?;
                            state.configure(driver, url).map_err(RuntimeError::Message)?;
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
            "connect".to_string(),
            builtin("database.connect", 1, move |mut args, _| {
                let config = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |_| {
                            let (driver, url) =
                                parse_db_config(config.clone(), "database.connect")?;
                            let connection = state.connect(driver, url).map_err(RuntimeError::Message)?;
                            Ok(Value::DbConnection(connection))
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    fields.insert(
        "close".to_string(),
        builtin("database.close", 1, move |mut args, _| {
            let connection = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let connection = expect_db_connection(connection.clone(), "database.close")?;
                    connection.close().map_err(RuntimeError::Message)?;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    {
        let state = state.clone();
        fields.insert(
            "load".to_string(),
            builtin("database.load", 1, move |mut args, _| {
                let table = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |_| match default_db_connection(&state)? {
                            Some(connection) => load_rows_from_connection(table.clone(), &connection),
                            None => {
                                let (_, _, rows) = table_parts(table.clone(), "database.load")?;
                                Ok(list_value(rows.iter().cloned().collect()))
                            }
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    fields.insert(
        "loadOn".to_string(),
        builtin("database.loadOn", 2, move |mut args, _| {
            let table = args.pop().unwrap();
            let connection = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let connection = expect_db_connection(connection.clone(), "database.loadOn")?;
                    load_rows_from_connection(table.clone(), &connection)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    {
        let state = state.clone();
        fields.insert(
            "runQuery".to_string(),
            builtin("database.runQuery", 1, move |mut args, _| {
                let query = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |runtime| {
                            let connection = require_default_db_connection(&state)?;
                            let fields = expect_record(query.clone(), "database.runQuery")?;
                            let run_fn = fields
                                .get("run")
                                .ok_or_else(|| {
                                    RuntimeError::Message(
                                        "database.runQuery expects Query with 'run' field"
                                            .to_string(),
                                    )
                                })?
                                .clone();
                            let inner_effect =
                                runtime.apply(run_fn, Value::DbConnection(connection))?;
                            runtime.run_effect_value(inner_effect)
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
                        move |runtime| match default_db_connection(&state)? {
                            Some(connection) => apply_delta_on_connection(
                                table.clone(),
                                delta.clone(),
                                &connection,
                                runtime,
                            ),
                            None => apply_delta(table.clone(), delta.clone(), runtime),
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    fields.insert(
        "applyDeltaOn".to_string(),
        builtin("database.applyDeltaOn", 3, move |mut args, _| {
            let delta = args.pop().unwrap();
            let table = args.pop().unwrap();
            let connection = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let connection =
                        expect_db_connection(connection.clone(), "database.applyDeltaOn")?;
                    apply_delta_on_connection(table.clone(), delta.clone(), &connection, runtime)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    {
        let state = state.clone();
        fields.insert(
            "runMigrations".to_string(),
            builtin("database.runMigrations", 1, move |mut args, _| {
                let tables = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |_| match default_db_connection(&state)? {
                            Some(connection) => {
                                run_migrations_on_connection(tables.clone(), &connection)
                            }
                            None => Ok(Value::Unit),
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    fields.insert(
        "runMigrationsOn".to_string(),
        builtin("database.runMigrationsOn", 2, move |mut args, _| {
            let tables = args.pop().unwrap();
            let connection = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let connection =
                        expect_db_connection(connection.clone(), "database.runMigrationsOn")?;
                    run_migrations_on_connection(tables.clone(), &connection)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

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
                            let connection = require_default_db_connection(&state)?;
                            configure_sqlite_on_connection(config.clone(), &connection)
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    fields.insert(
        "configureSqliteOn".to_string(),
        builtin("database.configureSqliteOn", 2, move |mut args, _| {
            let config = args.pop().unwrap();
            let connection = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let connection =
                        expect_db_connection(connection.clone(), "database.configureSqliteOn")?;
                    configure_sqlite_on_connection(config.clone(), &connection)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    for name in ["beginTx", "commitTx", "rollbackTx"] {
        let state = state.clone();
        fields.insert(
            name.to_string(),
            builtin(&format!("database.{name}"), 1, move |_, _| {
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |_| {
                            let connection = require_default_db_connection(&state)?;
                            match name {
                                "beginTx" => connection
                                    .begin_transaction()
                                    .map_err(RuntimeError::Message)?,
                                "commitTx" => connection
                                    .commit_transaction()
                                    .map_err(RuntimeError::Message)?,
                                _ => connection
                                    .rollback_transaction()
                                    .map_err(RuntimeError::Message)?,
                            }
                            Ok(Value::Unit)
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    for name in ["beginTxOn", "commitTxOn", "rollbackTxOn"] {
        fields.insert(
            name.to_string(),
            builtin(&format!("database.{name}"), 1, move |mut args, _| {
                let connection = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new(move |_| {
                        let connection = expect_db_connection(connection.clone(), &format!("database.{name}"))?;
                        match name {
                            "beginTxOn" => connection
                                .begin_transaction()
                                .map_err(RuntimeError::Message)?,
                            "commitTxOn" => connection
                                .commit_transaction()
                                .map_err(RuntimeError::Message)?,
                            _ => connection
                                .rollback_transaction()
                                .map_err(RuntimeError::Message)?,
                        }
                        Ok(Value::Unit)
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
                            let connection = require_default_db_connection(&state)?;
                            let name =
                                require_sql_identifier(raw_name.clone(), "database.savepoint", "name")?;
                            connection.savepoint(name).map_err(RuntimeError::Message)?;
                            Ok(Value::Unit)
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    fields.insert(
        "savepointOn".to_string(),
        builtin("database.savepointOn", 2, move |mut args, _| {
            let raw_name = args.pop().unwrap();
            let connection = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let connection =
                        expect_db_connection(connection.clone(), "database.savepointOn")?;
                    let name =
                        require_sql_identifier(raw_name.clone(), "database.savepointOn", "name")?;
                    connection.savepoint(name).map_err(RuntimeError::Message)?;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

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
                            let connection = require_default_db_connection(&state)?;
                            let name = require_sql_identifier(
                                raw_name.clone(),
                                "database.releaseSavepoint",
                                "name",
                            )?;
                            connection
                                .release_savepoint(name)
                                .map_err(RuntimeError::Message)?;
                            Ok(Value::Unit)
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    fields.insert(
        "releaseSavepointOn".to_string(),
        builtin("database.releaseSavepointOn", 2, move |mut args, _| {
            let raw_name = args.pop().unwrap();
            let connection = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let connection = expect_db_connection(
                        connection.clone(),
                        "database.releaseSavepointOn",
                    )?;
                    let name = require_sql_identifier(
                        raw_name.clone(),
                        "database.releaseSavepointOn",
                        "name",
                    )?;
                    connection
                        .release_savepoint(name)
                        .map_err(RuntimeError::Message)?;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

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
                            let connection = require_default_db_connection(&state)?;
                            let name = require_sql_identifier(
                                raw_name.clone(),
                                "database.rollbackToSavepoint",
                                "name",
                            )?;
                            connection
                                .rollback_to_savepoint(name)
                                .map_err(RuntimeError::Message)?;
                            Ok(Value::Unit)
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    fields.insert(
        "rollbackToSavepointOn".to_string(),
        builtin("database.rollbackToSavepointOn", 2, move |mut args, _| {
            let raw_name = args.pop().unwrap();
            let connection = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let connection = expect_db_connection(
                        connection.clone(),
                        "database.rollbackToSavepointOn",
                    )?;
                    let name = require_sql_identifier(
                        raw_name.clone(),
                        "database.rollbackToSavepointOn",
                        "name",
                    )?;
                    connection
                        .rollback_to_savepoint(name)
                        .map_err(RuntimeError::Message)?;
                    Ok(Value::Unit)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

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
                            let connection = require_default_db_connection(&state)?;
                            run_migration_sql_on_connection(statements_value.clone(), &connection)
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    fields.insert(
        "runMigrationSqlOn".to_string(),
        builtin("database.runMigrationSqlOn", 2, move |mut args, _| {
            let statements_value = args.pop().unwrap();
            let connection = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let connection =
                        expect_db_connection(connection.clone(), "database.runMigrationSqlOn")?;
                    run_migration_sql_on_connection(statements_value.clone(), &connection)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    fields.insert("ins".to_string(), builtin_constructor("Insert", 1));
    fields.insert("upd".to_string(), builtin_constructor("Update", 2));
    fields.insert("del".to_string(), builtin_constructor("Delete", 1));
    fields.insert("ups".to_string(), builtin_constructor("Upsert", 3));
    fields.insert("pool".to_string(), build_database_pool_record());
    Value::Record(Arc::new(fields))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn require_sql_identifier_accepts_alphanumeric_and_underscore() {
        let name = match require_sql_identifier(
            Value::Text("savepoint_1".to_string()),
            "database.savepointOn",
            "name",
        ) {
            Ok(name) => name,
            Err(_) => panic!("valid identifier should succeed"),
        };
        assert_eq!(name, "savepoint_1");
    }

    #[test]
    fn require_sql_identifier_rejects_non_identifier_text() {
        let err = require_sql_identifier(
            Value::Text("not-ok!".to_string()),
            "database.savepointOn",
            "name",
        )
        .expect_err("invalid identifier should fail");
        match err {
            RuntimeError::Message(message) => {
                assert!(message.contains("SQL identifier"));
            }
            _ => panic!("unexpected error type"),
        }
    }
}
