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
pub enum KernelTextPart {
    Text { text: String },
    Expr { expr: KernelExpr },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "kind")]
pub enum KernelExpr {
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
        parts: Vec<KernelTextPart>,
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
        body: Box<KernelExpr>,
    },
    App {
        id: u32,
        func: Box<KernelExpr>,
        arg: Box<KernelExpr>,
    },
    Call {
        id: u32,
        func: Box<KernelExpr>,
        args: Vec<KernelExpr>,
    },
    List {
        id: u32,
        items: Vec<KernelListItem>,
    },
    Tuple {
        id: u32,
        items: Vec<KernelExpr>,
    },
    Record {
        id: u32,
        fields: Vec<KernelRecordField>,
    },
    Patch {
        id: u32,
        target: Box<KernelExpr>,
        fields: Vec<KernelRecordField>,
    },
    FieldAccess {
        id: u32,
        base: Box<KernelExpr>,
        field: String,
    },
    Index {
        id: u32,
        base: Box<KernelExpr>,
        index: Box<KernelExpr>,
    },
    Match {
        id: u32,
        scrutinee: Box<KernelExpr>,
        arms: Vec<KernelMatchArm>,
    },
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
    Raw {
        id: u32,
        text: String,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KernelListItem {
    pub expr: KernelExpr,
    pub spread: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KernelRecordField {
    pub spread: bool,
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
    Wildcard {
        id: u32,
    },
    Var {
        id: u32,
        name: String,
    },
    Literal {
        id: u32,
        value: KernelLiteral,
    },
    Constructor {
        id: u32,
        name: String,
        args: Vec<KernelPattern>,
    },
    Tuple {
        id: u32,
        items: Vec<KernelPattern>,
    },
    List {
        id: u32,
        items: Vec<KernelPattern>,
        rest: Option<Box<KernelPattern>>,
    },
    Record {
        id: u32,
        fields: Vec<KernelRecordPatternField>,
    },
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
    Filter {
        expr: KernelExpr,
    },
    Yield {
        expr: KernelExpr,
    },
    Recurse {
        expr: KernelExpr,
    },
    Expr {
        expr: KernelExpr,
    },
}

struct IdGen {
    next: u32,
}

impl IdGen {
    fn new(start: u32) -> Self {
        Self { next: start }
    }

    fn next(&mut self) -> u32 {
        let id = self.next;
        self.next += 1;
        id
    }
}

pub fn lower_hir(program: HirProgram) -> KernelProgram {
    let mut id_gen = IdGen::new(find_max_id_program(&program) + 1);
    let modules = program
        .modules
        .into_iter()
        .map(|m| lower_module(m, &mut id_gen))
        .collect();
    KernelProgram { modules }
}

fn lower_module(module: HirModule, id_gen: &mut IdGen) -> KernelModule {
    KernelModule {
        name: module.name,
        defs: module
            .defs
            .into_iter()
            .map(|d| lower_def(d, id_gen))
            .collect(),
    }
}

fn lower_def(def: HirDef, id_gen: &mut IdGen) -> KernelDef {
    KernelDef {
        name: def.name,
        expr: lower_expr(def.expr, id_gen),
    }
}

fn lower_expr(expr: HirExpr, id_gen: &mut IdGen) -> KernelExpr {
    match expr {
        HirExpr::Var { id, name } => KernelExpr::Var { id, name },
        HirExpr::LitNumber { id, text } => KernelExpr::LitNumber { id, text },
        HirExpr::LitString { id, text } => KernelExpr::LitString { id, text },
        HirExpr::TextInterpolate { id, parts } => KernelExpr::TextInterpolate {
            id,
            parts: parts
                .into_iter()
                .map(|part| match part {
                    crate::hir::HirTextPart::Text { text } => KernelTextPart::Text { text },
                    crate::hir::HirTextPart::Expr { expr } => KernelTextPart::Expr {
                        expr: lower_expr(expr, id_gen),
                    },
                })
                .collect(),
        },
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
            body: Box::new(lower_expr(*body, id_gen)),
        },
        HirExpr::App { id, func, arg } => KernelExpr::App {
            id,
            func: Box::new(lower_expr(*func, id_gen)),
            arg: Box::new(lower_expr(*arg, id_gen)),
        },
        HirExpr::Call { id, func, args } => KernelExpr::Call {
            id,
            func: Box::new(lower_expr(*func, id_gen)),
            args: args.into_iter().map(|a| lower_expr(a, id_gen)).collect(),
        },
        HirExpr::List { id, items } => KernelExpr::List {
            id,
            items: items
                .into_iter()
                .map(|i| lower_list_item(i, id_gen))
                .collect(),
        },
        HirExpr::Tuple { id, items } => KernelExpr::Tuple {
            id,
            items: items.into_iter().map(|e| lower_expr(e, id_gen)).collect(),
        },
        HirExpr::Record { id, fields } => KernelExpr::Record {
            id,
            fields: fields
                .into_iter()
                .map(|f| lower_record_field(f, id_gen))
                .collect(),
        },
        HirExpr::Patch { id, target, fields } => KernelExpr::Patch {
            id,
            target: Box::new(lower_expr(*target, id_gen)),
            fields: fields
                .into_iter()
                .map(|f| lower_record_field(f, id_gen))
                .collect(),
        },
        HirExpr::FieldAccess { id, base, field } => KernelExpr::FieldAccess {
            id,
            base: Box::new(lower_expr(*base, id_gen)),
            field,
        },
        HirExpr::Index { id, base, index } => KernelExpr::Index {
            id,
            base: Box::new(lower_expr(*base, id_gen)),
            index: Box::new(lower_expr(*index, id_gen)),
        },
        HirExpr::Match {
            id,
            scrutinee,
            arms,
        } => KernelExpr::Match {
            id,
            scrutinee: Box::new(lower_expr(*scrutinee, id_gen)),
            arms: arms
                .into_iter()
                .map(|a| lower_match_arm(a, id_gen))
                .collect(),
        },
        HirExpr::If {
            id,
            cond,
            then_branch,
            else_branch,
        } => KernelExpr::If {
            id,
            cond: Box::new(lower_expr(*cond, id_gen)),
            then_branch: Box::new(lower_expr(*then_branch, id_gen)),
            else_branch: Box::new(lower_expr(*else_branch, id_gen)),
        },
        HirExpr::Binary {
            id,
            op,
            left,
            right,
        } => KernelExpr::Binary {
            id,
            op,
            left: Box::new(lower_expr(*left, id_gen)),
            right: Box::new(lower_expr(*right, id_gen)),
        },
        HirExpr::Block {
            id,
            block_kind,
            items,
        } => match block_kind {
            HirBlockKind::Generate => lower_generate_block(items, id_gen),
            _ => KernelExpr::Block {
                id,
                block_kind: lower_block_kind(block_kind),
                items: items
                    .into_iter()
                    .map(|i| lower_block_item(i, id_gen))
                    .collect(),
            },
        },
        HirExpr::Raw { id, text } => KernelExpr::Raw { id, text },
    }
}

fn lower_generate_block(items: Vec<HirBlockItem>, id_gen: &mut IdGen) -> KernelExpr {
    if items.is_empty() {
        return gen_empty(id_gen);
    }

    let item = items[0].clone();
    let rest = items[1..].to_vec();

    match item {
        HirBlockItem::Yield { expr } => {
            let yield_expr = gen_yield(lower_expr(expr, id_gen), id_gen);
            if rest.is_empty() {
                yield_expr
            } else {
                gen_append(yield_expr, lower_generate_block(rest, id_gen), id_gen)
            }
        }
        HirBlockItem::Bind { pattern, expr } => {
            let src = lower_expr(expr, id_gen);
            let next = lower_generate_block(rest, id_gen);
            let param_name = format!("_gen_bind_{}", id_gen.next());
            let param_var = KernelExpr::Var {
                id: id_gen.next(),
                name: param_name.clone(),
            };
            let body = KernelExpr::Match {
                id: id_gen.next(),
                scrutinee: Box::new(param_var),
                arms: vec![KernelMatchArm {
                    pattern: lower_pattern(pattern, id_gen),
                    guard: None,
                    body: next,
                }],
            };
            let func = KernelExpr::Lambda {
                id: id_gen.next(),
                param: param_name,
                body: Box::new(body),
            };
            gen_bind(src, func, id_gen)
        }
        HirBlockItem::Expr { expr } => {
            // Treat as generator to spread/append
            let head = lower_expr(expr, id_gen);
            if rest.is_empty() {
                head
            } else {
                gen_append(head, lower_generate_block(rest, id_gen), id_gen)
            }
        }
        HirBlockItem::Filter { expr } => {
            let cond = lower_expr(expr, id_gen);
            let next = lower_generate_block(rest, id_gen);
            gen_if(cond, next, id_gen)
        }
        HirBlockItem::Recurse { .. } => {
            // Unsupported for now
            gen_empty(id_gen)
        }
    }
}

// Generator A = (R -> A -> R) -> R -> R
// \k -> \z -> z
fn gen_empty(id_gen: &mut IdGen) -> KernelExpr {
    let k = format!("_k_{}", id_gen.next());
    let z = format!("_z_{}", id_gen.next());
    KernelExpr::Lambda {
        id: id_gen.next(),
        param: k,
        body: Box::new(KernelExpr::Lambda {
            id: id_gen.next(),
            param: z.clone(),
            body: Box::new(KernelExpr::Var {
                id: id_gen.next(),
                name: z,
            }),
        }),
    }
}

// \k -> \z -> k z x
fn gen_yield(val: KernelExpr, id_gen: &mut IdGen) -> KernelExpr {
    let k_name = format!("_k_{}", id_gen.next());
    let z_name = format!("_z_{}", id_gen.next());
    let k = KernelExpr::Var {
        id: id_gen.next(),
        name: k_name.clone(),
    };
    let z = KernelExpr::Var {
        id: id_gen.next(),
        name: z_name.clone(),
    };

    // k z val
    let k_app_z = KernelExpr::App {
        id: id_gen.next(),
        func: Box::new(k),
        arg: Box::new(z),
    };
    let k_app_z_val = KernelExpr::App {
        id: id_gen.next(),
        func: Box::new(k_app_z),
        arg: Box::new(val),
    };

    KernelExpr::Lambda {
        id: id_gen.next(),
        param: k_name,
        body: Box::new(KernelExpr::Lambda {
            id: id_gen.next(),
            param: z_name,
            body: Box::new(k_app_z_val),
        }),
    }
}

// \k -> \z -> g2 k (g1 k z)
fn gen_append(g1: KernelExpr, g2: KernelExpr, id_gen: &mut IdGen) -> KernelExpr {
    let k_name = format!("_k_{}", id_gen.next());
    let z_name = format!("_z_{}", id_gen.next());
    let k = KernelExpr::Var {
        id: id_gen.next(),
        name: k_name.clone(),
    };
    let z = KernelExpr::Var {
        id: id_gen.next(),
        name: z_name.clone(),
    };

    // g1 k z
    let g1_k = KernelExpr::App {
        id: id_gen.next(),
        func: Box::new(g1),
        arg: Box::new(k.clone()),
    };
    let g1_k_z = KernelExpr::App {
        id: id_gen.next(),
        func: Box::new(g1_k),
        arg: Box::new(z.clone()),
    };

    // g2 k (g1 k z)
    let g2_k = KernelExpr::App {
        id: id_gen.next(),
        func: Box::new(g2),
        arg: Box::new(k),
    };
    let g2_k_res = KernelExpr::App {
        id: id_gen.next(),
        func: Box::new(g2_k),
        arg: Box::new(g1_k_z),
    };

    KernelExpr::Lambda {
        id: id_gen.next(),
        param: k_name,
        body: Box::new(KernelExpr::Lambda {
            id: id_gen.next(),
            param: z_name,
            body: Box::new(g2_k_res),
        }),
    }
}

// \k -> \z -> if cond then next(k, z) else z
fn gen_if(cond: KernelExpr, next: KernelExpr, id_gen: &mut IdGen) -> KernelExpr {
    let k_name = format!("_k_{}", id_gen.next());
    let z_name = format!("_z_{}", id_gen.next());
    let k = KernelExpr::Var {
        id: id_gen.next(),
        name: k_name.clone(),
    };
    let z = KernelExpr::Var {
        id: id_gen.next(),
        name: z_name.clone(),
    };

    // next k z
    let next_k = KernelExpr::App {
        id: id_gen.next(),
        func: Box::new(next),
        arg: Box::new(k.clone()),
    };
    let next_k_z = KernelExpr::App {
        id: id_gen.next(),
        func: Box::new(next_k),
        arg: Box::new(z.clone()),
    };

    let if_expr = KernelExpr::If {
        id: id_gen.next(),
        cond: Box::new(cond),
        then_branch: Box::new(next_k_z),
        else_branch: Box::new(z),
    };

    KernelExpr::Lambda {
        id: id_gen.next(),
        param: k_name,
        body: Box::new(KernelExpr::Lambda {
            id: id_gen.next(),
            param: z_name,
            body: Box::new(if_expr),
        }),
    }
}

// genBind(g, f)
// \k -> \z -> g (\acc -> \x -> f(x) k acc) z
fn gen_bind(g: KernelExpr, f: KernelExpr, id_gen: &mut IdGen) -> KernelExpr {
    let k_name = format!("_k_{}", id_gen.next());
    let z_name = format!("_z_{}", id_gen.next());
    let k = KernelExpr::Var {
        id: id_gen.next(),
        name: k_name.clone(),
    };
    let z = KernelExpr::Var {
        id: id_gen.next(),
        name: z_name.clone(),
    };

    // \acc -> \x -> ...
    let acc_name = format!("_acc_{}", id_gen.next());
    let x_name = format!("_x_{}", id_gen.next());
    let acc = KernelExpr::Var {
        id: id_gen.next(),
        name: acc_name.clone(),
    };
    let x = KernelExpr::Var {
        id: id_gen.next(),
        name: x_name.clone(),
    };

    // f(x)
    let fx = KernelExpr::App {
        id: id_gen.next(),
        func: Box::new(f),
        arg: Box::new(x),
    };
    // f(x) k
    let fx_k = KernelExpr::App {
        id: id_gen.next(),
        func: Box::new(fx),
        arg: Box::new(k.clone()),
    };
    // f(x) k acc
    let fx_k_acc = KernelExpr::App {
        id: id_gen.next(),
        func: Box::new(fx_k),
        arg: Box::new(acc),
    };

    let step_fn = KernelExpr::Lambda {
        id: id_gen.next(),
        param: acc_name,
        body: Box::new(KernelExpr::Lambda {
            id: id_gen.next(),
            param: x_name,
            body: Box::new(fx_k_acc),
        }),
    };

    // g step_fn z
    let g_step = KernelExpr::App {
        id: id_gen.next(),
        func: Box::new(g),
        arg: Box::new(step_fn),
    };
    let g_step_z = KernelExpr::App {
        id: id_gen.next(),
        func: Box::new(g_step),
        arg: Box::new(z),
    };

    KernelExpr::Lambda {
        id: id_gen.next(),
        param: k_name,
        body: Box::new(KernelExpr::Lambda {
            id: id_gen.next(),
            param: z_name,
            body: Box::new(g_step_z),
        }),
    }
}

fn lower_list_item(item: HirListItem, id_gen: &mut IdGen) -> KernelListItem {
    KernelListItem {
        expr: lower_expr(item.expr, id_gen),
        spread: item.spread,
    }
}

fn lower_record_field(field: HirRecordField, id_gen: &mut IdGen) -> KernelRecordField {
    KernelRecordField {
        spread: field.spread,
        path: field
            .path
            .into_iter()
            .map(|s| lower_path_segment(s, id_gen))
            .collect(),
        value: lower_expr(field.value, id_gen),
    }
}

fn lower_path_segment(seg: HirPathSegment, id_gen: &mut IdGen) -> KernelPathSegment {
    match seg {
        HirPathSegment::Field(name) => KernelPathSegment::Field(name),
        HirPathSegment::Index(expr) => KernelPathSegment::Index(lower_expr(expr, id_gen)),
    }
}

fn lower_match_arm(arm: HirMatchArm, id_gen: &mut IdGen) -> KernelMatchArm {
    KernelMatchArm {
        pattern: lower_pattern(arm.pattern, id_gen),
        guard: arm.guard.map(|e| lower_expr(e, id_gen)),
        body: lower_expr(arm.body, id_gen),
    }
}

fn lower_pattern(pattern: HirPattern, id_gen: &mut IdGen) -> KernelPattern {
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
            args: args.into_iter().map(|p| lower_pattern(p, id_gen)).collect(),
        },
        HirPattern::Tuple { id, items } => KernelPattern::Tuple {
            id,
            items: items
                .into_iter()
                .map(|p| lower_pattern(p, id_gen))
                .collect(),
        },
        HirPattern::List { id, items, rest } => KernelPattern::List {
            id,
            items: items
                .into_iter()
                .map(|p| lower_pattern(p, id_gen))
                .collect(),
            rest: rest.map(|p| Box::new(lower_pattern(*p, id_gen))),
        },
        HirPattern::Record { id, fields } => KernelPattern::Record {
            id,
            fields: fields
                .into_iter()
                .map(|f| lower_record_pattern_field(f, id_gen))
                .collect(),
        },
    }
}

fn lower_record_pattern_field(
    field: HirRecordPatternField,
    id_gen: &mut IdGen,
) -> KernelRecordPatternField {
    KernelRecordPatternField {
        path: field.path,
        pattern: lower_pattern(field.pattern, id_gen),
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

fn lower_block_item(item: HirBlockItem, id_gen: &mut IdGen) -> KernelBlockItem {
    match item {
        HirBlockItem::Bind { pattern, expr } => KernelBlockItem::Bind {
            pattern: lower_pattern(pattern, id_gen),
            expr: lower_expr(expr, id_gen),
        },
        HirBlockItem::Filter { expr } => KernelBlockItem::Filter {
            expr: lower_expr(expr, id_gen),
        },
        HirBlockItem::Yield { expr } => KernelBlockItem::Yield {
            expr: lower_expr(expr, id_gen),
        },
        HirBlockItem::Recurse { expr } => KernelBlockItem::Recurse {
            expr: lower_expr(expr, id_gen),
        },
        HirBlockItem::Expr { expr } => KernelBlockItem::Expr {
            expr: lower_expr(expr, id_gen),
        },
    }
}

fn find_max_id_program(program: &HirProgram) -> u32 {
    let mut max = 0;
    for module in &program.modules {
        for def in &module.defs {
            find_max_id_expr(&def.expr, &mut max);
        }
    }
    max
}

fn find_max_id_expr(expr: &HirExpr, max: &mut u32) {
    match expr {
        HirExpr::Var { id, .. }
        | HirExpr::LitNumber { id, .. }
        | HirExpr::LitString { id, .. }
        | HirExpr::LitSigil { id, .. }
        | HirExpr::LitBool { id, .. }
        | HirExpr::LitDateTime { id, .. }
        | HirExpr::Raw { id, .. } => {
            if *id > *max {
                *max = *id;
            }
        }
        HirExpr::TextInterpolate { id, parts } => {
            if *id > *max {
                *max = *id;
            }
            for part in parts {
                if let crate::hir::HirTextPart::Expr { expr } = part {
                    find_max_id_expr(expr, max);
                }
            }
        }
        HirExpr::Lambda { id, body, .. } => {
            if *id > *max {
                *max = *id;
            }
            find_max_id_expr(body, max);
        }
        HirExpr::App { id, func, arg } => {
            if *id > *max {
                *max = *id;
            }
            find_max_id_expr(func, max);
            find_max_id_expr(arg, max);
        }
        HirExpr::Call { id, func, args } => {
            if *id > *max {
                *max = *id;
            }
            find_max_id_expr(func, max);
            for arg in args {
                find_max_id_expr(arg, max);
            }
        }
        HirExpr::List { id, items } => {
            if *id > *max {
                *max = *id;
            }
            for item in items {
                find_max_id_expr(&item.expr, max);
            }
        }
        HirExpr::Tuple { id, items } => {
            if *id > *max {
                *max = *id;
            }
            for item in items {
                find_max_id_expr(item, max);
            }
        }
        HirExpr::Record { id, fields } | HirExpr::Patch { id, fields, .. } => {
            if *id > *max {
                *max = *id;
            }
            if let HirExpr::Patch { target, .. } = expr {
                find_max_id_expr(target, max);
            }
            for field in fields {
                find_max_id_expr(&field.value, max);
                for seg in &field.path {
                    if let HirPathSegment::Index(idx) = seg {
                        find_max_id_expr(idx, max);
                    }
                }
            }
        }
        HirExpr::FieldAccess { id, base, .. } => {
            if *id > *max {
                *max = *id;
            }
            find_max_id_expr(base, max);
        }
        HirExpr::Index { id, base, index } => {
            if *id > *max {
                *max = *id;
            }
            find_max_id_expr(base, max);
            find_max_id_expr(index, max);
        }
        HirExpr::Match {
            id,
            scrutinee,
            arms,
        } => {
            if *id > *max {
                *max = *id;
            }
            find_max_id_expr(scrutinee, max);
            for arm in arms {
                find_max_id_pattern(&arm.pattern, max);
                if let Some(guard) = &arm.guard {
                    find_max_id_expr(guard, max);
                }
                find_max_id_expr(&arm.body, max);
            }
        }
        HirExpr::If {
            id,
            cond,
            then_branch,
            else_branch,
        } => {
            if *id > *max {
                *max = *id;
            }
            find_max_id_expr(cond, max);
            find_max_id_expr(then_branch, max);
            find_max_id_expr(else_branch, max);
        }
        HirExpr::Binary {
            id, left, right, ..
        } => {
            if *id > *max {
                *max = *id;
            }
            find_max_id_expr(left, max);
            find_max_id_expr(right, max);
        }
        HirExpr::Block { id, items, .. } => {
            if *id > *max {
                *max = *id;
            }
            for item in items {
                match item {
                    HirBlockItem::Bind { pattern, expr } => {
                        find_max_id_pattern(pattern, max);
                        find_max_id_expr(expr, max);
                    }
                    HirBlockItem::Filter { expr }
                    | HirBlockItem::Yield { expr }
                    | HirBlockItem::Recurse { expr }
                    | HirBlockItem::Expr { expr } => {
                        find_max_id_expr(expr, max);
                    }
                }
            }
        }
    }
}

fn find_max_id_pattern(pattern: &HirPattern, max: &mut u32) {
    match pattern {
        HirPattern::Wildcard { id }
        | HirPattern::Var { id, .. }
        | HirPattern::Literal { id, .. } => {
            if *id > *max {
                *max = *id;
            }
        }
        HirPattern::Constructor { id, args, .. } => {
            if *id > *max {
                *max = *id;
            }
            for arg in args {
                find_max_id_pattern(arg, max);
            }
        }
        HirPattern::Tuple { id, items } => {
            if *id > *max {
                *max = *id;
            }
            for item in items {
                find_max_id_pattern(item, max);
            }
        }
        HirPattern::List { id, items, rest } => {
            if *id > *max {
                *max = *id;
            }
            for item in items {
                find_max_id_pattern(item, max);
            }
            if let Some(rest) = rest {
                find_max_id_pattern(rest, max);
            }
        }
        HirPattern::Record { id, fields } => {
            if *id > *max {
                *max = *id;
            }
            for field in fields {
                find_max_id_pattern(&field.pattern, max);
            }
        }
    }
}
