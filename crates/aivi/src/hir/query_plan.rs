use std::collections::{HashMap, HashSet};

pub(crate) const DB_QUERY_COMPILED_BUILTIN: &str = "__db_query_compiled";
pub(crate) const DB_QUERY_COUNT_BUILTIN: &str = "__db_query_count";
pub(crate) const DB_QUERY_EXISTS_BUILTIN: &str = "__db_query_exists";
pub(crate) const DB_QUERY_ERROR_BUILTIN: &str = "__db_query_error";

const DB_MODULE: &str = "aivi.database";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledQueryPlan {
    pub sources: Vec<CompiledQuerySource>,
    pub filters: Vec<CompiledScalarExpr>,
    pub projection: CompiledProjection,
    pub order_by: Vec<CompiledOrderBy>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub aggregate: CompiledAggregate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledQuerySource {
    pub alias: String,
    pub source_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledOrderBy {
    pub expr: CompiledScalarExpr,
    pub descending: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum CompiledAggregate {
    None,
    Count,
    Exists,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum CompiledProjection {
    Row {
        alias: String,
    },
    Scalar {
        expr: CompiledScalarExpr,
    },
    Record {
        fields: Vec<CompiledProjectionField>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledProjectionField {
    pub name: String,
    pub value: CompiledProjection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum CompiledScalarExpr {
    Column {
        alias: String,
        field: String,
    },
    IntLit {
        value: i64,
    },
    FloatLit {
        value: f64,
    },
    TextLit {
        value: String,
    },
    BoolLit {
        value: bool,
    },
    DateTimeLit {
        value: String,
    },
    UnaryNeg {
        expr: Box<CompiledScalarExpr>,
    },
    Binary {
        op: String,
        left: Box<CompiledScalarExpr>,
        right: Box<CompiledScalarExpr>,
    },
}

#[derive(Debug, Clone)]
pub struct StaticCompiledQuery {
    pub plan: CompiledQueryPlan,
    pub source_exprs: Vec<Expr>,
}

#[derive(Debug, Clone, Default)]
struct QueryCompileEnv {
    aliases: HashSet<String>,
    lets: HashMap<String, CompiledProjection>,
}

pub(crate) fn compile_static_query(expr: &Expr) -> Result<StaticCompiledQuery, String> {
    compile_static_query_with_env(expr, &QueryCompileEnv::default(), None)
}

pub(crate) fn is_database_helper(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Ident(crate::surface::SpannedName { name: ident, .. }) => {
            ident == &format!("{DB_MODULE}.{name}") || ident == name
        }
        _ => false,
    }
}

fn database_helper_name(expr: &Expr) -> Option<&str> {
    let Expr::Ident(crate::surface::SpannedName { name, .. }) = expr else {
        return None;
    };
    let local = name.strip_prefix(&format!("{DB_MODULE}.")).unwrap_or(name);
    match local {
        "from" | "where_" | "select" | "orderBy" | "limit" | "offset" | "count"
        | "exists" | "queryOf" | "guard_" => Some(local),
        _ => None,
    }
}

fn flatten_call_args<'a>(expr: &'a Expr, out: &mut Vec<&'a Expr>) -> &'a Expr {
    match expr {
        Expr::Call { func, args, .. } => {
            let head = flatten_call_args(func, out);
            out.extend(args.iter());
            head
        }
        _ => expr,
    }
}

fn database_helper_invocation(expr: &Expr) -> Option<(&str, Vec<&Expr>)> {
    let mut args = Vec::new();
    let head = flatten_call_args(expr, &mut args);
    Some((database_helper_name(head)?, args))
}

pub(crate) fn is_query_do_block(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Block {
            kind: crate::surface::BlockKind::Do { monad },
            ..
        } if monad.name == "Query"
    )
}

fn compile_static_query_with_env(
    expr: &Expr,
    env: &QueryCompileEnv,
    base: Option<StaticCompiledQuery>,
) -> Result<StaticCompiledQuery, String> {
    if let Some((helper, args)) = database_helper_invocation(expr) {
        match helper {
            "from" => {
                if args.len() != 1 {
                    return Err("db.from expects exactly one table argument".to_string());
                }
                if base.is_some() {
                    return Err("db.from cannot appear after query composition has started".to_string());
                }
                let alias = format!("q{}", 0usize);
                return Ok(StaticCompiledQuery {
                    plan: CompiledQueryPlan {
                        sources: vec![CompiledQuerySource {
                            alias: alias.clone(),
                            source_index: 0,
                        }],
                        filters: Vec::new(),
                        projection: CompiledProjection::Row { alias },
                        order_by: Vec::new(),
                        limit: None,
                        offset: None,
                        aggregate: CompiledAggregate::None,
                    },
                    source_exprs: vec![args[0].clone()],
                });
            }
            "where_" => {
                if args.len() != 2 {
                    return Err("db.where_ expects predicate and query".to_string());
                }
                let mut inner = compile_static_query_with_env(args[1], env, base)?;
                let pred = compile_lambda_scalar(args[0], &inner.plan.projection)?;
                inner.plan.filters.push(pred);
                return Ok(inner);
            }
            "select" => {
                if args.len() != 2 {
                    return Err("db.select expects mapper and query".to_string());
                }
                let mut inner = compile_static_query_with_env(args[1], env, base)?;
                inner.plan.projection = compile_lambda_projection(args[0], &inner.plan.projection)?;
                return Ok(inner);
            }
            "orderBy" => {
                if args.len() != 2 {
                    return Err("db.orderBy expects key function and query".to_string());
                }
                let mut inner = compile_static_query_with_env(args[1], env, base)?;
                let key = compile_lambda_scalar(args[0], &inner.plan.projection)?;
                inner.plan.order_by.push(CompiledOrderBy {
                    expr: key,
                    descending: false,
                });
                return Ok(inner);
            }
            "limit" => {
                if args.len() != 2 {
                    return Err("db.limit expects n and query".to_string());
                }
                let mut inner = compile_static_query_with_env(args[1], env, base)?;
                inner.plan.limit = Some(compile_const_int(args[0], env)?);
                return Ok(inner);
            }
            "offset" => {
                if args.len() != 2 {
                    return Err("db.offset expects n and query".to_string());
                }
                let mut inner = compile_static_query_with_env(args[1], env, base)?;
                inner.plan.offset = Some(compile_const_int(args[0], env)?);
                return Ok(inner);
            }
            "count" => {
                if args.len() != 1 {
                    return Err("db.count expects query".to_string());
                }
                let mut inner = compile_static_query_with_env(args[0], env, base)?;
                inner.plan.aggregate = CompiledAggregate::Count;
                return Ok(inner);
            }
            "exists" => {
                if args.len() != 1 {
                    return Err("db.exists expects query".to_string());
                }
                let mut inner = compile_static_query_with_env(args[0], env, base)?;
                inner.plan.aggregate = CompiledAggregate::Exists;
                return Ok(inner);
            }
            "queryOf" => {
                if args.len() != 1 {
                    return Err("db.queryOf expects one value".to_string());
                }
                let mut inner = base.ok_or_else(|| {
                    "db.queryOf only lowers to SQL inside a query that already has a source".to_string()
                })?;
                inner.plan.projection = compile_value_expr(args[0], env)?;
                return Ok(inner);
            }
            _ => {}
        }
    }

    match expr {
        Expr::Block {
            kind: crate::surface::BlockKind::Do { monad },
            items,
            ..
        } if monad.name == "Query" => compile_query_do_block(items),
        _ => Err("expression is outside the lowered SQL-backed Query subset".to_string()),
    }
}

fn compile_query_do_block(items: &[crate::surface::BlockItem]) -> Result<StaticCompiledQuery, String> {
    let mut env = QueryCompileEnv::default();
    let mut source_exprs = Vec::new();
    let mut sources = Vec::new();
    let mut filters = Vec::new();

    for (index, item) in items.iter().enumerate() {
        let is_last = index + 1 == items.len();
        match item {
            BlockItem::Bind { pattern, expr, .. } => {
                if is_last {
                    return Err("a final query bind is not supported".to_string());
                }
                let alias = match pattern {
                    Pattern::Ident(name) | Pattern::SubjectIdent(name) => name.name.clone(),
                    _ => return Err("query bindings must use simple identifiers".to_string()),
                };
                let Expr::Call { func, args, .. } = expr else {
                    return Err("query bindings must come from db.from".to_string());
                };
                if !is_database_helper(func, "from") || args.len() != 1 {
                    return Err("query bindings must come from db.from table".to_string());
                }
                let source_index = source_exprs.len();
                source_exprs.push(args[0].clone());
                sources.push(CompiledQuerySource {
                    alias: alias.clone(),
                    source_index,
                });
                env.aliases.insert(alias);
            }
            BlockItem::Let { pattern, expr, .. } => {
                if is_last {
                    return Err("a final query let-binding is not supported".to_string());
                }
                let name = match pattern {
                    Pattern::Ident(name) | Pattern::SubjectIdent(name) => name.name.clone(),
                    _ => return Err("query let-bindings must use simple identifiers".to_string()),
                };
                let value = compile_value_expr(expr, &env)?;
                env.lets.insert(name, value);
            }
            BlockItem::Expr { expr, .. } if !is_last && is_guard_call(expr) => {
                let arg = guard_arg(expr).expect("guard_ arg should exist");
                filters.push(compile_scalar_expr(arg, &env)?);
            }
            BlockItem::Expr { expr, .. } if is_last => {
                let base = StaticCompiledQuery {
                    plan: CompiledQueryPlan {
                        sources,
                        filters,
                        projection: CompiledProjection::Record { fields: Vec::new() },
                        order_by: Vec::new(),
                        limit: None,
                        offset: None,
                        aggregate: CompiledAggregate::None,
                    },
                    source_exprs,
                };
                let lowered = compile_static_query_with_env(expr, &env, Some(base))?;
                return match lowered.plan.projection {
                    CompiledProjection::Record { ref fields } if fields.is_empty() => {
                        Err("query blocks must finish with db.queryOf or a query helper around db.queryOf".to_string())
                    }
                    _ => Ok(lowered),
                };
            }
            BlockItem::Expr { .. } => {
                return Err("only db.guard_ may appear before the final query expression".to_string())
            }
            _ => {
                return Err(
                    "this do Query form is not in the lowered SQL-backed subset".to_string(),
                )
            }
        }
    }

    Err("empty do Query blocks are not supported by the SQL-backed lowering".to_string())
}

fn is_guard_call(expr: &Expr) -> bool {
    matches!(database_helper_invocation(expr), Some(("guard_", args)) if args.len() == 1)
}

fn guard_arg(expr: &Expr) -> Option<&Expr> {
    match database_helper_invocation(expr) {
        Some(("guard_", args)) if args.len() == 1 => Some(args[0]),
        _ => None,
    }
}

fn compile_lambda_projection(
    expr: &Expr,
    input: &CompiledProjection,
) -> Result<CompiledProjection, String> {
    match expr {
        Expr::FieldSection { field, .. } => project_field(input, &field.name),
        Expr::Lambda { params, body, .. } if params.len() == 1 => {
            let mut env = QueryCompileEnv::default();
            bind_lambda_param(&mut env, &params[0], input.clone())?;
            compile_value_expr(body, &env)
        }
        _ => Err("query helper lambda is not in the lowered SQL-backed subset".to_string()),
    }
}

fn compile_lambda_scalar(expr: &Expr, input: &CompiledProjection) -> Result<CompiledScalarExpr, String> {
    let projection = compile_lambda_projection(expr, input)?;
    projection_into_scalar(projection)
}

fn bind_lambda_param(
    env: &mut QueryCompileEnv,
    pattern: &Pattern,
    value: CompiledProjection,
) -> Result<(), String> {
    match pattern {
        Pattern::Ident(name) | Pattern::SubjectIdent(name) => {
            env.lets.insert(name.name.clone(), value);
            Ok(())
        }
        Pattern::Wildcard(_) => Ok(()),
        _ => Err("query helper lambdas must use a simple identifier parameter".to_string()),
    }
}

fn compile_const_int(expr: &Expr, env: &QueryCompileEnv) -> Result<i64, String> {
    match compile_scalar_expr(expr, env)? {
        CompiledScalarExpr::IntLit { value } => Ok(value),
        _ => Err("db.limit/db.offset currently require an integer literal or let-bound integer".to_string()),
    }
}

fn compile_value_expr(expr: &Expr, env: &QueryCompileEnv) -> Result<CompiledProjection, String> {
    match expr {
        Expr::Ident(name) => {
            if let Some(value) = env.lets.get(&name.name) {
                return Ok(value.clone());
            }
            if env.aliases.contains(&name.name) {
                return Ok(CompiledProjection::Row {
                    alias: name.name.clone(),
                });
            }
            Err(format!(
                "query expression references unsupported identifier '{}'",
                name.name
            ))
        }
        Expr::FieldAccess { base, field, .. } => {
            let base = compile_value_expr(base, env)?;
            project_field(&base, &field.name)
        }
        Expr::Record { fields, .. } => compile_record_projection(fields, env),
        Expr::Literal(literal) => Ok(CompiledProjection::Scalar {
            expr: compile_literal_scalar(literal)?,
        }),
        Expr::UnaryNeg { expr, .. } => Ok(CompiledProjection::Scalar {
            expr: CompiledScalarExpr::UnaryNeg {
                expr: Box::new(compile_scalar_expr(expr, env)?),
            },
        }),
        Expr::Binary { op, left, right, .. } => Ok(CompiledProjection::Scalar {
            expr: CompiledScalarExpr::Binary {
                op: op.clone(),
                left: Box::new(compile_scalar_expr(left, env)?),
                right: Box::new(compile_scalar_expr(right, env)?),
            },
        }),
        _ => Err("query expression is not in the lowered SQL-backed subset".to_string()),
    }
}

fn compile_record_projection(
    fields: &[crate::surface::RecordField],
    env: &QueryCompileEnv,
) -> Result<CompiledProjection, String> {
    let mut compiled = Vec::with_capacity(fields.len());
    for field in fields {
        if field.spread {
            return Err("record spreads are not supported in lowered queries".to_string());
        }
        if field.path.len() != 1 {
            return Err("record projection fields must have a single field name".to_string());
        }
        let field_name = match &field.path[0] {
            crate::surface::PathSegment::Field(name) => name.name.clone(),
            _ => {
                return Err(
                    "record projection fields must use plain field names in lowered queries"
                        .to_string(),
                )
            }
        };
        compiled.push(CompiledProjectionField {
            name: field_name,
            value: compile_value_expr(&field.value, env)?,
        });
    }
    Ok(CompiledProjection::Record { fields: compiled })
}

fn compile_scalar_expr(expr: &Expr, env: &QueryCompileEnv) -> Result<CompiledScalarExpr, String> {
    let projection = compile_value_expr(expr, env)?;
    projection_into_scalar(projection)
}

fn projection_into_scalar(projection: CompiledProjection) -> Result<CompiledScalarExpr, String> {
    match projection {
        CompiledProjection::Scalar { expr } => Ok(expr),
        CompiledProjection::Row { .. } => Err("row values are not valid scalar SQL expressions".to_string()),
        CompiledProjection::Record { .. } => {
            Err("record values are not valid scalar SQL expressions".to_string())
        }
    }
}

fn project_field(base: &CompiledProjection, field_name: &str) -> Result<CompiledProjection, String> {
    match base {
        CompiledProjection::Row { alias } => Ok(CompiledProjection::Scalar {
            expr: CompiledScalarExpr::Column {
                alias: alias.clone(),
                field: field_name.to_string(),
            },
        }),
        CompiledProjection::Record { fields } => fields
            .iter()
            .find(|field| field.name == field_name)
            .map(|field| field.value.clone())
            .ok_or_else(|| format!("record projection has no field '{field_name}'")),
        CompiledProjection::Scalar { .. } => {
            Err("field access on scalar query expressions is not supported".to_string())
        }
    }
}

fn compile_literal_scalar(
    literal: &crate::surface::Literal,
) -> Result<CompiledScalarExpr, String> {
    match literal {
        crate::surface::Literal::Number { text, .. } => {
            if let Ok(value) = text.parse::<i64>() {
                Ok(CompiledScalarExpr::IntLit { value })
            } else if let Ok(value) = text.parse::<f64>() {
                Ok(CompiledScalarExpr::FloatLit { value })
            } else {
                Err(format!("unsupported numeric literal '{text}' in lowered query"))
            }
        }
        crate::surface::Literal::String { text, .. } => Ok(CompiledScalarExpr::TextLit {
            value: text.clone(),
        }),
        crate::surface::Literal::Bool { value, .. } => {
            Ok(CompiledScalarExpr::BoolLit { value: *value })
        }
        crate::surface::Literal::DateTime { text, .. } => Ok(CompiledScalarExpr::DateTimeLit {
            value: text.clone(),
        }),
        crate::surface::Literal::Sigil { .. } => {
            Err("sigil literals are not supported in lowered queries".to_string())
        }
    }
}
