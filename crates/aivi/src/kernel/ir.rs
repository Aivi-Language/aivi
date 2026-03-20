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

fn is_recursive_generated_binding_name(name: &str) -> bool {
    name.starts_with("__loop") || name.starts_with("__flow_restart_")
}

fn wrap_recursive_generated_binding(
    pattern: &HirPattern,
    raw_src: HirExpr,
    id_gen: &mut IdGen,
) -> HirExpr {
    let binding_name = match pattern {
        HirPattern::Var { name, .. } => name.clone(),
        _ => unreachable!(),
    };
    let fix_body = HirExpr::Lambda {
        id: id_gen.next(),
        param: binding_name,
        body: Box::new(raw_src),
        location: None,
    };
    HirExpr::App {
        id: id_gen.next(),
        func: Box::new(HirExpr::Var {
            id: id_gen.next(),
            name: "__fix".to_string(),
            location: None,
        }),
        arg: Box::new(fix_body),
        location: None,
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
        location: def.location,
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
        HirExpr::Lambda {
            id,
            param,
            body,
            location,
        } => HirExpr::Lambda {
            id,
            param,
            body: Box::new(desugar_expr(*body, id_gen)),
            location,
        },
        HirExpr::App {
            id,
            func,
            arg,
            location,
        } => HirExpr::App {
            id,
            func: Box::new(desugar_expr(*func, id_gen)),
            arg: Box::new(desugar_expr(*arg, id_gen)),
            location,
        },
        HirExpr::Call {
            id,
            func,
            args,
            location,
        } => HirExpr::Call {
            id,
            func: Box::new(desugar_expr(*func, id_gen)),
            args: args
                .into_iter()
                .map(|a| desugar_expr(a, id_gen))
                .collect(),
            location,
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
            location,
        } => HirExpr::Pipe {
            id,
            pipe_id,
            step,
            label,
            log_time,
            func: Box::new(desugar_expr(*func, id_gen)),
            arg: Box::new(desugar_expr(*arg, id_gen)),
            location,
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
        HirExpr::FieldAccess {
            id,
            base,
            field,
            location,
        } => HirExpr::FieldAccess {
            id,
            base: Box::new(desugar_expr(*base, id_gen)),
            field,
            location,
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
            location,
        } => HirExpr::If {
            id,
            cond: Box::new(desugar_expr(*cond, id_gen)),
            then_branch: Box::new(desugar_expr(*then_branch, id_gen)),
            else_branch: Box::new(desugar_expr(*else_branch, id_gen)),
            location,
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
            HirBlockKind::Do { monad } if monad == "Effect" => {
                lower_effect_block(items, id_gen)
            }
            HirBlockKind::Do { .. } => {
                // Generic `do M` is already desugared at HIR level into chain/lambda calls.
                // If we still get one here, treat remaining items as a plain block.
                lower_plain_block(items, id_gen)
            }
            HirBlockKind::Managed => lower_managed_block(id, items, id_gen),
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

// ── effect-style block desugaring ─────────────────────────────────────────────
//
// Transforms:
//   x <- e1; e2        →  bind e1 (λx → e2)
//   x = e1; e2         →  (λx → e2) e1      (let-binding, non-monadic)
//   expr; rest          →  bind expr (λ_ → rest)
//   yield expr          →  expr               (final expression)
//   recurse expr        →  expr               (final expression, tail call)
//
// Empty block → pure Unit

fn lower_effect_block(items: Vec<HirBlockItem>, id_gen: &mut IdGen) -> HirExpr {
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
        location: None,
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
                                        location: None,
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
                                location: None,
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
                                        location: None,
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
                                        location: None,
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
                location: None,
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
            location: None,
        }),
        arg: Box::new(func),
        location: None,
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
        location: None,
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

// ── managed cleanup block desugaring ──────────────────────────────────────────
//
// managed { acquire; yield x; cleanup }
//   → __makeResource (λ_ → <desugared acquire yielding (x, λ_ → <desugared cleanup>)>)

fn lower_managed_block(id: u32, items: Vec<HirBlockItem>, id_gen: &mut IdGen) -> HirExpr {
    let yield_pos = items
        .iter()
        .position(|item| matches!(item, HirBlockItem::Yield { .. }));

    let acquire_body = match yield_pos {
        Some(pos) => {
            let acquire_items = items[..pos].to_vec();
            let yield_expr = match &items[pos] {
                HirBlockItem::Yield { expr } => expr.clone(),
                _ => unreachable!("yield position must point at a yield item"),
            };
            let cleanup_items = items[pos + 1..].to_vec();
            let cleanup_body = if cleanup_items.is_empty() {
                effect_pure_unit(id_gen)
            } else {
                HirExpr::Block {
                    id: id_gen.next(),
                    block_kind: HirBlockKind::Do {
                        monad: "Effect".to_string(),
                    },
                    items: cleanup_items,
                }
            };
            let cleanup_lambda = HirExpr::Lambda {
                id: id_gen.next(),
                param: format!("_res_unused_{}", id_gen.next()),
                body: Box::new(cleanup_body),
                        location: None,
};
            let mut bundled_acquire_items = acquire_items;
            bundled_acquire_items.push(HirBlockItem::Yield {
                expr: HirExpr::Tuple {
                    id: id_gen.next(),
                    items: vec![yield_expr, cleanup_lambda],
                },
            });
            lower_effect_block(bundled_acquire_items, id_gen)
        }
        None => {
            let acquire_effect = lower_effect_block(items, id_gen);
            let cleanup_lambda = HirExpr::Lambda {
                id: id_gen.next(),
                param: format!("_res_unused_{}", id_gen.next()),
                body: Box::new(effect_pure_unit(id_gen)),
                        location: None,
};
            let value_param = format!("_resource_value_{}", id_gen.next());
            effect_bind(
                acquire_effect,
                HirExpr::Lambda {
                    id: id_gen.next(),
                    param: value_param.clone(),
                    body: Box::new(effect_pure(
                        HirExpr::Tuple {
                            id: id_gen.next(),
                            items: vec![
                                HirExpr::Var {
                                    id: id_gen.next(),
                                    name: value_param,
                                
                                    location: None,
                                },
                                cleanup_lambda,
                            ],
                        },
                        id_gen,
                    )),
                
                    location: None,
    },
                id_gen,
            )
        }
    };
    let acquire_lambda = HirExpr::Lambda {
        id: id_gen.next(),
        param: format!("_res_unused_{}", id_gen.next()),
        body: Box::new(acquire_body),
        location: None,
};

    HirExpr::Call {
        id,
        func: Box::new(HirExpr::Var {
            id: id_gen.next(),
            name: "__makeResource".to_string(),
        
            location: None,
        }),
        args: vec![acquire_lambda],
        location: None,
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
                let lowered_expr = match pattern {
                    HirPattern::Var { name, .. }
                        if is_recursive_generated_binding_name(name) =>
                    {
                        wrap_recursive_generated_binding(pattern, lowered_expr, id_gen)
                    }
                    _ => lowered_expr,
                };
                let rest = lower_plain_items(items, idx + 1, id_gen);
                let param = pattern_to_param(pattern, id_gen);
                let body = wrap_pattern_match(param.clone(), pattern, rest, id_gen);
                HirExpr::App {
                    id: id_gen.next(),
                    func: Box::new(HirExpr::Lambda {
                        id: id_gen.next(),
                        param,
                        body: Box::new(body),
                                        location: None,
}),
                    arg: Box::new(lowered_expr),
                    location: None,
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
                                        location: None,
}),
                    arg: Box::new(lowered_expr),
                    location: None,
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
                location: None,
            }
        }
    }
}
