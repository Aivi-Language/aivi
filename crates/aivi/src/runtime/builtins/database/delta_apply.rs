use super::util::{builtin_with_db_patch_meta, make_none, make_some};
use crate::runtime::values::DbPatchRuntimeMeta;

pub(super) fn build_patch_compiled_builtin() -> Value {
    builtin("__db_patch_compiled", 3, |mut args, _| {
        let fallback = args.pop().unwrap();
        let captures_value = args.pop().unwrap();
        let plan_json_value = args.pop().unwrap();
        let plan_json = expect_text(plan_json_value, "__db_patch_compiled")?;
        let captures = expect_list(captures_value, "__db_patch_compiled")?;
        Ok(builtin_with_db_patch_meta(
            "__db_patch_compiled.value",
            1,
            Some(std::sync::Arc::new(DbPatchRuntimeMeta {
                plan_json: Some(plan_json),
                captures: captures.iter().cloned().collect(),
                error: None,
            })),
            move |mut apply_args, runtime| {
                let arg = apply_args.pop().unwrap();
                runtime.apply(fallback.clone(), arg)
            },
        ))
    })
}

fn expect_runtime_bool(value: Value, ctx: &str) -> Result<bool, RuntimeError> {
    match value {
        Value::Bool(value) => Ok(value),
        other => Err(RuntimeError::Message(format!(
            "{ctx} expects Bool, got {}",
            crate::runtime::format_value(&other)
        ))),
    }
}

pub(super) fn build_patch_error_builtin() -> Value {
    builtin("__db_patch_error", 2, |mut args, _| {
        let fallback = args.pop().unwrap();
        let message_value = args.pop().unwrap();
        let message = expect_text(message_value, "__db_patch_error")?;
        Ok(builtin_with_db_patch_meta(
            "__db_patch_error.value",
            1,
            Some(std::sync::Arc::new(DbPatchRuntimeMeta {
                plan_json: None,
                captures: Vec::new(),
                error: Some(message),
            })),
            move |mut apply_args, runtime| {
                let arg = apply_args.pop().unwrap();
                runtime.apply(fallback.clone(), arg)
            },
        ))
    })
}

fn require_compiled_patch(
    patch: &Value,
    ctx: &str,
) -> Result<(crate::hir::CompiledDbPatchPlan, Vec<Value>), RuntimeError> {
    let Value::Builtin(builtin) = patch else {
        return Err(RuntimeError::Message(format!(
            "{ctx} requires a patch block in the lowered SQL-backed subset"
        )));
    };
    let Some(meta) = builtin.imp.db_patch_meta.as_ref() else {
        return Err(RuntimeError::Message(format!(
            "{ctx} requires a patch block in the lowered SQL-backed subset"
        )));
    };
    if let Some(error) = &meta.error {
        return Err(RuntimeError::Message(format!("{ctx}: {error}")));
    }
    let plan_json = meta.plan_json.as_ref().ok_or_else(|| {
        RuntimeError::Message(format!("{ctx} is missing compiled selector patch metadata"))
    })?;
    let plan: crate::hir::CompiledDbPatchPlan = serde_json::from_str(plan_json).map_err(|err| {
        RuntimeError::Message(format!(
            "{ctx} could not decode compiled selector patch plan: {err}"
        ))
    })?;
    Ok((plan, meta.captures.clone()))
}

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
    Ok(list_value(load_rows_from_storage(connection, &name)?))
}

fn apply_delta_on_connection(
    table: Value,
    delta: Value,
    connection: &aivi_database::DbConnection,
    runtime: &mut Runtime,
) -> Result<Value, RuntimeError> {
    let (name, columns, _rows) = table_parts(table, "database.applyDelta")?;
    let columns_json = encode_json(&columns)?;
    let initial_storage_columns_json = serde_json::to_string(
        &build_query_storage_table(name.clone(), columns.clone(), &[])?.columns,
    )
    .map_err(|err| {
        RuntimeError::Message(format!(
            "database.applyDelta could not encode storage columns for '{name}': {err}"
        ))
    })?;

    for _attempt in 0..3 {
        let entry = connection
            .load_table(name.clone())
            .map_err(RuntimeError::Message)?;

        if entry.is_none() {
            connection
                .migrate_table(
                    name.clone(),
                    columns_json.clone(),
                    initial_storage_columns_json.clone(),
                )
                .map_err(RuntimeError::Message)?;
            continue;
        }

        let (rev, _stored_cols, _stored_storage_cols) = entry.unwrap();
        let current_rows = load_rows_from_storage(connection, &name)?;
        let new_rows = apply_delta_rows(&current_rows, delta.clone(), runtime)?;
        let storage_table = build_query_storage_table(name.clone(), columns.clone(), &new_rows)?;
        let storage_columns_json = serde_json::to_string(&storage_table.columns).map_err(|err| {
            RuntimeError::Message(format!(
                "database.applyDelta could not encode storage columns for '{name}': {err}"
            ))
        })?;
        let saved = connection.replace_table_storage(
            rev,
            name.clone(),
            columns_json.clone(),
            storage_columns_json,
            storage_table,
        );
        match saved {
            Ok(_new_rev) => {
                return Ok(make_table(name.clone(), columns.clone(), new_rows));
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
        let storage_columns_json = serde_json::to_string(
            &build_query_storage_table(name.clone(), columns.clone(), &[])?.columns,
        )
        .map_err(|err| {
            RuntimeError::Message(format!(
                "database.runMigrations could not encode storage columns for '{name}': {err}"
            ))
        })?;
        connection
            .migrate_table(name.clone(), columns_json, storage_columns_json)
            .map_err(RuntimeError::Message)?;
    }
    Ok(Value::Unit)
}

enum SelectorMetaState {
    Missing,
    Error(String),
    Compiled {
        plan: crate::hir::CompiledDbSelectionPlan,
        captures: Vec<Value>,
    },
}

fn selection_parts_with_meta(
    selection: Value,
    ctx: &str,
) -> Result<(Value, Value, SelectorMetaState), RuntimeError> {
    let fields = expect_record(selection, ctx)?;
    let table = fields
        .get("table")
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects DbSelection.table")))?
        .clone();
    let pred = fields
        .get("pred")
        .ok_or_else(|| RuntimeError::Message(format!("{ctx} expects DbSelection.pred")))?
        .clone();
    let meta = match fields.get(crate::hir::DB_SELECTION_META_FIELD) {
        None => SelectorMetaState::Missing,
        Some(meta_value) => {
            let meta_fields = expect_record(meta_value.clone(), ctx)?;
            if let Some(error) = meta_fields.get("error") {
                SelectorMetaState::Error(expect_text(error.clone(), ctx)?)
            } else {
                let plan_json = expect_text(
                    meta_fields
                        .get("planJson")
                        .ok_or_else(|| {
                            RuntimeError::Message(format!("{ctx} selection metadata missing planJson"))
                        })?
                        .clone(),
                    ctx,
                )?;
                let captures = expect_list(
                    meta_fields
                        .get("captures")
                        .ok_or_else(|| {
                            RuntimeError::Message(format!("{ctx} selection metadata missing captures"))
                        })?
                        .clone(),
                    ctx,
                )?;
                let plan: crate::hir::CompiledDbSelectionPlan = serde_json::from_str(&plan_json)
                    .map_err(|err| {
                        RuntimeError::Message(format!(
                            "{ctx} could not decode compiled selector plan: {err}"
                        ))
                    })?;
                SelectorMetaState::Compiled {
                    plan,
                    captures: captures.iter().cloned().collect(),
                }
            }
        }
    };
    Ok((table, pred, meta))
}

fn require_selector_meta(
    selection: Value,
    ctx: &str,
) -> Result<(Value, crate::hir::CompiledDbSelectionPlan, Vec<Value>), RuntimeError> {
    let (table, _pred, meta) = selection_parts_with_meta(selection, ctx)?;
    match meta {
        SelectorMetaState::Compiled { plan, captures } => Ok((table, plan, captures)),
        SelectorMetaState::Error(message) => Err(RuntimeError::Message(format!("{ctx}: {message}"))),
        SelectorMetaState::Missing => Err(RuntimeError::Message(format!(
            "{ctx} requires a selector predicate in the lowered SQL-backed subset"
        ))),
    }
}

fn ensure_persisted_table_entry(
    table: &Value,
    connection: &aivi_database::DbConnection,
    ctx: &str,
) -> Result<(String, Value, i64, String, String), RuntimeError> {
    let (name, columns, _rows) = table_parts(table.clone(), ctx)?;
    connection.ensure_schema().map_err(RuntimeError::Message)?;
    let columns_json = encode_json(&columns)?;
    let storage_columns_json = serde_json::to_string(
        &build_query_storage_table(name.clone(), columns.clone(), &[])?.columns,
    )
    .map_err(|err| {
        RuntimeError::Message(format!(
            "{ctx} could not encode storage columns for '{name}': {err}"
        ))
    })?;
    if connection
        .load_table(name.clone())
        .map_err(RuntimeError::Message)?
        .is_none()
    {
        connection
            .migrate_table(name.clone(), columns_json.clone(), storage_columns_json)
            .map_err(RuntimeError::Message)?;
    }
    let Some((rev, stored_columns_json, stored_storage_columns_json)) = connection
        .load_table(name.clone())
        .map_err(RuntimeError::Message)?
    else {
        return Err(RuntimeError::Message(format!(
            "{ctx} could not load persisted metadata for '{name}'"
        )));
    };
    Ok((name, columns, rev, stored_columns_json, stored_storage_columns_json))
}

fn load_runtime_schema_from_storage(
    name: &str,
    storage_columns_json: &str,
    ctx: &str,
) -> Result<RuntimeTableSchema, RuntimeError> {
    validate_identifier(name, ctx)?;
    let storage_columns: Vec<aivi_database::QueryColumn> = serde_json::from_str(storage_columns_json)
        .map_err(|err| {
            RuntimeError::Message(format!(
                "{ctx} could not decode persisted storage columns for '{name}': {err}"
            ))
        })?;
    Ok(RuntimeTableSchema {
        name: name.to_string(),
        storage_name: aivi_database::query_storage_name(name),
        columns: storage_columns
            .into_iter()
            .map(|column| RuntimeColumn {
                name: column.name,
                kind: column.column_type,
                not_null: column.not_null,
            })
            .collect(),
    })
}

fn selector_sql_context(
    schema: &RuntimeTableSchema,
) -> (
    std::collections::HashMap<String, RuntimeTableSchema>,
    std::collections::HashMap<String, String>,
) {
    (
        std::collections::HashMap::from([("q0".to_string(), schema.clone())]),
        std::collections::HashMap::from([("q0".to_string(), "t0".to_string())]),
    )
}

fn render_selector_where_sql(
    schema: &RuntimeTableSchema,
    plan: &crate::hir::CompiledDbSelectionPlan,
    captures: &[Value],
    ctx: &str,
) -> Result<String, RuntimeError> {
    let (schemas, aliases) = selector_sql_context(schema);
    render_scalar_expr(&plan.predicate, &schemas, &aliases, captures).map_err(|err| {
        RuntimeError::Message(format!(
            "{ctx} could not render selector predicate for '{}': {err}",
            schema.name
        ))
    })
}

fn query_storage_row_json(
    connection: &aivi_database::DbConnection,
    sql: String,
    ctx: &str,
) -> Result<Vec<Value>, RuntimeError> {
    let rows = connection.query_sql(sql).map_err(RuntimeError::Message)?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let json = match row.as_slice() {
            [QueryCell::Text(json)] => json.as_str(),
            other => {
                return Err(RuntimeError::Message(format!(
                    "{ctx} returned unexpected storage row shape: {other:?}"
                )))
            }
        };
        out.push(decode_json(json)?);
    }
    Ok(out)
}

fn load_table_snapshot(
    name: String,
    columns: Value,
    connection: &aivi_database::DbConnection,
) -> Result<Value, RuntimeError> {
    let rows = load_rows_from_storage(connection, &name)?;
    Ok(make_table(
        name,
        columns,
        rows,
    ))
}

fn storage_has_rows(
    connection: &aivi_database::DbConnection,
    storage_name: &str,
) -> Result<bool, RuntimeError> {
    let rows = connection
        .query_sql(format!("SELECT __aivi_ord FROM {storage_name} LIMIT 1"))
        .map_err(RuntimeError::Message)?;
    Ok(!rows.is_empty())
}

fn next_storage_ordinal(
    connection: &aivi_database::DbConnection,
    storage_name: &str,
) -> Result<i64, RuntimeError> {
    let rows = connection
        .query_sql(format!(
            "SELECT COALESCE(MAX(__aivi_ord), -1) + 1 FROM {storage_name}"
        ))
        .map_err(RuntimeError::Message)?;
    match rows.as_slice() {
        [row] => match row.as_slice() {
            [QueryCell::Int(value)] => Ok(*value),
            other => Err(RuntimeError::Message(format!(
                "database selector next ordinal returned unexpected row shape: {other:?}"
            ))),
        },
        other => Err(RuntimeError::Message(format!(
            "database selector next ordinal returned unexpected result set: {other:?}"
        ))),
    }
}

fn build_insert_sql_for_schema(
    schema: &RuntimeTableSchema,
    row: &Value,
    ordinal: i64,
    ctx: &str,
) -> Result<String, RuntimeError> {
    let fields = expect_record(row.clone(), ctx)?;
    let mut column_names = vec!["__aivi_ord".to_string(), "__aivi_row_json".to_string()];
    let mut values = vec![
        ordinal.to_string(),
        render_required_runtime_scalar_literal(&Value::Text(encode_json(row)?))?,
    ];
    for column in &schema.columns {
        column_names.push(column.name.clone());
        let cell = match fields.get(&column.name) {
            Some(value) => runtime_value_to_query_cell(value, column.kind, column.not_null)?,
            None if !column.not_null => QueryCell::Null,
            None => {
                return Err(RuntimeError::Message(format!(
                    "{ctx} insert row is missing field '{}'",
                    column.name
                )))
            }
        };
        values.push(match cell {
            QueryCell::Null => "NULL".to_string(),
            QueryCell::Int(value) => value.to_string(),
            QueryCell::Float(value) => value.to_string(),
            QueryCell::Bool(value) => {
                if value { "TRUE".to_string() } else { "FALSE".to_string() }
            }
            QueryCell::Text(value) => format!("'{}'", value.replace('\'', "''")),
        });
    }
    Ok(format!(
        "INSERT INTO {} ({}) VALUES ({})",
        schema.storage_name,
        column_names.join(", "),
        values.join(", ")
    ))
}

fn build_update_sql_for_schema(
    schema: &RuntimeTableSchema,
    row: &Value,
    ordinal: i64,
    ctx: &str,
) -> Result<String, RuntimeError> {
    let fields = expect_record(row.clone(), ctx)?;
    let mut assignments = vec![format!(
        "__aivi_row_json = {}",
        render_required_runtime_scalar_literal(&Value::Text(encode_json(row)?))?
    )];
    for column in &schema.columns {
        let cell = match fields.get(&column.name) {
            Some(value) => runtime_value_to_query_cell(value, column.kind, column.not_null)?,
            None if !column.not_null => QueryCell::Null,
            None => {
                return Err(RuntimeError::Message(format!(
                    "{ctx} updated row is missing field '{}'",
                    column.name
                )))
            }
        };
        assignments.push(format!(
            "{} = {}",
            column.name,
            match cell {
                QueryCell::Null => "NULL".to_string(),
                QueryCell::Int(value) => value.to_string(),
                QueryCell::Float(value) => value.to_string(),
                QueryCell::Bool(value) => {
                    if value { "TRUE".to_string() } else { "FALSE".to_string() }
                }
                QueryCell::Text(value) => format!("'{}'", value.replace('\'', "''")),
            }
        ));
    }
    Ok(format!(
        "UPDATE {} SET {} WHERE __aivi_ord = {}",
        schema.storage_name,
        assignments.join(", "),
        ordinal
    ))
}

fn projectable_schema_for_row(
    name: String,
    columns: Value,
    row: &Value,
    ctx: &str,
) -> Result<QueryTable, RuntimeError> {
    build_query_storage_table(name, columns, std::slice::from_ref(row)).map_err(|err| {
        RuntimeError::Message(format!("{ctx} could not project selector upsert row: {err}"))
    })
}

fn selector_rows_in_memory(
    selection: Value,
    ctx: &str,
    runtime: &mut Runtime,
) -> Result<Vec<Value>, RuntimeError> {
    let (table, pred, _meta) = selection_parts_with_meta(selection, ctx)?;
    let (_name, _columns, rows) = table_parts(table, ctx)?;
    let mut matched = Vec::new();
    for row in rows.iter() {
        let keep = runtime.apply(pred.clone(), row.clone())?;
        if expect_runtime_bool(keep, ctx)? {
            matched.push(row.clone());
        }
    }
    Ok(matched)
}

fn selector_rows_on_connection(
    selection: Value,
    connection: &aivi_database::DbConnection,
    ctx: &str,
) -> Result<Vec<Value>, RuntimeError> {
    let (table, plan, captures) = require_selector_meta(selection, ctx)?;
    let (name, _columns, _rev, _columns_json, storage_columns_json) =
        ensure_persisted_table_entry(&table, connection, ctx)?;
    let schema = load_runtime_schema_from_storage(&name, &storage_columns_json, ctx)?;
    if schema.columns.is_empty() && !storage_has_rows(connection, &schema.storage_name)? {
        return Ok(Vec::new());
    }
    let where_sql = render_selector_where_sql(&schema, &plan, &captures, ctx)?;
    query_storage_row_json(
        connection,
        format!(
            "SELECT t0.__aivi_row_json FROM {} AS t0 WHERE {} ORDER BY t0.__aivi_ord ASC",
            schema.storage_name, where_sql
        ),
        ctx,
    )
}

fn selector_first_on_connection(
    selection: Value,
    connection: &aivi_database::DbConnection,
    ctx: &str,
) -> Result<Value, RuntimeError> {
    let rows = selector_rows_on_connection(selection, connection, ctx)?;
    Ok(rows
        .into_iter()
        .next()
        .map(make_some)
        .unwrap_or_else(make_none))
}

fn selector_delete_on_connection(
    selection: Value,
    connection: &aivi_database::DbConnection,
    ctx: &str,
) -> Result<Value, RuntimeError> {
    let (table, plan, captures) = require_selector_meta(selection, ctx)?;
    let (name, columns, _rev, _columns_json, storage_columns_json) =
        ensure_persisted_table_entry(&table, connection, ctx)?;
    let schema = load_runtime_schema_from_storage(&name, &storage_columns_json, ctx)?;
    if schema.columns.is_empty() && !storage_has_rows(connection, &schema.storage_name)? {
        return load_table_snapshot(name, columns, connection);
    }
    let where_sql = render_selector_where_sql(&schema, &plan, &captures, ctx)?;
    connection
        .execute_sql(format!(
            "DELETE FROM {} WHERE __aivi_ord IN (SELECT t0.__aivi_ord FROM {} AS t0 WHERE {})",
            schema.storage_name,
            schema.storage_name,
            where_sql
        ))
        .map_err(RuntimeError::Message)?;
    load_table_snapshot(name, columns, connection)
}

fn selector_update_on_connection(
    selection: Value,
    patch: Value,
    connection: &aivi_database::DbConnection,
    runtime: &mut Runtime,
    ctx: &str,
) -> Result<Value, RuntimeError> {
    let (table, plan, captures) = require_selector_meta(selection, ctx)?;
    let _compiled_patch = require_compiled_patch(&patch, ctx)?;
    let (name, columns, _rev, _columns_json, storage_columns_json) =
        ensure_persisted_table_entry(&table, connection, ctx)?;
    let schema = load_runtime_schema_from_storage(&name, &storage_columns_json, ctx)?;
    if schema.columns.is_empty() && !storage_has_rows(connection, &schema.storage_name)? {
        return load_table_snapshot(name, columns, connection);
    }
    let where_sql = render_selector_where_sql(&schema, &plan, &captures, ctx)?;
    let matched_rows = connection
        .query_sql(format!(
            "SELECT t0.__aivi_ord, t0.__aivi_row_json FROM {} AS t0 WHERE {} ORDER BY t0.__aivi_ord ASC",
            schema.storage_name, where_sql
        ))
        .map_err(RuntimeError::Message)?;
    for matched in matched_rows {
        let (ordinal, row_json) = match matched.as_slice() {
            [QueryCell::Int(ordinal), QueryCell::Text(row_json)] => (*ordinal, row_json.as_str()),
            other => {
                return Err(RuntimeError::Message(format!(
                    "{ctx} matched row query returned unexpected row shape: {other:?}"
                )))
            }
        };
        let updated = runtime.apply(patch.clone(), decode_json(row_json)?)?;
        connection
            .execute_sql(build_update_sql_for_schema(&schema, &updated, ordinal, ctx)?)
            .map_err(RuntimeError::Message)?;
    }
    load_table_snapshot(name, columns, connection)
}

fn selector_upsert_on_connection(
    selection: Value,
    value: Value,
    patch: Value,
    connection: &aivi_database::DbConnection,
    runtime: &mut Runtime,
    ctx: &str,
) -> Result<Value, RuntimeError> {
    let (table, plan, captures) = require_selector_meta(selection, ctx)?;
    let _compiled_patch = require_compiled_patch(&patch, ctx)?;
    let (name, columns, rev, columns_json, storage_columns_json) =
        ensure_persisted_table_entry(&table, connection, ctx)?;
    let schema = load_runtime_schema_from_storage(&name, &storage_columns_json, ctx)?;
    let storage_has_rows = storage_has_rows(connection, &schema.storage_name)?;
    if !storage_has_rows && schema.columns.is_empty() {
        let single_row_table = projectable_schema_for_row(name.clone(), columns.clone(), &value, ctx)?;
        let next_storage_columns_json =
            serde_json::to_string(&single_row_table.columns).map_err(|err| {
                RuntimeError::Message(format!(
                    "{ctx} could not encode inferred storage columns for '{name}': {err}"
                ))
            })?;
        connection
            .replace_table_storage(
                rev,
                name.clone(),
                columns_json,
                next_storage_columns_json,
                single_row_table,
            )
            .map_err(RuntimeError::Message)?;
        return load_table_snapshot(name, columns, connection);
    }

    if schema.columns.is_empty() {
        return Err(RuntimeError::Message(format!(
            "{ctx} cannot upsert into a selector-backed table without projected columns"
        )));
    }

    let where_sql = render_selector_where_sql(&schema, &plan, &captures, ctx)?;
    let matches = connection
        .query_sql(format!(
            "SELECT 1 FROM {} AS t0 WHERE {} LIMIT 1",
            schema.storage_name, where_sql
        ))
        .map_err(RuntimeError::Message)?;
    if !matches.is_empty() {
        return selector_update_on_connection(
            Value::Record(std::sync::Arc::new(std::collections::HashMap::from([
                ("table".to_string(), table.clone()),
                (
                    crate::hir::DB_SELECTION_META_FIELD.to_string(),
                    Value::Record(std::sync::Arc::new(std::collections::HashMap::from([
                        (
                            "planJson".to_string(),
                            Value::Text(
                                serde_json::to_string(&plan).map_err(|err| {
                                    RuntimeError::Message(format!(
                                        "{ctx} could not re-encode selector plan: {err}"
                                    ))
                                })?,
                            ),
                        ),
                        ("captures".to_string(), list_value(captures.to_vec())),
                    ]))),
                ),
                ("pred".to_string(), Value::Unit),
            ]))),
            patch,
            connection,
            runtime,
            ctx,
        );
    }

    let desired_schema = projectable_schema_for_row(name.clone(), columns.clone(), &value, ctx)?;
    let current_columns: std::collections::HashSet<_> =
        schema.columns.iter().map(|column| column.name.clone()).collect();
    let desired_columns: std::collections::HashSet<_> =
        desired_schema.columns.iter().map(|column| column.name.clone()).collect();
    if !desired_columns.is_subset(&current_columns) {
        return Err(RuntimeError::Message(format!(
            "{ctx} cannot insert selector upsert row because it introduces new projected columns"
        )));
    }
    let ordinal = next_storage_ordinal(connection, &schema.storage_name)?;
    let insert_sql = build_insert_sql_for_schema(&schema, &value, ordinal, ctx)?;
    connection
        .execute_sql(insert_sql)
        .map_err(RuntimeError::Message)?;
    load_table_snapshot(name, columns, connection)
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
            "rows".to_string(),
            builtin("database.rows", 1, move |mut args, _| {
                let selection = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |runtime| match default_db_connection(&state)? {
                            Some(connection) => Ok(list_value(selector_rows_on_connection(
                                selection.clone(),
                                &connection,
                                "database.rows",
                            )?)),
                            None => Ok(list_value(selector_rows_in_memory(
                                selection.clone(),
                                "database.rows",
                                runtime,
                            )?)),
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    fields.insert(
        "rowsOn".to_string(),
        builtin("database.rowsOn", 2, move |mut args, _| {
            let selection = args.pop().unwrap();
            let connection = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let connection = expect_db_connection(connection.clone(), "database.rowsOn")?;
                    Ok(list_value(selector_rows_on_connection(
                        selection.clone(),
                        &connection,
                        "database.rowsOn",
                    )?))
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    {
        let state = state.clone();
        fields.insert(
            "first".to_string(),
            builtin("database.first", 1, move |mut args, _| {
                let selection = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |runtime| match default_db_connection(&state)? {
                            Some(connection) => selector_first_on_connection(
                                selection.clone(),
                                &connection,
                                "database.first",
                            ),
                            None => Ok(selector_rows_in_memory(
                                selection.clone(),
                                "database.first",
                                runtime,
                            )?
                            .into_iter()
                            .next()
                            .map(make_some)
                            .unwrap_or_else(make_none)),
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    fields.insert(
        "firstOn".to_string(),
        builtin("database.firstOn", 2, move |mut args, _| {
            let selection = args.pop().unwrap();
            let connection = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let connection = expect_db_connection(connection.clone(), "database.firstOn")?;
                    selector_first_on_connection(selection.clone(), &connection, "database.firstOn")
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    {
        let state = state.clone();
        fields.insert(
            "delete".to_string(),
            builtin("database.delete", 1, move |mut args, _| {
                let selection = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |runtime| match default_db_connection(&state)? {
                            Some(connection) => selector_delete_on_connection(
                                selection.clone(),
                                &connection,
                                "database.delete",
                            ),
                            None => {
                                let (table, pred, _meta) =
                                    selection_parts_with_meta(selection.clone(), "database.delete")?;
                                apply_delta(
                                    table,
                                    Value::Constructor {
                                        name: "Delete".to_string(),
                                        args: vec![pred],
                                    },
                                    runtime,
                                )
                            }
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    fields.insert(
        "deleteOn".to_string(),
        builtin("database.deleteOn", 2, move |mut args, _| {
            let selection = args.pop().unwrap();
            let connection = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let connection = expect_db_connection(connection.clone(), "database.deleteOn")?;
                    selector_delete_on_connection(selection.clone(), &connection, "database.deleteOn")
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    {
        let state = state.clone();
        fields.insert(
            "update".to_string(),
            builtin("database.update", 2, move |mut args, _| {
                let patch = args.pop().unwrap();
                let selection = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |runtime| match default_db_connection(&state)? {
                            Some(connection) => selector_update_on_connection(
                                selection.clone(),
                                patch.clone(),
                                &connection,
                                runtime,
                                "database.update",
                            ),
                            None => {
                                let (table, pred, _meta) =
                                    selection_parts_with_meta(selection.clone(), "database.update")?;
                                apply_delta(
                                    table,
                                    Value::Constructor {
                                        name: "Update".to_string(),
                                        args: vec![pred, patch.clone()],
                                    },
                                    runtime,
                                )
                            }
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    fields.insert(
        "updateOn".to_string(),
        builtin("database.updateOn", 3, move |mut args, _| {
            let patch = args.pop().unwrap();
            let selection = args.pop().unwrap();
            let connection = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let connection = expect_db_connection(connection.clone(), "database.updateOn")?;
                    selector_update_on_connection(
                        selection.clone(),
                        patch.clone(),
                        &connection,
                        runtime,
                        "database.updateOn",
                    )
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    {
        let state = state.clone();
        fields.insert(
            "upsert".to_string(),
            builtin("database.upsert", 3, move |mut args, _| {
                let patch = args.pop().unwrap();
                let value = args.pop().unwrap();
                let selection = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |runtime| match default_db_connection(&state)? {
                            Some(connection) => selector_upsert_on_connection(
                                selection.clone(),
                                value.clone(),
                                patch.clone(),
                                &connection,
                                runtime,
                                "database.upsert",
                            ),
                            None => {
                                let (table, pred, _meta) =
                                    selection_parts_with_meta(selection.clone(), "database.upsert")?;
                                apply_delta(
                                    table,
                                    Value::Constructor {
                                        name: "Upsert".to_string(),
                                        args: vec![pred, value.clone(), patch.clone()],
                                    },
                                    runtime,
                                )
                            }
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    fields.insert(
        "upsertOn".to_string(),
        builtin("database.upsertOn", 4, move |mut args, _| {
            let patch = args.pop().unwrap();
            let value = args.pop().unwrap();
            let selection = args.pop().unwrap();
            let connection = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let connection = expect_db_connection(connection.clone(), "database.upsertOn")?;
                    selector_upsert_on_connection(
                        selection.clone(),
                        value.clone(),
                        patch.clone(),
                        &connection,
                        runtime,
                        "database.upsertOn",
                    )
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
