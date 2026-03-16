use crate::hir::{
    HirBlockItem, HirBlockKind, HirDef, HirExpr, HirListItem, HirMatchArm, HirModule,
    HirMockSubstitution, HirPathSegment, HirPattern, HirProgram, HirRecordField, HirTextPart,
};

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

pub fn desugar_blocks(program: HirProgram) -> HirProgram {
    let mut id_gen = IdGen::new(find_max_id_program(&program) + 1);
    let modules = program
        .modules
        .into_iter()
        .map(|m| desugar_module(m, &mut id_gen))
        .collect();
    HirProgram { modules }
}

fn desugar_module(module: HirModule, id_gen: &mut IdGen) -> HirModule {
    let module_name = module.name.clone();
    let mut defs = Vec::with_capacity(module.defs.len() * 2);
    for def in module.defs {
        let base = desugar_def(def.clone(), id_gen);
        defs.push(base);

        // Emit an additional qualified alias so `some.module.name` can be referenced without
        // colliding with builtins or other unqualified imports.
        let mut qualified = desugar_def(def, id_gen);
        qualified.name = format!("{module_name}.{}", qualified.name);
        defs.push(qualified);
    }
    HirModule {
        name: module.name,
        defs,
    }
}

fn desugar_def(def: HirDef, id_gen: &mut IdGen) -> HirDef {
    HirDef {
        name: def.name,
        expr: desugar_expr(def.expr, id_gen),
    }
}

fn desugar_expr(expr: HirExpr, id_gen: &mut IdGen) -> HirExpr {
    match expr {
        // Leaves — no children to recurse into
        HirExpr::Var { .. }
        | HirExpr::LitNumber { .. }
        | HirExpr::LitString { .. }
        | HirExpr::LitSigil { .. }
        | HirExpr::LitBool { .. }
        | HirExpr::LitDateTime { .. }
        | HirExpr::Raw { .. } => expr,

        HirExpr::TextInterpolate { id, parts } => HirExpr::TextInterpolate {
            id,
            parts: parts
                .into_iter()
                .map(|part| match part {
                    HirTextPart::Text { text } => HirTextPart::Text { text },
                    HirTextPart::Expr { expr } => HirTextPart::Expr {
                        expr: desugar_expr(expr, id_gen),
                    },
                })
                .collect(),
        },
        HirExpr::Lambda { id, param, body } => HirExpr::Lambda {
            id,
            param,
            body: Box::new(desugar_expr(*body, id_gen)),
        },
        HirExpr::App { id, func, arg } => HirExpr::App {
            id,
            func: Box::new(desugar_expr(*func, id_gen)),
            arg: Box::new(desugar_expr(*arg, id_gen)),
        },
        HirExpr::Call { id, func, args } => HirExpr::Call {
            id,
            func: Box::new(desugar_expr(*func, id_gen)),
            args: args
                .into_iter()
                .map(|a| desugar_expr(a, id_gen))
                .collect(),
        },
        HirExpr::DebugFn {
            id,
            fn_name,
            arg_vars,
            log_args,
            log_return,
            log_time,
            body,
        } => HirExpr::DebugFn {
            id,
            fn_name,
            arg_vars,
            log_args,
            log_return,
            log_time,
            body: Box::new(desugar_expr(*body, id_gen)),
        },
        HirExpr::Pipe {
            id,
            pipe_id,
            step,
            label,
            log_time,
            func,
            arg,
        } => HirExpr::Pipe {
            id,
            pipe_id,
            step,
            label,
            log_time,
            func: Box::new(desugar_expr(*func, id_gen)),
            arg: Box::new(desugar_expr(*arg, id_gen)),
        },
        HirExpr::List { id, items } => HirExpr::List {
            id,
            items: items
                .into_iter()
                .map(|i| HirListItem {
                    expr: desugar_expr(i.expr, id_gen),
                    spread: i.spread,
                })
                .collect(),
        },
        HirExpr::Tuple { id, items } => HirExpr::Tuple {
            id,
            items: items
                .into_iter()
                .map(|e| desugar_expr(e, id_gen))
                .collect(),
        },
        HirExpr::Record { id, fields } => HirExpr::Record {
            id,
            fields: fields
                .into_iter()
                .map(|f| desugar_record_field(f, id_gen))
                .collect(),
        },
        HirExpr::Patch { id, target, fields } => HirExpr::Patch {
            id,
            target: Box::new(desugar_expr(*target, id_gen)),
            fields: fields
                .into_iter()
                .map(|f| desugar_record_field(f, id_gen))
                .collect(),
        },
        HirExpr::FieldAccess { id, base, field } => HirExpr::FieldAccess {
            id,
            base: Box::new(desugar_expr(*base, id_gen)),
            field,
        },
        HirExpr::Index {
            id,
            base,
            index,
            location,
        } => HirExpr::Index {
            id,
            base: Box::new(desugar_expr(*base, id_gen)),
            index: Box::new(desugar_expr(*index, id_gen)),
            location,
        },
        HirExpr::Match {
            id,
            scrutinee,
            arms,
            location,
        } => HirExpr::Match {
            id,
            scrutinee: Box::new(desugar_expr(*scrutinee, id_gen)),
            arms: arms
                .into_iter()
                .map(|a| HirMatchArm {
                    pattern: a.pattern,
                    guard: a.guard.map(|g| desugar_expr(g, id_gen)),
                    guard_negated: a.guard_negated,
                    body: desugar_expr(a.body, id_gen),
                })
                .collect(),
            location,
        },
        HirExpr::If {
            id,
            cond,
            then_branch,
            else_branch,
        } => HirExpr::If {
            id,
            cond: Box::new(desugar_expr(*cond, id_gen)),
            then_branch: Box::new(desugar_expr(*then_branch, id_gen)),
            else_branch: Box::new(desugar_expr(*else_branch, id_gen)),
        },
        HirExpr::Binary {
            id,
            op,
            left,
            right,
            location,
        } => HirExpr::Binary {
            id,
            op,
            left: Box::new(desugar_expr(*left, id_gen)),
            right: Box::new(desugar_expr(*right, id_gen)),
            location,
        },

        // ── THE ACTUAL WORK — block desugaring ──
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

        HirExpr::Mock {
            id,
            substitutions,
            body,
        } => HirExpr::Mock {
            id,
            substitutions: substitutions
                .into_iter()
                .map(|sub| HirMockSubstitution {
                    path: sub.path,
                    snapshot: sub.snapshot,
                    value: sub.value.map(|v| desugar_expr(v, id_gen)),
                })
                .collect(),
            body: Box::new(desugar_expr(*body, id_gen)),
        },
    }
}

fn desugar_record_field(field: HirRecordField, id_gen: &mut IdGen) -> HirRecordField {
    HirRecordField {
        spread: field.spread,
        path: field
            .path
            .into_iter()
            .map(|seg| match seg {
                HirPathSegment::Index(expr) => {
                    HirPathSegment::Index(desugar_expr(expr, id_gen))
                }
                other => other,
            })
            .collect(),
        value: desugar_expr(field.value, id_gen),
    }
}

// ── Generator Church-encoding ─────────────────────────────────────────────────

fn lower_generate_block(items: Vec<HirBlockItem>, id_gen: &mut IdGen) -> HirExpr {
    if items.is_empty() {
        return gen_empty(id_gen);
    }

    let item = items[0].clone();
    let rest = items[1..].to_vec();

    match item {
        HirBlockItem::Yield { expr } => {
            let yield_expr = gen_yield(desugar_expr(expr, id_gen), id_gen);
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
            let raw_src = desugar_expr(expr, id_gen);
            if is_monadic {
                // `x <- expr` in a generate block: iterate over expr as a generator.
                // Wrap with __asGenerator so bare lists are implicitly converted.
                let src = HirExpr::App {
                    id: id_gen.next(),
                    func: Box::new(HirExpr::Var {
                        id: id_gen.next(),
                        name: "__asGenerator".to_string(),
                    
                        location: None,
                    }),
                    arg: Box::new(raw_src),
                };
                let next = lower_generate_block(rest, id_gen);
                let param_name = format!("_gen_bind_{}", id_gen.next());
                let param_var = HirExpr::Var {
                    id: id_gen.next(),
                    name: param_name.clone(),
                
                    location: None,
                };
                let body = HirExpr::Match {
                    id: id_gen.next(),
                    scrutinee: Box::new(param_var),
                    arms: vec![HirMatchArm {
                        pattern,
                        guard: None,
                        guard_negated: false,
                        body: next,
                    }],
                    location: None,
                };
                let func = HirExpr::Lambda {
                    id: id_gen.next(),
                    param: param_name,
                    body: Box::new(body),
                };
                gen_bind(src, func, id_gen)
            } else {
                // `s = expr` in a generate block: plain let-binding.
                // Check if this is a recursive __loop binding (from loop/recurse desugaring).
                let is_recursive_loop = matches!(&pattern, HirPattern::Var { name, .. } if name.starts_with("__loop"));

                let next = lower_generate_block(rest, id_gen);
                let param_name = format!("_gen_let_{}", id_gen.next());
                let param_var = HirExpr::Var {
                    id: id_gen.next(),
                    name: param_name.clone(),
                
                    location: None,
                };
                let body = HirExpr::Match {
                    id: id_gen.next(),
                    scrutinee: Box::new(param_var),
                    arms: vec![HirMatchArm {
                        pattern: pattern.clone(),
                        guard: None,
                        guard_negated: false,
                        body: next,
                    }],
                    location: None,
                };

                let value = if is_recursive_loop {
                    // Recursive binding: __loop0 = λn → body_using___loop0
                    // Desugar to: __fix (λ__loop0 → raw_src)
                    // so __loop0 is bound before raw_src references it.
                    let loop_name = match &pattern {
                        HirPattern::Var { name, .. } => name.clone(),
                        _ => unreachable!(),
                    };
                    let fix_body = HirExpr::Lambda {
                        id: id_gen.next(),
                        param: loop_name,
                        body: Box::new(raw_src),
                    };
                    HirExpr::App {
                        id: id_gen.next(),
                        func: Box::new(HirExpr::Var {
                            id: id_gen.next(),
                            name: "__fix".to_string(),
                        
                            location: None,
                        }),
                        arg: Box::new(fix_body),
                    }
                } else {
                    raw_src
                };

                // (\param -> body) value  — immediately-applied lambda = let binding
                HirExpr::App {
                    id: id_gen.next(),
                    func: Box::new(HirExpr::Lambda {
                        id: id_gen.next(),
                        param: param_name,
                        body: Box::new(body),
                    }),
                    arg: Box::new(value),
                }
            }
        }
        HirBlockItem::Expr { expr } => {
            // Treat as sub-generator to spread/append.
            // Wrap with __asGenerator so Unit (e.g. from empty else branches
            // in loop/recurse) becomes an empty generator.
            let raw = desugar_expr(expr, id_gen);
            let head = HirExpr::App {
                id: id_gen.next(),
                func: Box::new(HirExpr::Var {
                    id: id_gen.next(),
                    name: "__asGenerator".to_string(),
                
                    location: None,
                }),
                arg: Box::new(raw),
            };
            if rest.is_empty() {
                head
            } else {
                gen_append(head, lower_generate_block(rest, id_gen), id_gen)
            }
        }
        HirBlockItem::Filter { expr } => {
            let cond = desugar_expr(expr, id_gen);
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
fn gen_empty(id_gen: &mut IdGen) -> HirExpr {
    let k = format!("_k_{}", id_gen.next());
    let z = format!("_z_{}", id_gen.next());
    HirExpr::Lambda {
        id: id_gen.next(),
        param: k,
        body: Box::new(HirExpr::Lambda {
            id: id_gen.next(),
            param: z.clone(),
            body: Box::new(HirExpr::Var {
                id: id_gen.next(),
                name: z,
            
                location: None,
            }),
        }),
    }
}

// \k -> \z -> k z x
fn gen_yield(val: HirExpr, id_gen: &mut IdGen) -> HirExpr {
    let k_name = format!("_k_{}", id_gen.next());
    let z_name = format!("_z_{}", id_gen.next());
    let k = HirExpr::Var {
        id: id_gen.next(),
        name: k_name.clone(),
    
        location: None,
    };
    let z = HirExpr::Var {
        id: id_gen.next(),
        name: z_name.clone(),
    
        location: None,
    };

    // k z val
    let k_app_z = HirExpr::App {
        id: id_gen.next(),
        func: Box::new(k),
        arg: Box::new(z),
    };
    let k_app_z_val = HirExpr::App {
        id: id_gen.next(),
        func: Box::new(k_app_z),
        arg: Box::new(val),
    };

    HirExpr::Lambda {
        id: id_gen.next(),
        param: k_name,
        body: Box::new(HirExpr::Lambda {
            id: id_gen.next(),
            param: z_name,
            body: Box::new(k_app_z_val),
        }),
    }
}

// \k -> \z -> g2 k (g1 k z)
fn gen_append(g1: HirExpr, g2: HirExpr, id_gen: &mut IdGen) -> HirExpr {
    let k_name = format!("_k_{}", id_gen.next());
    let z_name = format!("_z_{}", id_gen.next());
    let k = HirExpr::Var {
        id: id_gen.next(),
        name: k_name.clone(),
    
        location: None,
    };
    let z = HirExpr::Var {
        id: id_gen.next(),
        name: z_name.clone(),
    
        location: None,
    };

    // g1 k z
    let g1_k = HirExpr::App {
        id: id_gen.next(),
        func: Box::new(g1),
        arg: Box::new(k.clone()),
    };
    let g1_k_z = HirExpr::App {
        id: id_gen.next(),
        func: Box::new(g1_k),
        arg: Box::new(z.clone()),
    };

    // g2 k (g1 k z)
    let g2_k = HirExpr::App {
        id: id_gen.next(),
        func: Box::new(g2),
        arg: Box::new(k),
    };
    let g2_k_res = HirExpr::App {
        id: id_gen.next(),
        func: Box::new(g2_k),
        arg: Box::new(g1_k_z),
    };

    HirExpr::Lambda {
        id: id_gen.next(),
        param: k_name,
        body: Box::new(HirExpr::Lambda {
            id: id_gen.next(),
            param: z_name,
            body: Box::new(g2_k_res),
        }),
    }
}

// \k -> \z -> if cond then next(k, z) else z
fn gen_if(cond: HirExpr, next: HirExpr, id_gen: &mut IdGen) -> HirExpr {
    let k_name = format!("_k_{}", id_gen.next());
    let z_name = format!("_z_{}", id_gen.next());
    let k = HirExpr::Var {
        id: id_gen.next(),
        name: k_name.clone(),
    
        location: None,
    };
    let z = HirExpr::Var {
        id: id_gen.next(),
        name: z_name.clone(),
    
        location: None,
    };

    // next k z
    let next_k = HirExpr::App {
        id: id_gen.next(),
        func: Box::new(next),
        arg: Box::new(k.clone()),
    };
    let next_k_z = HirExpr::App {
        id: id_gen.next(),
        func: Box::new(next_k),
        arg: Box::new(z.clone()),
    };

    let if_expr = HirExpr::If {
        id: id_gen.next(),
        cond: Box::new(cond),
        then_branch: Box::new(next_k_z),
        else_branch: Box::new(z),
    };

    HirExpr::Lambda {
        id: id_gen.next(),
        param: k_name,
        body: Box::new(HirExpr::Lambda {
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

fn lower_do_effect_block(items: Vec<HirBlockItem>, id_gen: &mut IdGen) -> HirExpr {
    if items.is_empty() {
        return effect_pure_unit(id_gen);
    }
    let chain = lower_do_effect_items(&items, 0, id_gen);
    // Wrap in __withResourceScope so resource cleanups are scoped to this block
    HirExpr::Call {
        id: id_gen.next(),
        func: Box::new(HirExpr::Var {
            id: id_gen.next(),
            name: "__withResourceScope".to_string(),
        
            location: None,
        }),
        args: vec![chain],
    }
}

fn lower_do_effect_items(items: &[HirBlockItem], idx: usize, id_gen: &mut IdGen) -> HirExpr {
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
            let lowered_expr = desugar_expr(expr.clone(), id_gen);
            let rest = if is_last {
                // Tail bind: bind e (λx → pure x)
                let param = format!("_do_{}", id_gen.next());
                let body = effect_pure(
                    HirExpr::Var {
                        id: id_gen.next(),
                        name: param.clone(),
                    
                        location: None,
                    },
                    id_gen,
                );
                return effect_bind(
                    lowered_expr,
                    HirExpr::Lambda {
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
                HirExpr::Lambda {
                    id: id_gen.next(),
                    param,
                    body: Box::new(body),
                },
                id_gen,
            )
        }
        HirBlockItem::Expr { expr } => {
            let lowered_expr = desugar_expr(expr.clone(), id_gen);
            if is_last {
                // Final expression in do block
                lowered_expr
            } else {
                // expr; rest  →  bind expr (λ_ → rest)
                let rest = lower_do_effect_items(items, idx + 1, id_gen);
                let param = format!("_do_{}", id_gen.next());
                effect_bind(
                    lowered_expr,
                    HirExpr::Lambda {
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
            let lowered_expr = desugar_expr(expr.clone(), id_gen);
            let wrapped = effect_pure(lowered_expr, id_gen);
            if is_last {
                wrapped
            } else {
                let rest = lower_do_effect_items(items, idx + 1, id_gen);
                let param = format!("_do_{}", id_gen.next());
                effect_bind(
                    wrapped,
                    HirExpr::Lambda {
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
            desugar_expr(expr.clone(), id_gen)
        }
        HirBlockItem::Filter { expr } => {
            // Filters in effect blocks: guard
            let lowered_expr = desugar_expr(expr.clone(), id_gen);
            let rest = if is_last {
                effect_pure_unit(id_gen)
            } else {
                lower_do_effect_items(items, idx + 1, id_gen)
            };
            HirExpr::If {
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
    body: HirExpr,
    id_gen: &mut IdGen,
) -> HirExpr {
    match pattern {
        HirPattern::Var { name, .. } if *name == param => body,
        HirPattern::Wildcard { .. } => body,
        _ => HirExpr::Match {
            id: id_gen.next(),
            scrutinee: Box::new(HirExpr::Var {
                id: id_gen.next(),
                name: param,
            
                location: None,
            }),
            arms: vec![HirMatchArm {
                pattern: pattern.clone(),
                guard: None,
                guard_negated: false,
                body,
            }],
            location: None,
        },
    }
}

/// `bind e f` → App (App (Var "bind") e) f
fn effect_bind(expr: HirExpr, func: HirExpr, id_gen: &mut IdGen) -> HirExpr {
    HirExpr::App {
        id: id_gen.next(),
        func: Box::new(HirExpr::App {
            id: id_gen.next(),
            func: Box::new(HirExpr::Var {
                id: id_gen.next(),
                name: "bind".to_string(),
            
                location: None,
            }),
            arg: Box::new(expr),
        }),
        arg: Box::new(func),
    }
}

/// `pure e` → App (Var "pure") e
fn effect_pure(expr: HirExpr, id_gen: &mut IdGen) -> HirExpr {
    HirExpr::App {
        id: id_gen.next(),
        func: Box::new(HirExpr::Var {
            id: id_gen.next(),
            name: "pure".to_string(),
        
            location: None,
        }),
        arg: Box::new(expr),
    }
}

/// `pure Unit`
fn effect_pure_unit(id_gen: &mut IdGen) -> HirExpr {
    effect_pure(
        HirExpr::Var {
            id: id_gen.next(),
            name: "Unit".to_string(),
        
            location: None,
        },
        id_gen,
    )
}

// ── resource { ... } desugaring ───────────────────────────────────────────────
//
// resource { acquire; yield x; cleanup }
//   → __makeResource (λ_ → <desugared acquire+yield>) (λ_ → <desugared cleanup>)

fn lower_resource_block(id: u32, items: Vec<HirBlockItem>, id_gen: &mut IdGen) -> HirExpr {
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
    let acquire_lambda = HirExpr::Lambda {
        id: id_gen.next(),
        param: format!("_res_unused_{}", id_gen.next()),
        body: Box::new(acquire_body),
    };

    let cleanup_body = if cleanup_items.is_empty() {
        effect_pure_unit(id_gen)
    } else {
        lower_do_effect_block(cleanup_items, id_gen)
    };
    let cleanup_lambda = HirExpr::Lambda {
        id: id_gen.next(),
        param: format!("_res_unused_{}", id_gen.next()),
        body: Box::new(cleanup_body),
    };

    HirExpr::Call {
        id,
        func: Box::new(HirExpr::Var {
            id: id_gen.next(),
            name: "__makeResource".to_string(),
        
            location: None,
        }),
        args: vec![acquire_lambda, cleanup_lambda],
    }
}

// ── plain { ... } desugaring ──────────────────────────────────────────────────
//
// { x = e1; y = e2; body }  →  (λx → (λy → body) e2) e1

fn lower_plain_block(items: Vec<HirBlockItem>, id_gen: &mut IdGen) -> HirExpr {
    if items.is_empty() {
        return HirExpr::Var {
            id: id_gen.next(),
            name: "Unit".to_string(),
        
            location: None,
        };
    }
    lower_plain_items(&items, 0, id_gen)
}

fn lower_plain_items(items: &[HirBlockItem], idx: usize, id_gen: &mut IdGen) -> HirExpr {
    if idx >= items.len() {
        return HirExpr::Var {
            id: id_gen.next(),
            name: "Unit".to_string(),
        
            location: None,
        };
    }

    let item = &items[idx];
    let is_last = idx + 1 >= items.len();

    match item {
        HirBlockItem::Bind { pattern, expr, .. } => {
            let lowered_expr = desugar_expr(expr.clone(), id_gen);
            if is_last {
                lowered_expr
            } else {
                let rest = lower_plain_items(items, idx + 1, id_gen);
                let param = pattern_to_param(pattern, id_gen);
                let body = wrap_pattern_match(param.clone(), pattern, rest, id_gen);
                HirExpr::App {
                    id: id_gen.next(),
                    func: Box::new(HirExpr::Lambda {
                        id: id_gen.next(),
                        param,
                        body: Box::new(body),
                    }),
                    arg: Box::new(lowered_expr),
                }
            }
        }
        HirBlockItem::Expr { expr } | HirBlockItem::Yield { expr } => {
            let lowered_expr = desugar_expr(expr.clone(), id_gen);
            if is_last {
                lowered_expr
            } else {
                let rest = lower_plain_items(items, idx + 1, id_gen);
                let param = format!("_plain_{}", id_gen.next());
                HirExpr::App {
                    id: id_gen.next(),
                    func: Box::new(HirExpr::Lambda {
                        id: id_gen.next(),
                        param,
                        body: Box::new(rest),
                    }),
                    arg: Box::new(lowered_expr),
                }
            }
        }
        HirBlockItem::Recurse { expr } => desugar_expr(expr.clone(), id_gen),
        HirBlockItem::Filter { expr } => {
            let lowered_expr = desugar_expr(expr.clone(), id_gen);
            let rest = if is_last {
                HirExpr::Var {
                    id: id_gen.next(),
                    name: "Unit".to_string(),
                
                    location: None,
                }
            } else {
                lower_plain_items(items, idx + 1, id_gen)
            };
            HirExpr::If {
                id: id_gen.next(),
                cond: Box::new(lowered_expr),
                then_branch: Box::new(rest),
                else_branch: Box::new(HirExpr::Var {
                    id: id_gen.next(),
                    name: "Unit".to_string(),
                
                    location: None,
                }),
            }
        }
    }
}
