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
    DebugFn {
        id: u32,
        fn_name: String,
        arg_vars: Vec<String>,
        log_args: bool,
        log_return: bool,
        log_time: bool,
        body: Box<KernelExpr>,
    },
    Pipe {
        id: u32,
        pipe_id: u32,
        step: u32,
        label: String,
        log_time: bool,
        func: Box<KernelExpr>,
        arg: Box<KernelExpr>,
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
        #[serde(skip)]
        location: Option<String>,
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
    All,
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
    At {
        id: u32,
        name: String,
        pattern: Box<KernelPattern>,
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
    /// `do M { ... }` — monadic block. `monad` is the type constructor name.
    Do { monad: String },
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
    let module_name = module.name.clone();
    let mut defs = Vec::with_capacity(module.defs.len() * 2);
    for def in module.defs {
        let base = lower_def(def.clone(), id_gen);
        defs.push(base);

        // Emit an additional qualified alias so `some.module.name` can be referenced without
        // colliding with builtins or other unqualified imports.
        let mut qualified = lower_def(def, id_gen);
        qualified.name = format!("{module_name}.{}", qualified.name);
        defs.push(qualified);
    }
    KernelModule {
        name: module.name,
        defs,
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
        HirExpr::DebugFn {
            id,
            fn_name,
            arg_vars,
            log_args,
            log_return,
            log_time,
            body,
        } => KernelExpr::DebugFn {
            id,
            fn_name,
            arg_vars,
            log_args,
            log_return,
            log_time,
            body: Box::new(lower_expr(*body, id_gen)),
        },
        HirExpr::Pipe {
            id,
            pipe_id,
            step,
            label,
            log_time,
            func,
            arg,
        } => KernelExpr::Pipe {
            id,
            pipe_id,
            step,
            label,
            log_time,
            func: Box::new(lower_expr(*func, id_gen)),
            arg: Box::new(lower_expr(*arg, id_gen)),
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
        HirExpr::Index { id, base, index, location } => KernelExpr::Index {
            id,
            base: Box::new(lower_expr(*base, id_gen)),
            index: Box::new(lower_expr(*index, id_gen)),
            location,
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
