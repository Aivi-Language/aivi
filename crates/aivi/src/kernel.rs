use serde::{Deserialize, Serialize};

use crate::hir::{
    HirBlockItem, HirBlockKind, HirDef, HirExpr, HirListItem, HirLiteral, HirMatchArm, HirModule,
    HirPathSegment, HirPattern, HirProgram, HirRecordField, HirRecordPatternField,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KernelProgram {
    pub modules: Vec<KernelModule>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KernelModule {
    pub name: String,
    pub defs: Vec<KernelDef>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KernelDef {
    pub name: String,
    pub expr: KernelExpr,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "kind")]
pub enum KernelExpr {
    Var { id: u32, name: String },
    LitNumber { id: u32, text: String },
    LitString { id: u32, text: String },
    LitSigil {
        id: u32,
        tag: String,
        body: String,
        flags: String,
    },
    LitBool { id: u32, value: bool },
    LitDateTime { id: u32, text: String },
    Lambda { id: u32, param: String, body: Box<KernelExpr> },
    App { id: u32, func: Box<KernelExpr>, arg: Box<KernelExpr> },
    Call { id: u32, func: Box<KernelExpr>, args: Vec<KernelExpr> },
    List { id: u32, items: Vec<KernelListItem> },
    Tuple { id: u32, items: Vec<KernelExpr> },
    Record { id: u32, fields: Vec<KernelRecordField> },
    Patch { id: u32, target: Box<KernelExpr>, fields: Vec<KernelRecordField> },
    FieldAccess { id: u32, base: Box<KernelExpr>, field: String },
    Index { id: u32, base: Box<KernelExpr>, index: Box<KernelExpr> },
    Match { id: u32, scrutinee: Box<KernelExpr>, arms: Vec<KernelMatchArm> },
    If {
        id: u32,
        cond: Box<KernelExpr>,
        then_branch: Box<KernelExpr>,
        else_branch: Box<KernelExpr>,
    },
    Binary {
        id: u32,
        op: String,
        left: Box<KernelExpr>,
        right: Box<KernelExpr>,
    },
    Block {
        id: u32,
        block_kind: KernelBlockKind,
        items: Vec<KernelBlockItem>,
    },
    Raw { id: u32, text: String },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KernelListItem {
    pub expr: KernelExpr,
    pub spread: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KernelRecordField {
    pub path: Vec<KernelPathSegment>,
    pub value: KernelExpr,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum KernelPathSegment {
    Field(String),
    Index(KernelExpr),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KernelMatchArm {
    pub pattern: KernelPattern,
    pub guard: Option<KernelExpr>,
    pub body: KernelExpr,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum KernelPattern {
    Wildcard { id: u32 },
    Var { id: u32, name: String },
    Literal { id: u32, value: KernelLiteral },
    Constructor { id: u32, name: String, args: Vec<KernelPattern> },
    Tuple { id: u32, items: Vec<KernelPattern> },
    List {
        id: u32,
        items: Vec<KernelPattern>,
        rest: Option<Box<KernelPattern>>,
    },
    Record { id: u32, fields: Vec<KernelRecordPatternField> },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KernelRecordPatternField {
    pub path: Vec<String>,
    pub pattern: KernelPattern,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum KernelLiteral {
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
pub enum KernelBlockKind {
    Plain,
    Effect,
    Generate,
    Resource,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum KernelBlockItem {
    Bind {
        pattern: KernelPattern,
        expr: KernelExpr,
    },
    Filter { expr: KernelExpr },
    Yield { expr: KernelExpr },
    Recurse { expr: KernelExpr },
    Expr { expr: KernelExpr },
}


pub fn lower_hir(program: HirProgram) -> KernelProgram {
    KernelProgram {
        modules: program.modules.into_iter().map(lower_module).collect(),
    }
}

fn lower_module(module: HirModule) -> KernelModule {
    KernelModule {
        name: module.name,
        defs: module.defs.into_iter().map(lower_def).collect(),
    }
}

fn lower_def(def: HirDef) -> KernelDef {
    KernelDef {
        name: def.name,
        expr: lower_expr(def.expr),
    }
}

fn lower_expr(expr: HirExpr) -> KernelExpr {
    match expr {
        HirExpr::Var { id, name } => KernelExpr::Var { id, name },
        HirExpr::LitNumber { id, text } => KernelExpr::LitNumber { id, text },
        HirExpr::LitString { id, text } => KernelExpr::LitString { id, text },
        HirExpr::LitSigil {
            id,
            tag,
            body,
            flags,
        } => KernelExpr::LitSigil {
            id,
            tag,
            body,
            flags,
        },
        HirExpr::LitBool { id, value } => KernelExpr::LitBool { id, value },
        HirExpr::LitDateTime { id, text } => KernelExpr::LitDateTime { id, text },
        HirExpr::Lambda { id, param, body } => KernelExpr::Lambda {
            id,
            param,
            body: Box::new(lower_expr(*body)),
        },
        HirExpr::App { id, func, arg } => KernelExpr::App {
            id,
            func: Box::new(lower_expr(*func)),
            arg: Box::new(lower_expr(*arg)),
        },
        HirExpr::Call { id, func, args } => KernelExpr::Call {
            id,
            func: Box::new(lower_expr(*func)),
            args: args.into_iter().map(lower_expr).collect(),
        },
        HirExpr::List { id, items } => KernelExpr::List {
            id,
            items: items.into_iter().map(lower_list_item).collect(),
        },
        HirExpr::Tuple { id, items } => KernelExpr::Tuple {
            id,
            items: items.into_iter().map(lower_expr).collect(),
        },
        HirExpr::Record { id, fields } => KernelExpr::Record {
            id,
            fields: fields.into_iter().map(lower_record_field).collect(),
        },
        HirExpr::Patch { id, target, fields } => KernelExpr::Patch {
            id,
            target: Box::new(lower_expr(*target)),
            fields: fields.into_iter().map(lower_record_field).collect(),
        },
        HirExpr::FieldAccess { id, base, field } => KernelExpr::FieldAccess {
            id,
            base: Box::new(lower_expr(*base)),
            field,
        },
        HirExpr::Index { id, base, index } => KernelExpr::Index {
            id,
            base: Box::new(lower_expr(*base)),
            index: Box::new(lower_expr(*index)),
        },
        HirExpr::Match {
            id,
            scrutinee,
            arms,
        } => KernelExpr::Match {
            id,
            scrutinee: Box::new(lower_expr(*scrutinee)),
            arms: arms.into_iter().map(lower_match_arm).collect(),
        },
        HirExpr::If {
            id,
            cond,
            then_branch,
            else_branch,
        } => KernelExpr::If {
            id,
            cond: Box::new(lower_expr(*cond)),
            then_branch: Box::new(lower_expr(*then_branch)),
            else_branch: Box::new(lower_expr(*else_branch)),
        },
        HirExpr::Binary {
            id,
            op,
            left,
            right,
        } => KernelExpr::Binary {
            id,
            op,
            left: Box::new(lower_expr(*left)),
            right: Box::new(lower_expr(*right)),
        },
        HirExpr::Block {
            id,
            block_kind,
            items,
        } => KernelExpr::Block {
            id,
            block_kind: lower_block_kind(block_kind),
            items: items.into_iter().map(lower_block_item).collect(),
        },
        HirExpr::Raw { id, text } => KernelExpr::Raw { id, text },
    }
}

fn lower_list_item(item: HirListItem) -> KernelListItem {
    KernelListItem {
        expr: lower_expr(item.expr),
        spread: item.spread,
    }
}

fn lower_record_field(field: HirRecordField) -> KernelRecordField {
    KernelRecordField {
        path: field.path.into_iter().map(lower_path_segment).collect(),
        value: lower_expr(field.value),
    }
}

fn lower_path_segment(seg: HirPathSegment) -> KernelPathSegment {
    match seg {
        HirPathSegment::Field(name) => KernelPathSegment::Field(name),
        HirPathSegment::Index(expr) => KernelPathSegment::Index(lower_expr(expr)),
    }
}

fn lower_match_arm(arm: HirMatchArm) -> KernelMatchArm {
    KernelMatchArm {
        pattern: lower_pattern(arm.pattern),
        guard: arm.guard.map(lower_expr),
        body: lower_expr(arm.body),
    }
}

fn lower_pattern(pattern: HirPattern) -> KernelPattern {
    match pattern {
        HirPattern::Wildcard { id } => KernelPattern::Wildcard { id },
        HirPattern::Var { id, name } => KernelPattern::Var { id, name },
        HirPattern::Literal { id, value } => KernelPattern::Literal {
            id,
            value: lower_literal(value),
        },
        HirPattern::Constructor { id, name, args } => KernelPattern::Constructor {
            id,
            name,
            args: args.into_iter().map(lower_pattern).collect(),
        },
        HirPattern::Tuple { id, items } => KernelPattern::Tuple {
            id,
            items: items.into_iter().map(lower_pattern).collect(),
        },
        HirPattern::List { id, items, rest } => KernelPattern::List {
            id,
            items: items.into_iter().map(lower_pattern).collect(),
            rest: rest.map(|p| Box::new(lower_pattern(*p))),
        },
        HirPattern::Record { id, fields } => KernelPattern::Record {
            id,
            fields: fields.into_iter().map(lower_record_pattern_field).collect(),
        },
    }
}

fn lower_record_pattern_field(field: HirRecordPatternField) -> KernelRecordPatternField {
    KernelRecordPatternField {
        path: field.path,
        pattern: lower_pattern(field.pattern),
    }
}

fn lower_literal(lit: HirLiteral) -> KernelLiteral {
    match lit {
        HirLiteral::Number(text) => KernelLiteral::Number(text),
        HirLiteral::String(text) => KernelLiteral::String(text),
        HirLiteral::Sigil { tag, body, flags } => KernelLiteral::Sigil { tag, body, flags },
        HirLiteral::Bool(value) => KernelLiteral::Bool(value),
        HirLiteral::DateTime(text) => KernelLiteral::DateTime(text),
    }
}

fn lower_block_kind(kind: HirBlockKind) -> KernelBlockKind {
    match kind {
        HirBlockKind::Plain => KernelBlockKind::Plain,
        HirBlockKind::Effect => KernelBlockKind::Effect,
        HirBlockKind::Generate => KernelBlockKind::Generate,
        HirBlockKind::Resource => KernelBlockKind::Resource,
    }
}

fn lower_block_item(item: HirBlockItem) -> KernelBlockItem {
    match item {
        HirBlockItem::Bind { pattern, expr } => KernelBlockItem::Bind {
            pattern: lower_pattern(pattern),
            expr: lower_expr(expr),
        },
        HirBlockItem::Filter { expr } => KernelBlockItem::Filter {
            expr: lower_expr(expr),
        },
        HirBlockItem::Yield { expr } => KernelBlockItem::Yield {
            expr: lower_expr(expr),
        },
        HirBlockItem::Recurse { expr } => KernelBlockItem::Recurse {
            expr: lower_expr(expr),
        },
        HirBlockItem::Expr { expr } => KernelBlockItem::Expr {
            expr: lower_expr(expr),
        },
    }
}
