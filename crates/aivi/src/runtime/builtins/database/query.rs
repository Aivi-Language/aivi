use aivi_database::{QueryCell, QueryColumn, QueryColumnType, QueryRow, QueryTable};

use crate::hir::{
    CompiledAggregate, CompiledOrderBy, CompiledProjection, CompiledQueryPlan,
    CompiledQuerySource, CompiledScalarExpr,
};

const QUERY_META_FIELD: &str = "__aiviQueryPlan";

#[derive(Clone, Copy)]
enum ScalarKind {
    Int,
    Float,
    Bool,
    Text,
}

#[derive(Clone)]
struct RuntimeColumn {
    name: String,
    kind: QueryColumnType,
    not_null: bool,
}

#[derive(Clone)]
struct RuntimeTableSchema {
    name: String,
    columns: Vec<RuntimeColumn>,
}

pub(super) fn build_query_compiled_builtin() -> Value {
    builtin("__db_query_compiled", 2, |mut args, _| {
        let sources_value = args.pop().unwrap();
        let plan_json_value = args.pop().unwrap();
        let plan_json = expect_text(plan_json_value, "__db_query_compiled")?;
        let sources = expect_list(sources_value, "__db_query_compiled")?;
        Ok(build_compiled_query_value(
            plan_json,
            sources.iter().cloned().collect(),
        ))
    })
}

pub(super) fn build_query_count_builtin() -> Value {
    builtin("__db_query_count", 1, |mut args, _| {
        let query = args.pop().unwrap();
        build_count_query(query)
    })
}

pub(super) fn build_query_exists_builtin() -> Value {
    builtin("__db_query_exists", 1, |mut args, _| {
        let query = args.pop().unwrap();
        build_exists_query(query)
    })
}

pub(super) fn build_query_error_builtin() -> Value {
    builtin("__db_query_error", 1, |mut args, _| {
        let message = expect_text(args.pop().unwrap(), "__db_query_error")?;
        Ok(make_query_error_value(message))
    })
}

fn make_query_value(run: Value, plan_json: Option<String>, sources: Vec<Value>) -> Value {
    let mut fields = HashMap::new();
    fields.insert("run".to_string(), run);
    if let Some(plan_json) = plan_json {
        let mut meta = HashMap::new();
        meta.insert("planJson".to_string(), Value::Text(plan_json));
        meta.insert("sources".to_string(), list_value(sources));
        fields.insert(QUERY_META_FIELD.to_string(), Value::Record(Arc::new(meta)));
    }
    Value::Record(Arc::new(fields))
}

fn make_query_error_value(message: String) -> Value {
    let run = builtin("__db_query_error.run", 1, move |_args, _| {
        let effect = EffectValue::Thunk {
            func: Arc::new({
                let message = message.clone();
                move |_| Err(RuntimeError::Message(message.clone()))
            }),
        };
        Ok(Value::Effect(Arc::new(effect)))
    });
    make_query_value(run, None, Vec::new())
}

fn build_compiled_query_value(plan_json: String, sources: Vec<Value>) -> Value {
    let meta_plan_json = plan_json.clone();
    let meta_sources = sources.clone();
    let run = builtin("__db_query_compiled.run", 1, move |mut args, _| {
        let connection = args.pop().unwrap();
        let effect = EffectValue::Thunk {
            func: Arc::new({
                let plan_json = plan_json.clone();
                let sources = sources.clone();
                move |_| {
                    let connection = expect_db_connection(connection.clone(), "database.query.run")?;
                    execute_compiled_query(&connection, &plan_json, &sources)
                }
            }),
        };
        Ok(Value::Effect(Arc::new(effect)))
    });
    make_query_value(run, Some(meta_plan_json), meta_sources)
}

fn extract_query_meta(query: &Value) -> Result<Option<(String, Vec<Value>)>, RuntimeError> {
    let fields = expect_record(query.clone(), "database.query meta")?;
    let Some(meta_value) = fields.get(QUERY_META_FIELD) else {
        return Ok(None);
    };
    let meta = expect_record(meta_value.clone(), "database.query meta")?;
    let plan_json = expect_text(
        meta.get("planJson")
            .ok_or_else(|| RuntimeError::Message("database query plan is missing planJson".to_string()))?
            .clone(),
        "database.query meta",
    )?;
    let sources = expect_list(
        meta.get("sources")
            .ok_or_else(|| RuntimeError::Message("database query plan is missing sources".to_string()))?
            .clone(),
        "database.query meta",
    )?;
    Ok(Some((plan_json, sources.iter().cloned().collect())))
}

fn build_count_query(query: Value) -> Result<Value, RuntimeError> {
    if let Some((plan_json, sources)) = extract_query_meta(&query)? {
        let mut plan: CompiledQueryPlan = serde_json::from_str(&plan_json)
            .map_err(|err| RuntimeError::Message(format!("database query plan decode error: {err}")))?;
        plan.aggregate = CompiledAggregate::Count;
        let next_json = serde_json::to_string(&plan)
            .map_err(|err| RuntimeError::Message(format!("database query plan encode error: {err}")))?;
        return Ok(build_compiled_query_value(next_json, sources));
    }

    let run = builtin("__db_query_count.run", 1, move |mut args, runtime| {
        let connection = args.pop().unwrap();
        let inner_effect = query_run_field(query.clone(), Value::DbConnection(expect_db_connection(connection, "database.count")?), runtime)?;
        let effect = EffectValue::Thunk {
            func: Arc::new(move |runtime| {
                let result = runtime.run_effect_value(inner_effect.clone())?;
                let rows = expect_list(result, "database.count")?;
                Ok(list_value(vec![Value::Int(rows.len() as i64)]))
            }),
        };
        Ok(Value::Effect(Arc::new(effect)))
    });
    Ok(make_query_value(run, None, Vec::new()))
}

fn build_exists_query(query: Value) -> Result<Value, RuntimeError> {
    if let Some((plan_json, sources)) = extract_query_meta(&query)? {
        let mut plan: CompiledQueryPlan = serde_json::from_str(&plan_json)
            .map_err(|err| RuntimeError::Message(format!("database query plan decode error: {err}")))?;
        plan.aggregate = CompiledAggregate::Exists;
        let next_json = serde_json::to_string(&plan)
            .map_err(|err| RuntimeError::Message(format!("database query plan encode error: {err}")))?;
        return Ok(build_compiled_query_value(next_json, sources));
    }

    let run = builtin("__db_query_exists.run", 1, move |mut args, runtime| {
        let connection = args.pop().unwrap();
        let inner_effect = query_run_field(query.clone(), Value::DbConnection(expect_db_connection(connection, "database.exists")?), runtime)?;
        let effect = EffectValue::Thunk {
            func: Arc::new(move |runtime| {
                let result = runtime.run_effect_value(inner_effect.clone())?;
                let rows = expect_list(result, "database.exists")?;
                Ok(list_value(vec![Value::Bool(!rows.is_empty())]))
            }),
        };
        Ok(Value::Effect(Arc::new(effect)))
    });
    Ok(make_query_value(run, None, Vec::new()))
}

fn query_run_field(query: Value, connection: Value, runtime: &mut Runtime) -> Result<Value, RuntimeError> {
    let fields = expect_record(query, "database query")?;
    let run_fn = fields
        .get("run")
        .ok_or_else(|| RuntimeError::Message("query is missing run field".to_string()))?
        .clone();
    runtime.apply(run_fn, connection)
}

fn execute_compiled_query(
    connection: &aivi_database::DbConnection,
    plan_json: &str,
    sources: &[Value],
) -> Result<Value, RuntimeError> {
    let plan: CompiledQueryPlan = serde_json::from_str(plan_json)
        .map_err(|err| RuntimeError::Message(format!("database query plan decode error: {err}")))?;
    let schemas = build_runtime_schemas(&plan.sources, sources)?;
    let sql = build_query_sql(&plan, &schemas)?;
    let rows = connection.query_sql(sql).map_err(RuntimeError::Message)?;
    decode_query_rows(&plan, &schemas, rows)
}

fn build_runtime_schemas(
    plan_sources: &[CompiledQuerySource],
    sources: &[Value],
) -> Result<HashMap<String, RuntimeTableSchema>, RuntimeError> {
    let mut schemas = HashMap::new();
    for source in plan_sources {
        let table = sources.get(source.source_index).ok_or_else(|| {
            RuntimeError::Message(format!(
                "database query source index {} is out of bounds",
                source.source_index
            ))
        })?;
        let (name, columns, _rows) = table_parts(table.clone(), "database.query schema")?;
        validate_identifier(&name, "database.query table name")?;
        let columns = parse_runtime_columns(columns)?;
        schemas.insert(source.alias.clone(), RuntimeTableSchema { name, columns });
    }
    Ok(schemas)
}

fn parse_runtime_columns(columns: Value) -> Result<Vec<RuntimeColumn>, RuntimeError> {
    let columns = expect_list(columns, "database.query columns")?;
    let mut out = Vec::with_capacity(columns.len());
    for column in columns.iter() {
        let fields = expect_record(column.clone(), "database.query column")?;
        let name = expect_text(
            fields
                .get("name")
                .ok_or_else(|| RuntimeError::Message("database.query column missing name".to_string()))?
                .clone(),
            "database.query column name",
        )?;
        validate_identifier(&name, "database.query column name")?;
        let type_value = fields
            .get("type")
            .ok_or_else(|| RuntimeError::Message("database.query column missing type".to_string()))?
            .clone();
        let constraints = expect_list(
            fields
                .get("constraints")
                .ok_or_else(|| RuntimeError::Message("database.query column missing constraints".to_string()))?
                .clone(),
            "database.query column constraints",
        )?;
        let not_null = constraints.iter().any(|constraint| {
            matches!(constraint, Value::Constructor { name, args } if name == "NotNull" && args.is_empty())
        });
        let kind = match type_value {
            Value::Constructor { name, args } if args.is_empty() && name == "IntType" => {
                QueryColumnType::Int
            }
            Value::Constructor { name, args } if args.is_empty() && name == "BoolType" => {
                QueryColumnType::Bool
            }
            Value::Constructor { name, args } if args.is_empty() && name == "TimestampType" => {
                QueryColumnType::Timestamp
            }
            Value::Constructor { name, .. } if name == "Varchar" => QueryColumnType::Text,
            other => {
                return Err(RuntimeError::Message(format!(
                    "database.query does not support column type {}",
                    crate::runtime::format_value(&other)
                )))
            }
        };
        out.push(RuntimeColumn {
            name,
            kind,
            not_null,
        });
    }
    Ok(out)
}

fn validate_identifier(name: &str, ctx: &str) -> Result<(), RuntimeError> {
    if !name.is_empty() && name.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
        Ok(())
    } else {
        Err(RuntimeError::Message(format!(
            "{ctx} expects SQL identifier [A-Za-z0-9_]+, got '{name}'"
        )))
    }
}

fn build_query_sql(
    plan: &CompiledQueryPlan,
    schemas: &HashMap<String, RuntimeTableSchema>,
) -> Result<String, RuntimeError> {
    let sql_aliases = build_sql_aliases(plan);
    let mut from_parts = Vec::new();
    for (index, source) in plan.sources.iter().enumerate() {
        let schema = schemas.get(&source.alias).ok_or_else(|| {
            RuntimeError::Message(format!("missing schema for query alias '{}'", source.alias))
        })?;
        let sql_alias = sql_aliases.get(&source.alias).ok_or_else(|| {
            RuntimeError::Message(format!("missing SQL alias for '{}'", source.alias))
        })?;
        let part = format!("{} {}", schema.name, sql_alias);
        if index == 0 {
            from_parts.push(format!("FROM {part}"));
        } else {
            from_parts.push(format!("CROSS JOIN {part}"));
        }
    }

    let where_sql = if plan.filters.is_empty() {
        String::new()
    } else {
        format!(
            " WHERE {}",
            plan.filters
                .iter()
                .map(|expr| render_scalar_expr(expr, schemas, &sql_aliases))
                .collect::<Result<Vec<_>, _>>()?
                .join(" AND ")
        )
    };

    let mut order_exprs = plan
        .order_by
        .iter()
        .map(|order| render_order_by(order, schemas, &sql_aliases))
        .collect::<Result<Vec<_>, _>>()?;
    if matches!(plan.aggregate, CompiledAggregate::None) {
        if order_exprs.is_empty() {
            for source in &plan.sources {
                order_exprs.push(format!(
                    "{}.__aivi_rowid ASC",
                    sql_aliases
                        .get(&source.alias)
                        .ok_or_else(|| RuntimeError::Message(format!(
                            "missing SQL alias for '{}'",
                            source.alias
                        )))?
                ));
            }
        } else {
            for source in &plan.sources {
                order_exprs.push(format!(
                    "{}.__aivi_rowid ASC",
                    sql_aliases
                        .get(&source.alias)
                        .ok_or_else(|| RuntimeError::Message(format!(
                            "missing SQL alias for '{}'",
                            source.alias
                        )))?
                ));
            }
        }
    }
    let order_sql = if order_exprs.is_empty() {
        String::new()
    } else {
        format!(" ORDER BY {}", order_exprs.join(", "))
    };

    let limit_sql = match (plan.limit, plan.offset) {
        (Some(limit), Some(offset)) => format!(" LIMIT {} OFFSET {}", limit.max(0), offset.max(0)),
        (Some(limit), None) => format!(" LIMIT {}", limit.max(0)),
        (None, Some(offset)) => format!(" LIMIT -1 OFFSET {}", offset.max(0)),
        (None, None) => String::new(),
    };

    let from_where_order_limit = format!(
        "{}{}{}{}",
        from_parts.join(" "),
        where_sql,
        order_sql,
        limit_sql,
    );

    match plan.aggregate {
        CompiledAggregate::None => {
            let select_sql = render_select_projection(&plan.projection, schemas, &sql_aliases)?;
            Ok(format!("SELECT {} {}", select_sql.join(", "), from_where_order_limit))
        }
        CompiledAggregate::Count => Ok(format!(
            "SELECT COUNT(*) FROM (SELECT 1 {}) __aivi_count_src",
            from_where_order_limit
        )),
        CompiledAggregate::Exists => {
            let exists_tail = if plan.limit.is_some() || plan.offset.is_some() {
                from_where_order_limit
            } else {
                format!("{from_where_order_limit} LIMIT 1")
            };
            Ok(format!("SELECT 1 {}", exists_tail))
        }
    }
}

fn build_sql_aliases(plan: &CompiledQueryPlan) -> HashMap<String, String> {
    plan.sources
        .iter()
        .enumerate()
        .map(|(index, source)| (source.alias.clone(), format!("t{index}")))
        .collect()
}

fn render_order_by(
    order: &CompiledOrderBy,
    schemas: &HashMap<String, RuntimeTableSchema>,
    sql_aliases: &HashMap<String, String>,
) -> Result<String, RuntimeError> {
    Ok(format!(
        "{} {}",
        render_scalar_expr(&order.expr, schemas, sql_aliases)?,
        if order.descending { "DESC" } else { "ASC" }
    ))
}

fn render_select_projection(
    projection: &CompiledProjection,
    schemas: &HashMap<String, RuntimeTableSchema>,
    sql_aliases: &HashMap<String, String>,
) -> Result<Vec<String>, RuntimeError> {
    match projection {
        CompiledProjection::Row { alias } => Ok(vec![format!(
            "{}.__aivi_row_json",
            sql_aliases
                .get(alias)
                .ok_or_else(|| RuntimeError::Message(format!("missing SQL alias for '{}'", alias)))?
        )]),
        CompiledProjection::Scalar { expr } => Ok(vec![render_scalar_expr(
            expr, schemas, sql_aliases,
        )?]),
        CompiledProjection::Record { fields } => {
            let mut out = Vec::new();
            for field in fields {
                out.extend(render_select_projection(&field.value, schemas, sql_aliases)?);
            }
            Ok(out)
        }
    }
}

fn render_scalar_expr(
    expr: &CompiledScalarExpr,
    schemas: &HashMap<String, RuntimeTableSchema>,
    sql_aliases: &HashMap<String, String>,
) -> Result<String, RuntimeError> {
    match expr {
        CompiledScalarExpr::Column { alias, field } => {
            let schema = schemas.get(alias).ok_or_else(|| {
                RuntimeError::Message(format!("unknown query alias '{}'", alias))
            })?;
            if !schema.columns.iter().any(|column| column.name == *field) {
                return Err(RuntimeError::Message(format!(
                    "unknown query field '{}.{}'",
                    alias, field
                )));
            }
            Ok(format!(
                "{}.{}",
                sql_aliases
                    .get(alias)
                    .ok_or_else(|| RuntimeError::Message(format!(
                        "missing SQL alias for '{}'",
                        alias
                    )))?,
                field
            ))
        }
        CompiledScalarExpr::IntLit { value } => Ok(value.to_string()),
        CompiledScalarExpr::FloatLit { value } => Ok(value.to_string()),
        CompiledScalarExpr::TextLit { value } => Ok(format!("'{}'", value.replace('\'', "''"))),
        CompiledScalarExpr::BoolLit { value } => {
            if *value {
                Ok("TRUE".to_string())
            } else {
                Ok("FALSE".to_string())
            }
        }
        CompiledScalarExpr::DateTimeLit { value } => {
            Ok(format!("'{}'", value.replace('\'', "''")))
        }
        CompiledScalarExpr::UnaryNeg { expr } => Ok(format!(
            "(-{})",
            render_scalar_expr(expr, schemas, sql_aliases)?
        )),
        CompiledScalarExpr::Binary { op, left, right } => {
            let sql_op = match op.as_str() {
                "==" => "=",
                "!=" => "<>",
                "&&" => "AND",
                "||" => "OR",
                other => other,
            };
            Ok(format!(
                "({} {} {})",
                render_scalar_expr(left, schemas, sql_aliases)?,
                sql_op,
                render_scalar_expr(right, schemas, sql_aliases)?
            ))
        }
    }
}

fn decode_query_rows(
    plan: &CompiledQueryPlan,
    schemas: &HashMap<String, RuntimeTableSchema>,
    rows: Vec<Vec<QueryCell>>,
) -> Result<Value, RuntimeError> {
    match plan.aggregate {
        CompiledAggregate::Count => {
            let count = match rows.first().and_then(|row| row.first()) {
                Some(QueryCell::Int(value)) => *value,
                Some(QueryCell::Float(value)) => *value as i64,
                Some(other) => {
                    return Err(RuntimeError::Message(format!(
                        "database.count expected integer result, got {:?}",
                        other
                    )))
                }
                None => 0,
            };
            Ok(list_value(vec![Value::Int(count)]))
        }
        CompiledAggregate::Exists => Ok(list_value(vec![Value::Bool(!rows.is_empty())])),
        CompiledAggregate::None => {
            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                let (value, consumed) = decode_projection(&plan.projection, schemas, &row, 0)?;
                if consumed != row.len() {
                    return Err(RuntimeError::Message(
                        "database.query returned more columns than expected".to_string(),
                    ));
                }
                out.push(value);
            }
            Ok(list_value(out))
        }
    }
}

fn decode_projection(
    projection: &CompiledProjection,
    schemas: &HashMap<String, RuntimeTableSchema>,
    row: &[QueryCell],
    start: usize,
) -> Result<(Value, usize), RuntimeError> {
    match projection {
        CompiledProjection::Row { .. } => {
            let cell = row.get(start).ok_or_else(|| {
                RuntimeError::Message("database.query row json column is missing".to_string())
            })?;
            let QueryCell::Text(json) = cell else {
                return Err(RuntimeError::Message(
                    "database.query expected row json text cell".to_string(),
                ));
            };
            Ok((decode_json(json)?, start + 1))
        }
        CompiledProjection::Scalar { expr } => {
            let cell = row.get(start).ok_or_else(|| {
                RuntimeError::Message("database.query scalar column is missing".to_string())
            })?;
            Ok((cell_to_value(cell, infer_scalar_kind(expr, schemas)?)?, start + 1))
        }
        CompiledProjection::Record { fields } => {
            let mut out = HashMap::new();
            let mut cursor = start;
            for field in fields {
                let (value, next) = decode_projection(&field.value, schemas, row, cursor)?;
                out.insert(field.name.clone(), value);
                cursor = next;
            }
            Ok((Value::Record(Arc::new(out)), cursor))
        }
    }
}

fn infer_scalar_kind(
    expr: &CompiledScalarExpr,
    schemas: &HashMap<String, RuntimeTableSchema>,
) -> Result<ScalarKind, RuntimeError> {
    match expr {
        CompiledScalarExpr::Column { alias, field } => {
            let schema = schemas.get(alias).ok_or_else(|| {
                RuntimeError::Message(format!("unknown query alias '{}'", alias))
            })?;
            let column = schema
                .columns
                .iter()
                .find(|column| column.name == *field)
                .ok_or_else(|| RuntimeError::Message(format!("unknown query field '{}.{}'", alias, field)))?;
            Ok(match column.kind {
                QueryColumnType::Int => ScalarKind::Int,
                QueryColumnType::Bool => ScalarKind::Bool,
                QueryColumnType::Float => ScalarKind::Float,
                QueryColumnType::Text | QueryColumnType::Timestamp => ScalarKind::Text,
            })
        }
        CompiledScalarExpr::IntLit { .. } => Ok(ScalarKind::Int),
        CompiledScalarExpr::FloatLit { .. } => Ok(ScalarKind::Float),
        CompiledScalarExpr::TextLit { .. } | CompiledScalarExpr::DateTimeLit { .. } => {
            Ok(ScalarKind::Text)
        }
        CompiledScalarExpr::BoolLit { .. } => Ok(ScalarKind::Bool),
        CompiledScalarExpr::UnaryNeg { expr } => infer_scalar_kind(expr, schemas),
        CompiledScalarExpr::Binary { op, left, right } => match op.as_str() {
            "==" | "!=" | ">" | ">=" | "<" | "<=" | "&&" | "||" => Ok(ScalarKind::Bool),
            "/" => Ok(ScalarKind::Float),
            _ => {
                let left_kind = infer_scalar_kind(left, schemas)?;
                let right_kind = infer_scalar_kind(right, schemas)?;
                if matches!(left_kind, ScalarKind::Float) || matches!(right_kind, ScalarKind::Float) {
                    Ok(ScalarKind::Float)
                } else {
                    Ok(ScalarKind::Int)
                }
            }
        },
    }
}

fn cell_to_value(cell: &QueryCell, kind: ScalarKind) -> Result<Value, RuntimeError> {
    match (cell, kind) {
        (QueryCell::Int(value), ScalarKind::Int) => Ok(Value::Int(*value)),
        (QueryCell::Int(value), ScalarKind::Bool) => Ok(Value::Bool(*value != 0)),
        (QueryCell::Float(value), ScalarKind::Float) => Ok(Value::Float(*value)),
        (QueryCell::Float(value), ScalarKind::Int) => Ok(Value::Int(*value as i64)),
        (QueryCell::Bool(value), ScalarKind::Bool) => Ok(Value::Bool(*value)),
        (QueryCell::Text(value), ScalarKind::Text) => Ok(Value::Text(value.clone())),
        (QueryCell::Text(value), ScalarKind::Int) => value
            .parse::<i64>()
            .map(Value::Int)
            .map_err(|_| RuntimeError::Message(format!("database.query could not decode int from '{value}'"))),
        (QueryCell::Text(value), ScalarKind::Float) => value
            .parse::<f64>()
            .map(Value::Float)
            .map_err(|_| RuntimeError::Message(format!("database.query could not decode float from '{value}'"))),
        (QueryCell::Text(value), ScalarKind::Bool) => match value.as_str() {
            "0" | "false" | "FALSE" | "f" | "F" => Ok(Value::Bool(false)),
            "1" | "true" | "TRUE" | "t" | "T" => Ok(Value::Bool(true)),
            _ => Err(RuntimeError::Message(format!(
                "database.query could not decode bool from '{value}'"
            ))),
        },
        (QueryCell::Null, _) => Err(RuntimeError::Message(
            "database.query encountered NULL in a non-optional projection".to_string(),
        )),
        (other, _) => Err(RuntimeError::Message(format!(
            "database.query could not decode cell {:?}",
            other
        ))),
    }
}

fn build_query_table_mirror(
    name: String,
    columns_value: Value,
    rows: &[Value],
) -> Result<Option<QueryTable>, RuntimeError> {
    let runtime_columns = parse_runtime_columns(columns_value)?;
    if runtime_columns.is_empty() {
        return Ok(None);
    }
    let columns = runtime_columns
        .iter()
        .map(|column| QueryColumn {
            name: column.name.clone(),
            column_type: column.kind,
            not_null: column.not_null,
        })
        .collect::<Vec<_>>();
    let mut query_rows = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        let row_fields = expect_record(row.clone(), "database.query mirror row")?;
        let mut values = Vec::with_capacity(runtime_columns.len());
        for column in &runtime_columns {
            let value = row_fields.get(&column.name).ok_or_else(|| {
                RuntimeError::Message(format!(
                    "database.query mirror row is missing field '{}'",
                    column.name
                ))
            })?;
            values.push(runtime_value_to_query_cell(value, column.kind, column.not_null)?);
        }
        query_rows.push(QueryRow {
            row_index: index as i64,
            row_json: encode_json(row)?,
            values,
        });
    }
    Ok(Some(QueryTable {
        name,
        columns,
        rows: query_rows,
    }))
}

fn runtime_value_to_query_cell(
    value: &Value,
    kind: QueryColumnType,
    not_null: bool,
) -> Result<QueryCell, RuntimeError> {
    match value {
        Value::Constructor { name, args } if name == "Some" && args.len() == 1 => {
            return runtime_value_to_query_cell(&args[0], kind, not_null);
        }
        Value::Constructor { name, args } if name == "None" && args.is_empty() => {
            if not_null {
                return Err(RuntimeError::Message(
                    "database.query mirror cannot persist None into a NOT NULL column".to_string(),
                ));
            }
            return Ok(QueryCell::Null);
        }
        _ => {}
    }
    match (value, kind) {
        (Value::Int(value), QueryColumnType::Int) => Ok(QueryCell::Int(*value)),
        (Value::Bool(value), QueryColumnType::Bool) => Ok(QueryCell::Bool(*value)),
        (Value::Text(value), QueryColumnType::Text) => Ok(QueryCell::Text(value.clone())),
        (Value::DateTime(value), QueryColumnType::Timestamp) => Ok(QueryCell::Text(value.clone())),
        (Value::Text(value), QueryColumnType::Timestamp) => Ok(QueryCell::Text(value.clone())),
        (Value::Float(value), QueryColumnType::Float) => Ok(QueryCell::Float(*value)),
        (other, _) => Err(RuntimeError::Message(format!(
            "database.query mirror does not support persisting {}",
            crate::runtime::format_value(other)
        ))),
    }
}

fn sync_query_table_if_possible(
    connection: &aivi_database::DbConnection,
    name: String,
    columns_value: Value,
    rows: &[Value],
) -> Result<(), RuntimeError> {
    if let Some(table) = build_query_table_mirror(name, columns_value, rows)? {
        connection.sync_query_table(table).map_err(RuntimeError::Message)?;
    }
    Ok(())
}

#[cfg(test)]
mod query_tests {
    use super::*;

    #[test]
    fn runtime_value_to_query_cell_unwraps_some_text() {
        let cell = match runtime_value_to_query_cell(
            &Value::Constructor {
                name: "Some".to_string(),
                args: vec![Value::Text("hello".to_string())],
            },
            QueryColumnType::Text,
            false,
        ) {
            Ok(cell) => cell,
            Err(_) => panic!("Some text should persist"),
        };
        assert!(matches!(cell, QueryCell::Text(value) if value == "hello"));
    }

    #[test]
    fn runtime_value_to_query_cell_maps_none_to_null_for_nullable_columns() {
        let cell = match runtime_value_to_query_cell(
            &Value::Constructor {
                name: "None".to_string(),
                args: Vec::new(),
            },
            QueryColumnType::Text,
            false,
        ) {
            Ok(cell) => cell,
            Err(_) => panic!("None should map to NULL for nullable columns"),
        };
        assert!(matches!(cell, QueryCell::Null));
    }

    #[test]
    fn runtime_value_to_query_cell_rejects_none_for_not_null_columns() {
        let err = runtime_value_to_query_cell(
            &Value::Constructor {
                name: "None".to_string(),
                args: Vec::new(),
            },
            QueryColumnType::Text,
            true,
        )
        .expect_err("NOT NULL columns should reject None");
        match err {
            RuntimeError::Message(message) => {
                assert!(message.contains("NOT NULL"));
            }
            _ => panic!("unexpected error type"),
        }
    }
}
