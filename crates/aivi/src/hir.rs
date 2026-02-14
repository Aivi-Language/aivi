use serde::{Deserialize, Serialize};

use crate::surface::{
    BlockItem, BlockKind, Decorator, Def, DomainItem, Expr, Module, ModuleItem, Pattern, TextPart,
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
    #[serde(default)]
    pub inline: bool,
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
    Effect,
    Generate,
    Resource,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum HirBlockItem {
    Bind { pattern: HirPattern, expr: HirExpr },
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
                let inline = has_decorator(&def.decorators, "inline");
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
                    inline,
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

fn has_decorator(decorators: &[Decorator], name: &str) -> bool {
    decorators
        .iter()
        .any(|decorator| decorator.name.name == name)
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
    let fn_name = format!("{}.{}", module.name.name, def.name.name);
    let debug_params = debug_params.filter(|_| !def.params.is_empty());

    let mut ctx = LowerCtx {
        debug: debug_params.map(|params| LowerDebug {
            fn_name: fn_name.clone(),
            params,
            source: module_source,
            next_pipe_id: 1,
        }),
    };

    let body_hir = lower_expr_ctx(def.expr, id_gen, &mut ctx, false);
    let body_hir = if let Some(debug) = &ctx.debug {
        HirExpr::DebugFn {
            id: id_gen.next(),
            fn_name: debug.fn_name.clone(),
            arg_vars: debug_arg_vars(&def.params),
            log_args: debug.params.args,
            log_return: debug.params.ret,
            log_time: debug.params.time,
            body: Box::new(body_hir),
        }
    } else {
        body_hir
    };

    if def.params.is_empty() {
        body_hir
    } else {
        lower_lambda_hir(def.params, body_hir, id_gen)
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

fn lower_expr_inner_ctx(expr: Expr, id_gen: &mut IdGen, ctx: &mut LowerCtx<'_>, in_pipe_left: bool) -> HirExpr {
    match expr {
        Expr::Ident(name) => HirExpr::Var {
            id: id_gen.next(),
            name: name.name,
        },
        Expr::TextInterpolate { parts, .. } => HirExpr::TextInterpolate {
            id: id_gen.next(),
            parts: parts
                .into_iter()
                .map(|part| match part {
                    TextPart::Text { text, .. } => HirTextPart::Text { text },
                    TextPart::Expr { expr, .. } => HirTextPart::Expr {
                        expr: lower_expr_ctx(*expr, id_gen, ctx, false),
                    },
                })
                .collect(),
        },
        Expr::Literal(literal) => match literal {
            crate::surface::Literal::Number { text, .. } => {
                fn split_suffixed(text: &str) -> Option<(String, String)> {
                    let mut chars = text.chars().peekable();
                    let mut number = String::new();
                    if matches!(chars.peek(), Some('-')) {
                        number.push('-');
                        chars.next();
                    }
                    let mut saw_digit = false;
                    let mut saw_dot = false;
                    while let Some(&ch) = chars.peek() {
                        if ch.is_ascii_digit() {
                            saw_digit = true;
                            number.push(ch);
                            chars.next();
                            continue;
                        }
                        if ch == '.' && !saw_dot {
                            saw_dot = true;
                            number.push(ch);
                            chars.next();
                            continue;
                        }
                        break;
                    }
                    if !saw_digit {
                        return None;
                    }
                    let suffix: String = chars.collect();
                    if suffix.is_empty() {
                        return None;
                    }
                    if !suffix
                        .chars()
                        .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
                    {
                        return None;
                    }
                    Some((number, suffix))
                }

                if let Some((number, suffix)) = split_suffixed(&text) {
                    let template_name = format!("1{suffix}");
                    return HirExpr::App {
                        id: id_gen.next(),
                        func: Box::new(HirExpr::Var {
                            id: id_gen.next(),
                            name: template_name,
                        }),
                        arg: Box::new(HirExpr::LitNumber {
                            id: id_gen.next(),
                            text: number,
                        }),
                    };
                }

                HirExpr::LitNumber {
                    id: id_gen.next(),
                    text,
                }
            }
            crate::surface::Literal::String { text, .. } => HirExpr::LitString {
                id: id_gen.next(),
                text,
            },
            crate::surface::Literal::Sigil {
                tag, body, flags, ..
            } => HirExpr::LitSigil {
                id: id_gen.next(),
                tag,
                body,
                flags,
            },
            crate::surface::Literal::Bool { value, .. } => HirExpr::LitBool {
                id: id_gen.next(),
                value,
            },
            crate::surface::Literal::DateTime { text, .. } => HirExpr::LitDateTime {
                id: id_gen.next(),
                text,
            },
        },
        Expr::List { items, .. } => HirExpr::List {
            id: id_gen.next(),
            items: items
                .into_iter()
                .map(|item| HirListItem {
                    expr: lower_expr_ctx(item.expr, id_gen, ctx, false),
                    spread: item.spread,
                })
                .collect(),
        },
        Expr::Tuple { items, .. } => HirExpr::Tuple {
            id: id_gen.next(),
            items: items
                .into_iter()
                .map(|item| lower_expr_ctx(item, id_gen, ctx, false))
                .collect(),
        },
        Expr::Record { fields, .. } => HirExpr::Record {
            id: id_gen.next(),
            fields: fields
                .into_iter()
                .map(|field| HirRecordField {
                    spread: field.spread,
                    path: field
                        .path
                        .into_iter()
                        .map(|segment| match segment {
                            crate::surface::PathSegment::Field(name) => {
                                HirPathSegment::Field(name.name)
                            }
                            crate::surface::PathSegment::Index(expr, _) => {
                                HirPathSegment::Index(lower_expr_ctx(expr, id_gen, ctx, false))
                            }
                            crate::surface::PathSegment::All(_) => HirPathSegment::All,
                        })
                        .collect(),
                    value: lower_expr_ctx(field.value, id_gen, ctx, false),
                })
                .collect(),
        },
        Expr::PatchLit { fields, .. } => {
            let param = format!("__patch_target{}", id_gen.next());
            let target = HirExpr::Var {
                id: id_gen.next(),
                name: param.clone(),
            };
            let patch = HirExpr::Patch {
                id: id_gen.next(),
                target: Box::new(target),
                fields: fields
                    .into_iter()
                    .map(|field| HirRecordField {
                        spread: field.spread,
                        path: field
                            .path
                            .into_iter()
                            .map(|segment| match segment {
                                crate::surface::PathSegment::Field(name) => {
                                    HirPathSegment::Field(name.name)
                                }
                                crate::surface::PathSegment::Index(expr, _) => {
                                    HirPathSegment::Index(lower_expr_ctx(expr, id_gen, ctx, false))
                                }
                                crate::surface::PathSegment::All(_) => HirPathSegment::All,
                            })
                            .collect(),
                        value: lower_expr_ctx(field.value, id_gen, ctx, false),
                    })
                    .collect(),
            };
            HirExpr::Lambda {
                id: id_gen.next(),
                param,
                body: Box::new(patch),
            }
        }
        Expr::FieldAccess { base, field, .. } => HirExpr::FieldAccess {
            id: id_gen.next(),
            base: Box::new(lower_expr_ctx(*base, id_gen, ctx, false)),
            field: field.name,
        },
        Expr::FieldSection { field, .. } => {
            let param = "_arg0".to_string();
            let var = HirExpr::Var {
                id: id_gen.next(),
                name: param.clone(),
            };
            let body = HirExpr::FieldAccess {
                id: id_gen.next(),
                base: Box::new(var),
                field: field.name,
            };
            HirExpr::Lambda {
                id: id_gen.next(),
                param,
                body: Box::new(body),
            }
        }
        Expr::Index { base, index, .. } => HirExpr::Index {
            id: id_gen.next(),
            base: Box::new(lower_expr_ctx(*base, id_gen, ctx, false)),
            index: Box::new(lower_expr_ctx(*index, id_gen, ctx, false)),
        },
        Expr::Call { func, args, .. } => HirExpr::Call {
            id: id_gen.next(),
            func: Box::new(lower_expr_ctx(*func, id_gen, ctx, false)),
            args: args
                .into_iter()
                .map(|arg| lower_expr_ctx(arg, id_gen, ctx, false))
                .collect(),
        },
        Expr::Lambda { params, body, .. } => {
            let body = lower_expr_ctx(*body, id_gen, ctx, false);
            lower_lambda_hir(params, body, id_gen)
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            let scrutinee = if let Some(scrutinee) = scrutinee {
                lower_expr_ctx(*scrutinee, id_gen, ctx, false)
            } else {
                let param = "_arg0".to_string();
                let var = HirExpr::Var {
                    id: id_gen.next(),
                    name: param.clone(),
                };
                let match_expr = HirExpr::Match {
                    id: id_gen.next(),
                    scrutinee: Box::new(var),
                    arms: arms
                        .into_iter()
                        .map(|arm| HirMatchArm {
                            pattern: lower_pattern(arm.pattern, id_gen),
                            guard: arm
                                .guard
                                .map(|guard| lower_expr_ctx(guard, id_gen, ctx, false)),
                            body: lower_expr_ctx(arm.body, id_gen, ctx, false),
                        })
                        .collect(),
                };
                return HirExpr::Lambda {
                    id: id_gen.next(),
                    param,
                    body: Box::new(match_expr),
                };
            };
            HirExpr::Match {
                id: id_gen.next(),
                scrutinee: Box::new(scrutinee),
                arms: arms
                    .into_iter()
                    .map(|arm| HirMatchArm {
                        pattern: lower_pattern(arm.pattern, id_gen),
                        guard: arm
                            .guard
                            .map(|guard| lower_expr_ctx(guard, id_gen, ctx, false)),
                        body: lower_expr_ctx(arm.body, id_gen, ctx, false),
                    })
                    .collect(),
            }
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => HirExpr::If {
            id: id_gen.next(),
            cond: Box::new(lower_expr_ctx(*cond, id_gen, ctx, false)),
            then_branch: Box::new(lower_expr_ctx(*then_branch, id_gen, ctx, false)),
            else_branch: Box::new(lower_expr_ctx(*else_branch, id_gen, ctx, false)),
        },
        Expr::Binary {
            op, left, right, ..
        } => {
            if op == "|>" {
                let debug_pipes = ctx.debug.as_ref().is_some_and(|d| d.params.pipes);
                if debug_pipes && !in_pipe_left {
                    return lower_pipe_chain(*left, *right, id_gen, ctx);
                }
                let left = lower_expr_ctx(*left, id_gen, ctx, true);
                let right = lower_expr_ctx(*right, id_gen, ctx, false);
                return HirExpr::App {
                    id: id_gen.next(),
                    func: Box::new(right),
                    arg: Box::new(left),
                };
            }
            if op == "<|" {
                if let Expr::Record { fields, .. } = *right.clone() {
                    return HirExpr::Patch {
                        id: id_gen.next(),
                        target: Box::new(lower_expr_ctx(*left, id_gen, ctx, false)),
                        fields: fields
                            .into_iter()
                            .map(|field| HirRecordField {
                                spread: field.spread,
                                path: field
                                    .path
                                    .into_iter()
                                    .map(|segment| match segment {
                                        crate::surface::PathSegment::Field(name) => {
                                            HirPathSegment::Field(name.name)
                                        }
                                        crate::surface::PathSegment::Index(expr, _) => {
                                            HirPathSegment::Index(lower_expr_ctx(expr, id_gen, ctx, false))
                                        }
                                        crate::surface::PathSegment::All(_) => HirPathSegment::All,
                                    })
                                    .collect(),
                                value: lower_expr_ctx(field.value, id_gen, ctx, false),
                            })
                            .collect(),
                    };
                }
            }
            HirExpr::Binary {
                id: id_gen.next(),
                op,
                left: Box::new(lower_expr_ctx(*left, id_gen, ctx, false)),
                right: Box::new(lower_expr_ctx(*right, id_gen, ctx, false)),
            }
        }
        Expr::Block { kind, items, .. } => {
            let block_kind = lower_block_kind(&kind);
            HirExpr::Block {
                id: id_gen.next(),
                block_kind: block_kind.clone(),
                items: items
                    .into_iter()
                    .map(|item| lower_block_item_ctx(item, &kind, &block_kind, id_gen, ctx))
                    .collect(),
            }
        }
        Expr::Raw { text, .. } => HirExpr::Raw {
            id: id_gen.next(),
            text,
        },
    }
}

fn surface_expr_span(expr: &Expr) -> crate::diagnostics::Span {
    match expr {
        Expr::Ident(name) => name.span.clone(),
        Expr::Literal(literal) => match literal {
            crate::surface::Literal::Number { span, .. }
            | crate::surface::Literal::String { span, .. }
            | crate::surface::Literal::Sigil { span, .. }
            | crate::surface::Literal::Bool { span, .. }
            | crate::surface::Literal::DateTime { span, .. } => span.clone(),
        },
        Expr::TextInterpolate { span, .. }
        | Expr::List { span, .. }
        | Expr::Tuple { span, .. }
        | Expr::Record { span, .. }
        | Expr::PatchLit { span, .. }
        | Expr::FieldAccess { span, .. }
        | Expr::FieldSection { span, .. }
        | Expr::Index { span, .. }
        | Expr::Call { span, .. }
        | Expr::Lambda { span, .. }
        | Expr::Match { span, .. }
        | Expr::If { span, .. }
        | Expr::Binary { span, .. }
        | Expr::Block { span, .. }
        | Expr::Raw { span, .. } => span.clone(),
    }
}

fn slice_source_by_span(source: &str, span: &crate::diagnostics::Span) -> Option<String> {
    let lines: Vec<&str> = source.split('\n').collect();
    let start_line = span.start.line.checked_sub(1)?;
    let end_line = span.end.line.checked_sub(1)?;
    if start_line >= lines.len() || end_line >= lines.len() {
        return None;
    }

    fn slice_line(line: &str, start_col: usize, end_col: usize) -> String {
        let chars: Vec<char> = line.chars().collect();
        let start = start_col.saturating_sub(1).min(chars.len());
        let end = end_col.min(chars.len());
        chars[start..end].iter().collect()
    }

    if start_line == end_line {
        return Some(slice_line(lines[start_line], span.start.column, span.end.column));
    }

    let mut out = String::new();
    out.push_str(&slice_line(
        lines[start_line],
        span.start.column,
        lines[start_line].chars().count(),
    ));
    out.push('\n');
    for line in lines.iter().take(end_line).skip(start_line + 1) {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(&slice_line(lines[end_line], 1, span.end.column));
    Some(out)
}

fn normalize_debug_label(label: &str) -> String {
    let mut out = String::new();
    let mut prev_ws = false;
    for ch in label.chars() {
        if ch.is_whitespace() {
            if !prev_ws {
                out.push(' ');
                prev_ws = true;
            }
        } else {
            out.push(ch);
            prev_ws = false;
        }
    }
    out.trim().to_string()
}

fn lower_pipe_chain(left: Expr, right: Expr, id_gen: &mut IdGen, ctx: &mut LowerCtx<'_>) -> HirExpr {
    let Some(_) = ctx.debug.as_ref() else {
        let left = lower_expr_ctx(left, id_gen, ctx, true);
        let right = lower_expr_ctx(right, id_gen, ctx, false);
        return HirExpr::App {
            id: id_gen.next(),
            func: Box::new(right),
            arg: Box::new(left),
        };
    };

    let right_span = surface_expr_span(&right);
    let mut steps: Vec<(Expr, crate::diagnostics::Span)> = vec![(right, right_span)];
    let mut base = left;
    while let Expr::Binary {
        op,
        left,
        right,
        span,
    } = base
    {
        if op != "|>" {
            base = Expr::Binary { op, left, right, span };
            break;
        }
        let step_span = surface_expr_span(&right);
        steps.push((*right, step_span));
        base = *left;
    }
    steps.reverse();

    let (pipe_id, source, log_time) = {
        let debug = ctx.debug.as_mut().expect("debug ctx");
        (debug.alloc_pipe_id(), debug.source, debug.params.time)
    };
    let mut acc = lower_expr_ctx(base, id_gen, ctx, false);
    for (idx, (step_expr, step_span)) in steps.into_iter().enumerate() {
        let func = lower_expr_ctx(step_expr, id_gen, ctx, false);
        let label = source
            .and_then(|src| slice_source_by_span(src, &step_span))
            .map(|s| normalize_debug_label(&s))
            .unwrap_or_else(|| "<unknown>".to_string());
        acc = HirExpr::Pipe {
            id: id_gen.next(),
            pipe_id,
            step: (idx as u32) + 1,
            label,
            log_time,
            func: Box::new(func),
            arg: Box::new(acc),
        };
    }
    acc
}

fn lower_lambda_hir(params: Vec<Pattern>, body: HirExpr, id_gen: &mut IdGen) -> HirExpr {
    let mut acc = body;
    for (index, param) in params.into_iter().rev().enumerate() {
        match param {
            Pattern::Ident(name) => {
                acc = HirExpr::Lambda {
                    id: id_gen.next(),
                    param: name.name,
                    body: Box::new(acc),
                };
            }
            Pattern::Wildcard(_) => {
                acc = HirExpr::Lambda {
                    id: id_gen.next(),
                    param: format!("_arg{}", index),
                    body: Box::new(acc),
                };
            }
            other => {
                let param_name = format!("_arg{}", index);
                let match_expr = HirExpr::Match {
                    id: id_gen.next(),
                    scrutinee: Box::new(HirExpr::Var {
                        id: id_gen.next(),
                        name: param_name.clone(),
                    }),
                    arms: vec![HirMatchArm {
                        pattern: lower_pattern(other, id_gen),
                        guard: None,
                        body: acc,
                    }],
                };
                acc = HirExpr::Lambda {
                    id: id_gen.next(),
                    param: param_name,
                    body: Box::new(match_expr),
                };
            }
        }
    }
    acc
}

fn lower_block_kind(kind: &BlockKind) -> HirBlockKind {
    match kind {
        BlockKind::Plain => HirBlockKind::Plain,
        BlockKind::Effect => HirBlockKind::Effect,
        BlockKind::Generate => HirBlockKind::Generate,
        BlockKind::Resource => HirBlockKind::Resource,
    }
}

fn lower_block_item_ctx(
    item: BlockItem,
    surface_kind: &BlockKind,
    hir_kind: &HirBlockKind,
    id_gen: &mut IdGen,
    ctx: &mut LowerCtx<'_>,
) -> HirBlockItem {
    match item {
        BlockItem::Bind { pattern, expr, .. } => HirBlockItem::Bind {
            pattern: lower_pattern(pattern, id_gen),
            expr: lower_expr_ctx(expr, id_gen, ctx, false),
        },
        BlockItem::Let { pattern, expr, .. } => {
            let lowered_expr = lower_expr_ctx(expr, id_gen, ctx, false);
            let expr = if matches!(surface_kind, BlockKind::Effect)
                && matches!(hir_kind, HirBlockKind::Effect)
            {
                // `name = expr` inside `effect { ... }` is a pure let-binding and must not
                // implicitly run effects even if `expr` produces an `Effect` value.
                HirExpr::Call {
                    id: id_gen.next(),
                    func: Box::new(HirExpr::Var {
                        id: id_gen.next(),
                        name: "pure".to_string(),
                    }),
                    args: vec![lowered_expr],
                }
            } else {
                lowered_expr
            };
            HirBlockItem::Bind {
                pattern: lower_pattern(pattern, id_gen),
                expr,
            }
        }
        BlockItem::Filter { expr, .. } => HirBlockItem::Filter {
            expr: lower_expr_ctx(expr, id_gen, ctx, false),
        },
        BlockItem::Yield { expr, .. } => HirBlockItem::Yield {
            expr: lower_expr_ctx(expr, id_gen, ctx, false),
        },
        BlockItem::Recurse { expr, .. } => HirBlockItem::Recurse {
            expr: lower_expr_ctx(expr, id_gen, ctx, false),
        },
        BlockItem::Expr { expr, .. } => HirBlockItem::Expr {
            expr: lower_expr_ctx(expr, id_gen, ctx, false),
        },
    }
}

fn lower_pattern(pattern: Pattern, id_gen: &mut IdGen) -> HirPattern {
    match pattern {
        Pattern::Wildcard(_) => HirPattern::Wildcard { id: id_gen.next() },
        Pattern::Ident(name) => HirPattern::Var {
            id: id_gen.next(),
            name: name.name,
        },
        Pattern::Literal(literal) => HirPattern::Literal {
            id: id_gen.next(),
            value: match literal {
                crate::surface::Literal::Number { text, .. } => HirLiteral::Number(text),
                crate::surface::Literal::String { text, .. } => HirLiteral::String(text),
                crate::surface::Literal::Sigil {
                    tag, body, flags, ..
                } => HirLiteral::Sigil { tag, body, flags },
                crate::surface::Literal::Bool { value, .. } => HirLiteral::Bool(value),
                crate::surface::Literal::DateTime { text, .. } => HirLiteral::DateTime(text),
            },
        },
        Pattern::Constructor { name, args, .. } => HirPattern::Constructor {
            id: id_gen.next(),
            name: name.name,
            args: args
                .into_iter()
                .map(|arg| lower_pattern(arg, id_gen))
                .collect(),
        },
        Pattern::Tuple { items, .. } => HirPattern::Tuple {
            id: id_gen.next(),
            items: items
                .into_iter()
                .map(|item| lower_pattern(item, id_gen))
                .collect(),
        },
        Pattern::List { items, rest, .. } => HirPattern::List {
            id: id_gen.next(),
            items: items
                .into_iter()
                .map(|item| lower_pattern(item, id_gen))
                .collect(),
            rest: rest.map(|rest| Box::new(lower_pattern(*rest, id_gen))),
        },
        Pattern::Record { fields, .. } => HirPattern::Record {
            id: id_gen.next(),
            fields: fields
                .into_iter()
                .map(|field| HirRecordPatternField {
                    path: field.path.into_iter().map(|name| name.name).collect(),
                    pattern: lower_pattern(field.pattern, id_gen),
                })
                .collect(),
        },
    }
}

fn contains_placeholder(expr: &Expr) -> bool {
    match expr {
        Expr::Ident(name) => name.name == "_",
        Expr::Literal(_) => false,
        Expr::TextInterpolate { parts, .. } => parts.iter().any(|part| match part {
            TextPart::Text { .. } => false,
            TextPart::Expr { expr, .. } => contains_placeholder(expr),
        }),
        Expr::List { items, .. } => items.iter().any(|item| contains_placeholder(&item.expr)),
        Expr::Tuple { items, .. } => items.iter().any(contains_placeholder),
        Expr::Record { fields, .. } => fields.iter().any(|field| {
            field.path.iter().any(|segment| match segment {
                crate::surface::PathSegment::Index(expr, _) => contains_placeholder(expr),
                crate::surface::PathSegment::Field(_) | crate::surface::PathSegment::All(_) => {
                    false
                }
            }) || contains_placeholder(&field.value)
        }),
        Expr::PatchLit { fields, .. } => fields.iter().any(|field| {
            field.path.iter().any(|segment| match segment {
                crate::surface::PathSegment::Index(expr, _) => contains_placeholder(expr),
                crate::surface::PathSegment::Field(_) | crate::surface::PathSegment::All(_) => {
                    false
                }
            }) || contains_placeholder(&field.value)
        }),
        Expr::FieldAccess { base, .. } => contains_placeholder(base),
        // Field sections (`.field`) are handled directly during lowering.
        Expr::FieldSection { .. } => false,
        Expr::Index { base, index, .. } => {
            contains_placeholder(base) || contains_placeholder(index)
        }
        Expr::Call { func, args, .. } => {
            contains_placeholder(func) || args.iter().any(contains_placeholder)
        }
        Expr::Lambda { body, .. } => contains_placeholder(body),
        Expr::Match {
            scrutinee, arms, ..
        } => {
            scrutinee.as_deref().is_some_and(contains_placeholder)
                || arms.iter().any(|arm| {
                    arm.guard.as_ref().is_some_and(contains_placeholder)
                        || contains_placeholder(&arm.body)
                })
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            contains_placeholder(cond)
                || contains_placeholder(then_branch)
                || contains_placeholder(else_branch)
        }
        Expr::Binary { left, right, .. } => {
            contains_placeholder(left) || contains_placeholder(right)
        }
        Expr::Block { items, .. } => items.iter().any(|item| match item {
            BlockItem::Bind { expr, .. } => contains_placeholder(expr),
            BlockItem::Let { expr, .. } => contains_placeholder(expr),
            BlockItem::Filter { expr, .. }
            | BlockItem::Yield { expr, .. }
            | BlockItem::Recurse { expr, .. }
            | BlockItem::Expr { expr, .. } => contains_placeholder(expr),
        }),
        Expr::Raw { .. } => false,
    }
}

fn desugar_placeholder_lambdas(expr: Expr) -> Expr {
    let expr = match expr {
        Expr::Ident(name) => {
            // Don't desugar a placeholder `_` at the leaf; let the smallest
            // enclosing expression scope capture it. A bare `_` is handled in
            // `lower_expr_ctx` (which special-cases a leaf `_` into a lambda).
            if name.name == "_" {
                return Expr::Ident(name);
            }
            Expr::Ident(name)
        }
        Expr::Literal(_) | Expr::Raw { .. } | Expr::FieldSection { .. } => expr,
        Expr::TextInterpolate { parts, span } => Expr::TextInterpolate {
            parts: parts
                .into_iter()
                .map(|part| match part {
                    TextPart::Text { .. } => part,
                    TextPart::Expr { expr, span } => TextPart::Expr {
                        expr: Box::new(desugar_placeholder_lambdas(*expr)),
                        span,
                    },
                })
                .collect(),
            span,
        },
        Expr::List { items, span } => Expr::List {
            items: items
                .into_iter()
                .map(|item| crate::surface::ListItem {
                    expr: desugar_placeholder_lambdas(item.expr),
                    spread: item.spread,
                    span: item.span,
                })
                .collect(),
            span,
        },
        Expr::Tuple { items, span } => Expr::Tuple {
            items: items.into_iter().map(desugar_placeholder_lambdas).collect(),
            span,
        },
        Expr::Record { fields, span } => Expr::Record {
            fields: fields
                .into_iter()
                .map(|field| crate::surface::RecordField {
                    spread: field.spread,
                    path: field
                        .path
                        .into_iter()
                        .map(|segment| match segment {
                            crate::surface::PathSegment::Field(name) => {
                                crate::surface::PathSegment::Field(name)
                            }
                            crate::surface::PathSegment::Index(expr, span) => {
                                crate::surface::PathSegment::Index(
                                    desugar_placeholder_lambdas(expr),
                                    span,
                                )
                            }
                            crate::surface::PathSegment::All(span) => {
                                crate::surface::PathSegment::All(span)
                            }
                        })
                        .collect(),
                    value: desugar_placeholder_lambdas(field.value),
                    span: field.span,
                })
                .collect(),
            span,
        },
        Expr::PatchLit { fields, span } => Expr::PatchLit {
            fields: fields
                .into_iter()
                .map(|field| crate::surface::RecordField {
                    spread: field.spread,
                    path: field
                        .path
                        .into_iter()
                        .map(|segment| match segment {
                            crate::surface::PathSegment::Field(name) => {
                                crate::surface::PathSegment::Field(name)
                            }
                            crate::surface::PathSegment::Index(expr, span) => {
                                crate::surface::PathSegment::Index(
                                    desugar_placeholder_lambdas(expr),
                                    span,
                                )
                            }
                            crate::surface::PathSegment::All(span) => {
                                crate::surface::PathSegment::All(span)
                            }
                        })
                        .collect(),
                    value: desugar_placeholder_lambdas(field.value),
                    span: field.span,
                })
                .collect(),
            span,
        },
        Expr::FieldAccess { base, field, span } => Expr::FieldAccess {
            base: Box::new(desugar_placeholder_lambdas(*base)),
            field,
            span,
        },
        Expr::Index { base, index, span } => Expr::Index {
            base: Box::new(desugar_placeholder_lambdas(*base)),
            index: Box::new(desugar_placeholder_lambdas(*index)),
            span,
        },
        Expr::Call { func, args, span } => Expr::Call {
            func: Box::new(desugar_placeholder_lambdas(*func)),
            args: args.into_iter().map(desugar_placeholder_lambdas).collect(),
            span,
        },
        Expr::Lambda { params, body, span } => Expr::Lambda {
            params,
            body: Box::new(desugar_placeholder_lambdas(*body)),
            span,
        },
        Expr::Match {
            scrutinee,
            arms,
            span,
        } => Expr::Match {
            scrutinee: scrutinee.map(|expr| Box::new(desugar_placeholder_lambdas(*expr))),
            arms: arms
                .into_iter()
                .map(|arm| crate::surface::MatchArm {
                    pattern: arm.pattern,
                    guard: arm.guard.map(desugar_placeholder_lambdas),
                    body: desugar_placeholder_lambdas(arm.body),
                    span: arm.span,
                })
                .collect(),
            span,
        },
        Expr::If {
            cond,
            then_branch,
            else_branch,
            span,
        } => Expr::If {
            cond: Box::new(desugar_placeholder_lambdas(*cond)),
            then_branch: Box::new(desugar_placeholder_lambdas(*then_branch)),
            else_branch: Box::new(desugar_placeholder_lambdas(*else_branch)),
            span,
        },
        Expr::Binary {
            op,
            left,
            right,
            span,
        } => Expr::Binary {
            op,
            left: Box::new(desugar_placeholder_lambdas(*left)),
            right: Box::new(desugar_placeholder_lambdas(*right)),
            span,
        },
        Expr::Block { kind, items, span } => Expr::Block {
            kind,
            items: items
                .into_iter()
                .map(|item| match item {
                    BlockItem::Bind {
                        pattern,
                        expr,
                        span,
                    } => BlockItem::Bind {
                        pattern,
                        expr: desugar_placeholder_lambdas(expr),
                        span,
                    },
                    BlockItem::Let {
                        pattern,
                        expr,
                        span,
                    } => BlockItem::Let {
                        pattern,
                        expr: desugar_placeholder_lambdas(expr),
                        span,
                    },
                    BlockItem::Filter { expr, span } => BlockItem::Filter {
                        expr: desugar_placeholder_lambdas(expr),
                        span,
                    },
                    BlockItem::Yield { expr, span } => BlockItem::Yield {
                        expr: desugar_placeholder_lambdas(expr),
                        span,
                    },
                    BlockItem::Recurse { expr, span } => BlockItem::Recurse {
                        expr: desugar_placeholder_lambdas(expr),
                        span,
                    },
                    BlockItem::Expr { expr, span } => BlockItem::Expr {
                        expr: desugar_placeholder_lambdas(expr),
                        span,
                    },
                })
                .collect(),
            span,
        },
    };

    if !contains_placeholder(&expr) {
        return expr;
    }

    let (rewritten, params) = replace_holes(expr);
    let span = match &rewritten {
        Expr::Ident(name) => name.span.clone(),
        Expr::Literal(lit) => match lit {
            crate::surface::Literal::Number { span, .. }
            | crate::surface::Literal::String { span, .. }
            | crate::surface::Literal::Sigil { span, .. }
            | crate::surface::Literal::Bool { span, .. }
            | crate::surface::Literal::DateTime { span, .. } => span.clone(),
        },
        Expr::TextInterpolate { span, .. }
        | Expr::List { span, .. }
        | Expr::Tuple { span, .. }
        | Expr::Record { span, .. }
        | Expr::PatchLit { span, .. }
        | Expr::FieldAccess { span, .. }
        | Expr::FieldSection { span, .. }
        | Expr::Index { span, .. }
        | Expr::Call { span, .. }
        | Expr::Lambda { span, .. }
        | Expr::Match { span, .. }
        | Expr::If { span, .. }
        | Expr::Binary { span, .. }
        | Expr::Block { span, .. }
        | Expr::Raw { span, .. } => span.clone(),
    };

    Expr::Lambda {
        params: params
            .into_iter()
            .map(|name| {
                Pattern::Ident(crate::surface::SpannedName {
                    name,
                    span: span.clone(),
                })
            })
            .collect(),
        body: Box::new(rewritten),
        span,
    }
}

fn replace_holes(expr: Expr) -> (Expr, Vec<String>) {
    let mut counter = 0;
    let mut params = Vec::new();
    let rewritten = replace_holes_inner(expr, &mut counter, &mut params);
    (rewritten, params)
}

fn replace_holes_inner(expr: Expr, counter: &mut u32, params: &mut Vec<String>) -> Expr {
    match expr {
        Expr::Ident(name) if name.name == "_" => {
            let param = format!("_arg{}", counter);
            *counter += 1;
            params.push(param.clone());
            Expr::Ident(crate::surface::SpannedName {
                name: param,
                span: name.span,
            })
        }
        Expr::Ident(_) | Expr::Literal(_) | Expr::Raw { .. } => expr,
        Expr::TextInterpolate { parts, span } => Expr::TextInterpolate {
            parts: parts
                .into_iter()
                .map(|part| match part {
                    TextPart::Text { .. } => part,
                    TextPart::Expr { expr, span } => TextPart::Expr {
                        expr: Box::new(replace_holes_inner(*expr, counter, params)),
                        span,
                    },
                })
                .collect(),
            span,
        },
        Expr::List { items, span } => Expr::List {
            items: items
                .into_iter()
                .map(|item| crate::surface::ListItem {
                    expr: replace_holes_inner(item.expr, counter, params),
                    spread: item.spread,
                    span: item.span,
                })
                .collect(),
            span,
        },
        Expr::Tuple { items, span } => Expr::Tuple {
            items: items
                .into_iter()
                .map(|item| replace_holes_inner(item, counter, params))
                .collect(),
            span,
        },
        Expr::Record { fields, span } => Expr::Record {
            fields: fields
                .into_iter()
                .map(|field| crate::surface::RecordField {
                    spread: field.spread,
                    path: field
                        .path
                        .into_iter()
                        .map(|segment| match segment {
                            crate::surface::PathSegment::Field(name) => {
                                crate::surface::PathSegment::Field(name)
                            }
                            crate::surface::PathSegment::Index(expr, span) => {
                                crate::surface::PathSegment::Index(
                                    replace_holes_inner(expr, counter, params),
                                    span,
                                )
                            }
                            crate::surface::PathSegment::All(span) => {
                                crate::surface::PathSegment::All(span)
                            }
                        })
                        .collect(),
                    value: replace_holes_inner(field.value, counter, params),
                    span: field.span,
                })
                .collect(),
            span,
        },
        Expr::PatchLit { fields, span } => Expr::PatchLit {
            fields: fields
                .into_iter()
                .map(|field| crate::surface::RecordField {
                    spread: field.spread,
                    path: field
                        .path
                        .into_iter()
                        .map(|segment| match segment {
                            crate::surface::PathSegment::Field(name) => {
                                crate::surface::PathSegment::Field(name)
                            }
                            crate::surface::PathSegment::Index(expr, span) => {
                                crate::surface::PathSegment::Index(
                                    replace_holes_inner(expr, counter, params),
                                    span,
                                )
                            }
                            crate::surface::PathSegment::All(span) => {
                                crate::surface::PathSegment::All(span)
                            }
                        })
                        .collect(),
                    value: replace_holes_inner(field.value, counter, params),
                    span: field.span,
                })
                .collect(),
            span,
        },
        Expr::FieldAccess { base, field, span } => Expr::FieldAccess {
            base: Box::new(replace_holes_inner(*base, counter, params)),
            field,
            span,
        },
        Expr::FieldSection { .. } => expr,
        Expr::Index { base, index, span } => Expr::Index {
            base: Box::new(replace_holes_inner(*base, counter, params)),
            index: Box::new(replace_holes_inner(*index, counter, params)),
            span,
        },
        Expr::Call { func, args, span } => Expr::Call {
            func: Box::new(replace_holes_inner(*func, counter, params)),
            args: args
                .into_iter()
                .map(|arg| replace_holes_inner(arg, counter, params))
                .collect(),
            span,
        },
        Expr::Lambda {
            params: lambda_params,
            body,
            span,
        } => Expr::Lambda {
            params: lambda_params,
            body: Box::new(replace_holes_inner(*body, counter, params)),
            span,
        },
        Expr::Match {
            scrutinee,
            arms,
            span,
        } => Expr::Match {
            scrutinee: scrutinee.map(|expr| Box::new(replace_holes_inner(*expr, counter, params))),
            arms: arms
                .into_iter()
                .map(|arm| crate::surface::MatchArm {
                    pattern: arm.pattern,
                    guard: arm
                        .guard
                        .map(|guard| replace_holes_inner(guard, counter, params)),
                    body: replace_holes_inner(arm.body, counter, params),
                    span: arm.span,
                })
                .collect(),
            span,
        },
        Expr::If {
            cond,
            then_branch,
            else_branch,
            span,
        } => Expr::If {
            cond: Box::new(replace_holes_inner(*cond, counter, params)),
            then_branch: Box::new(replace_holes_inner(*then_branch, counter, params)),
            else_branch: Box::new(replace_holes_inner(*else_branch, counter, params)),
            span,
        },
        Expr::Binary {
            op,
            left,
            right,
            span,
        } => Expr::Binary {
            op,
            left: Box::new(replace_holes_inner(*left, counter, params)),
            right: Box::new(replace_holes_inner(*right, counter, params)),
            span,
        },
        Expr::Block { kind, items, span } => Expr::Block {
            kind,
            items: items
                .into_iter()
                .map(|item| match item {
                    BlockItem::Bind {
                        pattern,
                        expr,
                        span,
                    } => BlockItem::Bind {
                        pattern,
                        expr: replace_holes_inner(expr, counter, params),
                        span,
                    },
                    BlockItem::Let {
                        pattern,
                        expr,
                        span,
                    } => BlockItem::Let {
                        pattern,
                        expr: replace_holes_inner(expr, counter, params),
                        span,
                    },
                    BlockItem::Filter { expr, span } => BlockItem::Filter {
                        expr: replace_holes_inner(expr, counter, params),
                        span,
                    },
                    BlockItem::Yield { expr, span } => BlockItem::Yield {
                        expr: replace_holes_inner(expr, counter, params),
                        span,
                    },
                    BlockItem::Recurse { expr, span } => BlockItem::Recurse {
                        expr: replace_holes_inner(expr, counter, params),
                        span,
                    },
                    BlockItem::Expr { expr, span } => BlockItem::Expr {
                        expr: replace_holes_inner(expr, counter, params),
                        span,
                    },
                })
                .collect(),
            span,
        },
    }
}

#[derive(Default)]
struct IdGen {
    next: u32,
}

impl IdGen {
    fn next(&mut self) -> u32 {
        let id = self.next;
        self.next += 1;
        id
    }
}

#[cfg(test)]
mod debug_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn contains_debug_nodes(expr: &HirExpr) -> bool {
        match expr {
            HirExpr::DebugFn { .. } => true,
            HirExpr::Pipe { .. } => true,
            HirExpr::Lambda { body, .. } => contains_debug_nodes(body),
            HirExpr::App { func, arg, .. } => contains_debug_nodes(func) || contains_debug_nodes(arg),
            HirExpr::Call { func, args, .. } => {
                contains_debug_nodes(func) || args.iter().any(contains_debug_nodes)
            }
            HirExpr::TextInterpolate { parts, .. } => parts.iter().any(|p| match p {
                HirTextPart::Expr { expr } => contains_debug_nodes(expr),
                _ => false,
            }),
            HirExpr::List { items, .. } => items.iter().any(|i| contains_debug_nodes(&i.expr)),
            HirExpr::Tuple { items, .. } => items.iter().any(contains_debug_nodes),
            HirExpr::Record { fields, .. } => fields.iter().any(|f| contains_debug_nodes(&f.value)),
            HirExpr::Patch { target, fields, .. } => {
                contains_debug_nodes(target) || fields.iter().any(|f| contains_debug_nodes(&f.value))
            }
            HirExpr::FieldAccess { base, .. } => contains_debug_nodes(base),
            HirExpr::Index { base, index, .. } => contains_debug_nodes(base) || contains_debug_nodes(index),
            HirExpr::Match { scrutinee, arms, .. } => {
                contains_debug_nodes(scrutinee) || arms.iter().any(|a| contains_debug_nodes(&a.body))
            }
            HirExpr::If { cond, then_branch, else_branch, .. } => {
                contains_debug_nodes(cond) || contains_debug_nodes(then_branch) || contains_debug_nodes(else_branch)
            }
            HirExpr::Binary { left, right, .. } => contains_debug_nodes(left) || contains_debug_nodes(right),
            HirExpr::Block { items, .. } => items.iter().any(|i| match i {
                HirBlockItem::Bind { expr, .. } | HirBlockItem::Expr { expr } => contains_debug_nodes(expr),
                _ => false,
            }),
            HirExpr::Var { .. }
            | HirExpr::LitNumber { .. }
            | HirExpr::LitString { .. }
            | HirExpr::LitSigil { .. }
            | HirExpr::LitBool { .. }
            | HirExpr::LitDateTime { .. }
            | HirExpr::Raw { .. } => false,
        }
    }

    fn collect_pipes(expr: &HirExpr, out: &mut Vec<(u32, u32, String)>) {
        match expr {
            HirExpr::Pipe {
                pipe_id, step, label, func, arg, ..
            } => {
                out.push((*pipe_id, *step, label.clone()));
                collect_pipes(func, out);
                collect_pipes(arg, out);
            }
            HirExpr::DebugFn { body, .. } => collect_pipes(body, out),
            HirExpr::Lambda { body, .. } => collect_pipes(body, out),
            HirExpr::App { func, arg, .. } => {
                collect_pipes(func, out);
                collect_pipes(arg, out);
            }
            HirExpr::Call { func, args, .. } => {
                collect_pipes(func, out);
                for arg in args {
                    collect_pipes(arg, out);
                }
            }
            HirExpr::TextInterpolate { parts, .. } => {
                for part in parts {
                    if let HirTextPart::Expr { expr } = part {
                        collect_pipes(expr, out);
                    }
                }
            }
            HirExpr::List { items, .. } => {
                for item in items {
                    collect_pipes(&item.expr, out);
                }
            }
            HirExpr::Tuple { items, .. } => {
                for item in items {
                    collect_pipes(item, out);
                }
            }
            HirExpr::Record { fields, .. } => {
                for field in fields {
                    collect_pipes(&field.value, out);
                }
            }
            HirExpr::Patch { target, fields, .. } => {
                collect_pipes(target, out);
                for field in fields {
                    collect_pipes(&field.value, out);
                }
            }
            HirExpr::FieldAccess { base, .. } => collect_pipes(base, out),
            HirExpr::Index { base, index, .. } => {
                collect_pipes(base, out);
                collect_pipes(index, out);
            }
            HirExpr::Match { scrutinee, arms, .. } => {
                collect_pipes(scrutinee, out);
                for arm in arms {
                    collect_pipes(&arm.body, out);
                }
            }
            HirExpr::If { cond, then_branch, else_branch, .. } => {
                collect_pipes(cond, out);
                collect_pipes(then_branch, out);
                collect_pipes(else_branch, out);
            }
            HirExpr::Binary { left, right, .. } => {
                collect_pipes(left, out);
                collect_pipes(right, out);
            }
            HirExpr::Block { items, .. } => {
                for item in items {
                    match item {
                        HirBlockItem::Bind { expr, .. } | HirBlockItem::Expr { expr } => {
                            collect_pipes(expr, out);
                        }
                        _ => {}
                    }
                }
            }
            HirExpr::Var { .. }
            | HirExpr::LitNumber { .. }
            | HirExpr::LitString { .. }
            | HirExpr::LitSigil { .. }
            | HirExpr::LitBool { .. }
            | HirExpr::LitDateTime { .. }
            | HirExpr::Raw { .. } => {}
        }
    }

    fn with_debug_trace(enabled: bool, f: impl FnOnce()) {
        super::DEBUG_TRACE_OVERRIDE.with(|cell| {
            let prev = cell.get();
            cell.set(Some(enabled));
            f();
            cell.set(prev);
        });
    }

    fn write_temp_source(source: &str) -> std::path::PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let mut path = std::env::temp_dir();
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let filename = format!("aivi_debug_{}_{}.aivi", std::process::id(), id);
        path.push(filename);
        std::fs::write(&path, source).expect("write temp source");
        path
    }

    #[test]
    fn debug_erased_when_flag_off() {
        let source = r#"
module test.debug

@debug(pipes, args, return, time)
f x = x |> g 1 |> h
"#;
        let path = write_temp_source(source);
        with_debug_trace(false, || {
            let (modules, diags) = crate::surface::parse_modules(&path, source);
            assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
            let program = desugar_modules(&modules);
            let module = program.modules.into_iter().next().expect("module");
            let def = module.defs.into_iter().find(|d| d.name == "f").expect("f");
            assert!(!contains_debug_nodes(&def.expr));
        });
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn debug_instruments_pipes_and_labels() {
        let source = r#"
module test.debug

g n x = x + n
h x = x * 2

@debug(pipes, time)
f x = x |> g 1 |> h
"#;
        let path = write_temp_source(source);
        with_debug_trace(true, || {
            let (modules, diags) = crate::surface::parse_modules(&path, source);
            assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
            let surface_def = match &modules[0].items[2] {
                ModuleItem::Def(def) => def,
                other => panic!("expected def item, got {other:?}"),
            };
            let params = super::parse_debug_params(&surface_def.decorators).expect("debug params");
            assert!(params.pipes);
            assert!(params.time);
            let program = desugar_modules(&modules);
            let module = program.modules.into_iter().next().expect("module");
            let def = module.defs.into_iter().find(|d| d.name == "f").expect("f");

            assert!(contains_debug_nodes(&def.expr));

            let mut pipes = Vec::new();
            collect_pipes(&def.expr, &mut pipes);
            pipes.sort_by_key(|(pipe_id, step, _)| (*pipe_id, *step));
            assert_eq!(pipes.len(), 2);
            assert_eq!(pipes[0].0, 1);
            assert_eq!(pipes[0].1, 1);
            assert_eq!(pipes[0].2, "g 1");
            assert_eq!(pipes[1].0, 1);
            assert_eq!(pipes[1].1, 2);
            assert_eq!(pipes[1].2, "h");
        });
        let _ = std::fs::remove_file(path);
    }
}
