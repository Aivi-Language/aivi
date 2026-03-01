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
    Raw {
        id: u32,
        text: String,
    },
    /// Scoped binding substitution: save → set → eval body → restore.
    Mock {
        id: u32,
        substitutions: Vec<KernelMockSubstitution>,
        body: Box<KernelExpr>,
    },
}

/// A single mock substitution at the Kernel level.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KernelMockSubstitution {
    /// Fully-qualified dotted path (e.g. `"aivi.rest.get"`).
    pub path: String,
    /// Whether this is a snapshot mock (record/replay).
    pub snapshot: bool,
    /// Replacement expression (`None` for snapshot mocks).
    pub value: Option<KernelExpr>,
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
            HirBlockKind::Do { monad } if monad == "Effect" => {
                lower_do_effect_block(items, id_gen)
            }
            HirBlockKind::Do { .. } => {
                // Generic `do M` is already desugared at HIR level into chain/lambda calls.
                // If we still get one here, treat remaining items as a plain block.
                lower_plain_block(items, id_gen)
            }
            HirBlockKind::Resource => lower_resource_block(id, items, id_gen),
            HirBlockKind::Plain => lower_plain_block(items, id_gen),
        },
        HirExpr::Raw { id, text } => KernelExpr::Raw { id, text },
        HirExpr::Mock {
            id,
            substitutions,
            body,
        } => KernelExpr::Mock {
            id,
            substitutions: substitutions
                .into_iter()
                .map(|sub| KernelMockSubstitution {
                    path: sub.path,
                    snapshot: sub.snapshot,
                    value: sub.value.map(|v| lower_expr(v, id_gen)),
                })
                .collect(),
            body: Box::new(lower_expr(*body, id_gen)),
        },
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
        HirBlockItem::Bind {
            pattern,
            expr,
            is_monadic,
        } => {
            let raw_src = lower_expr(expr, id_gen);
            if is_monadic {
                // `x <- expr` in a generate block: iterate over expr as a generator.
                // Wrap with __asGenerator so bare lists are implicitly converted.
                let src = KernelExpr::App {
                    id: id_gen.next(),
                    func: Box::new(KernelExpr::Var {
                        id: id_gen.next(),
                        name: "__asGenerator".to_string(),
                    }),
                    arg: Box::new(raw_src),
                };
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
            } else {
                // `s = expr` in a generate block: plain let-binding.
                // Wrap next in a lambda applied to raw_src.
                let next = lower_generate_block(rest, id_gen);
                let param_name = format!("_gen_let_{}", id_gen.next());
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
                // (\param -> body) raw_src  — immediately-applied lambda = let binding
                KernelExpr::App {
                    id: id_gen.next(),
                    func: Box::new(KernelExpr::Lambda {
                        id: id_gen.next(),
                        param: param_name,
                        body: Box::new(body),
                    }),
                    arg: Box::new(raw_src),
                }
            }
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

// ── do Effect { ... } desugaring ──────────────────────────────────────────────
//
// Transforms:
//   x <- e1; e2        →  bind e1 (λx → e2)
//   x = e1; e2         →  (λx → e2) e1      (let-binding, non-monadic)
//   expr; rest          →  bind expr (λ_ → rest)
//   yield expr          →  expr               (final expression)
//   recurse expr        →  expr               (final expression, tail call)
//
// Empty block → pure Unit

fn lower_do_effect_block(items: Vec<HirBlockItem>, id_gen: &mut IdGen) -> KernelExpr {
    if items.is_empty() {
        return effect_pure_unit(id_gen);
    }
    let chain = lower_do_effect_items(&items, 0, id_gen);
    // Wrap in __withResourceScope so resource cleanups are scoped to this block
    KernelExpr::Call {
        id: id_gen.next(),
        func: Box::new(KernelExpr::Var {
            id: id_gen.next(),
            name: "__withResourceScope".to_string(),
        }),
        args: vec![chain],
    }
}

fn lower_do_effect_items(items: &[HirBlockItem], idx: usize, id_gen: &mut IdGen) -> KernelExpr {
    if idx >= items.len() {
        return effect_pure_unit(id_gen);
    }

    let item = &items[idx];
    let is_last = idx + 1 >= items.len();

    match item {
        HirBlockItem::Bind {
            pattern, expr, ..
        } => {
            // Both monadic (`x <- e`) and non-monadic (`x = e`) binds use `bind`.
            // Non-monadic let-binds are already wrapped in `pure(expr)` by the HIR
            // lowering (lower_blocks_and_patterns.rs), so they are effectively monadic
            // by this point.
            let lowered_expr = lower_expr(expr.clone(), id_gen);
            let rest = if is_last {
                // Tail bind: bind e (λx → pure x)
                let param = format!("_do_{}", id_gen.next());
                let body = effect_pure(
                    KernelExpr::Var {
                        id: id_gen.next(),
                        name: param.clone(),
                    },
                    id_gen,
                );
                return effect_bind(
                    lowered_expr,
                    KernelExpr::Lambda {
                        id: id_gen.next(),
                        param,
                        body: Box::new(body),
                    },
                    id_gen,
                );
            } else {
                lower_do_effect_items(items, idx + 1, id_gen)
            };

            // x <- e; rest  →  bind e (λx → rest)
            let param = pattern_to_param(pattern, id_gen);
            let body = wrap_pattern_match(param.clone(), pattern, rest, id_gen);
            effect_bind(
                lowered_expr,
                KernelExpr::Lambda {
                    id: id_gen.next(),
                    param,
                    body: Box::new(body),
                },
                id_gen,
            )
        }
        HirBlockItem::Expr { expr } => {
            let lowered_expr = lower_expr(expr.clone(), id_gen);
            if is_last {
                // Final expression in do block
                lowered_expr
            } else {
                // expr; rest  →  bind expr (λ_ → rest)
                let rest = lower_do_effect_items(items, idx + 1, id_gen);
                let param = format!("_do_{}", id_gen.next());
                effect_bind(
                    lowered_expr,
                    KernelExpr::Lambda {
                        id: id_gen.next(),
                        param,
                        body: Box::new(rest),
                    },
                    id_gen,
                )
            }
        }
        HirBlockItem::Yield { expr } => {
            // yield expr produces a plain value — wrap in `pure` so the bind
            // chain sees an Effect (bind internally calls run_effect_value on
            // the continuation's result).
            let lowered_expr = lower_expr(expr.clone(), id_gen);
            let wrapped = effect_pure(lowered_expr, id_gen);
            if is_last {
                wrapped
            } else {
                let rest = lower_do_effect_items(items, idx + 1, id_gen);
                let param = format!("_do_{}", id_gen.next());
                effect_bind(
                    wrapped,
                    KernelExpr::Lambda {
                        id: id_gen.next(),
                        param,
                        body: Box::new(rest),
                    },
                    id_gen,
                )
            }
        }
        HirBlockItem::Recurse { expr } => {
            // recurse expr → the expr itself (tail call)
            lower_expr(expr.clone(), id_gen)
        }
        HirBlockItem::Filter { expr } => {
            // Filters in effect blocks: guard
            let lowered_expr = lower_expr(expr.clone(), id_gen);
            let rest = if is_last {
                effect_pure_unit(id_gen)
            } else {
                lower_do_effect_items(items, idx + 1, id_gen)
            };
            KernelExpr::If {
                id: id_gen.next(),
                cond: Box::new(lowered_expr),
                then_branch: Box::new(rest),
                else_branch: Box::new(effect_pure_unit(id_gen)),
            }
        }
    }
}

/// Extract a simple param name from a pattern; complex patterns use a temp var + match.
fn pattern_to_param(pattern: &HirPattern, id_gen: &mut IdGen) -> String {
    match pattern {
        HirPattern::Var { name, .. } => name.clone(),
        HirPattern::Wildcard { .. } => format!("_do_{}", id_gen.next()),
        _ => format!("_do_{}", id_gen.next()),
    }
}

/// If the pattern is not a simple var or wildcard, wrap the body in a match.
fn wrap_pattern_match(
    param: String,
    pattern: &HirPattern,
    body: KernelExpr,
    id_gen: &mut IdGen,
) -> KernelExpr {
    match pattern {
        HirPattern::Var { name, .. } if *name == param => body,
        HirPattern::Wildcard { .. } => body,
        _ => KernelExpr::Match {
            id: id_gen.next(),
            scrutinee: Box::new(KernelExpr::Var {
                id: id_gen.next(),
                name: param,
            }),
            arms: vec![KernelMatchArm {
                pattern: lower_pattern(pattern.clone(), id_gen),
                guard: None,
                body,
            }],
        },
    }
}

/// `bind e f` → App (App (Var "bind") e) f
fn effect_bind(expr: KernelExpr, func: KernelExpr, id_gen: &mut IdGen) -> KernelExpr {
    KernelExpr::App {
        id: id_gen.next(),
        func: Box::new(KernelExpr::App {
            id: id_gen.next(),
            func: Box::new(KernelExpr::Var {
                id: id_gen.next(),
                name: "bind".to_string(),
            }),
            arg: Box::new(expr),
        }),
        arg: Box::new(func),
    }
}

/// `pure e` → App (Var "pure") e
fn effect_pure(expr: KernelExpr, id_gen: &mut IdGen) -> KernelExpr {
    KernelExpr::App {
        id: id_gen.next(),
        func: Box::new(KernelExpr::Var {
            id: id_gen.next(),
            name: "pure".to_string(),
        }),
        arg: Box::new(expr),
    }
}

/// `pure Unit`
fn effect_pure_unit(id_gen: &mut IdGen) -> KernelExpr {
    effect_pure(
        KernelExpr::Var {
            id: id_gen.next(),
            name: "Unit".to_string(),
        },
        id_gen,
    )
}

// ── resource { ... } desugaring ───────────────────────────────────────────────
//
// resource { acquire; yield x; cleanup }
//   → __makeResource (λ_ → <desugared acquire+yield>) (λ_ → <desugared cleanup>)

fn lower_resource_block(id: u32, items: Vec<HirBlockItem>, id_gen: &mut IdGen) -> KernelExpr {
    let yield_pos = items
        .iter()
        .position(|item| matches!(item, HirBlockItem::Yield { .. }));

    let (acquire_items, cleanup_items) = match yield_pos {
        Some(pos) => {
            let (acq, rest) = items.split_at(pos + 1);
            (acq.to_vec(), rest.to_vec())
        }
        None => (items, vec![]),
    };

    let acquire_body = lower_do_effect_block(acquire_items, id_gen);
    let acquire_lambda = KernelExpr::Lambda {
        id: id_gen.next(),
        param: format!("_res_unused_{}", id_gen.next()),
        body: Box::new(acquire_body),
    };

    let cleanup_body = if cleanup_items.is_empty() {
        effect_pure_unit(id_gen)
    } else {
        lower_do_effect_block(cleanup_items, id_gen)
    };
    let cleanup_lambda = KernelExpr::Lambda {
        id: id_gen.next(),
        param: format!("_res_unused_{}", id_gen.next()),
        body: Box::new(cleanup_body),
    };

    KernelExpr::Call {
        id,
        func: Box::new(KernelExpr::Var {
            id: id_gen.next(),
            name: "__makeResource".to_string(),
        }),
        args: vec![acquire_lambda, cleanup_lambda],
    }
}

// ── plain { ... } desugaring ──────────────────────────────────────────────────
//
// { x = e1; y = e2; body }  →  (λx → (λy → body) e2) e1

fn lower_plain_block(items: Vec<HirBlockItem>, id_gen: &mut IdGen) -> KernelExpr {
    if items.is_empty() {
        return KernelExpr::Var {
            id: id_gen.next(),
            name: "Unit".to_string(),
        };
    }
    lower_plain_items(&items, 0, id_gen)
}

fn lower_plain_items(items: &[HirBlockItem], idx: usize, id_gen: &mut IdGen) -> KernelExpr {
    if idx >= items.len() {
        return KernelExpr::Var {
            id: id_gen.next(),
            name: "Unit".to_string(),
        };
    }

    let item = &items[idx];
    let is_last = idx + 1 >= items.len();

    match item {
        HirBlockItem::Bind { pattern, expr, .. } => {
            let lowered_expr = lower_expr(expr.clone(), id_gen);
            if is_last {
                lowered_expr
            } else {
                let rest = lower_plain_items(items, idx + 1, id_gen);
                let param = pattern_to_param(pattern, id_gen);
                let body = wrap_pattern_match(param.clone(), pattern, rest, id_gen);
                KernelExpr::App {
                    id: id_gen.next(),
                    func: Box::new(KernelExpr::Lambda {
                        id: id_gen.next(),
                        param,
                        body: Box::new(body),
                    }),
                    arg: Box::new(lowered_expr),
                }
            }
        }
        HirBlockItem::Expr { expr } | HirBlockItem::Yield { expr } => {
            let lowered_expr = lower_expr(expr.clone(), id_gen);
            if is_last {
                lowered_expr
            } else {
                let rest = lower_plain_items(items, idx + 1, id_gen);
                let param = format!("_plain_{}", id_gen.next());
                KernelExpr::App {
                    id: id_gen.next(),
                    func: Box::new(KernelExpr::Lambda {
                        id: id_gen.next(),
                        param,
                        body: Box::new(rest),
                    }),
                    arg: Box::new(lowered_expr),
                }
            }
        }
        HirBlockItem::Recurse { expr } => lower_expr(expr.clone(), id_gen),
        HirBlockItem::Filter { expr } => {
            let lowered_expr = lower_expr(expr.clone(), id_gen);
            let rest = if is_last {
                KernelExpr::Var {
                    id: id_gen.next(),
                    name: "Unit".to_string(),
                }
            } else {
                lower_plain_items(items, idx + 1, id_gen)
            };
            KernelExpr::If {
                id: id_gen.next(),
                cond: Box::new(lowered_expr),
                then_branch: Box::new(rest),
                else_branch: Box::new(KernelExpr::Var {
                    id: id_gen.next(),
                    name: "Unit".to_string(),
                }),
            }
        }
    }
}
