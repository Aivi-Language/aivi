use aivi_database::{QueryCell, QueryColumn, QueryColumnType, QueryRow, QueryTable};

use aivi_core::CompiledAggregateFn;

use crate::hir::{
    CompiledAggregate, CompiledOrderBy, CompiledProjection, CompiledProjectionField,
    CompiledQueryPlan, CompiledQuerySource, CompiledScalarExpr,
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
    storage_name: String,
    columns: Vec<RuntimeColumn>,
}

struct DecodeProjectionCtx<'a> {
    connection: &'a aivi_database::DbConnection,
    schemas: &'a HashMap<String, RuntimeTableSchema>,
    sources: &'a [Value],
    captures: &'a [Value],
    hidden_rows: &'a HashMap<String, Value>,
}

pub(super) fn build_query_compiled_builtin() -> Value {
    builtin("__db_query_compiled", 3, |mut args, _| {
        let captures_value = args.pop().unwrap();
        let sources_value = args.pop().unwrap();
        let plan_json_value = args.pop().unwrap();
        let plan_json = expect_text(plan_json_value, "__db_query_compiled")?;
        let sources = expect_list(sources_value, "__db_query_compiled")?;
        let captures = expect_list(captures_value, "__db_query_compiled")?;
        Ok(build_compiled_query_value(
            plan_json,
            sources.iter().cloned().collect(),
            captures.iter().cloned().collect(),
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

fn make_query_value(
    run: Value,
    plan_json: Option<String>,
    sources: Vec<Value>,
    captures: Vec<Value>,
) -> Value {
    let mut fields = HashMap::new();
    fields.insert("run".to_string(), run);
    if let Some(plan_json) = plan_json {
        let mut meta = HashMap::new();
        meta.insert("planJson".to_string(), Value::Text(plan_json));
        meta.insert("sources".to_string(), list_value(sources));
        meta.insert("captures".to_string(), list_value(captures));
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
    make_query_value(run, None, Vec::new(), Vec::new())
}

fn build_compiled_query_value(plan_json: String, sources: Vec<Value>, captures: Vec<Value>) -> Value {
    let meta_plan_json = plan_json.clone();
    let meta_sources = sources.clone();
    let meta_captures = captures.clone();
    let run = builtin("__db_query_compiled.run", 1, move |mut args, _| {
        let connection = args.pop().unwrap();
        let effect = EffectValue::Thunk {
            func: Arc::new({
                let plan_json = plan_json.clone();
                let sources = sources.clone();
                let captures = captures.clone();
                move |_| {
                    let connection = expect_db_connection(connection.clone(), "database.query.run")?;
                    execute_compiled_query(&connection, &plan_json, &sources, &captures)
                }
            }),
        };
        Ok(Value::Effect(Arc::new(effect)))
    });
    make_query_value(run, Some(meta_plan_json), meta_sources, meta_captures)
}

type QueryMeta = (String, Vec<Value>, Vec<Value>);

fn extract_query_meta(query: &Value) -> Result<Option<QueryMeta>, RuntimeError> {
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
    let captures = expect_list(
        meta.get("captures")
            .ok_or_else(|| RuntimeError::Message("database query plan is missing captures".to_string()))?
            .clone(),
        "database.query meta",
    )?;
    Ok(Some((
        plan_json,
        sources.iter().cloned().collect(),
        captures.iter().cloned().collect(),
    )))
}

fn build_count_query(query: Value) -> Result<Value, RuntimeError> {
    if let Some((plan_json, sources, captures)) = extract_query_meta(&query)? {
        let mut plan: CompiledQueryPlan = serde_json::from_str(&plan_json)
            .map_err(|err| RuntimeError::Message(format!("database query plan decode error: {err}")))?;
        plan.aggregate = CompiledAggregate::Count;
        let next_json = serde_json::to_string(&plan)
            .map_err(|err| RuntimeError::Message(format!("database query plan encode error: {err}")))?;
        return Ok(build_compiled_query_value(next_json, sources, captures));
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
    Ok(make_query_value(run, None, Vec::new(), Vec::new()))
}

fn build_exists_query(query: Value) -> Result<Value, RuntimeError> {
    if let Some((plan_json, sources, captures)) = extract_query_meta(&query)? {
        let mut plan: CompiledQueryPlan = serde_json::from_str(&plan_json)
            .map_err(|err| RuntimeError::Message(format!("database query plan decode error: {err}")))?;
        plan.aggregate = CompiledAggregate::Exists;
        let next_json = serde_json::to_string(&plan)
            .map_err(|err| RuntimeError::Message(format!("database query plan encode error: {err}")))?;
        return Ok(build_compiled_query_value(next_json, sources, captures));
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
    Ok(make_query_value(run, None, Vec::new(), Vec::new()))
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
    captures: &[Value],
) -> Result<Value, RuntimeError> {
    let plan: CompiledQueryPlan = serde_json::from_str(plan_json)
        .map_err(|err| RuntimeError::Message(format!("database query plan decode error: {err}")))?;
    let schemas = build_runtime_schemas(&plan.sources, sources)?;
    let sql = build_query_sql(&plan, &schemas, sources, captures)?;
    let rows = connection.query_sql(sql).map_err(RuntimeError::Message)?;
    decode_query_rows(connection, &plan, &schemas, sources, captures, rows)
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
        let (name, columns, _rows) = relation_parts(table.clone(), "database.query schema")?;
        validate_identifier(&name, "database.query table name")?;
        let columns = parse_runtime_columns(columns)?;
        let storage_name = aivi_database::query_storage_name(&name);
        schemas.insert(
            source.alias.clone(),
            RuntimeTableSchema {
                name,
                storage_name,
                columns,
            },
        );
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
            Value::Constructor { name, args } if args.is_empty() && name == "FloatType" => {
                QueryColumnType::Float
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

#[derive(Clone, Copy)]
struct InferredColumnState {
    kind: QueryColumnType,
    present_rows: usize,
    saw_nullish: bool,
}

fn infer_runtime_columns(rows: &[Value]) -> Result<Vec<RuntimeColumn>, RuntimeError> {
    let mut inferred = std::collections::BTreeMap::<String, InferredColumnState>::new();
    for row in rows {
        let fields = expect_record(row.clone(), "database.query infer row")?;
        for (name, value) in fields.iter() {
            validate_identifier(name, "database.query inferred column name")?;
            let (kind, nullish) = infer_runtime_column_kind(value)?;
            if let Some(kind) = kind {
                let entry = inferred.entry(name.clone()).or_insert(InferredColumnState {
                    kind,
                    present_rows: 0,
                    saw_nullish: false,
                });
                if entry.kind != kind {
                    return Err(RuntimeError::Message(format!(
                        "database.query inferred column '{name}' changed type across rows"
                    )));
                }
                entry.present_rows += 1;
                entry.saw_nullish |= nullish;
            }
        }
    }
    Ok(inferred
        .into_iter()
        .map(|(name, state)| RuntimeColumn {
            name,
            kind: state.kind,
            not_null: state.present_rows == rows.len() && !state.saw_nullish,
        })
        .collect())
}

fn infer_runtime_column_kind(
    value: &Value,
) -> Result<(Option<QueryColumnType>, bool), RuntimeError> {
    match value {
        Value::Constructor { name, args } if name == "Some" && args.len() == 1 => {
            let (kind, _) = infer_runtime_column_kind(&args[0])?;
            Ok((kind, true))
        }
        Value::Constructor { name, args } if name == "None" && args.is_empty() => Ok((None, true)),
        Value::Int(_) => Ok((Some(QueryColumnType::Int), false)),
        Value::Float(_) => Ok((Some(QueryColumnType::Float), false)),
        Value::Bool(_) => Ok((Some(QueryColumnType::Bool), false)),
        Value::Text(_) => Ok((Some(QueryColumnType::Text), false)),
        Value::DateTime(_) => Ok((Some(QueryColumnType::Timestamp), false)),
        other => Err(RuntimeError::Message(format!(
            "database.query cannot infer SQL storage column for {}",
            crate::runtime::format_value(other)
        ))),
    }
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
    sources: &[Value],
    captures: &[Value],
) -> Result<String, RuntimeError> {
    build_query_sql_with_outer(plan, schemas, sources, captures, &HashMap::new())
}

fn build_query_sql_aliases_and_tail(
    plan: &CompiledQueryPlan,
    schemas: &HashMap<String, RuntimeTableSchema>,
    sources: &[Value],
    captures: &[Value],
    outer_aliases: &HashMap<String, String>,
) -> Result<(HashMap<String, String>, String), RuntimeError> {
    let mut sql_aliases = outer_aliases.clone();
    sql_aliases.extend(build_sql_aliases(plan, outer_aliases.len()));
    let mut from_parts = Vec::new();
    for (index, source) in plan.sources.iter().enumerate() {
        let schema = schemas.get(&source.alias).ok_or_else(|| {
            RuntimeError::Message(format!("missing schema for query alias '{}'", source.alias))
        })?;
        let sql_alias = sql_aliases.get(&source.alias).ok_or_else(|| {
            RuntimeError::Message(format!("missing SQL alias for '{}'", source.alias))
        })?;
        let part = format!("{} {}", schema.storage_name, sql_alias);
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
                .map(|expr| render_scalar_expr(expr, schemas, sources, &sql_aliases, captures))
                .collect::<Result<Vec<_>, _>>()?
                .join(" AND ")
        )
    };

    let group_by_sql = if plan.group_by.is_empty() {
        String::new()
    } else {
        format!(
            " GROUP BY {}",
            plan.group_by
                .iter()
                .map(|expr| render_scalar_expr(expr, schemas, sources, &sql_aliases, captures))
                .collect::<Result<Vec<_>, _>>()?
                .join(", ")
        )
    };

    let having_sql = if plan.having.is_empty() {
        String::new()
    } else {
        format!(
            " HAVING {}",
            plan.having
                .iter()
                .map(|expr| render_scalar_expr(expr, schemas, sources, &sql_aliases, captures))
                .collect::<Result<Vec<_>, _>>()?
                .join(" AND ")
        )
    };

    let mut order_exprs = plan
        .order_by
        .iter()
        .map(|order| render_order_by(order, schemas, sources, &sql_aliases, captures))
        .collect::<Result<Vec<_>, _>>()?;
    if matches!(plan.aggregate, CompiledAggregate::None) && plan.group_by.is_empty() {
        if order_exprs.is_empty() {
            for source in &plan.sources {
                order_exprs.push(format!(
                    "{}.__aivi_ord ASC",
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
                    "{}.__aivi_ord ASC",
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

    Ok((
        sql_aliases,
        format!(
            "{}{}{}{}{}{}",
            from_parts.join(" "),
            where_sql,
            group_by_sql,
            having_sql,
            order_sql,
            limit_sql,
        ),
    ))
}

fn build_query_sql_with_outer(
    plan: &CompiledQueryPlan,
    schemas: &HashMap<String, RuntimeTableSchema>,
    sources: &[Value],
    captures: &[Value],
    outer_aliases: &HashMap<String, String>,
) -> Result<String, RuntimeError> {
    let (sql_aliases, from_where_order_limit) =
        build_query_sql_aliases_and_tail(plan, schemas, sources, captures, outer_aliases)?;

    match plan.aggregate {
        CompiledAggregate::None => {
            let mut select_sql =
                render_select_projection(&plan.projection, schemas, sources, &sql_aliases, captures)?;
            if projection_contains_nested_queries(&plan.projection) {
                for source in &plan.sources {
                    select_sql.extend(render_row_projection(&source.alias, schemas, &sql_aliases)?);
                }
            }
            let select_head = if plan.distinct {
                "SELECT DISTINCT"
            } else {
                "SELECT"
            };
            Ok(format!(
                "{select_head} {} {}",
                select_sql.join(", "),
                from_where_order_limit
            ))
        }
        CompiledAggregate::Count => {
            let mut inner_plan = plan.clone();
            inner_plan.aggregate = CompiledAggregate::None;
            let inner_sql =
                build_query_sql_with_outer(&inner_plan, schemas, sources, captures, outer_aliases)?;
            Ok(format!("SELECT COUNT(*) FROM ({inner_sql}) __aivi_count_src"))
        }
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

fn build_sql_aliases(plan: &CompiledQueryPlan, offset: usize) -> HashMap<String, String> {
    plan.sources
        .iter()
        .enumerate()
        .map(|(index, source)| (source.alias.clone(), format!("t{}", index + offset)))
        .collect()
}

fn render_order_by(
    order: &CompiledOrderBy,
    schemas: &HashMap<String, RuntimeTableSchema>,
    sources: &[Value],
    sql_aliases: &HashMap<String, String>,
    captures: &[Value],
) -> Result<String, RuntimeError> {
    Ok(format!(
        "{} {}",
        render_scalar_expr(&order.expr, schemas, sources, sql_aliases, captures)?,
        if order.descending { "DESC" } else { "ASC" }
    ))
}

fn render_select_projection(
    projection: &CompiledProjection,
    schemas: &HashMap<String, RuntimeTableSchema>,
    sources: &[Value],
    sql_aliases: &HashMap<String, String>,
    captures: &[Value],
) -> Result<Vec<String>, RuntimeError> {
    match projection {
        CompiledProjection::Row { alias, .. } => render_row_projection(alias, schemas, sql_aliases),
        CompiledProjection::Scalar { expr } => Ok(vec![render_scalar_expr(
            expr, schemas, sources, sql_aliases, captures,
        )?]),
        CompiledProjection::Record { fields } => {
            let mut out = Vec::new();
            for field in fields {
                out.extend(render_select_projection(
                    &field.value,
                    schemas,
                    sources,
                    sql_aliases,
                    captures,
                )?);
            }
            Ok(out)
        }
        CompiledProjection::NestedQuery { .. } => Ok(Vec::new()),
    }
}

fn render_row_projection(
    alias: &str,
    schemas: &HashMap<String, RuntimeTableSchema>,
    sql_aliases: &HashMap<String, String>,
) -> Result<Vec<String>, RuntimeError> {
    let schema = schema_for_alias(schemas, alias)?;
    let sql_alias = sql_aliases
        .get(alias)
        .ok_or_else(|| RuntimeError::Message(format!("missing SQL alias for '{}'", alias)))?;
    Ok(schema
        .columns
        .iter()
        .map(|column| format!("{sql_alias}.{}", column.name))
        .collect())
}

fn render_scalar_expr(
    expr: &CompiledScalarExpr,
    schemas: &HashMap<String, RuntimeTableSchema>,
    sources: &[Value],
    sql_aliases: &HashMap<String, String>,
    captures: &[Value],
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
        CompiledScalarExpr::OuterColumn { alias, field } => Ok(format!(
            "{}.{}",
            sql_aliases
                .get(alias)
                .ok_or_else(|| RuntimeError::Message(format!(
                    "missing SQL alias for outer '{}'",
                    alias
                )))?,
            field
        )),
        CompiledScalarExpr::Captured { capture_index } => {
            let value = query_capture_value(*capture_index, captures)?;
            render_required_runtime_scalar_literal(value)
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
            render_scalar_expr(expr, schemas, sources, sql_aliases, captures)?
        )),
        CompiledScalarExpr::Binary { op, left, right } => {
            if let Some(sql) =
                render_null_sensitive_binary(op, left, right, schemas, sources, sql_aliases, captures)?
            {
                return Ok(sql);
            }
            let sql_op = match op.as_str() {
                "==" => "=",
                "!=" => "<>",
                "&&" => "AND",
                "||" => "OR",
                other => other,
            };
            Ok(format!(
                "({} {} {})",
                render_scalar_expr(left, schemas, sources, sql_aliases, captures)?,
                sql_op,
                render_scalar_expr(right, schemas, sources, sql_aliases, captures)?
            ))
        }
        CompiledScalarExpr::Aggregate { aggregate, expr } => match aggregate {
            CompiledAggregateFn::Count => Ok("COUNT(*)".to_string()),
            CompiledAggregateFn::Sum => Ok(format!(
                "SUM({})",
                render_scalar_expr(
                    expr.as_ref().expect("SUM aggregate expression"),
                    schemas,
                    sources,
                    sql_aliases,
                    captures
                )?
            )),
            CompiledAggregateFn::Avg => Ok(format!(
                "AVG({})",
                render_scalar_expr(
                    expr.as_ref().expect("AVG aggregate expression"),
                    schemas,
                    sources,
                    sql_aliases,
                    captures
                )?
            )),
            CompiledAggregateFn::Min => Ok(format!(
                "MIN({})",
                render_scalar_expr(
                    expr.as_ref().expect("MIN aggregate expression"),
                    schemas,
                    sources,
                    sql_aliases,
                    captures
                )?
            )),
            CompiledAggregateFn::Max => Ok(format!(
                "MAX({})",
                render_scalar_expr(
                    expr.as_ref().expect("MAX aggregate expression"),
                    schemas,
                    sources,
                    sql_aliases,
                    captures
                )?
            )),
        },
        CompiledScalarExpr::Exists { query } => {
            let nested_schemas = build_runtime_schemas(&query.sources, sources)?;
            let child_sql =
                build_query_sql_with_outer(query, &nested_schemas, sources, captures, sql_aliases)?;
            Ok(format!("EXISTS ({child_sql})"))
        }
    }
}

fn render_required_runtime_scalar_literal(value: &Value) -> Result<String, RuntimeError> {
    render_runtime_scalar_literal(value)?.ok_or_else(|| {
                RuntimeError::Message(
                    "database query capture None is only supported with == and !=".to_string(),
                )
            })
}

fn render_null_sensitive_binary(
    op: &str,
    left: &CompiledScalarExpr,
    right: &CompiledScalarExpr,
    schemas: &HashMap<String, RuntimeTableSchema>,
    sources: &[Value],
    sql_aliases: &HashMap<String, String>,
    captures: &[Value],
) -> Result<Option<String>, RuntimeError> {
    let left_is_null = scalar_expr_is_null_capture(left, captures)?;
    let right_is_null = scalar_expr_is_null_capture(right, captures)?;
    if !left_is_null && !right_is_null {
        return Ok(None);
    }

    match op {
        "==" | "!=" => {}
        _ => {
            return Err(RuntimeError::Message(
                "database query capture None is only supported with == and !=".to_string(),
            ))
        }
    }

    if left_is_null && right_is_null {
        return Ok(Some(
            if op == "==" {
                "TRUE".to_string()
            } else {
                "FALSE".to_string()
            },
        ));
    }

    let non_null = if left_is_null { right } else { left };
    let rendered = render_scalar_expr(non_null, schemas, sources, sql_aliases, captures)?;
    Ok(Some(if op == "==" {
        format!("({rendered} IS NULL)")
    } else {
        format!("({rendered} IS NOT NULL)")
    }))
}

fn scalar_expr_is_null_capture(
    expr: &CompiledScalarExpr,
    captures: &[Value],
) -> Result<bool, RuntimeError> {
    match expr {
        CompiledScalarExpr::Captured { capture_index } => {
            let value = query_capture_value(*capture_index, captures)?;
            Ok(normalize_runtime_scalar(value)?.is_none())
        }
        _ => Ok(false),
    }
}

fn query_capture_value(capture_index: usize, captures: &[Value]) -> Result<&Value, RuntimeError> {
    captures.get(capture_index).ok_or_else(|| {
        RuntimeError::Message(format!(
            "database query capture index {} is out of bounds",
            capture_index
        ))
    })
}

fn normalize_runtime_scalar(value: &Value) -> Result<Option<&Value>, RuntimeError> {
    match value {
        Value::Constructor { name, args } if name == "Some" && args.len() == 1 => {
            normalize_runtime_scalar(&args[0])
        }
        Value::Constructor { name, args } if name == "None" && args.is_empty() => Ok(None),
        Value::Int(_)
        | Value::Float(_)
        | Value::Bool(_)
        | Value::Text(_)
        | Value::DateTime(_) => Ok(Some(value)),
        other => Err(RuntimeError::Message(format!(
            "database query capture is not a supported SQL scalar: {}",
            crate::runtime::format_value(other)
        ))),
    }
}

fn render_runtime_scalar_literal(value: &Value) -> Result<Option<String>, RuntimeError> {
    let Some(value) = normalize_runtime_scalar(value)? else {
        return Ok(None);
    };
    match value {
        Value::Int(value) => Ok(Some(value.to_string())),
        Value::Float(value) => Ok(Some(value.to_string())),
        Value::Bool(value) => {
            if *value {
                Ok(Some("TRUE".to_string()))
            } else {
                Ok(Some("FALSE".to_string()))
            }
        }
        Value::Text(value) | Value::DateTime(value) => {
            Ok(Some(format!("'{}'", value.replace('\'', "''"))))
        }
        _ => unreachable!("normalize_runtime_scalar only returns SQL scalar values"),
    }
}

fn decode_query_rows(
    connection: &aivi_database::DbConnection,
    plan: &CompiledQueryPlan,
    schemas: &HashMap<String, RuntimeTableSchema>,
    sources: &[Value],
    captures: &[Value],
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
            let visible_columns = projection_column_count(&plan.projection, schemas)?;
            let needs_hidden_rows = projection_contains_nested_queries(&plan.projection);
            for row in rows {
                if row.len() < visible_columns {
                    return Err(RuntimeError::Message(
                        "database.query returned fewer columns than expected".to_string(),
                    ));
                }
                let hidden_rows = if needs_hidden_rows {
                    decode_hidden_rows(&plan.sources, schemas, &row[visible_columns..])?
                } else {
                    HashMap::new()
                };
                let decode_ctx = DecodeProjectionCtx {
                    connection,
                    schemas,
                    sources,
                    captures,
                    hidden_rows: &hidden_rows,
                };
                let (value, consumed) = decode_projection(
                    &decode_ctx,
                    &plan.projection,
                    &row[..visible_columns],
                    0,
                )?;
                if consumed != visible_columns {
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
    ctx: &DecodeProjectionCtx<'_>,
    projection: &CompiledProjection,
    row: &[QueryCell],
    start: usize,
) -> Result<(Value, usize), RuntimeError> {
    match projection {
        CompiledProjection::Row { alias, .. } => decode_row_projection(alias, ctx.schemas, row, start),
        CompiledProjection::Scalar { expr } => {
            let cell = row.get(start).ok_or_else(|| {
                RuntimeError::Message("database.query scalar column is missing".to_string())
            })?;
            Ok((decode_scalar_cell(expr, ctx.schemas, ctx.captures, cell)?, start + 1))
        }
        CompiledProjection::Record { fields } => {
            let mut out = HashMap::new();
            let mut cursor = start;
            for field in fields {
                let (value, next) = decode_projection(ctx, &field.value, row, cursor)?;
                out.insert(field.name.clone(), value);
                cursor = next;
            }
            Ok((Value::Record(Arc::new(out)), cursor))
        }
        CompiledProjection::NestedQuery { query } => {
            let (bound_query, bound_captures) =
                bind_outer_columns(query, ctx.captures, ctx.hidden_rows)?;
            let nested_schemas = build_runtime_schemas(&bound_query.sources, ctx.sources)?;
            let sql = build_query_sql(&bound_query, &nested_schemas, ctx.sources, &bound_captures)?;
            let rows = ctx.connection.query_sql(sql).map_err(RuntimeError::Message)?;
            let value = decode_query_rows(
                ctx.connection,
                &bound_query,
                &nested_schemas,
                ctx.sources,
                &bound_captures,
                rows,
            )?;
            let values = expect_list(value, "database.query nested relation")?;
            Ok((list_value(values.iter().cloned().collect()), start))
        }
    }
}

fn schema_for_alias<'a>(
    schemas: &'a HashMap<String, RuntimeTableSchema>,
    alias: &str,
) -> Result<&'a RuntimeTableSchema, RuntimeError> {
    schemas
        .get(alias)
        .ok_or_else(|| RuntimeError::Message(format!("unknown query alias '{}'", alias)))
}

fn column_for_alias_field<'a>(
    schemas: &'a HashMap<String, RuntimeTableSchema>,
    alias: &str,
    field: &str,
) -> Result<&'a RuntimeColumn, RuntimeError> {
    let schema = schema_for_alias(schemas, alias)?;
    schema
        .columns
        .iter()
        .find(|column| column.name == field)
        .ok_or_else(|| RuntimeError::Message(format!("unknown query field '{}.{}'", alias, field)))
}

fn decode_row_projection(
    alias: &str,
    schemas: &HashMap<String, RuntimeTableSchema>,
    row: &[QueryCell],
    start: usize,
) -> Result<(Value, usize), RuntimeError> {
    let schema = schema_for_alias(schemas, alias)?;
    let mut out = HashMap::new();
    let mut cursor = start;
    for column in &schema.columns {
        let cell = row.get(cursor).ok_or_else(|| {
            RuntimeError::Message(format!(
                "database.query row column '{}.{}' is missing",
                schema.name, column.name
            ))
        })?;
        out.insert(column.name.clone(), column_cell_to_value(cell, column)?);
        cursor += 1;
    }
    Ok((Value::Record(Arc::new(out)), cursor))
}

fn decode_scalar_cell(
    expr: &CompiledScalarExpr,
    schemas: &HashMap<String, RuntimeTableSchema>,
    captures: &[Value],
    cell: &QueryCell,
) -> Result<Value, RuntimeError> {
    match expr {
        CompiledScalarExpr::Column { alias, field } => {
            let column = column_for_alias_field(schemas, alias, field)?;
            column_cell_to_value(cell, column)
        }
        _ => cell_to_value(cell, infer_scalar_kind(expr, schemas, captures)?),
    }
}

fn column_cell_to_value(cell: &QueryCell, column: &RuntimeColumn) -> Result<Value, RuntimeError> {
    let value = match cell {
        QueryCell::Null => {
            if column.not_null {
                return Err(RuntimeError::Message(format!(
                    "database.query encountered NULL in NOT NULL column '{}'",
                    column.name
                )));
            }
            return Ok(Value::Constructor {
                name: "None".to_string(),
                args: Vec::new(),
            });
        }
        _ => non_optional_column_cell_to_value(cell, column.kind)?,
    };
    if column.not_null {
        Ok(value)
    } else {
        Ok(Value::Constructor {
            name: "Some".to_string(),
            args: vec![value],
        })
    }
}

fn non_optional_column_cell_to_value(
    cell: &QueryCell,
    kind: QueryColumnType,
) -> Result<Value, RuntimeError> {
    match (cell, kind) {
        (QueryCell::Int(value), QueryColumnType::Int) => Ok(Value::Int(*value)),
        (QueryCell::Int(value), QueryColumnType::Bool) => Ok(Value::Bool(*value != 0)),
        (QueryCell::Float(value), QueryColumnType::Float) => Ok(Value::Float(*value)),
        (QueryCell::Float(value), QueryColumnType::Int) => Ok(Value::Int(*value as i64)),
        (QueryCell::Bool(value), QueryColumnType::Bool) => Ok(Value::Bool(*value)),
        (QueryCell::Text(value), QueryColumnType::Text) => Ok(Value::Text(value.clone())),
        (QueryCell::Text(value), QueryColumnType::Timestamp) => Ok(Value::DateTime(value.clone())),
        (QueryCell::Text(value), QueryColumnType::Int) => value
            .parse::<i64>()
            .map(Value::Int)
            .map_err(|_| RuntimeError::Message(format!("database.query could not decode int from '{value}'"))),
        (QueryCell::Text(value), QueryColumnType::Float) => value
            .parse::<f64>()
            .map(Value::Float)
            .map_err(|_| RuntimeError::Message(format!("database.query could not decode float from '{value}'"))),
        (QueryCell::Text(value), QueryColumnType::Bool) => match value.as_str() {
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

fn infer_scalar_kind(
    expr: &CompiledScalarExpr,
    schemas: &HashMap<String, RuntimeTableSchema>,
    captures: &[Value],
) -> Result<ScalarKind, RuntimeError> {
    match expr {
        CompiledScalarExpr::Column { alias, field } => {
            let column = column_for_alias_field(schemas, alias, field)?;
            Ok(match column.kind {
                QueryColumnType::Int => ScalarKind::Int,
                QueryColumnType::Bool => ScalarKind::Bool,
                QueryColumnType::Float => ScalarKind::Float,
                QueryColumnType::Text | QueryColumnType::Timestamp => ScalarKind::Text,
            })
        }
        CompiledScalarExpr::OuterColumn { .. } => Err(RuntimeError::Message(
            "database.query outer columns are only valid inside correlated subqueries".to_string(),
        )),
        CompiledScalarExpr::Captured { capture_index } => {
            let value = query_capture_value(*capture_index, captures)?;
            match normalize_runtime_scalar(value)? {
                Some(Value::Int(_)) => Ok(ScalarKind::Int),
                Some(Value::Float(_)) => Ok(ScalarKind::Float),
                Some(Value::Bool(_)) => Ok(ScalarKind::Bool),
                Some(Value::Text(_)) | Some(Value::DateTime(_)) => Ok(ScalarKind::Text),
                None => Err(RuntimeError::Message(
                    "database query capture None is only supported with == and !=".to_string(),
                )),
                Some(_) => unreachable!("normalize_runtime_scalar only returns SQL scalar values"),
            }
        }
        CompiledScalarExpr::IntLit { .. } => Ok(ScalarKind::Int),
        CompiledScalarExpr::FloatLit { .. } => Ok(ScalarKind::Float),
        CompiledScalarExpr::TextLit { .. } | CompiledScalarExpr::DateTimeLit { .. } => {
            Ok(ScalarKind::Text)
        }
        CompiledScalarExpr::BoolLit { .. } => Ok(ScalarKind::Bool),
        CompiledScalarExpr::UnaryNeg { expr } => infer_scalar_kind(expr, schemas, captures),
        CompiledScalarExpr::Binary { op, left, right } => match op.as_str() {
            "==" | "!=" | ">" | ">=" | "<" | "<=" | "&&" | "||" => Ok(ScalarKind::Bool),
            "/" => Ok(ScalarKind::Float),
            _ => {
                let left_kind = infer_scalar_kind(left, schemas, captures)?;
                let right_kind = infer_scalar_kind(right, schemas, captures)?;
                if matches!(left_kind, ScalarKind::Float) || matches!(right_kind, ScalarKind::Float) {
                    Ok(ScalarKind::Float)
                } else {
                    Ok(ScalarKind::Int)
                }
            }
        },
        CompiledScalarExpr::Aggregate { aggregate, .. } => Ok(match aggregate {
            CompiledAggregateFn::Count => ScalarKind::Int,
            CompiledAggregateFn::Avg => ScalarKind::Float,
            CompiledAggregateFn::Sum | CompiledAggregateFn::Min | CompiledAggregateFn::Max => {
                ScalarKind::Int
            }
        }),
        CompiledScalarExpr::Exists { .. } => Ok(ScalarKind::Bool),
    }
}

fn projection_contains_nested_queries(projection: &CompiledProjection) -> bool {
    match projection {
        CompiledProjection::NestedQuery { .. } => true,
        CompiledProjection::Record { fields } => fields
            .iter()
            .any(|field| projection_contains_nested_queries(&field.value)),
        _ => false,
    }
}

fn projection_column_count(
    projection: &CompiledProjection,
    schemas: &HashMap<String, RuntimeTableSchema>,
) -> Result<usize, RuntimeError> {
    match projection {
        CompiledProjection::Row { alias, .. } => Ok(schema_for_alias(schemas, alias)?.columns.len()),
        CompiledProjection::Scalar { .. } => Ok(1),
        CompiledProjection::Record { fields } => fields.iter().try_fold(0usize, |acc, field| {
            Ok(acc + projection_column_count(&field.value, schemas)?)
        }),
        CompiledProjection::NestedQuery { .. } => Ok(0),
    }
}

fn decode_hidden_rows(
    sources: &[CompiledQuerySource],
    schemas: &HashMap<String, RuntimeTableSchema>,
    cells: &[QueryCell],
) -> Result<HashMap<String, Value>, RuntimeError> {
    let mut out = HashMap::new();
    let mut cursor = 0usize;
    for source in sources {
        let (row, next) = decode_row_projection(&source.alias, schemas, cells, cursor)?;
        out.insert(source.alias.clone(), row);
        cursor = next;
    }
    if cursor != cells.len() {
        return Err(RuntimeError::Message(
            "database.query hidden projection columns did not line up with source rows".to_string(),
        ));
    }
    Ok(out)
}

fn bind_outer_columns(
    plan: &CompiledQueryPlan,
    captures: &[Value],
    hidden_rows: &HashMap<String, Value>,
) -> Result<(CompiledQueryPlan, Vec<Value>), RuntimeError> {
    let mut extra = Vec::new();
    let next = bind_outer_columns_in_plan(plan, captures.len(), &mut extra, hidden_rows)?;
    let mut combined = captures.to_vec();
    combined.extend(extra);
    Ok((next, combined))
}

fn bind_outer_columns_in_plan(
    plan: &CompiledQueryPlan,
    base_capture_index: usize,
    extra: &mut Vec<Value>,
    hidden_rows: &HashMap<String, Value>,
) -> Result<CompiledQueryPlan, RuntimeError> {
    let mut next = plan.clone();
    next.filters = next
        .filters
        .iter()
        .map(|expr| bind_outer_columns_in_scalar(expr, base_capture_index, extra, hidden_rows))
        .collect::<Result<Vec<_>, _>>()?;
    next.order_by = next
        .order_by
        .iter()
        .map(|order| {
            Ok(CompiledOrderBy {
                expr: bind_outer_columns_in_scalar(&order.expr, base_capture_index, extra, hidden_rows)?,
                descending: order.descending,
            })
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    next.group_by = next
        .group_by
        .iter()
        .map(|expr| bind_outer_columns_in_scalar(expr, base_capture_index, extra, hidden_rows))
        .collect::<Result<Vec<_>, _>>()?;
    next.having = next
        .having
        .iter()
        .map(|expr| bind_outer_columns_in_scalar(expr, base_capture_index, extra, hidden_rows))
        .collect::<Result<Vec<_>, _>>()?;
    next.projection = bind_outer_columns_in_projection(
        &next.projection,
        base_capture_index,
        extra,
        hidden_rows,
    )?;
    Ok(next)
}

fn bind_outer_columns_in_projection(
    projection: &CompiledProjection,
    base_capture_index: usize,
    extra: &mut Vec<Value>,
    hidden_rows: &HashMap<String, Value>,
) -> Result<CompiledProjection, RuntimeError> {
    match projection {
        CompiledProjection::Row {
            alias,
            relation_name,
            links,
        } => Ok(CompiledProjection::Row {
            alias: alias.clone(),
            relation_name: relation_name.clone(),
            links: links.clone(),
        }),
        CompiledProjection::Scalar { expr } => Ok(CompiledProjection::Scalar {
            expr: bind_outer_columns_in_scalar(expr, base_capture_index, extra, hidden_rows)?,
        }),
        CompiledProjection::Record { fields } => Ok(CompiledProjection::Record {
            fields: fields
                .iter()
                .map(|field| {
                    Ok(CompiledProjectionField {
                        name: field.name.clone(),
                        value: bind_outer_columns_in_projection(
                            &field.value,
                            base_capture_index,
                            extra,
                            hidden_rows,
                        )?,
                    })
                })
                .collect::<Result<Vec<_>, RuntimeError>>()?,
        }),
        CompiledProjection::NestedQuery { query } => Ok(CompiledProjection::NestedQuery {
            query: Box::new(bind_outer_columns_in_plan(
                query,
                base_capture_index,
                extra,
                hidden_rows,
            )?),
        }),
    }
}

fn bind_outer_columns_in_scalar(
    expr: &CompiledScalarExpr,
    base_capture_index: usize,
    extra: &mut Vec<Value>,
    hidden_rows: &HashMap<String, Value>,
) -> Result<CompiledScalarExpr, RuntimeError> {
    match expr {
        CompiledScalarExpr::OuterColumn { alias, field } => {
            let row = hidden_rows.get(alias).ok_or_else(|| {
                RuntimeError::Message(format!(
                    "database.query missing hidden row for outer alias '{}'",
                    alias
                ))
            })?;
            let fields = expect_record(row.clone(), "database.query hidden outer row")?;
            let value = fields.get(field).cloned().ok_or_else(|| {
                RuntimeError::Message(format!(
                    "database.query hidden outer row '{}' has no field '{}'",
                    alias, field
                ))
            })?;
            let index = base_capture_index + extra.len();
            extra.push(value);
            Ok(CompiledScalarExpr::Captured { capture_index: index })
        }
        CompiledScalarExpr::UnaryNeg { expr } => Ok(CompiledScalarExpr::UnaryNeg {
            expr: Box::new(bind_outer_columns_in_scalar(
                expr,
                base_capture_index,
                extra,
                hidden_rows,
            )?),
        }),
        CompiledScalarExpr::Binary { op, left, right } => Ok(CompiledScalarExpr::Binary {
            op: op.clone(),
            left: Box::new(bind_outer_columns_in_scalar(
                left,
                base_capture_index,
                extra,
                hidden_rows,
            )?),
            right: Box::new(bind_outer_columns_in_scalar(
                right,
                base_capture_index,
                extra,
                hidden_rows,
            )?),
        }),
        CompiledScalarExpr::Aggregate { aggregate, expr } => Ok(CompiledScalarExpr::Aggregate {
            aggregate: aggregate.clone(),
            expr: expr
                .as_ref()
                .map(|expr| {
                    bind_outer_columns_in_scalar(expr, base_capture_index, extra, hidden_rows)
                        .map(Box::new)
                })
                .transpose()?,
        }),
        CompiledScalarExpr::Exists { query } => Ok(CompiledScalarExpr::Exists {
            query: Box::new(bind_outer_columns_in_plan(
                query,
                base_capture_index,
                extra,
                hidden_rows,
            )?),
        }),
        other => Ok(other.clone()),
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

fn build_query_storage_table(
    name: String,
    columns_value: Value,
    rows: &[Value],
) -> Result<QueryTable, RuntimeError> {
    let runtime_columns = {
        let parsed = parse_runtime_columns(columns_value)?;
        if parsed.is_empty() {
            infer_runtime_columns(rows)?
        } else {
            parsed
        }
    };
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
            match row_fields.get(&column.name) {
                Some(value) => values.push(runtime_value_to_query_cell(
                    value,
                    column.kind,
                    column.not_null,
                )?),
                None if !column.not_null => values.push(QueryCell::Null),
                None => {
                    return Err(RuntimeError::Message(format!(
                        "database.query mirror row is missing field '{}'",
                        column.name
                    )))
                }
            }
        }
        query_rows.push(QueryRow {
            row_ordinal: index as i64,
            row_json: encode_json(row)?,
            values,
        });
    }
    Ok(QueryTable {
        name,
        columns,
        rows: query_rows,
    })
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

fn load_rows_from_storage(
    connection: &aivi_database::DbConnection,
    name: &str,
) -> Result<Vec<Value>, RuntimeError> {
    if connection
        .load_table(name.to_string())
        .map_err(RuntimeError::Message)?
        .is_none()
    {
        return Ok(Vec::new());
    }
    let rows = connection
        .query_sql(format!(
            "SELECT __aivi_row_json FROM {} ORDER BY __aivi_ord",
            aivi_database::query_storage_name(name)
        ))
        .map_err(RuntimeError::Message)?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let json = match row.as_slice() {
            [QueryCell::Text(json)] => json.as_str(),
            other => {
                return Err(RuntimeError::Message(format!(
                    "database.query storage read returned unexpected row shape: {other:?}"
                )))
            }
        };
        out.push(decode_json(json)?);
    }
    Ok(out)
}

#[cfg(test)]
mod query_tests {
    use super::*;

    fn test_connection() -> aivi_database::DbConnection {
        let state = aivi_database::DatabaseState::new();
        let connection = state
            .connect(aivi_database::Driver::Sqlite, ":memory:".to_string())
            .expect("sqlite connection");
        connection.ensure_schema().expect("ensure schema");
        connection
    }

    fn single_text_schema(not_null: bool) -> HashMap<String, RuntimeTableSchema> {
        HashMap::from([(
            "p".to_string(),
            RuntimeTableSchema {
                name: "products".to_string(),
                storage_name: "__aivi_query_storage_products".to_string(),
                columns: vec![RuntimeColumn {
                    name: "email".to_string(),
                    kind: QueryColumnType::Text,
                    not_null,
                }],
            },
        )])
    }

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
    fn render_scalar_expr_unwraps_some_capture() {
        let sql = render_scalar_expr(
            &CompiledScalarExpr::Captured { capture_index: 0 },
            &single_text_schema(false),
            &[],
            &HashMap::new(),
            &[Value::Constructor {
                name: "Some".to_string(),
                args: vec![Value::Text("hello".to_string())],
            }],
        )
        .unwrap_or_else(|_| panic!("Some capture should render"));

        assert_eq!(sql, "'hello'");
    }

    #[test]
    fn render_scalar_expr_uses_is_null_for_none_capture_equality() {
        let sql = render_scalar_expr(
            &CompiledScalarExpr::Binary {
                op: "==".to_string(),
                left: Box::new(CompiledScalarExpr::Column {
                    alias: "p".to_string(),
                    field: "email".to_string(),
                }),
                right: Box::new(CompiledScalarExpr::Captured { capture_index: 0 }),
            },
            &single_text_schema(false),
            &[],
            &HashMap::from([("p".to_string(), "t0".to_string())]),
            &[Value::Constructor {
                name: "None".to_string(),
                args: Vec::new(),
            }],
        )
        .unwrap_or_else(|_| panic!("None equality should render"));

        assert_eq!(sql, "(t0.email IS NULL)");
    }

    #[test]
    fn render_scalar_expr_uses_is_not_null_for_none_capture_inequality() {
        let sql = render_scalar_expr(
            &CompiledScalarExpr::Binary {
                op: "!=".to_string(),
                left: Box::new(CompiledScalarExpr::Column {
                    alias: "p".to_string(),
                    field: "email".to_string(),
                }),
                right: Box::new(CompiledScalarExpr::Captured { capture_index: 0 }),
            },
            &single_text_schema(false),
            &[],
            &HashMap::from([("p".to_string(), "t0".to_string())]),
            &[Value::Constructor {
                name: "None".to_string(),
                args: Vec::new(),
            }],
        )
        .unwrap_or_else(|_| panic!("None inequality should render"));

        assert_eq!(sql, "(t0.email IS NOT NULL)");
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

    #[test]
    fn parse_runtime_columns_accepts_float_type() {
        let columns = list_value(vec![Value::Record(Arc::new(HashMap::from([
            ("name".to_string(), Value::Text("amount".to_string())),
            (
                "type".to_string(),
                Value::Constructor {
                    name: "FloatType".to_string(),
                    args: Vec::new(),
                },
            ),
            ("constraints".to_string(), list_value(Vec::new())),
            (
                "default".to_string(),
                Value::Constructor {
                    name: "None".to_string(),
                    args: Vec::new(),
                },
            ),
        ])))]);

        let parsed = parse_runtime_columns(columns)
            .unwrap_or_else(|_| panic!("FloatType column should parse"));

        assert_eq!(parsed.len(), 1);
        assert!(matches!(parsed[0].kind, QueryColumnType::Float));
    }

    #[test]
    fn render_select_projection_expands_row_columns() {
        let mut schemas = HashMap::new();
        schemas.insert(
            "p".to_string(),
            RuntimeTableSchema {
                name: "products".to_string(),
                storage_name: "__aivi_query_storage_products".to_string(),
                columns: vec![
                    RuntimeColumn {
                        name: "id".to_string(),
                        kind: QueryColumnType::Int,
                        not_null: true,
                    },
                    RuntimeColumn {
                        name: "name".to_string(),
                        kind: QueryColumnType::Text,
                        not_null: true,
                    },
                ],
            },
        );
        let sql_aliases = HashMap::from([("p".to_string(), "t0".to_string())]);

        let sql = render_select_projection(
            &CompiledProjection::Row {
                alias: "p".to_string(),
                relation_name: "products".to_string(),
                links: Vec::new(),
            },
            &schemas,
            &[],
            &sql_aliases,
            &[],
        )
        .unwrap_or_else(|_| panic!("row projection should render"));

        assert_eq!(sql, vec!["t0.id".to_string(), "t0.name".to_string()]);
    }

    #[test]
    fn decode_row_projection_reconstructs_option_and_timestamp_fields() {
        let connection = test_connection();
        let mut schemas = HashMap::new();
        schemas.insert(
            "p".to_string(),
            RuntimeTableSchema {
                name: "products".to_string(),
                storage_name: "__aivi_query_storage_products".to_string(),
                columns: vec![
                    RuntimeColumn {
                        name: "id".to_string(),
                        kind: QueryColumnType::Int,
                        not_null: true,
                    },
                    RuntimeColumn {
                        name: "email".to_string(),
                        kind: QueryColumnType::Text,
                        not_null: false,
                    },
                    RuntimeColumn {
                        name: "createdAt".to_string(),
                        kind: QueryColumnType::Timestamp,
                        not_null: true,
                    },
                ],
            },
        );

        let row = vec![
            QueryCell::Int(1),
            QueryCell::Text("a@example.com".to_string()),
            QueryCell::Text("2024-01-02 03:04:05.000000".to_string()),
        ];
        let hidden_rows = HashMap::new();
        let decode_ctx = DecodeProjectionCtx {
            connection: &connection,
            schemas: &schemas,
            sources: &[],
            captures: &[],
            hidden_rows: &hidden_rows,
        };
        let (value, consumed) = decode_projection(
            &decode_ctx,
            &CompiledProjection::Row {
                alias: "p".to_string(),
                relation_name: "products".to_string(),
                links: Vec::new(),
            },
            &row,
            0,
        )
        .unwrap_or_else(|_| panic!("row projection should decode"));

        assert_eq!(consumed, 3);
        let fields = expect_record(value, "row projection test")
            .unwrap_or_else(|_| panic!("record result"));
        assert!(matches!(fields.get("id"), Some(Value::Int(1))));
        assert!(matches!(
            fields.get("email"),
            Some(Value::Constructor { name, args }) if name == "Some"
                && matches!(args.as_slice(), [Value::Text(value)] if value == "a@example.com")
        ));
        assert!(matches!(
            fields.get("createdAt"),
            Some(Value::DateTime(value)) if value == "2024-01-02 03:04:05.000000"
        ));
    }

    #[test]
    fn decode_scalar_projection_of_nullable_column_returns_none() {
        let connection = test_connection();
        let mut schemas = HashMap::new();
        schemas.insert(
            "p".to_string(),
            RuntimeTableSchema {
                name: "products".to_string(),
                storage_name: "__aivi_query_storage_products".to_string(),
                columns: vec![RuntimeColumn {
                    name: "email".to_string(),
                    kind: QueryColumnType::Text,
                    not_null: false,
                }],
            },
        );

        let hidden_rows = HashMap::new();
        let decode_ctx = DecodeProjectionCtx {
            connection: &connection,
            schemas: &schemas,
            sources: &[],
            captures: &[],
            hidden_rows: &hidden_rows,
        };
        let (value, consumed) = decode_projection(
            &decode_ctx,
            &CompiledProjection::Scalar {
                expr: CompiledScalarExpr::Column {
                    alias: "p".to_string(),
                    field: "email".to_string(),
                },
            },
            &[QueryCell::Null],
            0,
        )
        .unwrap_or_else(|_| panic!("nullable scalar column should decode"));

        assert_eq!(consumed, 1);
        assert!(matches!(
            value,
            Value::Constructor { name, args } if name == "None" && args.is_empty()
        ));
    }
}
