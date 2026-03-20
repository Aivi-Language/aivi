use super::util::{make_none, make_some};

pub(super) fn build_patch_compiled_builtin() -> Value {
    builtin("__db_patch_compiled", 3, |mut args, _| {
        let fallback = args.pop().unwrap();
        let _captures_value = args.pop().unwrap();
        let _plan_json_value = args.pop().unwrap();
        Ok(builtin("__db_patch_compiled.value", 1, move |mut apply_args, runtime| {
            let arg = apply_args.pop().unwrap();
            runtime.apply(fallback.clone(), arg)
        }))
    })
}

pub(super) fn build_patch_error_builtin() -> Value {
    builtin("__db_patch_error", 2, |mut args, _| {
        let fallback = args.pop().unwrap();
        let _message_value = args.pop().unwrap();
        Ok(builtin("__db_patch_error.value", 1, move |mut apply_args, runtime| {
            let arg = apply_args.pop().unwrap();
            runtime.apply(fallback.clone(), arg)
        }))
    })
}

fn insert_relation(source_relation: Value, value: Value) -> Result<Value, RuntimeError> {
    let (_name, _columns, rows) = relation_parts(source_relation.clone(), "database.insert")?;
    let mut new_rows = rows.iter().cloned().collect::<Vec<_>>();
    new_rows.push(value);
    relation_snapshot_with_rows(&source_relation, new_rows, "database.insert")
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
    relation: Value,
    connection: &aivi_database::DbConnection,
) -> Result<Value, RuntimeError> {
    let (name, _columns, _rows) = relation_parts(relation, "database.loadRelation")?;
    Ok(list_value(load_rows_from_storage(connection, &name)?))
}

fn insert_relation_on_connection(
    source_relation: Value,
    value: Value,
    connection: &aivi_database::DbConnection,
) -> Result<Value, RuntimeError> {
    let (name, columns, _rows) = relation_parts(source_relation.clone(), "database.insertOn")?;
    let columns_json = encode_json(&columns)?;
    let initial_storage_columns_json = serde_json::to_string(
        &build_query_storage_table(name.clone(), columns.clone(), &[])?.columns,
    )
    .map_err(|err| {
        RuntimeError::Message(format!(
            "database.insertOn could not encode storage columns for '{name}': {err}"
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
        let mut new_rows = current_rows;
        new_rows.push(value.clone());
        let storage_table = build_query_storage_table(name.clone(), columns.clone(), &new_rows)?;
        let storage_columns_json = serde_json::to_string(&storage_table.columns).map_err(|err| {
            RuntimeError::Message(format!(
                "database.insertOn could not encode storage columns for '{name}': {err}"
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
                return relation_snapshot_with_rows(&source_relation, new_rows, "database.insertOn");
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
        "database.insertOn failed due to concurrent writes; retry".to_string(),
    ))
}

fn run_migrations_on_connection(
    tables: Value,
    connection: &aivi_database::DbConnection,
) -> Result<Value, RuntimeError> {
    let tables = expect_list(tables, "database.runMigrations")?;
    connection.ensure_schema().map_err(RuntimeError::Message)?;

    for relation in tables.iter() {
        let (name, columns, _rows) = relation_parts(relation.clone(), "database.runMigrations")?;
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

struct QueryMutationContext {
    source_relation: Value,
    name: String,
    columns: Value,
    rev: i64,
    columns_json: String,
    schema: RuntimeTableSchema,
    plan: CompiledQueryPlan,
    sources: Vec<Value>,
    captures: Vec<Value>,
    target_alias: String,
}

struct DecodedCompiledQueryMeta {
    plan: CompiledQueryPlan,
    sources: Vec<Value>,
    captures: Vec<Value>,
}

struct RootRelationQueryMeta {
    source_relation: Value,
    plan: CompiledQueryPlan,
    sources: Vec<Value>,
    captures: Vec<Value>,
    target_alias: String,
}

fn decode_compiled_query_meta(
    query: &Value,
    ctx: &str,
) -> Result<Option<DecodedCompiledQueryMeta>, RuntimeError> {
    let Some((plan_json, sources, captures)) = extract_query_meta(query)? else {
        return Ok(None);
    };
    let plan: CompiledQueryPlan = serde_json::from_str(&plan_json).map_err(|err| {
        RuntimeError::Message(format!(
            "{ctx} could not decode compiled query plan: {err}"
        ))
    })?;
    Ok(Some(DecodedCompiledQueryMeta {
        plan,
        sources,
        captures,
    }))
}

fn require_root_relation_query_meta(
    query: Value,
    ctx: &str,
) -> Result<RootRelationQueryMeta, RuntimeError> {
    let Some(DecodedCompiledQueryMeta {
        plan,
        sources,
        captures,
    }) = decode_compiled_query_meta(&query, ctx)?
    else {
        return Err(RuntimeError::Message(format!(
            "{ctx} requires a lowered SQL-backed root relation query"
        )));
    };
    let source = plan.sources.first().ok_or_else(|| {
        RuntimeError::Message(format!("{ctx} query is missing a root relation source"))
    })?;
    let source_alias = source.alias.clone();
    let source_relation_name = source.relation_name.clone();
    let source_index = source.source_index;
    if !matches!(plan.aggregate, CompiledAggregate::None)
        || !plan.group_by.is_empty()
        || !plan.having.is_empty()
        || plan.grouped_projection
    {
        return Err(RuntimeError::Message(format!(
            "{ctx} requires a non-grouped root relation query"
        )));
    }
    match &plan.projection {
        CompiledProjection::Row {
            alias,
            relation_name,
            ..
        } if alias == &source_alias && relation_name == &source_relation_name => {}
        _ => {
            return Err(RuntimeError::Message(format!(
                "{ctx} requires a root relation query without selectMap or grouping"
            )))
        }
    }
    let source_relation = sources.get(source_index).cloned().ok_or_else(|| {
        RuntimeError::Message(format!(
            "{ctx} query source index {} is out of bounds",
            source_index
        ))
    })?;
    Ok(RootRelationQueryMeta {
        source_relation,
        plan,
        sources,
        captures,
        target_alias: source_alias,
    })
}

fn ensure_persisted_table_entry(
    table: &Value,
    connection: &aivi_database::DbConnection,
    ctx: &str,
) -> Result<(String, Value, i64, String, String), RuntimeError> {
    let (name, columns, _rows) = relation_parts(table.clone(), ctx)?;
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

fn relation_snapshot_with_rows(
    source_relation: &Value,
    rows: Vec<Value>,
    ctx: &str,
) -> Result<Value, RuntimeError> {
    let _ = relation_parts(source_relation.clone(), ctx)?;
    let mut fields = (*expect_record(source_relation.clone(), ctx)?).clone();
    fields.insert("rows".to_string(), list_value(rows));
    Ok(Value::Record(Arc::new(fields)))
}

fn load_relation_snapshot(
    source_relation: &Value,
    connection: &aivi_database::DbConnection,
    ctx: &str,
) -> Result<Value, RuntimeError> {
    let (name, _columns, _rows) = relation_parts(source_relation.clone(), ctx)?;
    let rows = load_rows_from_storage(connection, &name)?;
    relation_snapshot_with_rows(source_relation, rows, ctx)
}

fn prepare_query_mutation_context(
    query: Value,
    connection: &aivi_database::DbConnection,
    ctx: &str,
) -> Result<QueryMutationContext, RuntimeError> {
    let RootRelationQueryMeta {
        source_relation,
        plan,
        sources,
        captures,
        target_alias,
    } = require_root_relation_query_meta(query, ctx)?;
    let (name, columns, rev, columns_json, storage_columns_json) =
        ensure_persisted_table_entry(&source_relation, connection, ctx)?;
    let schema = load_runtime_schema_from_storage(&name, &storage_columns_json, ctx)?;
    Ok(QueryMutationContext {
        source_relation,
        name,
        columns,
        rev,
        columns_json,
        schema,
        plan,
        sources,
        captures,
        target_alias,
    })
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

fn query_target_alias_and_tail(
    context: &QueryMutationContext,
    ctx: &str,
) -> Result<(String, String), RuntimeError> {
    let schemas = build_runtime_schemas(&context.plan.sources, &context.sources)?;
    let (sql_aliases, tail) = build_query_sql_aliases_and_tail(
        &context.plan,
        &schemas,
        &context.sources,
        &context.captures,
        &HashMap::new(),
    )?;
    let sql_alias = sql_aliases
        .get(&context.target_alias)
        .cloned()
        .ok_or_else(|| {
            RuntimeError::Message(format!(
                "{ctx} missing target query alias '{}'",
                context.target_alias
            ))
        })?;
    Ok((sql_alias, tail))
}

fn query_rows_on_connection(
    query: Value,
    connection: &aivi_database::DbConnection,
    runtime: &mut Runtime,
    ctx: &str,
) -> Result<Vec<Value>, RuntimeError> {
    let inner_effect = query_run_field(query, Value::DbConnection(connection.clone()), runtime)?;
    let result = runtime.run_effect_value(inner_effect)?;
    let rows = expect_list(result, ctx)?;
    Ok(rows.iter().cloned().collect())
}

fn query_first_on_connection(
    query: Value,
    connection: &aivi_database::DbConnection,
    runtime: &mut Runtime,
    ctx: &str,
) -> Result<Value, RuntimeError> {
    let rows = query_rows_on_connection(query, connection, runtime, ctx)?;
    Ok(rows
        .into_iter()
        .next()
        .map(make_some)
        .unwrap_or_else(make_none))
}

fn query_matching_rows_on_connection(
    context: &QueryMutationContext,
    connection: &aivi_database::DbConnection,
    ctx: &str,
) -> Result<Vec<(i64, Value)>, RuntimeError> {
    if context.schema.columns.is_empty() && !storage_has_rows(connection, &context.schema.storage_name)? {
        return Ok(Vec::new());
    }
    let (sql_alias, query_tail) = query_target_alias_and_tail(context, ctx)?;
    let matched_rows = connection
        .query_sql(format!(
            "SELECT {sql_alias}.__aivi_ord, {sql_alias}.__aivi_row_json {query_tail}"
        ))
        .map_err(RuntimeError::Message)?;
    let mut out = Vec::with_capacity(matched_rows.len());
    for matched in matched_rows {
        let (ordinal, row_json) = match matched.as_slice() {
            [QueryCell::Int(ordinal), QueryCell::Text(row_json)] => (*ordinal, row_json.as_str()),
            other => {
                return Err(RuntimeError::Message(format!(
                    "{ctx} matched row query returned unexpected row shape: {other:?}"
                )))
            }
        };
        out.push((ordinal, decode_json(row_json)?));
    }
    Ok(out)
}

fn apply_query_patch_rows(
    context: &QueryMutationContext,
    matched_rows: Vec<(i64, Value)>,
    patch: Value,
    connection: &aivi_database::DbConnection,
    runtime: &mut Runtime,
    ctx: &str,
) -> Result<(), RuntimeError> {
    for (ordinal, row) in matched_rows {
        let updated = runtime.apply(patch.clone(), row)?;
        connection
            .execute_sql(build_update_sql_for_schema(&context.schema, &updated, ordinal, ctx)?)
            .map_err(RuntimeError::Message)?;
    }
    Ok(())
}

fn query_delete_on_connection(
    query: Value,
    connection: &aivi_database::DbConnection,
    ctx: &str,
) -> Result<Value, RuntimeError> {
    let context = prepare_query_mutation_context(query, connection, ctx)?;
    if context.schema.columns.is_empty() && !storage_has_rows(connection, &context.schema.storage_name)? {
        return load_relation_snapshot(&context.source_relation, connection, ctx);
    }
    let (sql_alias, query_tail) = query_target_alias_and_tail(&context, ctx)?;
    connection
        .execute_sql(format!(
            "DELETE FROM {} WHERE __aivi_ord IN (SELECT {sql_alias}.__aivi_ord {query_tail})",
            context.schema.storage_name,
        ))
        .map_err(RuntimeError::Message)?;
    load_relation_snapshot(&context.source_relation, connection, ctx)
}

fn query_update_on_connection(
    query: Value,
    patch: Value,
    connection: &aivi_database::DbConnection,
    runtime: &mut Runtime,
    ctx: &str,
) -> Result<Value, RuntimeError> {
    let context = prepare_query_mutation_context(query, connection, ctx)?;
    let matched_rows = query_matching_rows_on_connection(&context, connection, ctx)?;
    apply_query_patch_rows(&context, matched_rows, patch, connection, runtime, ctx)?;
    load_relation_snapshot(&context.source_relation, connection, ctx)
}

fn query_upsert_on_connection(
    query: Value,
    value: Value,
    patch: Value,
    connection: &aivi_database::DbConnection,
    runtime: &mut Runtime,
    ctx: &str,
) -> Result<Value, RuntimeError> {
    let context = prepare_query_mutation_context(query, connection, ctx)?;
    let has_rows = storage_has_rows(connection, &context.schema.storage_name)?;
    if !has_rows && context.schema.columns.is_empty() {
        let single_row_table =
            projectable_schema_for_row(context.name.clone(), context.columns.clone(), &value, ctx)?;
        let next_storage_columns_json =
            serde_json::to_string(&single_row_table.columns).map_err(|err| {
                RuntimeError::Message(format!(
                    "{ctx} could not encode inferred storage columns for '{}': {err}",
                    context.name
                ))
            })?;
        connection
            .replace_table_storage(
                context.rev,
                context.name.clone(),
                context.columns_json.clone(),
                next_storage_columns_json,
                single_row_table,
            )
            .map_err(RuntimeError::Message)?;
        return load_relation_snapshot(&context.source_relation, connection, ctx);
    }

    if context.schema.columns.is_empty() {
        return Err(RuntimeError::Message(format!(
            "{ctx} cannot upsert into a root relation query without declared columns"
        )));
    }

    let matched_rows = query_matching_rows_on_connection(&context, connection, ctx)?;
    if !matched_rows.is_empty() {
        apply_query_patch_rows(&context, matched_rows, patch, connection, runtime, ctx)?;
        return load_relation_snapshot(&context.source_relation, connection, ctx);
    }

    let desired_schema =
        projectable_schema_for_row(context.name.clone(), context.columns.clone(), &value, ctx)?;
    let current_columns: std::collections::HashSet<_> =
        context.schema.columns.iter().map(|column| column.name.clone()).collect();
    let desired_columns: std::collections::HashSet<_> =
        desired_schema.columns.iter().map(|column| column.name.clone()).collect();
    if !desired_columns.is_subset(&current_columns) {
        return Err(RuntimeError::Message(format!(
            "{ctx} cannot insert relation upsert row because it introduces new projected columns"
        )));
    }
    let ordinal = next_storage_ordinal(connection, &context.schema.storage_name)?;
    let insert_sql = build_insert_sql_for_schema(&context.schema, &value, ordinal, ctx)?;
    connection
        .execute_sql(insert_sql)
        .map_err(RuntimeError::Message)?;
    load_relation_snapshot(&context.source_relation, connection, ctx)
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
            "loadRelation".to_string(),
            builtin("database.loadRelation", 1, move |mut args, _| {
                let relation = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |_| match default_db_connection(&state)? {
                            Some(connection) => load_rows_from_connection(relation.clone(), &connection),
                            None => {
                                let (_, _, rows) =
                                    relation_parts(relation.clone(), "database.loadRelation")?;
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
        "loadRelationOn".to_string(),
        builtin("database.loadRelationOn", 2, move |mut args, _| {
            let relation = args.pop().unwrap();
            let connection = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let connection =
                        expect_db_connection(connection.clone(), "database.loadRelationOn")?;
                    load_rows_from_connection(relation.clone(), &connection)
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    {
        let state = state.clone();
        fields.insert(
            "insert".to_string(),
            builtin("database.insert", 2, move |mut args, _| {
                let value = args.pop().unwrap();
                let relation = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |_| match default_db_connection(&state)? {
                            Some(connection) => {
                                insert_relation_on_connection(relation.clone(), value.clone(), &connection)
                            }
                            None => insert_relation(relation.clone(), value.clone()),
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    fields.insert(
        "insertOn".to_string(),
        builtin("database.insertOn", 3, move |mut args, _| {
            let value = args.pop().unwrap();
            let relation = args.pop().unwrap();
            let connection = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let connection = expect_db_connection(connection.clone(), "database.insertOn")?;
                    insert_relation_on_connection(relation.clone(), value.clone(), &connection)
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
                let query = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |runtime| match default_db_connection(&state)? {
                            Some(connection) => Ok(list_value(query_rows_on_connection(
                                query.clone(),
                                &connection,
                                runtime,
                                "database.rows",
                            )?)),
                            None => Err(RuntimeError::Message(
                                "database backend is not configured".to_string(),
                            )),
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
            let query = args.pop().unwrap();
            let connection = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let connection = expect_db_connection(connection.clone(), "database.rowsOn")?;
                    Ok(list_value(query_rows_on_connection(
                        query.clone(),
                        &connection,
                        runtime,
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
                let query = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |runtime| match default_db_connection(&state)? {
                            Some(connection) => query_first_on_connection(
                                query.clone(),
                                &connection,
                                runtime,
                                "database.first",
                            ),
                            None => Err(RuntimeError::Message(
                                "database backend is not configured".to_string(),
                            )),
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
            let query = args.pop().unwrap();
            let connection = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let connection = expect_db_connection(connection.clone(), "database.firstOn")?;
                    query_first_on_connection(query.clone(), &connection, runtime, "database.firstOn")
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
                let query = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |_runtime| match default_db_connection(&state)? {
                            Some(connection) => query_delete_on_connection(
                                query.clone(),
                                &connection,
                                "database.delete",
                            ),
                            None => Err(RuntimeError::Message(
                                "database backend is not configured".to_string(),
                            )),
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
            let query = args.pop().unwrap();
            let connection = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |_| {
                    let connection = expect_db_connection(connection.clone(), "database.deleteOn")?;
                    query_delete_on_connection(query.clone(), &connection, "database.deleteOn")
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
                let query = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |runtime| match default_db_connection(&state)? {
                            Some(connection) => query_update_on_connection(
                                query.clone(),
                                patch.clone(),
                                &connection,
                                runtime,
                                "database.update",
                            ),
                            None => Err(RuntimeError::Message(
                                "database backend is not configured".to_string(),
                            )),
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
            let query = args.pop().unwrap();
            let connection = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let connection = expect_db_connection(connection.clone(), "database.updateOn")?;
                    query_update_on_connection(
                        query.clone(),
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
                let query = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |runtime| match default_db_connection(&state)? {
                            Some(connection) => query_upsert_on_connection(
                                query.clone(),
                                value.clone(),
                                patch.clone(),
                                &connection,
                                runtime,
                                "database.upsert",
                            ),
                            None => Err(RuntimeError::Message(
                                "database backend is not configured".to_string(),
                            )),
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
            let query = args.pop().unwrap();
            let connection = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let connection = expect_db_connection(connection.clone(), "database.upsertOn")?;
                    query_upsert_on_connection(
                        query.clone(),
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
            "count".to_string(),
            builtin("database.count", 1, move |mut args, _| {
                let query = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |runtime| {
                            let connection = require_default_db_connection(&state)?;
                            let counted = build_count_query(query.clone())?;
                            let fields = expect_record(counted, "database.count")?;
                            let run_fn = fields
                                .get("run")
                                .ok_or_else(|| {
                                    RuntimeError::Message(
                                        "database.count expects Query with 'run' field".to_string(),
                                    )
                                })?
                                .clone();
                            let inner_effect = runtime.apply(run_fn, Value::DbConnection(connection))?;
                            let result = runtime.run_effect_value(inner_effect)?;
                            let values = expect_list(result, "database.count")?;
                            match values.iter().next().cloned() {
                                Some(Value::Int(value)) => Ok(Value::Int(value)),
                                Some(other) => Err(RuntimeError::Message(format!(
                                    "database.count expected Int result, found {other:?}"
                                ))),
                                None => Ok(Value::Int(0)),
                            }
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    fields.insert(
        "countOn".to_string(),
        builtin("database.countOn", 2, move |mut args, _| {
            let query = args.pop().unwrap();
            let connection = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let connection = expect_db_connection(connection.clone(), "database.countOn")?;
                    let counted = build_count_query(query.clone())?;
                    let fields = expect_record(counted, "database.countOn")?;
                    let run_fn = fields
                        .get("run")
                        .ok_or_else(|| {
                            RuntimeError::Message(
                                "database.countOn expects Query with 'run' field".to_string(),
                            )
                        })?
                        .clone();
                    let inner_effect = runtime.apply(run_fn, Value::DbConnection(connection))?;
                    let result = runtime.run_effect_value(inner_effect)?;
                    let values = expect_list(result, "database.countOn")?;
                    match values.iter().next().cloned() {
                        Some(Value::Int(value)) => Ok(Value::Int(value)),
                        Some(other) => Err(RuntimeError::Message(format!(
                            "database.countOn expected Int result, found {other:?}"
                        ))),
                        None => Ok(Value::Int(0)),
                    }
                }),
            };
            Ok(Value::Effect(Arc::new(effect)))
        }),
    );

    {
        let state = state.clone();
        fields.insert(
            "exists".to_string(),
            builtin("database.exists", 1, move |mut args, _| {
                let query = args.pop().unwrap();
                let effect = EffectValue::Thunk {
                    func: Arc::new({
                        let state = state.clone();
                        move |runtime| {
                            let connection = require_default_db_connection(&state)?;
                            let exists_query = build_exists_query(query.clone())?;
                            let fields = expect_record(exists_query, "database.exists")?;
                            let run_fn = fields
                                .get("run")
                                .ok_or_else(|| {
                                    RuntimeError::Message(
                                        "database.exists expects Query with 'run' field".to_string(),
                                    )
                                })?
                                .clone();
                            let inner_effect =
                                runtime.apply(run_fn, Value::DbConnection(connection))?;
                            let result = runtime.run_effect_value(inner_effect)?;
                            let values = expect_list(result, "database.exists")?;
                            match values.iter().next().cloned() {
                                Some(Value::Bool(value)) => Ok(Value::Bool(value)),
                                Some(other) => Err(RuntimeError::Message(format!(
                                    "database.exists expected Bool result, found {other:?}"
                                ))),
                                None => Ok(Value::Bool(false)),
                            }
                        }
                    }),
                };
                Ok(Value::Effect(Arc::new(effect)))
            }),
        );
    }

    fields.insert(
        "existsOn".to_string(),
        builtin("database.existsOn", 2, move |mut args, _| {
            let query = args.pop().unwrap();
            let connection = args.pop().unwrap();
            let effect = EffectValue::Thunk {
                func: Arc::new(move |runtime| {
                    let connection = expect_db_connection(connection.clone(), "database.existsOn")?;
                    let exists_query = build_exists_query(query.clone())?;
                    let fields = expect_record(exists_query, "database.existsOn")?;
                    let run_fn = fields
                        .get("run")
                        .ok_or_else(|| {
                            RuntimeError::Message(
                                "database.existsOn expects Query with 'run' field".to_string(),
                            )
                        })?
                        .clone();
                    let inner_effect = runtime.apply(run_fn, Value::DbConnection(connection))?;
                    let result = runtime.run_effect_value(inner_effect)?;
                    let values = expect_list(result, "database.existsOn")?;
                    match values.iter().next().cloned() {
                        Some(Value::Bool(value)) => Ok(Value::Bool(value)),
                        Some(other) => Err(RuntimeError::Message(format!(
                            "database.existsOn expected Bool result, found {other:?}"
                        ))),
                        None => Ok(Value::Bool(false)),
                    }
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
