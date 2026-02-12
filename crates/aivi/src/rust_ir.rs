use serde::{Deserialize, Serialize};

use crate::kernel::{
    KernelBlockItem, KernelBlockKind, KernelDef, KernelExpr, KernelMatchArm, KernelModule,
    KernelPathSegment, KernelPattern, KernelProgram, KernelRecordField,
};
use crate::AiviError;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RustIrProgram {
    pub modules: Vec<RustIrModule>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RustIrModule {
    pub name: String,
    pub defs: Vec<RustIrDef>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RustIrDef {
    pub name: String,
    pub expr: RustIrExpr,
}

pub type BuiltinName = String;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "kind")]
pub enum RustIrTextPart {
    Text { text: String },
    Expr { expr: RustIrExpr },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "kind")]
pub enum RustIrExpr {
    Local {
        id: u32,
        name: String,
    },
    Global {
        id: u32,
        name: String,
    },
    Builtin {
        id: u32,
        builtin: BuiltinName,
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
        parts: Vec<RustIrTextPart>,
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
        body: Box<RustIrExpr>,
    },
    App {
        id: u32,
        func: Box<RustIrExpr>,
        arg: Box<RustIrExpr>,
    },
    Call {
        id: u32,
        func: Box<RustIrExpr>,
        args: Vec<RustIrExpr>,
    },
    List {
        id: u32,
        items: Vec<RustIrListItem>,
    },
    Tuple {
        id: u32,
        items: Vec<RustIrExpr>,
    },
    Record {
        id: u32,
        fields: Vec<RustIrRecordField>,
    },
    Patch {
        id: u32,
        target: Box<RustIrExpr>,
        fields: Vec<RustIrRecordField>,
    },
    FieldAccess {
        id: u32,
        base: Box<RustIrExpr>,
        field: String,
    },
    Index {
        id: u32,
        base: Box<RustIrExpr>,
        index: Box<RustIrExpr>,
    },
    Match {
        id: u32,
        scrutinee: Box<RustIrExpr>,
        arms: Vec<RustIrMatchArm>,
    },
    If {
        id: u32,
        cond: Box<RustIrExpr>,
        then_branch: Box<RustIrExpr>,
        else_branch: Box<RustIrExpr>,
    },
    Binary {
        id: u32,
        op: String,
        left: Box<RustIrExpr>,
        right: Box<RustIrExpr>,
    },
    Block {
        id: u32,
        block_kind: RustIrBlockKind,
        items: Vec<RustIrBlockItem>,
    },
    Raw {
        id: u32,
        text: String,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RustIrListItem {
    pub expr: RustIrExpr,
    pub spread: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RustIrRecordField {
    pub spread: bool,
    pub path: Vec<RustIrPathSegment>,
    pub value: RustIrExpr,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum RustIrPathSegment {
    Field(String),
    IndexValue(RustIrExpr),
    IndexFieldBool(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RustIrMatchArm {
    pub pattern: RustIrPattern,
    pub guard: Option<RustIrExpr>,
    pub body: RustIrExpr,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum RustIrPattern {
    Wildcard { id: u32 },
    Var { id: u32, name: String },
    Literal { id: u32, value: RustIrLiteral },
    Constructor {
        id: u32,
        name: String,
        args: Vec<RustIrPattern>,
    },
    Tuple {
        id: u32,
        items: Vec<RustIrPattern>,
    },
    List {
        id: u32,
        items: Vec<RustIrPattern>,
        rest: Option<Box<RustIrPattern>>,
    },
    Record {
        id: u32,
        fields: Vec<RustIrRecordPatternField>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RustIrRecordPatternField {
    pub path: Vec<String>,
    pub pattern: RustIrPattern,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum RustIrLiteral {
    Number(String),
    String(String),
    Sigil { tag: String, body: String, flags: String },
    Bool(bool),
    DateTime(String),
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum RustIrBlockKind {
    Plain,
    Effect,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum RustIrBlockItem {
    Bind {
        pattern: RustIrPattern,
        expr: RustIrExpr,
    },
    Expr {
        expr: RustIrExpr,
    },
}

pub fn lower_kernel(program: KernelProgram) -> Result<RustIrProgram, AiviError> {
    // Runtime/global namespace today is "flat": all defs from all modules end up in the same
    // global environment. For native codegen we mirror that so cross-module references lower.
    let globals: Vec<String> = program
        .modules
        .iter()
        .flat_map(|m| m.defs.iter().map(|d| d.name.clone()))
        .collect();

    let mut modules = Vec::new();
    for module in program.modules {
        modules.push(lower_module(module, &globals)?);
    }
    Ok(RustIrProgram { modules })
}

fn lower_module(module: KernelModule, globals: &[String]) -> Result<RustIrModule, AiviError> {
    let mut defs = Vec::new();
    for def in module.defs {
        defs.push(lower_def(def, globals)?);
    }
    Ok(RustIrModule {
        name: module.name,
        defs,
    })
}

fn lower_def(def: KernelDef, globals: &[String]) -> Result<RustIrDef, AiviError> {
    let mut locals = Vec::new();
    let expr = lower_expr(def.expr, globals, &mut locals)?;
    Ok(RustIrDef {
        name: def.name,
        expr,
    })
}

fn lower_expr(
    expr: KernelExpr,
    globals: &[String],
    locals: &mut Vec<String>,
) -> Result<RustIrExpr, AiviError> {
    Ok(match expr {
        KernelExpr::Var { id, name } => {
            if locals.iter().rev().any(|local| local == &name) {
                RustIrExpr::Local { id, name }
            } else if let Some(builtin) = resolve_builtin(&name) {
                RustIrExpr::Builtin { id, builtin }
            } else if globals.iter().any(|g| g == &name) {
                RustIrExpr::Global { id, name }
            } else {
                return Err(AiviError::Codegen(format!("unbound variable {name}")));
            }
        }
        KernelExpr::LitNumber { id, text } => RustIrExpr::LitNumber { id, text },
        KernelExpr::LitString { id, text } => RustIrExpr::LitString { id, text },
        KernelExpr::TextInterpolate { id, parts } => RustIrExpr::TextInterpolate {
            id,
            parts: parts
                .into_iter()
                .map(|part| {
                    Ok::<RustIrTextPart, AiviError>(match part {
                        crate::kernel::KernelTextPart::Text { text } => {
                            RustIrTextPart::Text { text }
                        }
                        crate::kernel::KernelTextPart::Expr { expr } => RustIrTextPart::Expr {
                            expr: lower_expr(expr, globals, locals)?,
                        },
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
        },
        KernelExpr::LitSigil {
            id,
            tag,
            body,
            flags,
        } => RustIrExpr::LitSigil {
            id,
            tag,
            body,
            flags,
        },
        KernelExpr::LitBool { id, value } => RustIrExpr::LitBool { id, value },
        KernelExpr::LitDateTime { id, text } => RustIrExpr::LitDateTime { id, text },
        KernelExpr::Lambda { id, param, body } => {
            locals.push(param.clone());
            let body = lower_expr(*body, globals, locals)?;
            locals.pop();
            RustIrExpr::Lambda {
                id,
                param,
                body: Box::new(body),
            }
        }
        KernelExpr::App { id, func, arg } => RustIrExpr::App {
            id,
            func: Box::new(lower_expr(*func, globals, locals)?),
            arg: Box::new(lower_expr(*arg, globals, locals)?),
        },
        KernelExpr::Call { id, func, args } => RustIrExpr::Call {
            id,
            func: Box::new(lower_expr(*func, globals, locals)?),
            args: args
                .into_iter()
                .map(|arg| lower_expr(arg, globals, locals))
                .collect::<Result<Vec<_>, _>>()?,
        },
        KernelExpr::List { id, items } => RustIrExpr::List {
            id,
            items: items
                .into_iter()
                .map(|item| {
                    Ok(RustIrListItem {
                        expr: lower_expr(item.expr, globals, locals)?,
                        spread: item.spread,
                    })
                })
                .collect::<Result<Vec<_>, AiviError>>()?,
        },
        KernelExpr::Tuple { id, items } => RustIrExpr::Tuple {
            id,
            items: items
                .into_iter()
                .map(|item| lower_expr(item, globals, locals))
                .collect::<Result<Vec<_>, _>>()?,
        },
        KernelExpr::Record { id, fields } => RustIrExpr::Record {
            id,
            fields: fields
                .into_iter()
                .map(|field| lower_record_field(field, globals, locals))
                .collect::<Result<Vec<_>, _>>()?,
        },
        KernelExpr::Patch { id, target, fields } => RustIrExpr::Patch {
            id,
            target: Box::new(lower_expr(*target, globals, locals)?),
            fields: fields
                .into_iter()
                .map(|field| lower_record_field(field, globals, locals))
                .collect::<Result<Vec<_>, _>>()?,
        },
        KernelExpr::FieldAccess { id, base, field } => RustIrExpr::FieldAccess {
            id,
            base: Box::new(lower_expr(*base, globals, locals)?),
            field,
        },
        KernelExpr::Index { id, base, index } => RustIrExpr::Index {
            id,
            base: Box::new(lower_expr(*base, globals, locals)?),
            index: Box::new(lower_expr(*index, globals, locals)?),
        },
        KernelExpr::Match {
            id,
            scrutinee,
            arms,
        } => RustIrExpr::Match {
            id,
            scrutinee: Box::new(lower_expr(*scrutinee, globals, locals)?),
            arms: arms
                .into_iter()
                .map(|arm| lower_match_arm(arm, globals, locals))
                .collect::<Result<Vec<_>, _>>()?,
        },
        KernelExpr::If {
            id,
            cond,
            then_branch,
            else_branch,
        } => RustIrExpr::If {
            id,
            cond: Box::new(lower_expr(*cond, globals, locals)?),
            then_branch: Box::new(lower_expr(*then_branch, globals, locals)?),
            else_branch: Box::new(lower_expr(*else_branch, globals, locals)?),
        },
        KernelExpr::Binary {
            id,
            op,
            left,
            right,
        } => RustIrExpr::Binary {
            id,
            op,
            left: Box::new(lower_expr(*left, globals, locals)?),
            right: Box::new(lower_expr(*right, globals, locals)?),
        },
        KernelExpr::Block {
            id,
            block_kind,
            items,
        } => RustIrExpr::Block {
            id,
            block_kind: lower_block_kind(block_kind)?,
            items: {
                let before = locals.len();
                let lowered = items
                    .into_iter()
                    .map(|item| lower_block_item(item, globals, locals))
                    .collect::<Result<Vec<_>, _>>()?;
                locals.truncate(before);
                lowered
            },
        },
        KernelExpr::Raw { id, text } => RustIrExpr::Raw { id, text },
    })
}

fn lower_record_field(
    field: KernelRecordField,
    globals: &[String],
    locals: &mut Vec<String>,
) -> Result<RustIrRecordField, AiviError> {
    let mut out_path = Vec::with_capacity(field.path.len());
    for seg in field.path {
        out_path.push(lower_path_segment(seg, globals, locals)?);
    }
    Ok(RustIrRecordField {
        spread: field.spread,
        path: out_path,
        value: lower_expr(field.value, globals, locals)?,
    })
}

fn lower_path_segment(
    seg: KernelPathSegment,
    globals: &[String],
    locals: &mut Vec<String>,
) -> Result<RustIrPathSegment, AiviError> {
    match seg {
        KernelPathSegment::Field(name) => Ok(RustIrPathSegment::Field(name)),
        KernelPathSegment::Index(expr) => match expr {
            KernelExpr::Var { id, name } => {
                let is_bound = locals.iter().rev().any(|local| local == &name)
                    || globals.iter().any(|g| g == &name)
                    || resolve_builtin(&name).is_some();
                if is_bound {
                    Ok(RustIrPathSegment::IndexValue(lower_expr(
                        KernelExpr::Var { id, name },
                        globals,
                        locals,
                    )?))
                } else {
                    Ok(RustIrPathSegment::IndexFieldBool(name))
                }
            }
            other => Ok(RustIrPathSegment::IndexValue(lower_expr(
                other, globals, locals,
            )?)),
        },
    }
}

fn lower_block_kind(kind: KernelBlockKind) -> Result<RustIrBlockKind, AiviError> {
    match kind {
        KernelBlockKind::Plain => Ok(RustIrBlockKind::Plain),
        KernelBlockKind::Effect => Ok(RustIrBlockKind::Effect),
        KernelBlockKind::Generate | KernelBlockKind::Resource => Err(AiviError::Codegen(
            "generate/resource blocks are not supported by the rustc backend yet".to_string(),
        )),
    }
}

fn lower_block_item(
    item: KernelBlockItem,
    globals: &[String],
    locals: &mut Vec<String>,
) -> Result<RustIrBlockItem, AiviError> {
    match item {
        KernelBlockItem::Bind { pattern, expr } => {
            let pat = lower_pattern(pattern)?;
            match &pat {
                RustIrPattern::Var { name, .. } => locals.push(name.clone()),
                RustIrPattern::Wildcard { .. } => {}
                _ => {
                    return Err(AiviError::Codegen(
                        "only wildcard/variable patterns are supported in block binds".to_string(),
                    ))
                }
            }
            Ok(RustIrBlockItem::Bind {
                pattern: pat,
                expr: lower_expr(expr, globals, locals)?,
            })
        }
        KernelBlockItem::Expr { expr } => Ok(RustIrBlockItem::Expr {
            expr: lower_expr(expr, globals, locals)?,
        }),
        KernelBlockItem::Filter { .. }
        | KernelBlockItem::Yield { .. }
        | KernelBlockItem::Recurse { .. } => Err(AiviError::Codegen(
            "generator/resource block items are not supported by the rustc backend yet".to_string(),
        )),
    }
}

fn lower_pattern(pattern: KernelPattern) -> Result<RustIrPattern, AiviError> {
    match pattern {
        KernelPattern::Wildcard { id } => Ok(RustIrPattern::Wildcard { id }),
        KernelPattern::Var { id, name } => Ok(RustIrPattern::Var { id, name }),
        KernelPattern::Literal { id, value } => Ok(RustIrPattern::Literal {
            id,
            value: lower_literal(value),
        }),
        KernelPattern::Constructor { id, name, args } => Ok(RustIrPattern::Constructor {
            id,
            name,
            args: args
                .into_iter()
                .map(lower_pattern)
                .collect::<Result<Vec<_>, _>>()?,
        }),
        KernelPattern::Tuple { id, items } => Ok(RustIrPattern::Tuple {
            id,
            items: items
                .into_iter()
                .map(lower_pattern)
                .collect::<Result<Vec<_>, _>>()?,
        }),
        KernelPattern::List { id, items, rest } => Ok(RustIrPattern::List {
            id,
            items: items
                .into_iter()
                .map(lower_pattern)
                .collect::<Result<Vec<_>, _>>()?,
            rest: rest.map(|p| lower_pattern(*p).map(Box::new)).transpose()?,
        }),
        KernelPattern::Record { id, fields } => Ok(RustIrPattern::Record {
            id,
            fields: fields
                .into_iter()
                .map(|f| {
                    Ok::<RustIrRecordPatternField, AiviError>(RustIrRecordPatternField {
                        path: f.path,
                        pattern: lower_pattern(f.pattern)?,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
        }),
    }
}

fn lower_match_arm(
    arm: KernelMatchArm,
    globals: &[String],
    locals: &mut Vec<String>,
) -> Result<RustIrMatchArm, AiviError> {
    // Pattern bindings are introduced as locals for the arm's guard/body.
    // We conservatively extend `locals` while lowering the guard/body.
    let before = locals.len();
    let mut binders = Vec::new();
    collect_pattern_binders(&arm.pattern, &mut binders);
    for name in binders {
        locals.push(name);
    }
    let guard = arm
        .guard
        .map(|g| lower_expr(g, globals, locals))
        .transpose()?;
    let body = lower_expr(arm.body, globals, locals)?;
    locals.truncate(before);
    Ok(RustIrMatchArm {
        pattern: lower_pattern(arm.pattern)?,
        guard,
        body,
    })
}

fn lower_literal(lit: crate::kernel::KernelLiteral) -> RustIrLiteral {
    match lit {
        crate::kernel::KernelLiteral::Number(text) => RustIrLiteral::Number(text),
        crate::kernel::KernelLiteral::String(text) => RustIrLiteral::String(text),
        crate::kernel::KernelLiteral::Sigil { tag, body, flags } => {
            RustIrLiteral::Sigil { tag, body, flags }
        }
        crate::kernel::KernelLiteral::Bool(value) => RustIrLiteral::Bool(value),
        crate::kernel::KernelLiteral::DateTime(text) => RustIrLiteral::DateTime(text),
    }
}

fn collect_pattern_binders(pattern: &KernelPattern, out: &mut Vec<String>) {
    match pattern {
        KernelPattern::Wildcard { .. } => {}
        KernelPattern::Var { name, .. } => out.push(name.clone()),
        KernelPattern::Literal { .. } => {}
        KernelPattern::Constructor { args, .. } => {
            for arg in args {
                collect_pattern_binders(arg, out);
            }
        }
        KernelPattern::Tuple { items, .. } => {
            for item in items {
                collect_pattern_binders(item, out);
            }
        }
        KernelPattern::List { items, rest, .. } => {
            for item in items {
                collect_pattern_binders(item, out);
            }
            if let Some(rest) = rest.as_deref() {
                collect_pattern_binders(rest, out);
            }
        }
        KernelPattern::Record { fields, .. } => {
            for field in fields {
                collect_pattern_binders(&field.pattern, out);
            }
        }
    }
}

fn resolve_builtin(name: &str) -> Option<BuiltinName> {
    // Keep this list in sync with `aivi_native_runtime::builtins`.
    let ok = matches!(
        name,
        "Unit"
            | "True"
            | "False"
            | "None"
            | "Some"
            | "Ok"
            | "Err"
            | "Closed"
            | "pure"
            | "fail"
            | "attempt"
            | "load"
            | "bind"
            | "print"
            | "println"
            | "map"
            | "chain"
            | "assertEq"
            | "file"
            | "system"
            | "clock"
            | "random"
            | "channel"
            | "concurrent"
            | "httpServer"
            | "text"
            | "regex"
            | "math"
            | "calendar"
            | "color"
            | "linalg"
            | "signal"
            | "graph"
            | "bigint"
            | "rational"
            | "decimal"
            | "url"
            | "http"
            | "https"
            | "sockets"
            | "streams"
            | "collections"
            | "console"
            | "crypto"
            | "logger"
            | "database"
            | "Map"
            | "Set"
            | "Queue"
            | "Deque"
            | "Heap"
    );
    if ok {
        Some(name.to_string())
    } else {
        None
    }
}
