use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::diagnostics::Span;
use crate::rust_ir::cg_type::CgType;

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
    /// Codegen-friendly type annotation. `Some(ty)` when the type checker was able to
    /// resolve this definition to a concrete ground type; `None` when the definition is
    /// polymorphic or the type was not available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cg_type: Option<CgType>,
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
    ConstructorValue {
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
    DebugFn {
        id: u32,
        fn_name: String,
        arg_vars: Vec<String>,
        log_args: bool,
        log_return: bool,
        log_time: bool,
        body: Box<RustIrExpr>,
    },
    Pipe {
        id: u32,
        pipe_id: u32,
        step: u32,
        label: String,
        log_time: bool,
        func: Box<RustIrExpr>,
        arg: Box<RustIrExpr>,
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
        #[serde(skip)]
        span: Option<Span>,
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
    /// Traverse/patch list elements for which the predicate returns `True`.
    ///
    /// This is used for patch paths like `items[price > 15].price`, where unbound names in the
    /// bracket expression are treated as implicit field accesses on the element.
    IndexPredicate(RustIrExpr),
    /// Traverse/patch all elements (e.g. `items[*]`).
    IndexAll,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RustIrMatchArm {
    pub pattern: RustIrPattern,
    pub guard: Option<RustIrExpr>,
    pub body: RustIrExpr,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum RustIrPattern {
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
        pattern: Box<RustIrPattern>,
    },
    Literal {
        id: u32,
        value: RustIrLiteral,
    },
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
    Sigil {
        tag: String,
        body: String,
        flags: String,
    },
    Bool(bool),
    DateTime(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum RustIrBlockKind {
    Plain,
    /// `do M { ... }` — monadic block. `monad` is the type constructor name.
    Do { monad: String },
    Generate,
    Resource,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum RustIrBlockItem {
    Bind {
        pattern: RustIrPattern,
        expr: RustIrExpr,
    },
    Filter {
        expr: RustIrExpr,
    },
    Yield {
        expr: RustIrExpr,
    },
    Recurse {
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
        cg_type: None,
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
            } else if is_constructor_name(&name) {
                // Qualified constructor references (e.g. `aivi.database.IntType`) are allowed;
                // constructors are matched by their unqualified name at runtime.
                let ctor = name.rsplit('.').next().unwrap_or(&name).to_string();
                RustIrExpr::ConstructorValue { id, name: ctor }
            } else {
                // Not in locals or this module's defs, but if it passed typechecking, it's likely an imported global.
                RustIrExpr::Global { id, name }
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
        KernelExpr::DebugFn {
            id,
            fn_name,
            arg_vars,
            log_args,
            log_return,
            log_time,
            body,
        } => RustIrExpr::DebugFn {
            id,
            fn_name,
            arg_vars,
            log_args,
            log_return,
            log_time,
            body: Box::new(lower_expr(*body, globals, locals)?),
        },
        KernelExpr::Pipe {
            id,
            pipe_id,
            step,
            label,
            log_time,
            func,
            arg,
        } => RustIrExpr::Pipe {
            id,
            pipe_id,
            step,
            label,
            log_time,
            func: Box::new(lower_expr(*func, globals, locals)?),
            arg: Box::new(lower_expr(*arg, globals, locals)?),
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
        KernelExpr::Index { id, base, index, span } => RustIrExpr::Index {
            id,
            base: Box::new(lower_expr(*base, globals, locals)?),
            index: Box::new(lower_expr(*index, globals, locals)?),
            span,
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
        KernelPathSegment::All => Ok(RustIrPathSegment::IndexAll),
        KernelPathSegment::Index(expr) => {
            // A bare unbound name `items[active]` is shorthand for patching elements where
            // `item.active == True`.
            if let KernelExpr::Var { id, name } = &expr {
                let reserved = is_reserved_selector_name(name);
                let is_bound = locals.iter().rev().any(|local| local == name)
                    || (!reserved && globals.iter().any(|g| g == name))
                    || (!reserved && resolve_builtin(name).is_some());
                if !is_bound {
                    return Ok(RustIrPathSegment::IndexFieldBool(name.clone()));
                }
                return Ok(RustIrPathSegment::IndexValue(lower_expr(
                    KernelExpr::Var {
                        id: *id,
                        name: name.clone(),
                    },
                    globals,
                    locals,
                )?));
            }

            // If the bracket expression contains unbound names (e.g. `price > 15`), treat those
            // names as implicit field accesses on the list element and compile the expression
            // as a predicate closure.
            let mut unbound: HashSet<String> = HashSet::new();
            let mut bound = Vec::new();
            collect_unbound_vars_in_kernel_expr(&expr, globals, locals, &mut bound, &mut unbound);
            if !unbound.is_empty() {
                let param = "__it".to_string();
                let rewritten = rewrite_implicit_field_vars(expr, &param, &unbound);
                let mut locals2 = locals.clone();
                locals2.push(param.clone());
                let body = lower_expr(rewritten, globals, &mut locals2)?;
                return Ok(RustIrPathSegment::IndexPredicate(RustIrExpr::Lambda {
                    id: rust_ir_expr_id(&body),
                    param,
                    body: Box::new(body),
                }));
            }

            Ok(RustIrPathSegment::IndexValue(lower_expr(
                expr, globals, locals,
            )?))
        }
    }
}

fn rust_ir_expr_id(expr: &RustIrExpr) -> u32 {
    match expr {
        RustIrExpr::Local { id, .. }
        | RustIrExpr::Global { id, .. }
        | RustIrExpr::Builtin { id, .. }
        | RustIrExpr::ConstructorValue { id, .. }
        | RustIrExpr::LitNumber { id, .. }
        | RustIrExpr::LitString { id, .. }
        | RustIrExpr::TextInterpolate { id, .. }
        | RustIrExpr::LitSigil { id, .. }
        | RustIrExpr::LitBool { id, .. }
        | RustIrExpr::LitDateTime { id, .. }
        | RustIrExpr::Lambda { id, .. }
        | RustIrExpr::App { id, .. }
        | RustIrExpr::Call { id, .. }
        | RustIrExpr::DebugFn { id, .. }
        | RustIrExpr::Pipe { id, .. }
        | RustIrExpr::List { id, .. }
        | RustIrExpr::Tuple { id, .. }
        | RustIrExpr::Record { id, .. }
        | RustIrExpr::Patch { id, .. }
        | RustIrExpr::FieldAccess { id, .. }
        | RustIrExpr::Index { id, .. }
        | RustIrExpr::Match { id, .. }
        | RustIrExpr::If { id, .. }
        | RustIrExpr::Binary { id, .. }
        | RustIrExpr::Block { id, .. }
        | RustIrExpr::Raw { id, .. } => *id,
    }
}
