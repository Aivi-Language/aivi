use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

pub(crate) const DB_QUERY_COMPILED_BUILTIN: &str = "__db_query_compiled";
pub(crate) const DB_QUERY_ERROR_BUILTIN: &str = "__db_query_error";
pub(crate) const DB_PATCH_COMPILED_BUILTIN: &str = "__db_patch_compiled";
pub(crate) const DB_PATCH_ERROR_BUILTIN: &str = "__db_patch_error";

#[derive(Debug, Clone)]
pub struct SurfaceDbIndex {
    defs: HashMap<String, Expr>,
    relations: HashMap<String, Expr>,
}

impl SurfaceDbIndex {
    fn resolve_def(&self, current_module: &str, name: &str) -> Option<&Expr> {
        self.defs
            .get(name)
            .or_else(|| self.defs.get(&format!("{current_module}.{name}")))
    }

    fn resolve_relation(&self, name: &str) -> Option<&Expr> {
        self.relations.get(name)
    }
}

pub(crate) fn build_surface_db_index(modules: &[crate::surface::Module]) -> SurfaceDbIndex {
    let mut defs = HashMap::new();
    for module in modules {
        for def in collect_surface_defs(module) {
            defs.insert(format!("{}.{}", module.name.name, def.name.name), def.expr);
        }
    }

    let empty_index = SurfaceDbIndex {
        defs: defs.clone(),
        relations: HashMap::new(),
    };
    let mut relations = HashMap::new();
    for (symbol, expr) in &defs {
        let current_module = symbol
            .rsplit_once('.')
            .map(|(module, _)| module)
            .unwrap_or("");
        if let Some(meta) = extract_relation_decl(expr, &empty_index, current_module) {
            relations
                .entry(meta.relation_name.clone())
                .or_insert_with(|| expr.clone());
        }
    }

    SurfaceDbIndex { defs, relations }
}

fn surface_expr_dotted_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Ident(name) => Some(name.name.clone()),
        Expr::FieldAccess { base, field, .. } => Some(format!(
            "{}.{}",
            surface_expr_dotted_name(base)?,
            field.name
        )),
        _ => None,
    }
}

fn flatten_surface_call_args<'a>(expr: &'a Expr, out: &mut Vec<&'a Expr>) -> &'a Expr {
    match expr {
        Expr::Call { func, args, .. } => {
            let head = flatten_surface_call_args(func, out);
            out.extend(args.iter());
            head
        }
        _ => expr,
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledQueryPlan {
    pub sources: Vec<CompiledQuerySource>,
    pub filters: Vec<CompiledScalarExpr>,
    pub projection: CompiledProjection,
    pub order_by: Vec<CompiledOrderBy>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub aggregate: CompiledAggregate,
    pub distinct: bool,
    pub group_by: Vec<CompiledScalarExpr>,
    pub having: Vec<CompiledScalarExpr>,
    pub group_key: Option<CompiledProjection>,
    pub group_source: Option<CompiledProjection>,
    pub grouped_projection: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    relation_query: Option<RelationQueryPlanMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledQuerySource {
    pub alias: String,
    pub source_index: usize,
    pub relation_name: String,
    pub links: Vec<CompiledRelationLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledRelationLink {
    pub name: String,
    pub target_relation_name: String,
    pub source_field: String,
    pub target_field: String,
    pub many: bool,
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
pub enum CompiledAggregateFn {
    Count,
    Sum,
    Avg,
    Min,
    Max,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum CompiledProjection {
    Row {
        alias: String,
        relation_name: String,
        links: Vec<CompiledRelationLink>,
    },
    Scalar {
        expr: CompiledScalarExpr,
    },
    Record {
        fields: Vec<CompiledProjectionField>,
    },
    NestedQuery {
        query: Box<CompiledQueryPlan>,
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
    OuterColumn {
        alias: String,
        field: String,
    },
    Captured {
        capture_index: usize,
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
    Aggregate {
        aggregate: CompiledAggregateFn,
        expr: Option<Box<CompiledScalarExpr>>,
    },
    Exists {
        query: Box<CompiledQueryPlan>,
    },
}

#[derive(Debug, Clone)]
pub struct StaticCompiledQuery {
    pub plan: CompiledQueryPlan,
    pub source_exprs: Vec<Expr>,
    pub capture_exprs: Vec<Expr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledDbPatchPlan {
    pub fields: Vec<CompiledDbPatchField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledDbPatchField {
    pub field: String,
    pub value: CompiledScalarExpr,
}

#[derive(Debug, Clone)]
pub struct StaticCompiledDbPatch {
    pub plan: CompiledDbPatchPlan,
    pub capture_exprs: Vec<Expr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RelationQueryPlanMeta {
    relation_name: String,
    relation_symbol: String,
    alias: String,
    source_index: usize,
    many: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    outer_aliases: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    filters: Vec<RelationScalarExprMeta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    order_by: Vec<RelationOrderByMeta>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    limit: Option<RelationScalarExprMeta>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    offset: Option<RelationScalarExprMeta>,
    #[serde(default, skip_serializing_if = "is_false")]
    distinct: bool,
    projection: RelationProjectionMeta,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    grouping: Option<RelationGroupingMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RelationOrderByMeta {
    expr: RelationScalarExprMeta,
    descending: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RelationGroupingMeta {
    keys: RelationProjectionMeta,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    having: Vec<RelationScalarExprMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
enum RelationProjectionMeta {
    Root {
        alias: String,
        relation_name: String,
    },
    Scalar {
        expr: RelationScalarExprMeta,
    },
    Record {
        fields: Vec<RelationProjectionFieldMeta>,
    },
    NestedQuery {
        query: Box<RelationQueryPlanMeta>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RelationProjectionFieldMeta {
    name: String,
    value: RelationProjectionMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
enum RelationScalarExprMeta {
    Column {
        alias: String,
        field: String,
    },
    Captured {
        capture_index: usize,
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
        expr: Box<RelationScalarExprMeta>,
    },
    Binary {
        op: String,
        left: Box<RelationScalarExprMeta>,
        right: Box<RelationScalarExprMeta>,
    },
    Aggregate {
        function: CompiledAggregateFn,
        expr: Option<Box<RelationScalarExprMeta>>,
    },
    Exists {
        query: Box<RelationQueryPlanMeta>,
    },
}

#[derive(Debug, Clone)]
struct RelationDeclMeta {
    relation_name: String,
    relation_symbol: String,
    definition_name: Option<String>,
    source_expr: Expr,
    fields: HashSet<String>,
    links: Vec<RelationLinkMeta>,
}

#[derive(Debug, Clone)]
struct RelationLinkMeta {
    name: String,
    target_relation_name: String,
    source_field: String,
    target_field: String,
    many: bool,
}

#[derive(Debug, Clone)]
struct RowScope {
    alias: String,
    relation: RelationDeclMeta,
}

#[derive(Debug, Clone)]
struct QueryWork {
    scope: RowScope,
    source_index: usize,
    many: bool,
    outers: Vec<RowScope>,
    filters: Vec<WorkScalarExpr>,
    order_by: Vec<WorkOrderBy>,
    limit: Option<i64>,
    offset: Option<i64>,
    distinct: bool,
    projection: WorkProjection,
    grouping: Option<WorkGrouping>,
}

#[derive(Debug, Clone)]
struct WorkOrderBy {
    expr: WorkScalarExpr,
    descending: bool,
}

#[derive(Debug, Clone)]
struct WorkGrouping {
    keys: WorkProjection,
    having: Vec<WorkScalarExpr>,
}

#[derive(Debug, Clone)]
enum WorkProjection {
    Row(Box<RowScope>),
    Scalar(WorkScalarExpr),
    Record { fields: Vec<WorkProjectionField> },
    NestedQuery { query: Box<QueryWork> },
}

#[derive(Debug, Clone)]
struct WorkProjectionField {
    name: String,
    value: WorkProjection,
}

#[derive(Debug, Clone)]
enum WorkScalarExpr {
    Column {
        alias: String,
        field: String,
    },
    Captured {
        capture_index: usize,
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
        expr: Box<WorkScalarExpr>,
    },
    Binary {
        op: String,
        left: Box<WorkScalarExpr>,
        right: Box<WorkScalarExpr>,
    },
    Aggregate {
        function: CompiledAggregateFn,
        expr: Option<Box<WorkScalarExpr>>,
    },
    Exists {
        query: Box<QueryWork>,
    },
}

#[derive(Debug, Clone)]
struct QueryCompileState<'a> {
    current_module: &'a str,
    db_index: &'a SurfaceDbIndex,
    captures: Rc<RefCell<Vec<Expr>>>,
    sources: Rc<RefCell<Vec<Expr>>>,
    next_alias: Rc<Cell<usize>>,
}

impl<'a> QueryCompileState<'a> {
    fn new(db_index: &'a SurfaceDbIndex, current_module: &'a str) -> Self {
        Self {
            current_module,
            db_index,
            captures: Rc::new(RefCell::new(Vec::new())),
            sources: Rc::new(RefCell::new(Vec::new())),
            next_alias: Rc::new(Cell::new(0)),
        }
    }

    fn alloc_alias(&self) -> String {
        let next = self.next_alias.get();
        self.next_alias.set(next.saturating_add(1));
        format!("q{next}")
    }

    fn push_source(&self, expr: Expr) -> usize {
        let mut sources = self.sources.borrow_mut();
        let index = sources.len();
        sources.push(expr);
        index
    }

    fn capture(&self, expr: Expr) -> usize {
        let mut captures = self.captures.borrow_mut();
        let index = captures.len();
        captures.push(expr);
        index
    }
}

#[derive(Debug, Clone, Default)]
struct RelationExprEnv {
    current: Option<RowScope>,
    outers: Vec<RowScope>,
    projected_fields: HashMap<String, WorkProjection>,
    group_key: Option<WorkProjection>,
    allow_aggregates: bool,
}

impl RelationExprEnv {
    fn for_query(query: &QueryWork) -> Self {
        Self {
            current: Some(query.scope.clone()),
            outers: query.outers.clone(),
            projected_fields: projected_field_map(&query.projection),
            group_key: query.grouping.as_ref().map(|grouping| grouping.keys.clone()),
            allow_aggregates: query.grouping.is_some(),
        }
    }
}

pub(crate) fn compile_static_query(
    expr: &Expr,
    db_index: &SurfaceDbIndex,
    current_module: &str,
) -> Result<StaticCompiledQuery, String> {
    if !expr_requires_relation_query_lowering(expr) {
        return Err("expression is outside the lowered relation query subset".to_string());
    }
    let state = QueryCompileState::new(db_index, current_module);
    let query = try_compile_query_work(expr, &state, &RelationExprEnv::default())?
        .ok_or_else(|| "expression is outside the lowered relation query subset".to_string())?;
    let relation_query = freeze_query_work(&query);
    let mut plan = compiled_plan_from_query(&query)?;
    plan.relation_query = Some(relation_query);
    let source_exprs = state.sources.borrow().clone();
    let capture_exprs = state.captures.borrow().clone();
    Ok(StaticCompiledQuery {
        plan,
        source_exprs,
        capture_exprs,
    })
}

fn expr_requires_relation_query_lowering(expr: &Expr) -> bool {
    match expr {
        Expr::Index { base, index, .. } => {
            expr_requires_relation_query_lowering(base)
                || (matches!(base.as_ref(), Expr::Ident(_) | Expr::FieldAccess { .. })
                    && index_expr_looks_like_relation_filter(index))
        }
        Expr::Binary { op, left, right, .. } if op == "|>" => {
            let left_is_query_root = matches!(
                left.as_ref(),
                Expr::Ident(_) | Expr::FieldAccess { .. } | Expr::Index { .. }
            );
            expr_requires_relation_query_lowering(left)
                || (left_is_query_root && expr_is_relation_query_stage(right))
        }
        _ => false,
    }
}

fn index_expr_looks_like_relation_filter(expr: &Expr) -> bool {
    if looks_like_db_selector_predicate(expr) {
        return true;
    }

    match expr {
        Expr::Index { base, index, .. } => {
            matches!(
                base.as_ref(),
                Expr::Ident(_) | Expr::FieldAccess { .. } | Expr::Index { .. }
            ) && index_expr_looks_like_relation_filter(index)
        }
        Expr::Binary { op, left, right, .. } if op == "|>" => {
            let left_is_query_root = matches!(
                left.as_ref(),
                Expr::Ident(_) | Expr::FieldAccess { .. } | Expr::Index { .. }
            );
            (left_is_query_root && expr_is_relation_query_stage(right))
                || expr_requires_relation_query_lowering(left)
        }
        _ => false,
    }
}

fn expr_is_relation_query_stage(expr: &Expr) -> bool {
    match expr {
        Expr::Call { func, .. } => helper_leaf_name(func).is_some(),
        Expr::FieldAccess { base, .. } => helper_leaf_name(base).is_some(),
        Expr::Ident(_) => helper_leaf_name(expr).is_some(),
        _ => false,
    }
}

fn helper_leaf_name(expr: &Expr) -> Option<&str> {
    let dotted = surface_expr_dotted_name(expr)?;
    let leaf = dotted.rsplit('.').next()?;
    match leaf {
        "relation" => Some("relation"),
        "hasMany" => Some("hasMany"),
        "belongsTo" => Some("belongsTo"),
        "orderBy" => Some("orderBy"),
        "limit" => Some("limit"),
        "offset" => Some("offset"),
        "distinct" => Some("distinct"),
        "selectMap" => Some("selectMap"),
        "groupBy" => Some("groupBy"),
        "having" => Some("having"),
        "asc" => Some("asc"),
        "desc" => Some("desc"),
        "count" => Some("count"),
        "sum" => Some("sum"),
        "avg" => Some("avg"),
        "min" => Some("min"),
        "max" => Some("max"),
        _ => None,
    }
}

fn try_compile_query_work(
    expr: &Expr,
    state: &QueryCompileState<'_>,
    env: &RelationExprEnv,
) -> Result<Option<QueryWork>, String> {
    match expr {
        Expr::Binary {
            op, left, right, ..
        } if op == "|>" => {
            let Some(mut query) = try_compile_query_work(left, state, env)? else {
                return Ok(None);
            };
            apply_query_stage(right, &mut query, state)?;
            Ok(Some(query))
        }
        Expr::Index { base, index, .. } => {
            let Some(mut query) = try_compile_query_work(base, state, env)? else {
                return Ok(None);
            };
            let row_env = RelationExprEnv::for_query(&query);
            query
                .filters
                .push(compile_work_scalar_expr(index, state, &row_env)?);
            Ok(Some(query))
        }
        _ => try_compile_query_root(expr, state, env),
    }
}

fn try_compile_query_root(
    expr: &Expr,
    state: &QueryCompileState<'_>,
    env: &RelationExprEnv,
) -> Result<Option<QueryWork>, String> {
    if let Some(linked) = try_compile_link_root(expr, state, env)? {
        return Ok(Some(linked));
    }
    let Some(relation) = extract_relation_decl(expr, state.db_index, state.current_module) else {
        return Ok(None);
    };
    let source_index = state.push_source(relation.source_expr.clone());
    let alias = state.alloc_alias();
    let scope = RowScope { alias, relation };
    Ok(Some(new_query_work(
        scope,
        source_index,
        true,
        collect_outer_scopes(env),
    )))
}

fn try_compile_link_root(
    expr: &Expr,
    state: &QueryCompileState<'_>,
    env: &RelationExprEnv,
) -> Result<Option<QueryWork>, String> {
    match expr {
        Expr::Ident(name) => {
            build_link_query(resolve_link_owner(env, &name.name), &name.name, state, env)
        }
        Expr::FieldSection { field, .. } => {
            build_link_query(env.current.as_ref(), &field.name, state, env)
        }
        Expr::FieldAccess { base, field, .. } => {
            let owner = resolve_scope_expr(base, env);
            build_link_query(owner.as_ref(), &field.name, state, env)
        }
        _ => Ok(None),
    }
}

fn build_link_query(
    owner: Option<&RowScope>,
    field_name: &str,
    state: &QueryCompileState<'_>,
    env: &RelationExprEnv,
) -> Result<Option<QueryWork>, String> {
    let Some(owner) = owner else {
        return Ok(None);
    };
    let Some(link) = owner
        .relation
        .links
        .iter()
        .find(|link| link.name == field_name)
        .cloned()
    else {
        return Ok(None);
    };
    let target_expr = state
        .db_index
        .resolve_relation(&link.target_relation_name)
        .ok_or_else(|| format!("unknown relation '{}'", link.target_relation_name))?
        .clone();
    let target_relation = extract_relation_decl(&target_expr, state.db_index, state.current_module)
        .ok_or_else(|| format!("unknown relation '{}'", link.target_relation_name))?;
    let source_index = state.push_source(target_relation.source_expr.clone());
    let alias = state.alloc_alias();
    let scope = RowScope {
        alias: alias.clone(),
        relation: target_relation,
    };
    let mut query = new_query_work(
        scope,
        source_index,
        link.many,
        collect_outer_scopes_with_owner(owner, env),
    );
    query.filters.push(WorkScalarExpr::Binary {
        op: "==".to_string(),
        left: Box::new(WorkScalarExpr::Column {
            alias,
            field: link.target_field,
        }),
        right: Box::new(WorkScalarExpr::Column {
            alias: owner.alias.clone(),
            field: link.source_field,
        }),
    });
    Ok(Some(query))
}

fn apply_query_stage(
    stage: &Expr,
    query: &mut QueryWork,
    state: &QueryCompileState<'_>,
) -> Result<(), String> {
    let row_env = RelationExprEnv::for_query(query);
    match stage {
        Expr::Ident(_) if helper_leaf_name(stage) == Some("distinct") => {
            query.distinct = true;
            Ok(())
        }
        Expr::FieldAccess { base, field, .. } => match helper_leaf_name(base) {
            Some("orderBy") => {
                query.order_by = vec![WorkOrderBy {
                    expr: compile_named_scalar(field.name.as_str(), state, &row_env)?,
                    descending: false,
                }];
                Ok(())
            }
            Some("selectMap") => {
                query.projection = compile_named_projection(field.name.as_str(), state, &row_env)?;
                Ok(())
            }
            Some("groupBy") => {
                query.grouping = Some(WorkGrouping {
                    keys: compile_named_projection(field.name.as_str(), state, &row_env)?,
                    having: Vec::new(),
                });
                Ok(())
            }
            _ => Err("query stage is outside the lowered relation query subset".to_string()),
        },
        Expr::Call { func, args, .. } => match helper_leaf_name(func) {
            Some("orderBy") if args.len() == 1 => {
                query.order_by = compile_order_terms(&args[0], state, &row_env)?;
                Ok(())
            }
            Some("limit") if args.len() == 1 => {
                query.limit = Some(compile_const_int_expr(&args[0], state, &row_env)?);
                Ok(())
            }
            Some("offset") if args.len() == 1 => {
                query.offset = Some(compile_const_int_expr(&args[0], state, &row_env)?);
                Ok(())
            }
            Some("distinct") if args.is_empty() => {
                query.distinct = true;
                Ok(())
            }
            Some("selectMap") if args.len() == 1 => {
                query.projection = if query.grouping.is_some() {
                    compile_group_projection_expr(&args[0], state, &row_env)?
                } else {
                    compile_work_projection_expr(&args[0], state, &row_env)?
                };
                Ok(())
            }
            Some("groupBy") if args.len() == 1 => {
                query.grouping = Some(WorkGrouping {
                    keys: compile_work_projection_expr(&args[0], state, &row_env)?,
                    having: Vec::new(),
                });
                Ok(())
            }
            Some("having") if args.len() == 1 => {
                if query.grouping.is_none() {
                    return Err("database.having requires a grouped relation query".to_string());
                }
                let having_env = RelationExprEnv::for_query(query);
                let predicate = compile_work_scalar_expr(&args[0], state, &having_env)?;
                if let Some(grouping) = query.grouping.as_mut() {
                    grouping.having.push(predicate);
                }
                Ok(())
            }
            _ => Err("query stage is outside the lowered relation query subset".to_string()),
        },
        _ => Err("query stage is outside the lowered relation query subset".to_string()),
    }
}

fn compile_order_terms(
    expr: &Expr,
    state: &QueryCompileState<'_>,
    env: &RelationExprEnv,
) -> Result<Vec<WorkOrderBy>, String> {
    match expr {
        Expr::Tuple { items, .. } => items
            .iter()
            .map(|item| compile_order_term(item, state, env))
            .collect(),
        _ => Ok(vec![compile_order_term(expr, state, env)?]),
    }
}

fn compile_order_term(
    expr: &Expr,
    state: &QueryCompileState<'_>,
    env: &RelationExprEnv,
) -> Result<WorkOrderBy, String> {
    if let Expr::Call { func, args, .. } = expr {
        if args.len() == 1 {
            match helper_leaf_name(func) {
                Some("asc") => {
                    return Ok(WorkOrderBy {
                        expr: compile_work_scalar_expr(&args[0], state, env)?,
                        descending: false,
                    })
                }
                Some("desc") => {
                    return Ok(WorkOrderBy {
                        expr: compile_work_scalar_expr(&args[0], state, env)?,
                        descending: true,
                    })
                }
                _ => {}
            }
        }
    }
    Ok(WorkOrderBy {
        expr: compile_work_scalar_expr(expr, state, env)?,
        descending: false,
    })
}

fn compile_named_projection(
    field_name: &str,
    state: &QueryCompileState<'_>,
    env: &RelationExprEnv,
) -> Result<WorkProjection, String> {
    if let Some(scope) = env.current.as_ref() {
        return project_field_from_scope(scope, field_name, state, env);
    }
    Err(format!("query field '{field_name}' is outside row scope"))
}

fn compile_named_scalar(
    field_name: &str,
    state: &QueryCompileState<'_>,
    env: &RelationExprEnv,
) -> Result<WorkScalarExpr, String> {
    projection_to_work_scalar(compile_named_projection(field_name, state, env)?)
}

fn compile_work_projection_expr(
    expr: &Expr,
    state: &QueryCompileState<'_>,
    env: &RelationExprEnv,
) -> Result<WorkProjection, String> {
    if let Some(query) = try_compile_query_work(expr, state, env)? {
        return Ok(WorkProjection::NestedQuery {
            query: Box::new(query),
        });
    }
    if let Some(field_name) = extract_accessor_field_name(expr) {
        if let Some(projected) = compile_projection_ident(field_name.as_str(), state, env) {
            return Ok(projected);
        }
        return compile_named_projection(field_name.as_str(), state, env).or_else(|_| {
            selector_capture_projection(expr, state, env)
                .ok_or_else(|| format!("query field accessor '{field_name}' is outside row scope"))
        });
    }
    match expr {
        Expr::Ident(name) => compile_projection_ident(&name.name, state, env)
            .or_else(|| selector_capture_projection(expr, state, env))
            .ok_or_else(|| {
                format!(
                    "query expression references unsupported identifier '{}'",
                    name.name
                )
            }),
        Expr::FieldSection { field, .. } => compile_projection_ident(&field.name, state, env)
            .or_else(|| selector_capture_projection(expr, state, env))
            .ok_or_else(|| {
                format!(
                    "query field accessor '.{}' is outside row scope",
                    field.name
                )
            }),
        Expr::FieldAccess { base, field, .. } => {
            if let Some(scope) = resolve_scope_expr(base, env) {
                return project_field_from_scope(&scope, &field.name, state, env);
            }
            match compile_work_projection_expr(base, state, env)? {
                WorkProjection::Row(scope) => {
                    project_field_from_scope(&scope, &field.name, state, env)
                }
                WorkProjection::Record { fields } => fields
                    .into_iter()
                    .find(|compiled| compiled.name == field.name)
                    .map(|compiled| compiled.value)
                    .ok_or_else(|| format!("record projection has no field '{}'", field.name)),
                WorkProjection::Scalar(_) => {
                    Err("field access on scalar query expressions is not supported".to_string())
                }
                WorkProjection::NestedQuery { .. } => Err(
                    "field access on nested relation query expressions is not supported"
                        .to_string(),
                ),
            }
        }
        Expr::Record { fields, .. } => {
            let mut compiled = Vec::with_capacity(fields.len());
            for field in fields {
                if field.spread {
                    return Err("record spreads are not supported in lowered queries".to_string());
                }
                if field.path.len() != 1 {
                    return Err(
                        "record projection fields must use plain field names in lowered queries"
                            .to_string(),
                    );
                }
                let field_name = match &field.path[0] {
                    crate::surface::PathSegment::Field(name) => name.name.clone(),
                    _ => return Err(
                        "record projection fields must use plain field names in lowered queries"
                            .to_string(),
                    ),
                };
                compiled.push(WorkProjectionField {
                    name: field_name,
                    value: compile_work_projection_expr(&field.value, state, env)?,
                });
            }
            Ok(WorkProjection::Record { fields: compiled })
        }
        Expr::Literal(literal) => Ok(WorkProjection::Scalar(literal_to_work_scalar(literal)?)),
        Expr::UnaryNeg { expr, .. } => Ok(WorkProjection::Scalar(WorkScalarExpr::UnaryNeg {
            expr: Box::new(compile_work_scalar_expr(expr, state, env)?),
        })),
        Expr::Binary {
            op, left, right, ..
        } => Ok(WorkProjection::Scalar(WorkScalarExpr::Binary {
            op: op.clone(),
            left: Box::new(compile_work_scalar_expr(left, state, env)?),
            right: Box::new(compile_work_scalar_expr(right, state, env)?),
        })),
        _ => selector_capture_projection(expr, state, env).ok_or_else(|| {
            "query expression is outside the lowered relation query subset".to_string()
        }),
    }
}

fn compile_group_projection_expr(
    expr: &Expr,
    state: &QueryCompileState<'_>,
    env: &RelationExprEnv,
) -> Result<WorkProjection, String> {
    if let Some(query) = try_compile_query_work(expr, state, env)? {
        return Ok(WorkProjection::NestedQuery {
            query: Box::new(query),
        });
    }
    match expr {
        Expr::Record { fields, .. } => {
            let mut compiled = Vec::with_capacity(fields.len());
            for field in fields {
                if field.spread {
                    return Err("record spreads are not supported in lowered queries".to_string());
                }
                if field.path.len() != 1 {
                    return Err(
                        "record projection fields must use plain field names in lowered queries"
                            .to_string(),
                    );
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
                compiled.push(WorkProjectionField {
                    name: field_name,
                    value: compile_group_projection_expr(&field.value, state, env)?,
                });
            }
            Ok(WorkProjection::Record { fields: compiled })
        }
        _ => Ok(WorkProjection::Scalar(compile_work_scalar_expr(
            expr, state, env,
        )?)),
    }
}

fn compile_work_scalar_expr(
    expr: &Expr,
    state: &QueryCompileState<'_>,
    env: &RelationExprEnv,
) -> Result<WorkScalarExpr, String> {
    if env.allow_aggregates {
        if let Some(aggregate) = compile_aggregate_scalar(expr, state, env)? {
            return Ok(aggregate);
        }
    }
    if let Some(query) = try_compile_query_work(expr, state, env)? {
        return Ok(WorkScalarExpr::Exists {
            query: Box::new(query),
        });
    }
    projection_to_work_scalar(compile_work_projection_expr(expr, state, env)?)
}

fn compile_const_int_expr(
    expr: &Expr,
    state: &QueryCompileState<'_>,
    env: &RelationExprEnv,
) -> Result<i64, String> {
    match compile_work_scalar_expr(expr, state, env)? {
        WorkScalarExpr::IntLit { value } => Ok(value),
        _ => Err("database.limit/database.offset require an integer literal".to_string()),
    }
}

fn compile_aggregate_scalar(
    expr: &Expr,
    state: &QueryCompileState<'_>,
    env: &RelationExprEnv,
) -> Result<Option<WorkScalarExpr>, String> {
    match expr {
        Expr::Ident(_) if helper_leaf_name(expr) == Some("count") => Ok(Some(WorkScalarExpr::Aggregate {
            function: CompiledAggregateFn::Count,
            expr: None,
        })),
        Expr::Call { func, args, .. } => {
            let function = match helper_leaf_name(func) {
                Some("count") => CompiledAggregateFn::Count,
                Some("sum") => CompiledAggregateFn::Sum,
                Some("avg") => CompiledAggregateFn::Avg,
                Some("min") => CompiledAggregateFn::Min,
                Some("max") => CompiledAggregateFn::Max,
                _ => return Ok(None),
            };
            match args.as_slice() {
                [] => Ok(Some(WorkScalarExpr::Aggregate {
                    function,
                    expr: None,
                })),
                [value] => Ok(Some(WorkScalarExpr::Aggregate {
                    function,
                    expr: Some(Box::new(compile_work_scalar_expr(value, state, env)?)),
                })),
                _ => Err("aggregate expressions accept at most one argument".to_string()),
            }
        }
        _ => Ok(None),
    }
}

fn compile_projection_ident(
    name: &str,
    _state: &QueryCompileState<'_>,
    env: &RelationExprEnv,
) -> Option<WorkProjection> {
    let leaf = name.rsplit('.').next().unwrap_or(name);
    if leaf == "key" {
        if let Some(group_key) = env.group_key.as_ref() {
            return Some(group_key.clone());
        }
    }
    if let Some(projected) = env
        .projected_fields
        .get(leaf)
        .or_else(|| env.projected_fields.get(name))
    {
        return Some(projected.clone());
    }
    if let Some(scope) = env.current.as_ref() {
        if scope_matches(scope, leaf) {
            return Some(WorkProjection::Row(Box::new(scope.clone())));
        }
        if scope.relation.fields.contains(leaf) {
            return Some(WorkProjection::Scalar(WorkScalarExpr::Column {
                alias: scope.alias.clone(),
                field: leaf.to_string(),
            }));
        }
    }
    let mut outer_matches = env
        .outers
        .iter()
        .filter(|scope| scope.relation.fields.contains(leaf) || scope_matches(scope, leaf))
        .cloned()
        .collect::<Vec<_>>();
    if outer_matches.len() == 1 {
        let scope = outer_matches.pop()?;
        if scope_matches(&scope, leaf) {
            return Some(WorkProjection::Row(Box::new(scope)));
        }
        return Some(WorkProjection::Scalar(WorkScalarExpr::Column {
            alias: scope.alias,
            field: leaf.to_string(),
        }));
    }
    None
}

fn selector_capture_projection(
    expr: &Expr,
    state: &QueryCompileState<'_>,
    env: &RelationExprEnv,
) -> Option<WorkProjection> {
    expr_can_capture(expr, env, state).then(|| {
        WorkProjection::Scalar(WorkScalarExpr::Captured {
            capture_index: state.capture(expr.clone()),
        })
    })
}

fn project_field_from_scope(
    scope: &RowScope,
    field_name: &str,
    state: &QueryCompileState<'_>,
    env: &RelationExprEnv,
) -> Result<WorkProjection, String> {
    if let Some(link) = scope
        .relation
        .links
        .iter()
        .find(|link| link.name == field_name)
        .cloned()
    {
        return Ok(WorkProjection::NestedQuery {
            query: Box::new(
                build_link_query(Some(scope), &link.name, state, env)?.expect("link exists"),
            ),
        });
    }
    if scope.relation.fields.contains(field_name) {
        return Ok(WorkProjection::Scalar(WorkScalarExpr::Column {
            alias: scope.alias.clone(),
            field: field_name.to_string(),
        }));
    }
    Err(format!(
        "relation '{}' has no field or relation '{}'",
        scope.relation.relation_name, field_name
    ))
}

fn projection_to_work_scalar(projection: WorkProjection) -> Result<WorkScalarExpr, String> {
    match projection {
        WorkProjection::Scalar(expr) => Ok(expr),
        WorkProjection::Row(_) => {
            Err("row values are not valid scalar SQL expressions".to_string())
        }
        WorkProjection::Record { .. } => {
            Err("record values are not valid scalar SQL expressions".to_string())
        }
        WorkProjection::NestedQuery { query } => Ok(WorkScalarExpr::Exists { query }),
    }
}

fn expr_can_capture(expr: &Expr, env: &RelationExprEnv, state: &QueryCompileState<'_>) -> bool {
    match expr {
        Expr::Ident(name) => {
            if compile_projection_ident(&name.name, state, env).is_some() {
                return false;
            }
            state
                .db_index
                .resolve_relation(&name.name)
                .or_else(|| state.db_index.resolve_def(state.current_module, &name.name))
                .is_some()
                || env.current.is_none()
                || env
                    .current
                    .as_ref()
                    .is_some_and(|scope| !scope.relation.fields.contains(&name.name))
        }
        Expr::Literal(_) => true,
        Expr::UnaryNeg { expr, .. } => expr_can_capture(expr, env, state),
        Expr::FieldAccess { base, .. } => expr_can_capture(base, env, state),
        Expr::Binary { left, right, .. } => {
            expr_can_capture(left, env, state) && expr_can_capture(right, env, state)
        }
        Expr::Call { func, args, .. } => {
            expr_can_capture(func, env, state)
                && args.iter().all(|arg| expr_can_capture(arg, env, state))
        }
        Expr::Tuple { items, .. } => items.iter().all(|item| expr_can_capture(item, env, state)),
        Expr::List { items, .. } => items
            .iter()
            .all(|item| expr_can_capture(&item.expr, env, state)),
        Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } => fields
            .iter()
            .all(|field| expr_can_capture(&field.value, env, state)),
        Expr::Flow { root, .. } => expr_can_capture(root, env, state),
        Expr::FieldSection { .. }
        | Expr::Index { .. }
        | Expr::Lambda { .. }
        | Expr::Match { .. }
        | Expr::If { .. }
        | Expr::TextInterpolate { .. }
        | Expr::Suffixed { .. }
        | Expr::Block { .. }
        | Expr::Mock { .. }
        | Expr::Raw { .. } => false,
    }
}

fn new_query_work(
    scope: RowScope,
    source_index: usize,
    many: bool,
    outers: Vec<RowScope>,
) -> QueryWork {
    let projection = WorkProjection::Row(Box::new(scope.clone()));
    QueryWork {
        scope,
        source_index,
        many,
        outers,
        filters: Vec::new(),
        order_by: Vec::new(),
        limit: None,
        offset: None,
        distinct: false,
        projection,
        grouping: None,
    }
}

fn collect_outer_scopes(env: &RelationExprEnv) -> Vec<RowScope> {
    let mut scopes = Vec::new();
    if let Some(current) = env.current.as_ref() {
        scopes.push(current.clone());
    }
    scopes.extend(env.outers.clone());
    dedupe_scopes(scopes)
}

fn collect_outer_scopes_with_owner(owner: &RowScope, env: &RelationExprEnv) -> Vec<RowScope> {
    let mut scopes = vec![owner.clone()];
    if let Some(current) = env.current.as_ref() {
        if current.alias != owner.alias {
            scopes.push(current.clone());
        }
    }
    for outer in &env.outers {
        if outer.alias != owner.alias {
            scopes.push(outer.clone());
        }
    }
    dedupe_scopes(scopes)
}

fn dedupe_scopes(scopes: Vec<RowScope>) -> Vec<RowScope> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for scope in scopes {
        if seen.insert(scope.alias.clone()) {
            out.push(scope);
        }
    }
    out
}

fn projected_field_map(projection: &WorkProjection) -> HashMap<String, WorkProjection> {
    match projection {
        WorkProjection::Record { fields } => fields
            .iter()
            .map(|field| (field.name.clone(), field.value.clone()))
            .collect(),
        _ => HashMap::new(),
    }
}

fn scope_matches(scope: &RowScope, name: &str) -> bool {
    scope.alias == name
        || scope.relation.relation_name == name
        || scope.relation.definition_name.as_deref() == Some(name)
        || scope
            .relation
            .relation_symbol
            .rsplit('.')
            .next()
            .is_some_and(|leaf| leaf == name)
}

fn resolve_scope_expr(expr: &Expr, env: &RelationExprEnv) -> Option<RowScope> {
    match expr {
        Expr::Ident(name) => resolve_scope_name(env, &name.name),
        Expr::FieldSection { field, .. } => env
            .current
            .as_ref()
            .filter(|scope| scope_matches(scope, &field.name))
            .cloned(),
        _ => None,
    }
}

fn resolve_scope_name(env: &RelationExprEnv, name: &str) -> Option<RowScope> {
    if let Some(current) = env.current.as_ref() {
        if scope_matches(current, name) {
            return Some(current.clone());
        }
    }
    env.outers
        .iter()
        .find(|scope| scope_matches(scope, name))
        .cloned()
}

fn resolve_link_owner<'a>(env: &'a RelationExprEnv, name: &str) -> Option<&'a RowScope> {
    if let Some(current) = env.current.as_ref() {
        if current.relation.links.iter().any(|link| link.name == name) {
            return Some(current);
        }
    }
    env.outers
        .iter()
        .find(|scope| scope.relation.links.iter().any(|link| link.name == name))
}

fn freeze_query_work(query: &QueryWork) -> RelationQueryPlanMeta {
    RelationQueryPlanMeta {
        relation_name: query.scope.relation.relation_name.clone(),
        relation_symbol: query.scope.relation.relation_symbol.clone(),
        alias: query.scope.alias.clone(),
        source_index: query.source_index,
        many: query.many,
        outer_aliases: query
            .outers
            .iter()
            .map(|scope| scope.alias.clone())
            .collect(),
        filters: query.filters.iter().map(freeze_work_scalar).collect(),
        order_by: query.order_by.iter().map(freeze_work_order).collect(),
        limit: query.limit.map(|value| RelationScalarExprMeta::IntLit { value }),
        offset: query.offset.map(|value| RelationScalarExprMeta::IntLit { value }),
        distinct: query.distinct,
        projection: freeze_work_projection(&query.projection),
        grouping: query.grouping.as_ref().map(freeze_work_grouping),
    }
}

fn freeze_work_order(order: &WorkOrderBy) -> RelationOrderByMeta {
    RelationOrderByMeta {
        expr: freeze_work_scalar(&order.expr),
        descending: order.descending,
    }
}

fn freeze_work_grouping(grouping: &WorkGrouping) -> RelationGroupingMeta {
    RelationGroupingMeta {
        keys: freeze_work_projection(&grouping.keys),
        having: grouping.having.iter().map(freeze_work_scalar).collect(),
    }
}

fn freeze_work_projection(projection: &WorkProjection) -> RelationProjectionMeta {
    match projection {
        WorkProjection::Row(scope) => RelationProjectionMeta::Root {
            alias: scope.alias.clone(),
            relation_name: scope.relation.relation_name.clone(),
        },
        WorkProjection::Scalar(expr) => RelationProjectionMeta::Scalar {
            expr: freeze_work_scalar(expr),
        },
        WorkProjection::Record { fields } => RelationProjectionMeta::Record {
            fields: fields
                .iter()
                .map(|field| RelationProjectionFieldMeta {
                    name: field.name.clone(),
                    value: freeze_work_projection(&field.value),
                })
                .collect(),
        },
        WorkProjection::NestedQuery { query } => RelationProjectionMeta::NestedQuery {
            query: Box::new(freeze_query_work(query)),
        },
    }
}

fn freeze_work_scalar(expr: &WorkScalarExpr) -> RelationScalarExprMeta {
    match expr {
        WorkScalarExpr::Column { alias, field } => RelationScalarExprMeta::Column {
            alias: alias.clone(),
            field: field.clone(),
        },
        WorkScalarExpr::Captured { capture_index } => RelationScalarExprMeta::Captured {
            capture_index: *capture_index,
        },
        WorkScalarExpr::IntLit { value } => RelationScalarExprMeta::IntLit { value: *value },
        WorkScalarExpr::FloatLit { value } => RelationScalarExprMeta::FloatLit { value: *value },
        WorkScalarExpr::TextLit { value } => RelationScalarExprMeta::TextLit {
            value: value.clone(),
        },
        WorkScalarExpr::BoolLit { value } => RelationScalarExprMeta::BoolLit { value: *value },
        WorkScalarExpr::DateTimeLit { value } => RelationScalarExprMeta::DateTimeLit {
            value: value.clone(),
        },
        WorkScalarExpr::UnaryNeg { expr } => RelationScalarExprMeta::UnaryNeg {
            expr: Box::new(freeze_work_scalar(expr)),
        },
        WorkScalarExpr::Binary { op, left, right } => RelationScalarExprMeta::Binary {
            op: op.clone(),
            left: Box::new(freeze_work_scalar(left)),
            right: Box::new(freeze_work_scalar(right)),
        },
        WorkScalarExpr::Aggregate { function, expr } => RelationScalarExprMeta::Aggregate {
            function: function.clone(),
            expr: expr.as_ref().map(|expr| Box::new(freeze_work_scalar(expr))),
        },
        WorkScalarExpr::Exists { query } => RelationScalarExprMeta::Exists {
            query: Box::new(freeze_query_work(query)),
        },
    }
}

fn compiled_plan_from_query(query: &QueryWork) -> Result<CompiledQueryPlan, String> {
    let grouping = query
        .grouping
        .as_ref()
        .map(|grouping| {
            Ok::<_, String>((
                flatten_group_by_projection(&grouping.keys, query)?,
                grouping
                    .having
                    .iter()
                    .map(|expr| compiled_scalar_from_work(expr, query))
                    .collect::<Result<Vec<_>, _>>()?,
                compiled_projection_from_work(&grouping.keys, query)?,
            ))
        })
        .transpose()?;
    let relation_links = compiled_relation_links(&query.scope.relation.links);
    let row_projection = CompiledProjection::Row {
        alias: query.scope.alias.clone(),
        relation_name: query.scope.relation.relation_name.clone(),
        links: relation_links.clone(),
    };
    Ok(CompiledQueryPlan {
        sources: vec![CompiledQuerySource {
            alias: query.scope.alias.clone(),
            source_index: query.source_index,
            relation_name: query.scope.relation.relation_name.clone(),
            links: relation_links,
        }],
        filters: query
            .filters
            .iter()
            .map(|expr| compiled_scalar_from_work(expr, query))
            .collect::<Result<_, _>>()?,
        projection: compiled_projection_from_work(&query.projection, query)?,
        order_by: query
            .order_by
            .iter()
            .map(|order| {
                Ok(CompiledOrderBy {
                    expr: compiled_scalar_from_work(&order.expr, query)?,
                    descending: order.descending,
                })
            })
            .collect::<Result<_, String>>()?,
        limit: query.limit,
        offset: query.offset,
        aggregate: CompiledAggregate::None,
        distinct: query.distinct,
        group_by: grouping
            .as_ref()
            .map(|(group_by, _, _)| group_by.clone())
            .unwrap_or_default(),
        having: grouping
            .as_ref()
            .map(|(_, having, _)| having.clone())
            .unwrap_or_default(),
        group_key: grouping
            .as_ref()
            .map(|(_, _, key)| key.clone()),
        group_source: query.grouping.as_ref().map(|_| row_projection.clone()),
        grouped_projection: query.grouping.is_some()
            && !matches!(&query.projection, WorkProjection::Row(_)),
        relation_query: None,
    })
}

fn compiled_projection_from_work(
    projection: &WorkProjection,
    root: &QueryWork,
) -> Result<CompiledProjection, String> {
    match projection {
        WorkProjection::Row(scope) => Ok(CompiledProjection::Row {
            alias: scope.alias.clone(),
            relation_name: scope.relation.relation_name.clone(),
            links: compiled_relation_links(&scope.relation.links),
        }),
        WorkProjection::Scalar(expr) => Ok(CompiledProjection::Scalar {
            expr: compiled_scalar_from_work(expr, root)?,
        }),
        WorkProjection::Record { fields } => Ok(CompiledProjection::Record {
            fields: fields
                .iter()
                .map(|field| {
                    Ok(CompiledProjectionField {
                        name: field.name.clone(),
                        value: compiled_projection_from_work(&field.value, root)?,
                    })
                })
                .collect::<Result<_, String>>()?,
        }),
        WorkProjection::NestedQuery { query } => Ok(CompiledProjection::NestedQuery {
            query: Box::new(compiled_plan_from_query(query)?),
        }),
    }
}

fn compiled_scalar_from_work(
    expr: &WorkScalarExpr,
    root: &QueryWork,
) -> Result<CompiledScalarExpr, String> {
    match expr {
        WorkScalarExpr::Column { alias, field } if *alias == root.scope.alias => {
            Ok(CompiledScalarExpr::Column {
                alias: alias.clone(),
                field: field.clone(),
            })
        }
        WorkScalarExpr::Column { alias, field } => Ok(CompiledScalarExpr::OuterColumn {
            alias: alias.clone(),
            field: field.clone(),
        }),
        WorkScalarExpr::Captured { capture_index } => Ok(CompiledScalarExpr::Captured {
            capture_index: *capture_index,
        }),
        WorkScalarExpr::IntLit { value } => Ok(CompiledScalarExpr::IntLit { value: *value }),
        WorkScalarExpr::FloatLit { value } => Ok(CompiledScalarExpr::FloatLit { value: *value }),
        WorkScalarExpr::TextLit { value } => Ok(CompiledScalarExpr::TextLit {
            value: value.clone(),
        }),
        WorkScalarExpr::BoolLit { value } => Ok(CompiledScalarExpr::BoolLit { value: *value }),
        WorkScalarExpr::DateTimeLit { value } => Ok(CompiledScalarExpr::DateTimeLit {
            value: value.clone(),
        }),
        WorkScalarExpr::UnaryNeg { expr } => Ok(CompiledScalarExpr::UnaryNeg {
            expr: Box::new(compiled_scalar_from_work(expr, root)?),
        }),
        WorkScalarExpr::Binary { op, left, right } => Ok(CompiledScalarExpr::Binary {
            op: op.clone(),
            left: Box::new(compiled_scalar_from_work(left, root)?),
            right: Box::new(compiled_scalar_from_work(right, root)?),
        }),
        WorkScalarExpr::Aggregate { function, expr } => Ok(CompiledScalarExpr::Aggregate {
            aggregate: function.clone(),
            expr: expr
                .as_ref()
                .map(|expr| compiled_scalar_from_work(expr, root))
                .transpose()?
                .map(Box::new),
        }),
        WorkScalarExpr::Exists { query } => Ok(CompiledScalarExpr::Exists {
            query: Box::new(compiled_plan_from_query(query)?),
        }),
    }
}

fn flatten_group_by_projection(
    projection: &WorkProjection,
    root: &QueryWork,
) -> Result<Vec<CompiledScalarExpr>, String> {
    match projection {
        WorkProjection::Scalar(expr) => Ok(vec![compiled_scalar_from_work(expr, root)?]),
        WorkProjection::Record { fields } => {
            let mut out = Vec::new();
            for field in fields {
                out.extend(flatten_group_by_projection(&field.value, root)?);
            }
            Ok(out)
        }
        WorkProjection::Row(_) => {
            Err("database.groupBy keys must be scalar, tuple, or record expressions".to_string())
        }
        WorkProjection::NestedQuery { .. } => Err(
            "database.groupBy keys cannot include nested relation queries".to_string(),
        ),
    }
}

fn compiled_relation_links(links: &[RelationLinkMeta]) -> Vec<CompiledRelationLink> {
    links.iter()
        .map(|link| CompiledRelationLink {
            name: link.name.clone(),
            target_relation_name: link.target_relation_name.clone(),
            source_field: link.source_field.clone(),
            target_field: link.target_field.clone(),
            many: link.many,
        })
        .collect()
}

fn extract_relation_decl(
    expr: &Expr,
    db_index: &SurfaceDbIndex,
    current_module: &str,
) -> Option<RelationDeclMeta> {
    let mut definition_name = None;
    let resolved = match surface_expr_dotted_name(expr) {
        Some(name) => {
            let resolved = db_index
                .resolve_def(current_module, &name)
                .or_else(|| db_index.resolve_relation(&name))
                .unwrap_or(expr);
            definition_name = Some(name.rsplit('.').next()?.to_string());
            resolved
        }
        None => expr,
    };
    let mut args = Vec::new();
    let head = flatten_surface_call_args(resolved, &mut args);
    if helper_leaf_name(head) != Some("relation") || args.len() < 2 {
        return None;
    }
    let relation_name = match args[0] {
        Expr::Literal(crate::surface::Literal::String { text, .. }) => text.clone(),
        _ => return None,
    };
    let source_expr = expr.clone();
    let fields = extract_relation_fields(args[1], db_index, current_module);
    let links = args
        .get(2)
        .and_then(|expr| extract_relation_links(expr))
        .unwrap_or_default();
    Some(RelationDeclMeta {
        relation_name,
        relation_symbol: surface_expr_dotted_name(expr)
            .or_else(|| definition_name.clone())
            .unwrap_or_else(|| "<relation>".to_string()),
        definition_name,
        source_expr,
        fields,
        links,
    })
}

fn extract_relation_fields(
    expr: &Expr,
    db_index: &SurfaceDbIndex,
    current_module: &str,
) -> HashSet<String> {
    let resolved = match surface_expr_dotted_name(expr) {
        Some(name) => db_index
            .resolve_def(current_module, &name)
            .or_else(|| db_index.resolve_relation(&name))
            .unwrap_or(expr),
        None => expr,
    };
    if let Some(relation) = extract_relation_decl(resolved, db_index, current_module) {
        return relation.fields;
    }
    if let Expr::List { items, .. } = resolved {
        return items.iter().filter_map(extract_table_column_name).collect();
    }
    let mut args = Vec::new();
    let head = flatten_surface_call_args(resolved, &mut args);
    if helper_leaf_name(head) != Some("relation") || args.len() < 2 {
        return HashSet::new();
    }
    match args.get(1) {
        Some(Expr::List { items, .. }) => items.iter().filter_map(extract_table_column_name).collect(),
        _ => HashSet::new(),
    }
}

fn extract_table_column_name(item: &crate::surface::ListItem) -> Option<String> {
    if item.spread {
        return None;
    }
    let Expr::Record { fields, .. } = &item.expr else {
        return None;
    };
    fields.iter().find_map(|field| match field.path.as_slice() {
        [crate::surface::PathSegment::Field(name)] if !field.spread && name.name == "name" => {
            match &field.value {
                Expr::Literal(crate::surface::Literal::String { text, .. }) => Some(text.clone()),
                _ => None,
            }
        }
        _ => None,
    })
}

fn extract_relation_links(expr: &Expr) -> Option<Vec<RelationLinkMeta>> {
    let Expr::List { items, .. } = expr else {
        return Some(Vec::new());
    };
    let mut links = Vec::with_capacity(items.len());
    for item in items {
        if item.spread {
            return None;
        }
        let Expr::Call { func, args, .. } = &item.expr else {
            return None;
        };
        let helper = helper_leaf_name(func)?;
        let name = match &args[0] {
            Expr::Literal(crate::surface::Literal::String { text, .. }) => text.clone(),
            _ => return None,
        };
        let (target_relation_name, source_field, target_field) = match args.as_slice() {
            [_, Expr::Literal(crate::surface::Literal::String { text, .. }), source, target] => (
                text.clone(),
                extract_accessor_field_name(source)?,
                extract_accessor_field_name(target)?,
            ),
            [_, Expr::FieldAccess { base, field: target_field, .. }] => {
                let Expr::FieldAccess { base, field: source_field, .. } = base.as_ref() else {
                    return None;
                };
                let Expr::Literal(crate::surface::Literal::String { text, .. }) = base.as_ref() else {
                    return None;
                };
                (
                    text.clone(),
                    source_field.name.clone(),
                    target_field.name.clone(),
                )
            }
            _ => return None,
        };
        links.push(RelationLinkMeta {
            name,
            target_relation_name,
            source_field,
            target_field,
            many: matches!(helper, "hasMany"),
        });
    }
    Some(links)
}

fn extract_accessor_field_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::FieldSection { field, .. } => Some(field.name.clone()),
        Expr::Lambda { params, body, .. } if params.len() == 1 => {
            let param = match &params[0] {
                Pattern::Ident(name) | Pattern::SubjectIdent(name) => &name.name,
                _ => return None,
            };
            match body.as_ref() {
                Expr::FieldAccess { base, field, .. } => match base.as_ref() {
                    Expr::Ident(name) if name.name == *param => Some(field.name.clone()),
                    _ => None,
                },
                _ => None,
            }
        }
        _ => None,
    }
}

fn literal_to_work_scalar(literal: &crate::surface::Literal) -> Result<WorkScalarExpr, String> {
    match literal {
        crate::surface::Literal::Number { text, .. } => {
            if let Ok(value) = text.parse::<i64>() {
                Ok(WorkScalarExpr::IntLit { value })
            } else if let Ok(value) = text.parse::<f64>() {
                Ok(WorkScalarExpr::FloatLit { value })
            } else {
                Err(format!(
                    "unsupported numeric literal '{text}' in lowered relation query"
                ))
            }
        }
        crate::surface::Literal::String { text, .. } => Ok(WorkScalarExpr::TextLit {
            value: text.clone(),
        }),
        crate::surface::Literal::Bool { value, .. } => {
            Ok(WorkScalarExpr::BoolLit { value: *value })
        }
        crate::surface::Literal::DateTime { text, .. } => Ok(WorkScalarExpr::DateTimeLit {
            value: text.clone(),
        }),
        crate::surface::Literal::Sigil { .. } => {
            Err("sigil literals are not supported in lowered relation queries".to_string())
        }
    }
}

#[derive(Debug, Clone)]
struct SelectorCompileEnv {
    aliases: HashSet<String>,
    lets: HashMap<String, CompiledProjection>,
    captures: Rc<RefCell<Vec<Expr>>>,
}

impl Default for SelectorCompileEnv {
    fn default() -> Self {
        Self {
            aliases: HashSet::new(),
            lets: HashMap::new(),
            captures: Rc::new(RefCell::new(Vec::new())),
        }
    }
}

pub(crate) fn compile_static_db_patch(
    fields: &[crate::surface::RecordField],
) -> Result<StaticCompiledDbPatch, String> {
    let env = SelectorCompileEnv::default();
    let mut compiled_fields = Vec::with_capacity(fields.len());
    for field in fields {
        if field.spread {
            return Err("selector patch lowering does not support record spread".to_string());
        }
        if field.path.len() != 1 {
            return Err("selector patch lowering requires plain field names".to_string());
        }
        let field_name = match &field.path[0] {
            crate::surface::PathSegment::Field(name) => name.name.clone(),
            _ => return Err("selector patch lowering requires plain field names".to_string()),
        };
        compiled_fields.push(CompiledDbPatchField {
            field: field_name,
            value: compile_selector_scalar_expr(&field.value, &env)?,
        });
    }
    let capture_exprs = env.captures.borrow().clone();
    Ok(StaticCompiledDbPatch {
        plan: CompiledDbPatchPlan {
            fields: compiled_fields,
        },
        capture_exprs,
    })
}

fn compile_selector_projection_expr(
    expr: &Expr,
    env: &SelectorCompileEnv,
) -> Result<CompiledProjection, String> {
    match expr {
        Expr::Ident(name) => {
            if let Some(value) = env.lets.get(&name.name) {
                return Ok(value.clone());
            }
            if env.aliases.contains(&name.name) {
                return Ok(CompiledProjection::Row {
                    alias: name.name.clone(),
                    relation_name: String::new(),
                    links: Vec::new(),
                });
            }
            Err(format!(
                "query expression references unsupported identifier '{}'",
                name.name
            ))
        }
        Expr::FieldSection { field, .. } => Err(format!(
            "query field accessor '.{}' is outside row scope",
            field.name
        )),
        Expr::FieldAccess { base, field, .. } => {
            let base = compile_selector_projection_expr(base, env)?;
            selector_project_field(&base, &field.name)
        }
        Expr::Record { fields, .. } => {
            let mut compiled = Vec::with_capacity(fields.len());
            for field in fields {
                if field.spread {
                    return Err("record spreads are not supported in lowered queries".to_string());
                }
                if field.path.len() != 1 {
                    return Err(
                        "record projection fields must use plain field names in lowered queries"
                            .to_string(),
                    );
                }
                let field_name = match &field.path[0] {
                    crate::surface::PathSegment::Field(name) => name.name.clone(),
                    _ => return Err(
                        "record projection fields must use plain field names in lowered queries"
                            .to_string(),
                    ),
                };
                compiled.push(CompiledProjectionField {
                    name: field_name,
                    value: compile_selector_projection_expr(&field.value, env)?,
                });
            }
            Ok(CompiledProjection::Record { fields: compiled })
        }
        Expr::Literal(literal) => Ok(CompiledProjection::Scalar {
            expr: compile_literal_scalar(literal)?,
        }),
        Expr::UnaryNeg { expr, .. } => Ok(CompiledProjection::Scalar {
            expr: CompiledScalarExpr::UnaryNeg {
                expr: Box::new(compile_selector_scalar_expr(expr, env)?),
            },
        }),
        Expr::Binary {
            op, left, right, ..
        } => Ok(CompiledProjection::Scalar {
            expr: CompiledScalarExpr::Binary {
                op: op.clone(),
                left: Box::new(compile_selector_scalar_expr(left, env)?),
                right: Box::new(compile_selector_scalar_expr(right, env)?),
            },
        }),
        _ if selector_expr_can_capture(expr, env) => Ok(CompiledProjection::Scalar {
            expr: CompiledScalarExpr::Captured {
                capture_index: capture_selector_expr(env, expr.clone()),
            },
        }),
        _ => Err("query expression is not in the lowered SQL-backed subset".to_string()),
    }
}

fn compile_selector_scalar_expr(
    expr: &Expr,
    env: &SelectorCompileEnv,
) -> Result<CompiledScalarExpr, String> {
    selector_projection_to_scalar(compile_selector_projection_expr(expr, env)?)
}

fn selector_project_field(
    base: &CompiledProjection,
    field_name: &str,
) -> Result<CompiledProjection, String> {
    match base {
        CompiledProjection::Row { alias, .. } => Ok(CompiledProjection::Scalar {
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
        CompiledProjection::NestedQuery { .. } => Err(
            "field access on nested relation query expressions is not supported".to_string(),
        ),
    }
}

fn selector_projection_to_scalar(
    projection: CompiledProjection,
) -> Result<CompiledScalarExpr, String> {
    match projection {
        CompiledProjection::Scalar { expr } => Ok(expr),
        CompiledProjection::Row { .. } => {
            Err("row values are not valid scalar SQL expressions".to_string())
        }
        CompiledProjection::Record { .. } => {
            Err("record values are not valid scalar SQL expressions".to_string())
        }
        CompiledProjection::NestedQuery { .. } => Err(
            "nested relation queries are not valid scalar SQL expressions".to_string(),
        ),
    }
}

fn capture_selector_expr(env: &SelectorCompileEnv, expr: Expr) -> usize {
    let mut captures = env.captures.borrow_mut();
    let index = captures.len();
    captures.push(expr);
    index
}

fn selector_expr_can_capture(expr: &Expr, env: &SelectorCompileEnv) -> bool {
    match expr {
        Expr::Ident(name) => {
            !env.aliases.contains(&name.name) && !env.lets.contains_key(&name.name)
        }
        Expr::Literal(_) => true,
        Expr::UnaryNeg { expr, .. } => selector_expr_can_capture(expr, env),
        Expr::FieldAccess { base, .. } => selector_expr_can_capture(base, env),
        Expr::Binary { left, right, .. } => {
            selector_expr_can_capture(left, env) && selector_expr_can_capture(right, env)
        }
        Expr::Call { func, args, .. } => {
            selector_expr_can_capture(func, env)
                && args.iter().all(|arg| selector_expr_can_capture(arg, env))
        }
        Expr::Flow { root, .. } => selector_expr_can_capture(root, env),
        _ => false,
    }
}

fn compile_literal_scalar(literal: &crate::surface::Literal) -> Result<CompiledScalarExpr, String> {
    match literal {
        crate::surface::Literal::Number { text, .. } => {
            if let Ok(value) = text.parse::<i64>() {
                Ok(CompiledScalarExpr::IntLit { value })
            } else if let Ok(value) = text.parse::<f64>() {
                Ok(CompiledScalarExpr::FloatLit { value })
            } else {
                Err(format!(
                    "unsupported numeric literal '{text}' in lowered query"
                ))
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
