use serde::{Deserialize, Serialize};

use crate::surface::{
    BlockItem, BlockKind, Decorator, Def, DomainItem, Expr, Module, ModuleItem, Pattern,
    SpannedName, TextPart,
};
use std::cell::Cell;

thread_local! {
    static DEBUG_TRACE_OVERRIDE: Cell<Option<bool>> = const { Cell::new(None) };
}

fn debug_trace_enabled() -> bool {
    DEBUG_TRACE_OVERRIDE.with(|cell| {
        cell.get()
            .unwrap_or_else(|| std::env::var("AIVI_DEBUG_TRACE").is_ok_and(|v| v == "1"))
    })
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HirProgram {
    pub modules: Vec<HirModule>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HirModule {
    pub name: String,
    pub defs: Vec<HirDef>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HirDef {
    pub name: String,
    pub expr: HirExpr,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "kind")]
pub enum HirTextPart {
    Text { text: String },
    Expr { expr: HirExpr },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "kind")]
pub enum HirExpr {
    Var {
        id: u32,
        name: String,
    },
    LitNumber {
        id: u32,
        text: String,
    },
    LitString {
        id: u32,
        text: String,
    },
    TextInterpolate {
        id: u32,
        parts: Vec<HirTextPart>,
    },
    LitSigil {
        id: u32,
        tag: String,
        body: String,
        flags: String,
    },
    LitBool {
        id: u32,
        value: bool,
    },
    LitDateTime {
        id: u32,
        text: String,
    },
    Lambda {
        id: u32,
        param: String,
        body: Box<HirExpr>,
    },
    App {
        id: u32,
        func: Box<HirExpr>,
        arg: Box<HirExpr>,
    },
    Call {
        id: u32,
        func: Box<HirExpr>,
        args: Vec<HirExpr>,
    },
    DebugFn {
        id: u32,
        fn_name: String,
        arg_vars: Vec<String>,
        log_args: bool,
        log_return: bool,
        log_time: bool,
        body: Box<HirExpr>,
    },
    Pipe {
        id: u32,
        pipe_id: u32,
        step: u32,
        label: String,
        log_time: bool,
        func: Box<HirExpr>,
        arg: Box<HirExpr>,
    },
    List {
        id: u32,
        items: Vec<HirListItem>,
    },
    Tuple {
        id: u32,
        items: Vec<HirExpr>,
    },
    Record {
        id: u32,
        fields: Vec<HirRecordField>,
    },
    Patch {
        id: u32,
        target: Box<HirExpr>,
        fields: Vec<HirRecordField>,
    },
    FieldAccess {
        id: u32,
        base: Box<HirExpr>,
        field: String,
    },
    Index {
        id: u32,
        base: Box<HirExpr>,
        index: Box<HirExpr>,
        #[serde(skip)]
        location: Option<String>,
    },
    Match {
        id: u32,
        scrutinee: Box<HirExpr>,
        arms: Vec<HirMatchArm>,
    },
    If {
        id: u32,
        cond: Box<HirExpr>,
        then_branch: Box<HirExpr>,
        else_branch: Box<HirExpr>,
    },
    Binary {
        id: u32,
        op: String,
        left: Box<HirExpr>,
        right: Box<HirExpr>,
    },
    Block {
        id: u32,
        block_kind: HirBlockKind,
        items: Vec<HirBlockItem>,
    },
    Raw {
        id: u32,
        text: String,
    },
    /// `mock path = value ... in body` — scoped binding substitution.
    Mock {
        id: u32,
        substitutions: Vec<HirMockSubstitution>,
        body: Box<HirExpr>,
    },
}

/// A single mock substitution at the HIR level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HirMockSubstitution {
    /// Fully-qualified dotted path (e.g. `"aivi.rest.get"`).
    pub path: String,
    /// Whether this is a snapshot mock (record/replay).
    pub snapshot: bool,
    /// Replacement expression (`None` for snapshot mocks).
    pub value: Option<HirExpr>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HirListItem {
    pub expr: HirExpr,
    pub spread: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HirRecordField {
    pub spread: bool,
    pub path: Vec<HirPathSegment>,
    pub value: HirExpr,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum HirPathSegment {
    Field(String),
    Index(HirExpr),
    All,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HirMatchArm {
    pub pattern: HirPattern,
    pub guard: Option<HirExpr>,
    pub body: HirExpr,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum HirPattern {
    Wildcard {
        id: u32,
    },
    Var {
        id: u32,
        name: String,
    },
    At {
        id: u32,
        name: String,
        pattern: Box<HirPattern>,
    },
    Literal {
        id: u32,
        value: HirLiteral,
    },
    Constructor {
        id: u32,
        name: String,
        args: Vec<HirPattern>,
    },
    Tuple {
        id: u32,
        items: Vec<HirPattern>,
    },
    List {
        id: u32,
        items: Vec<HirPattern>,
        rest: Option<Box<HirPattern>>,
    },
    Record {
        id: u32,
        fields: Vec<HirRecordPatternField>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HirRecordPatternField {
    pub path: Vec<String>,
    pub pattern: HirPattern,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum HirLiteral {
    Number(String),
    String(String),
    Sigil {
        tag: String,
        body: String,
        flags: String,
    },
    Bool(bool),
    DateTime(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum HirBlockKind {
    Plain,
    /// `do M { ... }` — monadic block. `monad` is the type constructor name
    /// (e.g. `"Effect"`, `"Option"`, `"Result"`).
    Do { monad: String },
    Generate,
    Resource,
}

impl HirBlockKind {
    /// Returns `true` when the block is the special `do Effect { ... }` form
    /// which supports effect-specific statements (`or`, `when`, `unless`, `given`, `on`,
    /// resource acquisition, `loop`/`recurse`).
    pub fn is_effect(&self) -> bool {
        matches!(self, HirBlockKind::Do { monad } if monad == "Effect")
    }

    /// Returns `true` for any `do M { ... }` block (generic monadic).
    pub fn is_do(&self) -> bool {
        matches!(self, HirBlockKind::Do { .. })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum HirBlockItem {
    Bind {
        pattern: HirPattern,
        expr: HirExpr,
        /// `true` when this was a `<-` bind, `false` for a `=` let-binding.
        is_monadic: bool,
    },
    Filter { expr: HirExpr },
    Yield { expr: HirExpr },
    Recurse { expr: HirExpr },
    Expr { expr: HirExpr },
}

pub fn desugar_modules(modules: &[Module]) -> HirProgram {
    let trace = std::env::var("AIVI_TRACE_DESUGAR").is_ok_and(|v| v == "1");
    let debug_trace = debug_trace_enabled();
    let mut id_gen = IdGen::default();
    let mut hir_modules = Vec::new();
    for (module_index, module) in modules.iter().enumerate() {
        if trace {
            eprintln!(
                "[AIVI_TRACE_DESUGAR] module {}/{}: {}",
                module_index + 1,
                modules.len(),
                module.name.name
            );
        }
        let module_source = if debug_trace && !module.path.starts_with("<embedded:") {
            std::fs::read_to_string(&module.path).ok()
        } else {
            None
        };
        let defs = collect_surface_defs(module)
            .into_iter()
            .map(|def| {
                let name = def.name.name.clone();
                let debug_params = if debug_trace {
                    parse_debug_params(&def.decorators)
                } else {
                    None
                };
                if trace {
                    eprintln!("[AIVI_TRACE_DESUGAR]   def {}.{}", module.name.name, name);
                }
                HirDef {
                    name,
                    expr: lower_def_expr(
                        module,
                        def,
                        debug_params,
                        module_source.as_deref(),
                        &mut id_gen,
                    ),
                }
            })
            .collect();
        hir_modules.push(HirModule {
            name: module.name.name.clone(),
            defs,
        });
    }
    HirProgram {
        modules: hir_modules,
    }
}

fn collect_surface_defs(module: &Module) -> Vec<Def> {
    let mut defs = Vec::new();
    for item in &module.items {
        match item {
            ModuleItem::Def(def) => defs.push(def.clone()),
            ModuleItem::InstanceDecl(instance) => defs.extend(instance.defs.clone()),
            ModuleItem::DomainDecl(domain) => {
                for domain_item in &domain.items {
                    match domain_item {
                        DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                            defs.push(def.clone());
                        }
                        DomainItem::TypeAlias(_) | DomainItem::TypeSig(_) => {}
                    }
                }
            }
            // Machine declarations produce runtime-registered globals. Emit a
            // synthetic def so that references inside JIT-compiled code resolve
            // via `Global` instead of `ConstructorValue`.
            ModuleItem::MachineDecl(machine_decl) => {
                let span = machine_decl.name.span.clone();
                defs.push(Def {
                    decorators: Vec::new(),
                    name: machine_decl.name.clone(),
                    params: Vec::new(),
                    expr: Expr::Ident(SpannedName {
                        name: "Unit".to_string(),
                        span: span.clone(),
                    }),
                    span,
                });
            }
            _ => {}
        }
    }
    defs
}

#[derive(Debug, Clone, Copy)]
struct DebugParams {
    pipes: bool,
    args: bool,
    ret: bool,
    time: bool,
}

fn parse_debug_params(decorators: &[Decorator]) -> Option<DebugParams> {
    let decorator = decorators.iter().find(|d| d.name.name == "debug")?;
    let mut names: Vec<&str> = Vec::new();
    match &decorator.arg {
        None => {}
        Some(Expr::Tuple { items, .. }) => {
            for item in items {
                if let Expr::Ident(name) = item {
                    names.push(name.name.as_str());
                }
            }
        }
        Some(Expr::Ident(name)) => names.push(name.name.as_str()),
        Some(_) => {}
    }

    if names.is_empty() {
        // `@debug()` / `@debug` defaults to function-level timing only.
        return Some(DebugParams {
            pipes: false,
            args: false,
            ret: false,
            time: true,
        });
    }

    Some(DebugParams {
        pipes: names.contains(&"pipes"),
        args: names.contains(&"args"),
        ret: names.contains(&"return"),
        time: names.contains(&"time"),
    })
}

struct LowerCtx<'a> {
    debug: Option<LowerDebug<'a>>,
    source_path: Option<&'a str>,
}

struct LowerDebug<'a> {
    fn_name: String,
    params: DebugParams,
    source: Option<&'a str>,
    next_pipe_id: u32,
}

impl LowerDebug<'_> {
    fn alloc_pipe_id(&mut self) -> u32 {
        let id = self.next_pipe_id;
        self.next_pipe_id = self.next_pipe_id.saturating_add(1);
        id
    }
}

fn debug_arg_vars(params: &[Pattern]) -> Vec<String> {
    let len = params.len();
    params
        .iter()
        .enumerate()
        .map(|(i, param)| match param {
            Pattern::Ident(name) => name.name.clone(),
            Pattern::SubjectIdent(name) => name.name.clone(),
            Pattern::At { name, .. } => name.name.clone(),
            _ => format!("_arg{}", len.saturating_sub(1).saturating_sub(i)),
        })
        .collect()
}

fn lower_def_expr(
    module: &Module,
    def: Def,
    debug_params: Option<DebugParams>,
    module_source: Option<&str>,
    id_gen: &mut IdGen,
) -> HirExpr {
    let Def {
        name,
        params,
        expr,
        ..
    } = def;
    let fn_name = format!("{}.{}", module.name.name, name.name);

    // `@debug(...)` is intended for functions. In v0.1 surface syntax, function parameters are
    // written as an explicit lambda on the RHS (`f = x y => ...`). Preserve `@debug` on this
    // common shape by treating a top-level lambda as the function binder when `def.params` is
    // empty.
    let (effective_params, effective_expr) = if params.is_empty() {
        match (debug_params.as_ref(), expr) {
            (Some(_), Expr::Lambda { params, body, .. }) => (params, *body),
            (_, expr) => (Vec::new(), expr),
        }
    } else {
        (params, expr)
    };

    let debug_params = debug_params.filter(|_| !effective_params.is_empty());

    let mut ctx = LowerCtx {
        debug: debug_params.map(|params| LowerDebug {
            fn_name: fn_name.clone(),
            params,
            source: module_source,
            next_pipe_id: 1,
        }),
        source_path: Some(&module.path),
    };

    let body_hir = lower_expr_ctx(effective_expr, id_gen, &mut ctx, false);
    let body_hir = if let Some(debug) = &ctx.debug {
        HirExpr::DebugFn {
            id: id_gen.next(),
            fn_name: debug.fn_name.clone(),
            arg_vars: debug_arg_vars(&effective_params),
            log_args: debug.params.args,
            log_return: debug.params.ret,
            log_time: debug.params.time,
            body: Box::new(body_hir),
        }
    } else {
        body_hir
    };

    if effective_params.is_empty() {
        body_hir
    } else {
        lower_lambda_hir(effective_params, body_hir, id_gen)
    }
}

fn lower_expr_ctx(expr: Expr, id_gen: &mut IdGen, ctx: &mut LowerCtx<'_>, in_pipe_left: bool) -> HirExpr {
    // Effect-block surface sugars (pure `=` bindings and `if ... else Unit` in statement position).
    let expr = crate::surface::desugar_effect_sugars(expr);

    // Placeholder-lambda sugar: rewrite `_` occurrences into a lambda at the
    // smallest expression scope that still contains `_`.
    let expr = desugar_placeholder_lambdas(expr);
    if let Expr::Ident(name) = &expr {
        if name.name == "_" {
            let param = "_arg0".to_string();
            return HirExpr::Lambda {
                id: id_gen.next(),
                param: param.clone(),
                body: Box::new(HirExpr::Var {
                    id: id_gen.next(),
                    name: param,
                }),
            };
        }
    }

    if let Expr::Binary {
        op, left, right, ..
    } = &expr
    {
        if op == "<|" && matches!(**right, Expr::Record { .. }) && !contains_placeholder(left) {
            return lower_expr_inner_ctx(expr, id_gen, ctx, in_pipe_left);
        }
    }
    if matches!(&expr, Expr::PatchLit { .. }) {
        return lower_expr_inner_ctx(expr, id_gen, ctx, in_pipe_left);
    }
    lower_expr_inner_ctx(expr, id_gen, ctx, in_pipe_left)
}
