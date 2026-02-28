//! RustIR-level function inlining pass.
//!
//! Runs after monomorphization and before Cranelift lowering. Replaces call
//! sites of small, non-recursive functions with the callee's body, eliminating
//! call overhead and exposing more code to Cranelift's local optimisations.
//!
//! Auto-inlines functions whose body cost is below `INLINE_THRESHOLD`.

use std::collections::HashMap;

use crate::rust_ir::{
    RustIrBlockItem, RustIrExpr, RustIrListItem, RustIrMatchArm, RustIrMockSubstitution,
    RustIrModule, RustIrPattern, RustIrRecordField, RustIrTextPart,
};

/// Maximum AST-node cost for automatic inlining.
const INLINE_THRESHOLD: u32 = 12;

/// Maximum inlining depth to prevent runaway expansion from mutual inlining.
const MAX_INLINE_DEPTH: u32 = 4;

// ---------------------------------------------------------------------------
// Phase 1: Expression size estimation
// ---------------------------------------------------------------------------

/// Count AST nodes as a proxy for code size.
pub(crate) fn expr_cost(expr: &RustIrExpr) -> u32 {
    match expr {
        RustIrExpr::Local { .. }
        | RustIrExpr::Global { .. }
        | RustIrExpr::Builtin { .. }
        | RustIrExpr::ConstructorValue { .. }
        | RustIrExpr::LitNumber { .. }
        | RustIrExpr::LitString { .. }
        | RustIrExpr::LitBool { .. }
        | RustIrExpr::LitDateTime { .. }
        | RustIrExpr::Raw { .. } => 1,

        RustIrExpr::LitSigil { .. } => 1,

        RustIrExpr::TextInterpolate { parts, .. } => {
            1 + parts
                .iter()
                .map(|p| match p {
                    RustIrTextPart::Text { .. } => 0,
                    RustIrTextPart::Expr { expr } => expr_cost(expr),
                })
                .sum::<u32>()
        }

        RustIrExpr::Lambda { body, .. } => 1 + expr_cost(body),

        RustIrExpr::App { func, arg, .. } | RustIrExpr::Pipe { func, arg, .. } => {
            1 + expr_cost(func) + expr_cost(arg)
        }

        RustIrExpr::Call { func, args, .. } => {
            1 + expr_cost(func) + args.iter().map(expr_cost).sum::<u32>()
        }

        RustIrExpr::DebugFn { body, .. } => 1 + expr_cost(body),

        RustIrExpr::List { items, .. } => 1 + items.iter().map(|i| expr_cost(&i.expr)).sum::<u32>(),

        RustIrExpr::Tuple { items, .. } => 1 + items.iter().map(expr_cost).sum::<u32>(),

        RustIrExpr::Record { fields, .. } => 1 + record_fields_cost(fields),

        RustIrExpr::Patch { target, fields, .. } => {
            1 + expr_cost(target) + record_fields_cost(fields)
        }

        RustIrExpr::FieldAccess { base, .. } => 1 + expr_cost(base),

        RustIrExpr::Index { base, index, .. } => 1 + expr_cost(base) + expr_cost(index),

        RustIrExpr::Match {
            scrutinee, arms, ..
        } => {
            1 + expr_cost(scrutinee)
                + arms
                    .iter()
                    .map(|a| {
                        pattern_cost(&a.pattern)
                            + a.guard.as_ref().map_or(0, expr_cost)
                            + expr_cost(&a.body)
                    })
                    .sum::<u32>()
        }

        RustIrExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => 1 + expr_cost(cond) + expr_cost(then_branch) + expr_cost(else_branch),

        RustIrExpr::Binary { left, right, .. } => 1 + expr_cost(left) + expr_cost(right),

        RustIrExpr::Block { items, .. } => 1 + block_items_cost(items),

        RustIrExpr::Mock {
            substitutions,
            body,
            ..
        } => {
            1 + substitutions
                .iter()
                .map(|s| s.value.as_ref().map_or(1, expr_cost))
                .sum::<u32>()
                + expr_cost(body)
        }
    }
}

fn record_fields_cost(fields: &[RustIrRecordField]) -> u32 {
    fields
        .iter()
        .map(|f| {
            expr_cost(&f.value)
                + f.path
                    .iter()
                    .map(|seg| match seg {
                        crate::rust_ir::RustIrPathSegment::IndexValue(e)
                        | crate::rust_ir::RustIrPathSegment::IndexPredicate(e) => expr_cost(e),
                        _ => 0,
                    })
                    .sum::<u32>()
        })
        .sum()
}

fn pattern_cost(pat: &RustIrPattern) -> u32 {
    match pat {
        RustIrPattern::Wildcard { .. }
        | RustIrPattern::Var { .. }
        | RustIrPattern::Literal { .. } => 1,
        RustIrPattern::At { pattern, .. } => 1 + pattern_cost(pattern),
        RustIrPattern::Constructor { args, .. } => 1 + args.iter().map(pattern_cost).sum::<u32>(),
        RustIrPattern::Tuple { items, .. } => 1 + items.iter().map(pattern_cost).sum::<u32>(),
        RustIrPattern::List { items, rest, .. } => {
            1 + items.iter().map(pattern_cost).sum::<u32>()
                + rest.as_ref().map_or(0, |r| pattern_cost(r))
        }
        RustIrPattern::Record { fields, .. } => {
            1 + fields.iter().map(|f| pattern_cost(&f.pattern)).sum::<u32>()
        }
    }
}

fn block_items_cost(items: &[RustIrBlockItem]) -> u32 {
    items
        .iter()
        .map(|item| match item {
            RustIrBlockItem::Bind { expr, .. } => 1 + expr_cost(expr),
            RustIrBlockItem::Filter { expr }
            | RustIrBlockItem::Yield { expr }
            | RustIrBlockItem::Recurse { expr }
            | RustIrBlockItem::Expr { expr } => expr_cost(expr),
        })
        .sum()
}

// ---------------------------------------------------------------------------
// Phase 2: Inline candidate collection
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct InlineCandidate {
    params: Vec<String>,
    body: RustIrExpr,
}

/// Build an index of functions eligible for inlining.
///
/// Returns a map from qualified function name → candidate (params + body).
fn collect_candidates(modules: &[RustIrModule]) -> HashMap<String, InlineCandidate> {
    let mut candidates = HashMap::new();

    for module in modules {
        for def in &module.defs {
            let qualified = format!("{}.{}", module.name, def.name);
            let (params, body) = peel_params(&def.expr);

            if params.is_empty() {
                continue; // not a function
            }

            // Check for self-recursion: body references own name
            if references_name(body, &def.name) || references_name(body, &qualified) {
                continue;
            }

            let cost = expr_cost(body);
            let eligible = cost <= INLINE_THRESHOLD;

            if eligible {
                candidates.insert(
                    qualified.clone(),
                    InlineCandidate {
                        params: params.clone(),
                        body: body.clone(),
                    },
                );
                // Also register under short name for intra-module calls
                candidates.insert(
                    def.name.clone(),
                    InlineCandidate {
                        params,
                        body: body.clone(),
                    },
                );
            }
        }
    }

    candidates
}

/// Peel Lambda wrappers to extract parameter names and the innermost body.
fn peel_params(expr: &RustIrExpr) -> (Vec<String>, &RustIrExpr) {
    let mut params = Vec::new();
    let mut cursor = expr;
    loop {
        match cursor {
            RustIrExpr::Lambda { param, body, .. } => {
                params.push(param.clone());
                cursor = body.as_ref();
            }
            _ => return (params, cursor),
        }
    }
}

/// Check if an expression references a given name via `Global`.
fn references_name(expr: &RustIrExpr, name: &str) -> bool {
    match expr {
        RustIrExpr::Global { name: n, .. } => n == name,
        RustIrExpr::Local { .. }
        | RustIrExpr::Builtin { .. }
        | RustIrExpr::ConstructorValue { .. }
        | RustIrExpr::LitNumber { .. }
        | RustIrExpr::LitString { .. }
        | RustIrExpr::LitBool { .. }
        | RustIrExpr::LitDateTime { .. }
        | RustIrExpr::LitSigil { .. }
        | RustIrExpr::Raw { .. } => false,

        RustIrExpr::TextInterpolate { parts, .. } => parts.iter().any(|p| match p {
            RustIrTextPart::Text { .. } => false,
            RustIrTextPart::Expr { expr } => references_name(expr, name),
        }),

        RustIrExpr::Lambda { body, .. } | RustIrExpr::DebugFn { body, .. } => {
            references_name(body, name)
        }

        RustIrExpr::App { func, arg, .. } | RustIrExpr::Pipe { func, arg, .. } => {
            references_name(func, name) || references_name(arg, name)
        }

        RustIrExpr::Call { func, args, .. } => {
            references_name(func, name) || args.iter().any(|a| references_name(a, name))
        }

        RustIrExpr::List { items, .. } => items.iter().any(|i| references_name(&i.expr, name)),

        RustIrExpr::Tuple { items, .. } => items.iter().any(|i| references_name(i, name)),

        RustIrExpr::Record { fields, .. } => fields.iter().any(|f| references_name(&f.value, name)),

        RustIrExpr::Patch { target, fields, .. } => {
            references_name(target, name) || fields.iter().any(|f| references_name(&f.value, name))
        }

        RustIrExpr::FieldAccess { base, .. } => references_name(base, name),

        RustIrExpr::Index { base, index, .. } => {
            references_name(base, name) || references_name(index, name)
        }

        RustIrExpr::Match {
            scrutinee, arms, ..
        } => {
            references_name(scrutinee, name)
                || arms.iter().any(|a| {
                    a.guard.as_ref().is_some_and(|g| references_name(g, name))
                        || references_name(&a.body, name)
                })
        }

        RustIrExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            references_name(cond, name)
                || references_name(then_branch, name)
                || references_name(else_branch, name)
        }

        RustIrExpr::Binary { left, right, .. } => {
            references_name(left, name) || references_name(right, name)
        }

        RustIrExpr::Block { items, .. } => items.iter().any(|item| match item {
            RustIrBlockItem::Bind { expr, .. }
            | RustIrBlockItem::Filter { expr }
            | RustIrBlockItem::Yield { expr }
            | RustIrBlockItem::Recurse { expr }
            | RustIrBlockItem::Expr { expr } => references_name(expr, name),
        }),

        RustIrExpr::Mock {
            substitutions,
            body,
            ..
        } => {
            substitutions
                .iter()
                .any(|s| s.value.as_ref().is_some_and(|v| references_name(v, name)))
                || references_name(body, name)
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 3 + 5: Call-site substitution with id freshening
// ---------------------------------------------------------------------------

/// Counter for generating fresh expression ids.
struct IdGen(u32);

impl IdGen {
    fn new(start: u32) -> Self {
        Self(start)
    }
    fn next(&mut self) -> u32 {
        let id = self.0;
        self.0 += 1;
        id
    }
}

/// Find the maximum expression id across all modules (for freshening).
fn max_expr_id(modules: &[RustIrModule]) -> u32 {
    let mut max_id = 0u32;
    for module in modules {
        for def in &module.defs {
            max_id = max_id.max(max_expr_id_in(&def.expr));
        }
    }
    max_id
}

fn max_expr_id_in(expr: &RustIrExpr) -> u32 {
    let id = expr_id(expr);
    let child_max = match expr {
        RustIrExpr::Local { .. }
        | RustIrExpr::Global { .. }
        | RustIrExpr::Builtin { .. }
        | RustIrExpr::ConstructorValue { .. }
        | RustIrExpr::LitNumber { .. }
        | RustIrExpr::LitString { .. }
        | RustIrExpr::LitBool { .. }
        | RustIrExpr::LitDateTime { .. }
        | RustIrExpr::LitSigil { .. }
        | RustIrExpr::Raw { .. } => 0,

        RustIrExpr::TextInterpolate { parts, .. } => parts
            .iter()
            .map(|p| match p {
                RustIrTextPart::Text { .. } => 0,
                RustIrTextPart::Expr { expr } => max_expr_id_in(expr),
            })
            .max()
            .unwrap_or(0),

        RustIrExpr::Lambda { body, .. } | RustIrExpr::DebugFn { body, .. } => max_expr_id_in(body),

        RustIrExpr::App { func, arg, .. } | RustIrExpr::Pipe { func, arg, .. } => {
            max_expr_id_in(func).max(max_expr_id_in(arg))
        }

        RustIrExpr::Call { func, args, .. } => args
            .iter()
            .map(max_expr_id_in)
            .max()
            .unwrap_or(0)
            .max(max_expr_id_in(func)),

        RustIrExpr::List { items, .. } => items
            .iter()
            .map(|i| max_expr_id_in(&i.expr))
            .max()
            .unwrap_or(0),

        RustIrExpr::Tuple { items, .. } => items.iter().map(max_expr_id_in).max().unwrap_or(0),

        RustIrExpr::Record { fields, .. } => fields
            .iter()
            .map(|f| max_expr_id_in(&f.value))
            .max()
            .unwrap_or(0),

        RustIrExpr::Patch { target, fields, .. } => fields
            .iter()
            .map(|f| max_expr_id_in(&f.value))
            .max()
            .unwrap_or(0)
            .max(max_expr_id_in(target)),

        RustIrExpr::FieldAccess { base, .. } => max_expr_id_in(base),

        RustIrExpr::Index { base, index, .. } => max_expr_id_in(base).max(max_expr_id_in(index)),

        RustIrExpr::Match {
            scrutinee, arms, ..
        } => arms
            .iter()
            .map(|a| {
                a.guard
                    .as_ref()
                    .map_or(0, max_expr_id_in)
                    .max(max_expr_id_in(&a.body))
            })
            .max()
            .unwrap_or(0)
            .max(max_expr_id_in(scrutinee)),

        RustIrExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => max_expr_id_in(cond)
            .max(max_expr_id_in(then_branch))
            .max(max_expr_id_in(else_branch)),

        RustIrExpr::Binary { left, right, .. } => max_expr_id_in(left).max(max_expr_id_in(right)),

        RustIrExpr::Block { items, .. } => items
            .iter()
            .map(|item| match item {
                RustIrBlockItem::Bind { expr, .. }
                | RustIrBlockItem::Filter { expr }
                | RustIrBlockItem::Yield { expr }
                | RustIrBlockItem::Recurse { expr }
                | RustIrBlockItem::Expr { expr } => max_expr_id_in(expr),
            })
            .max()
            .unwrap_or(0),

        RustIrExpr::Mock {
            substitutions,
            body,
            ..
        } => substitutions
            .iter()
            .filter_map(|s| s.value.as_ref().map(max_expr_id_in))
            .max()
            .unwrap_or(0)
            .max(max_expr_id_in(body)),
    };
    id.max(child_max)
}

fn expr_id(expr: &RustIrExpr) -> u32 {
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
        | RustIrExpr::Raw { id, .. }
        | RustIrExpr::Mock { id, .. } => *id,
    }
}

/// Inline call sites in a single expression. Returns the rewritten expression.
fn inline_expr(
    expr: RustIrExpr,
    candidates: &HashMap<String, InlineCandidate>,
    id_gen: &mut IdGen,
    depth: u32,
) -> RustIrExpr {
    if depth > MAX_INLINE_DEPTH {
        return expr;
    }

    // First, recursively inline in children (bottom-up)
    let expr = inline_children(expr, candidates, id_gen, depth);

    // Then try to inline this node if it's a call to a candidate
    match &expr {
        RustIrExpr::App { func, arg, .. } => {
            if let RustIrExpr::Global { name, .. } = func.as_ref() {
                if let Some(candidate) = candidates.get(name.as_str()) {
                    if candidate.params.len() == 1 {
                        let mut body = candidate.body.clone();
                        freshen_ids(&mut body, id_gen);
                        substitute(&mut body, &candidate.params[0], arg);
                        // Re-inline the result one level deeper
                        return inline_expr(body, candidates, id_gen, depth + 1);
                    }
                }
            }
        }

        RustIrExpr::Call { func, args, .. } => {
            if let RustIrExpr::Global { name, .. } = func.as_ref() {
                if let Some(candidate) = candidates.get(name.as_str()) {
                    if candidate.params.len() == args.len() {
                        let mut body = candidate.body.clone();
                        freshen_ids(&mut body, id_gen);
                        for (param, arg) in candidate.params.iter().zip(args.iter()) {
                            substitute(&mut body, param, arg);
                        }
                        return inline_expr(body, candidates, id_gen, depth + 1);
                    }
                }
            }
        }

        // Also handle Pipe (equivalent to App)
        RustIrExpr::Pipe { func, arg, .. } => {
            if let RustIrExpr::Global { name, .. } = func.as_ref() {
                if let Some(candidate) = candidates.get(name.as_str()) {
                    if candidate.params.len() == 1 {
                        let mut body = candidate.body.clone();
                        freshen_ids(&mut body, id_gen);
                        substitute(&mut body, &candidate.params[0], arg);
                        return inline_expr(body, candidates, id_gen, depth + 1);
                    }
                }
            }
        }

        _ => {}
    }

    expr
}

/// Recursively inline in all children of an expression.
fn inline_children(
    expr: RustIrExpr,
    candidates: &HashMap<String, InlineCandidate>,
    id_gen: &mut IdGen,
    depth: u32,
) -> RustIrExpr {
    match expr {
        // Leaves — no children
        e @ (RustIrExpr::Local { .. }
        | RustIrExpr::Global { .. }
        | RustIrExpr::Builtin { .. }
        | RustIrExpr::ConstructorValue { .. }
        | RustIrExpr::LitNumber { .. }
        | RustIrExpr::LitString { .. }
        | RustIrExpr::LitBool { .. }
        | RustIrExpr::LitDateTime { .. }
        | RustIrExpr::LitSigil { .. }
        | RustIrExpr::Raw { .. }) => e,

        RustIrExpr::TextInterpolate { id, parts } => RustIrExpr::TextInterpolate {
            id,
            parts: parts
                .into_iter()
                .map(|p| match p {
                    RustIrTextPart::Text { .. } => p,
                    RustIrTextPart::Expr { expr } => RustIrTextPart::Expr {
                        expr: inline_expr(expr, candidates, id_gen, depth),
                    },
                })
                .collect(),
        },

        RustIrExpr::Lambda { id, param, body } => RustIrExpr::Lambda {
            id,
            param,
            body: Box::new(inline_expr(*body, candidates, id_gen, depth)),
        },

        RustIrExpr::App { id, func, arg } => RustIrExpr::App {
            id,
            func: Box::new(inline_expr(*func, candidates, id_gen, depth)),
            arg: Box::new(inline_expr(*arg, candidates, id_gen, depth)),
        },

        RustIrExpr::Call { id, func, args } => RustIrExpr::Call {
            id,
            func: Box::new(inline_expr(*func, candidates, id_gen, depth)),
            args: args
                .into_iter()
                .map(|a| inline_expr(a, candidates, id_gen, depth))
                .collect(),
        },

        RustIrExpr::Pipe {
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
            func: Box::new(inline_expr(*func, candidates, id_gen, depth)),
            arg: Box::new(inline_expr(*arg, candidates, id_gen, depth)),
        },

        RustIrExpr::DebugFn {
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
            body: Box::new(inline_expr(*body, candidates, id_gen, depth)),
        },

        RustIrExpr::List { id, items } => RustIrExpr::List {
            id,
            items: items
                .into_iter()
                .map(|i| RustIrListItem {
                    expr: inline_expr(i.expr, candidates, id_gen, depth),
                    spread: i.spread,
                })
                .collect(),
        },

        RustIrExpr::Tuple { id, items } => RustIrExpr::Tuple {
            id,
            items: items
                .into_iter()
                .map(|i| inline_expr(i, candidates, id_gen, depth))
                .collect(),
        },

        RustIrExpr::Record { id, fields } => RustIrExpr::Record {
            id,
            fields: inline_record_fields(fields, candidates, id_gen, depth),
        },

        RustIrExpr::Patch { id, target, fields } => RustIrExpr::Patch {
            id,
            target: Box::new(inline_expr(*target, candidates, id_gen, depth)),
            fields: inline_record_fields(fields, candidates, id_gen, depth),
        },

        RustIrExpr::FieldAccess { id, base, field } => RustIrExpr::FieldAccess {
            id,
            base: Box::new(inline_expr(*base, candidates, id_gen, depth)),
            field,
        },

        RustIrExpr::Index {
            id,
            base,
            index,
            location,
        } => RustIrExpr::Index {
            id,
            base: Box::new(inline_expr(*base, candidates, id_gen, depth)),
            index: Box::new(inline_expr(*index, candidates, id_gen, depth)),
            location,
        },

        RustIrExpr::Match {
            id,
            scrutinee,
            arms,
        } => RustIrExpr::Match {
            id,
            scrutinee: Box::new(inline_expr(*scrutinee, candidates, id_gen, depth)),
            arms: arms
                .into_iter()
                .map(|a| RustIrMatchArm {
                    pattern: a.pattern,
                    guard: a.guard.map(|g| inline_expr(g, candidates, id_gen, depth)),
                    body: inline_expr(a.body, candidates, id_gen, depth),
                })
                .collect(),
        },

        RustIrExpr::If {
            id,
            cond,
            then_branch,
            else_branch,
        } => RustIrExpr::If {
            id,
            cond: Box::new(inline_expr(*cond, candidates, id_gen, depth)),
            then_branch: Box::new(inline_expr(*then_branch, candidates, id_gen, depth)),
            else_branch: Box::new(inline_expr(*else_branch, candidates, id_gen, depth)),
        },

        RustIrExpr::Binary {
            id,
            op,
            left,
            right,
        } => RustIrExpr::Binary {
            id,
            op,
            left: Box::new(inline_expr(*left, candidates, id_gen, depth)),
            right: Box::new(inline_expr(*right, candidates, id_gen, depth)),
        },

        RustIrExpr::Block {
            id,
            block_kind,
            items,
        } => RustIrExpr::Block {
            id,
            block_kind,
            items: inline_block_items(items, candidates, id_gen, depth),
        },

        RustIrExpr::Mock {
            id,
            substitutions,
            body,
        } => RustIrExpr::Mock {
            id,
            substitutions: substitutions
                .into_iter()
                .map(|sub| RustIrMockSubstitution {
                    path: sub.path,
                    snapshot: sub.snapshot,
                    value: sub.value.map(|v| inline_expr(v, candidates, id_gen, depth)),
                })
                .collect(),
            body: Box::new(inline_expr(*body, candidates, id_gen, depth)),
        },
    }
}

fn inline_record_fields(
    fields: Vec<RustIrRecordField>,
    candidates: &HashMap<String, InlineCandidate>,
    id_gen: &mut IdGen,
    depth: u32,
) -> Vec<RustIrRecordField> {
    fields
        .into_iter()
        .map(|f| RustIrRecordField {
            spread: f.spread,
            path: f
                .path
                .into_iter()
                .map(|seg| match seg {
                    crate::rust_ir::RustIrPathSegment::IndexValue(e) => {
                        crate::rust_ir::RustIrPathSegment::IndexValue(inline_expr(
                            e, candidates, id_gen, depth,
                        ))
                    }
                    crate::rust_ir::RustIrPathSegment::IndexPredicate(e) => {
                        crate::rust_ir::RustIrPathSegment::IndexPredicate(inline_expr(
                            e, candidates, id_gen, depth,
                        ))
                    }
                    other => other,
                })
                .collect(),
            value: inline_expr(f.value, candidates, id_gen, depth),
        })
        .collect()
}

fn inline_block_items(
    items: Vec<RustIrBlockItem>,
    candidates: &HashMap<String, InlineCandidate>,
    id_gen: &mut IdGen,
    depth: u32,
) -> Vec<RustIrBlockItem> {
    items
        .into_iter()
        .map(|item| match item {
            RustIrBlockItem::Bind { pattern, expr } => RustIrBlockItem::Bind {
                pattern,
                expr: inline_expr(expr, candidates, id_gen, depth),
            },
            RustIrBlockItem::Filter { expr } => RustIrBlockItem::Filter {
                expr: inline_expr(expr, candidates, id_gen, depth),
            },
            RustIrBlockItem::Yield { expr } => RustIrBlockItem::Yield {
                expr: inline_expr(expr, candidates, id_gen, depth),
            },
            RustIrBlockItem::Recurse { expr } => RustIrBlockItem::Recurse {
                expr: inline_expr(expr, candidates, id_gen, depth),
            },
            RustIrBlockItem::Expr { expr } => RustIrBlockItem::Expr {
                expr: inline_expr(expr, candidates, id_gen, depth),
            },
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Substitution: replace Local(param) with the argument expression
// ---------------------------------------------------------------------------

/// Replace all occurrences of `Local { name == param }` with a clone of `arg`.
fn substitute(expr: &mut RustIrExpr, param: &str, arg: &RustIrExpr) {
    match expr {
        RustIrExpr::Local { name, .. } if name == param => {
            *expr = arg.clone();
        }

        // Leaves
        RustIrExpr::Local { .. }
        | RustIrExpr::Global { .. }
        | RustIrExpr::Builtin { .. }
        | RustIrExpr::ConstructorValue { .. }
        | RustIrExpr::LitNumber { .. }
        | RustIrExpr::LitString { .. }
        | RustIrExpr::LitBool { .. }
        | RustIrExpr::LitDateTime { .. }
        | RustIrExpr::LitSigil { .. }
        | RustIrExpr::Raw { .. } => {}

        RustIrExpr::TextInterpolate { parts, .. } => {
            for p in parts {
                if let RustIrTextPart::Expr { expr } = p {
                    substitute(expr, param, arg);
                }
            }
        }

        RustIrExpr::Lambda {
            param: lam_param,
            body,
            ..
        } => {
            // Don't substitute into the body if the lambda shadows the param
            if lam_param != param {
                substitute(body, param, arg);
            }
        }

        RustIrExpr::App { func, arg: a, .. } | RustIrExpr::Pipe { func, arg: a, .. } => {
            substitute(func, param, arg);
            substitute(a, param, arg);
        }

        RustIrExpr::Call { func, args, .. } => {
            substitute(func, param, arg);
            for a in args {
                substitute(a, param, arg);
            }
        }

        RustIrExpr::DebugFn { body, .. } => {
            substitute(body, param, arg);
        }

        RustIrExpr::List { items, .. } => {
            for item in items {
                substitute(&mut item.expr, param, arg);
            }
        }

        RustIrExpr::Tuple { items, .. } => {
            for item in items {
                substitute(item, param, arg);
            }
        }

        RustIrExpr::Record { fields, .. } => {
            substitute_in_record_fields(fields, param, arg);
        }

        RustIrExpr::Patch { target, fields, .. } => {
            substitute(target, param, arg);
            substitute_in_record_fields(fields, param, arg);
        }

        RustIrExpr::FieldAccess { base, .. } => {
            substitute(base, param, arg);
        }

        RustIrExpr::Index { base, index, .. } => {
            substitute(base, param, arg);
            substitute(index, param, arg);
        }

        RustIrExpr::Match {
            scrutinee, arms, ..
        } => {
            substitute(scrutinee, param, arg);
            for arm in arms {
                // Don't substitute if the pattern binds the same name
                if !pattern_binds(&arm.pattern, param) {
                    if let Some(g) = &mut arm.guard {
                        substitute(g, param, arg);
                    }
                    substitute(&mut arm.body, param, arg);
                }
            }
        }

        RustIrExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            substitute(cond, param, arg);
            substitute(then_branch, param, arg);
            substitute(else_branch, param, arg);
        }

        RustIrExpr::Binary { left, right, .. } => {
            substitute(left, param, arg);
            substitute(right, param, arg);
        }

        RustIrExpr::Block { items, .. } => {
            substitute_in_block_items(items, param, arg);
        }

        RustIrExpr::Mock {
            substitutions,
            body,
            ..
        } => {
            for sub in substitutions {
                if let Some(v) = &mut sub.value {
                    substitute(v, param, arg);
                }
            }
            substitute(body, param, arg);
        }
    }
}

fn substitute_in_record_fields(fields: &mut [RustIrRecordField], param: &str, arg: &RustIrExpr) {
    for f in fields {
        substitute(&mut f.value, param, arg);
        for seg in &mut f.path {
            match seg {
                crate::rust_ir::RustIrPathSegment::IndexValue(e)
                | crate::rust_ir::RustIrPathSegment::IndexPredicate(e) => {
                    substitute(e, param, arg);
                }
                _ => {}
            }
        }
    }
}

fn substitute_in_block_items(items: &mut [RustIrBlockItem], param: &str, arg: &RustIrExpr) {
    let mut shadowed = false;
    for item in items {
        if shadowed {
            break;
        }
        match item {
            RustIrBlockItem::Bind { pattern, expr } => {
                substitute(expr, param, arg);
                // If this bind shadows our param, stop substituting
                if pattern_binds(pattern, param) {
                    shadowed = true;
                }
            }
            RustIrBlockItem::Filter { expr }
            | RustIrBlockItem::Yield { expr }
            | RustIrBlockItem::Recurse { expr }
            | RustIrBlockItem::Expr { expr } => {
                substitute(expr, param, arg);
            }
        }
    }
}

/// Check if a pattern binds a given name.
fn pattern_binds(pat: &RustIrPattern, name: &str) -> bool {
    match pat {
        RustIrPattern::Wildcard { .. } | RustIrPattern::Literal { .. } => false,
        RustIrPattern::Var { name: n, .. } => n == name,
        RustIrPattern::At {
            name: n, pattern, ..
        } => n == name || pattern_binds(pattern, name),
        RustIrPattern::Constructor { args, .. } => args.iter().any(|a| pattern_binds(a, name)),
        RustIrPattern::Tuple { items, .. } => items.iter().any(|i| pattern_binds(i, name)),
        RustIrPattern::List { items, rest, .. } => {
            items.iter().any(|i| pattern_binds(i, name))
                || rest.as_ref().is_some_and(|r| pattern_binds(r, name))
        }
        RustIrPattern::Record { fields, .. } => {
            fields.iter().any(|f| pattern_binds(&f.pattern, name))
        }
    }
}

// ---------------------------------------------------------------------------
// Id freshening
// ---------------------------------------------------------------------------

/// Assign fresh unique ids to every node in the expression tree.
fn freshen_ids(expr: &mut RustIrExpr, id_gen: &mut IdGen) {
    // Set this node's id
    set_expr_id(expr, id_gen.next());

    // Recurse into children
    match expr {
        RustIrExpr::Local { .. }
        | RustIrExpr::Global { .. }
        | RustIrExpr::Builtin { .. }
        | RustIrExpr::ConstructorValue { .. }
        | RustIrExpr::LitNumber { .. }
        | RustIrExpr::LitString { .. }
        | RustIrExpr::LitBool { .. }
        | RustIrExpr::LitDateTime { .. }
        | RustIrExpr::LitSigil { .. }
        | RustIrExpr::Raw { .. } => {}

        RustIrExpr::TextInterpolate { parts, .. } => {
            for p in parts {
                if let RustIrTextPart::Expr { expr } = p {
                    freshen_ids(expr, id_gen);
                }
            }
        }

        RustIrExpr::Lambda { body, .. } | RustIrExpr::DebugFn { body, .. } => {
            freshen_ids(body, id_gen);
        }

        RustIrExpr::App { func, arg, .. } | RustIrExpr::Pipe { func, arg, .. } => {
            freshen_ids(func, id_gen);
            freshen_ids(arg, id_gen);
        }

        RustIrExpr::Call { func, args, .. } => {
            freshen_ids(func, id_gen);
            for a in args {
                freshen_ids(a, id_gen);
            }
        }

        RustIrExpr::List { items, .. } => {
            for item in items {
                freshen_ids(&mut item.expr, id_gen);
            }
        }

        RustIrExpr::Tuple { items, .. } => {
            for item in items {
                freshen_ids(item, id_gen);
            }
        }

        RustIrExpr::Record { fields, .. } => {
            freshen_record_fields(fields, id_gen);
        }

        RustIrExpr::Patch { target, fields, .. } => {
            freshen_ids(target, id_gen);
            freshen_record_fields(fields, id_gen);
        }

        RustIrExpr::FieldAccess { base, .. } => {
            freshen_ids(base, id_gen);
        }

        RustIrExpr::Index { base, index, .. } => {
            freshen_ids(base, id_gen);
            freshen_ids(index, id_gen);
        }

        RustIrExpr::Match {
            scrutinee, arms, ..
        } => {
            freshen_ids(scrutinee, id_gen);
            for arm in arms {
                freshen_pattern_ids(&mut arm.pattern, id_gen);
                if let Some(g) = &mut arm.guard {
                    freshen_ids(g, id_gen);
                }
                freshen_ids(&mut arm.body, id_gen);
            }
        }

        RustIrExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            freshen_ids(cond, id_gen);
            freshen_ids(then_branch, id_gen);
            freshen_ids(else_branch, id_gen);
        }

        RustIrExpr::Binary { left, right, .. } => {
            freshen_ids(left, id_gen);
            freshen_ids(right, id_gen);
        }

        RustIrExpr::Block { items, .. } => {
            for item in items {
                match item {
                    RustIrBlockItem::Bind { pattern, expr } => {
                        freshen_pattern_ids(pattern, id_gen);
                        freshen_ids(expr, id_gen);
                    }
                    RustIrBlockItem::Filter { expr }
                    | RustIrBlockItem::Yield { expr }
                    | RustIrBlockItem::Recurse { expr }
                    | RustIrBlockItem::Expr { expr } => {
                        freshen_ids(expr, id_gen);
                    }
                }
            }
        }

        RustIrExpr::Mock {
            substitutions,
            body,
            ..
        } => {
            for sub in substitutions {
                if let Some(v) = &mut sub.value {
                    freshen_ids(v, id_gen);
                }
            }
            freshen_ids(body, id_gen);
        }
    }
}

fn freshen_record_fields(fields: &mut [RustIrRecordField], id_gen: &mut IdGen) {
    for f in fields {
        freshen_ids(&mut f.value, id_gen);
        for seg in &mut f.path {
            match seg {
                crate::rust_ir::RustIrPathSegment::IndexValue(e)
                | crate::rust_ir::RustIrPathSegment::IndexPredicate(e) => {
                    freshen_ids(e, id_gen);
                }
                _ => {}
            }
        }
    }
}

fn freshen_pattern_ids(pat: &mut RustIrPattern, id_gen: &mut IdGen) {
    match pat {
        RustIrPattern::Wildcard { id }
        | RustIrPattern::Var { id, .. }
        | RustIrPattern::Literal { id, .. } => {
            *id = id_gen.next();
        }
        RustIrPattern::At { id, pattern, .. } => {
            *id = id_gen.next();
            freshen_pattern_ids(pattern, id_gen);
        }
        RustIrPattern::Constructor { id, args, .. } => {
            *id = id_gen.next();
            for a in args {
                freshen_pattern_ids(a, id_gen);
            }
        }
        RustIrPattern::Tuple { id, items, .. } => {
            *id = id_gen.next();
            for i in items {
                freshen_pattern_ids(i, id_gen);
            }
        }
        RustIrPattern::List {
            id, items, rest, ..
        } => {
            *id = id_gen.next();
            for i in items {
                freshen_pattern_ids(i, id_gen);
            }
            if let Some(r) = rest {
                freshen_pattern_ids(r, id_gen);
            }
        }
        RustIrPattern::Record { id, fields, .. } => {
            *id = id_gen.next();
            for f in fields {
                freshen_pattern_ids(&mut f.pattern, id_gen);
            }
        }
    }
}

fn set_expr_id(expr: &mut RustIrExpr, new_id: u32) {
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
        | RustIrExpr::Raw { id, .. }
        | RustIrExpr::Mock { id, .. } => *id = new_id,
    }
}

// ---------------------------------------------------------------------------
// Phase 4: Public entry point — inline the whole program
// ---------------------------------------------------------------------------

/// Run the inlining pass on all modules in place.
pub(crate) fn inline_program(modules: &mut [RustIrModule]) {
    let candidates = collect_candidates(modules);
    if candidates.is_empty() {
        return;
    }

    let mut id_gen = IdGen::new(max_expr_id(modules) + 1);

    for module in modules.iter_mut() {
        for def in &mut module.defs {
            def.expr = inline_expr(
                std::mem::replace(
                    &mut def.expr,
                    RustIrExpr::Raw {
                        id: 0,
                        text: String::new(),
                    },
                ),
                &candidates,
                &mut id_gen,
                0,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rust_ir::RustIrDef;

    fn local(id: u32, name: &str) -> RustIrExpr {
        RustIrExpr::Local {
            id,
            name: name.to_string(),
        }
    }

    fn global(id: u32, name: &str) -> RustIrExpr {
        RustIrExpr::Global {
            id,
            name: name.to_string(),
        }
    }

    fn lit_int(id: u32) -> RustIrExpr {
        RustIrExpr::LitNumber {
            id,
            text: "42".to_string(),
        }
    }

    fn app(id: u32, func: RustIrExpr, arg: RustIrExpr) -> RustIrExpr {
        RustIrExpr::App {
            id,
            func: Box::new(func),
            arg: Box::new(arg),
        }
    }

    fn lambda(id: u32, param: &str, body: RustIrExpr) -> RustIrExpr {
        RustIrExpr::Lambda {
            id,
            param: param.to_string(),
            body: Box::new(body),
        }
    }

    fn binary(id: u32, op: &str, left: RustIrExpr, right: RustIrExpr) -> RustIrExpr {
        RustIrExpr::Binary {
            id,
            op: op.to_string(),
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    fn if_expr(id: u32, c: RustIrExpr, t: RustIrExpr, e: RustIrExpr) -> RustIrExpr {
        RustIrExpr::If {
            id,
            cond: Box::new(c),
            then_branch: Box::new(t),
            else_branch: Box::new(e),
        }
    }

    // -- expr_cost tests --

    #[test]
    fn cost_leaf_nodes() {
        assert_eq!(expr_cost(&lit_int(1)), 1);
        assert_eq!(expr_cost(&local(1, "x")), 1);
        assert_eq!(expr_cost(&global(1, "f")), 1);
    }

    #[test]
    fn cost_binary() {
        // 1(binary) + 1(left) + 1(right) = 3
        let e = binary(1, "+", lit_int(2), lit_int(3));
        assert_eq!(expr_cost(&e), 3);
    }

    #[test]
    fn cost_lambda() {
        // 1(lambda) + 1(body)
        let e = lambda(1, "x", local(2, "x"));
        assert_eq!(expr_cost(&e), 2);
    }

    #[test]
    fn cost_app() {
        // 1(app) + 1(func) + 1(arg) = 3
        let e = app(1, global(2, "f"), lit_int(3));
        assert_eq!(expr_cost(&e), 3);
    }

    #[test]
    fn cost_if_expression() {
        // 1(if) + 1(cond) + 1(then) + 1(else) = 4
        let e = if_expr(1, local(2, "b"), lit_int(3), lit_int(4));
        assert_eq!(expr_cost(&e), 4);
    }

    #[test]
    fn cost_nested() {
        // f(x + y) => app(1) + global(1) + binary(1+1+1) = 5
        let e = app(
            1,
            global(2, "f"),
            binary(3, "+", local(4, "x"), local(5, "y")),
        );
        assert_eq!(expr_cost(&e), 5);
    }

    // -- candidate collection tests --

    fn make_module(name: &str, defs: Vec<RustIrDef>) -> RustIrModule {
        RustIrModule {
            name: name.to_string(),
            defs,
        }
    }

    fn make_def(name: &str, expr: RustIrExpr) -> RustIrDef {
        RustIrDef {
            name: name.to_string(),
            expr,
            cg_type: None,
        }
    }

    #[test]
    fn candidate_small_function() {
        // f x = x  (identity, cost = 1, well under threshold)
        let def = make_def("f", lambda(1, "x", local(2, "x")));
        let modules = vec![make_module("Main", vec![def])];
        let candidates = collect_candidates(&modules);
        assert!(candidates.contains_key("Main.f"));
        assert!(candidates.contains_key("f"));
    }

    #[test]
    fn candidate_recursive_excluded() {
        // f x = f x  (self-recursive)
        let def = make_def("f", lambda(1, "x", app(2, global(3, "f"), local(4, "x"))));
        let modules = vec![make_module("Main", vec![def])];
        let candidates = collect_candidates(&modules);
        assert!(!candidates.contains_key("Main.f"));
    }

    #[test]
    fn candidate_not_a_function() {
        // x = 42  (not a function — no lambda)
        let def = make_def("x", lit_int(1));
        let modules = vec![make_module("Main", vec![def])];
        let candidates = collect_candidates(&modules);
        assert!(candidates.is_empty());
    }

    // -- substitution tests --

    #[test]
    fn substitute_simple() {
        // body: x + x, substitute x -> 42
        let mut body = binary(1, "+", local(2, "x"), local(3, "x"));
        substitute(&mut body, "x", &lit_int(99));
        // Both locals should be replaced
        match &body {
            RustIrExpr::Binary { left, right, .. } => {
                assert!(
                    matches!(left.as_ref(), RustIrExpr::LitNumber { text, .. } if text == "42")
                );
                assert!(
                    matches!(right.as_ref(), RustIrExpr::LitNumber { text, .. } if text == "42")
                );
            }
            _ => panic!("expected Binary"),
        }
    }

    #[test]
    fn substitute_respects_shadow() {
        // body: \x -> x  (lambda shadows param)
        let mut body = lambda(1, "x", local(2, "x"));
        substitute(&mut body, "x", &lit_int(99));
        // Inner x should NOT be replaced
        match &body {
            RustIrExpr::Lambda { body, .. } => {
                assert!(matches!(body.as_ref(), RustIrExpr::Local { name, .. } if name == "x"));
            }
            _ => panic!("expected Lambda"),
        }
    }

    // -- inlining integration test --

    #[test]
    fn inline_simple_identity() {
        // f x = x
        // main = f 42
        let f_def = make_def("f", lambda(1, "x", local(2, "x")));
        let main_def = make_def("main", app(10, global(11, "f"), lit_int(12)));
        let mut modules = vec![make_module("Main", vec![f_def, main_def])];
        inline_program(&mut modules);

        // main body should now be just 42 (the arg), not app(global("f"), 42)
        let main_body = &modules[0].defs[1].expr;
        assert!(
            matches!(main_body, RustIrExpr::LitNumber { text, .. } if text == "42"),
            "expected inlined literal, got {:?}",
            main_body
        );
    }

    #[test]
    fn inline_binary_function() {
        // add x y = x + y  (cost: lambda(1+lambda(1+binary(3))) = 6, under threshold)
        // main = add 1 2 (as Call)
        let add_def = make_def(
            "add",
            lambda(
                1,
                "x",
                lambda(2, "y", binary(3, "+", local(4, "x"), local(5, "y"))),
            ),
        );
        // Call with 2 args
        let main_def = make_def(
            "main",
            RustIrExpr::Call {
                id: 10,
                func: Box::new(global(11, "add")),
                args: vec![lit_int(12), lit_int(13)],
            },
        );
        let mut modules = vec![make_module("Main", vec![add_def, main_def])];
        inline_program(&mut modules);

        // main body should be binary(+, 42, 42)
        let main_body = &modules[0].defs[1].expr;
        assert!(
            matches!(main_body, RustIrExpr::Binary { op, .. } if op == "+"),
            "expected inlined binary, got {:?}",
            main_body
        );
    }

    #[test]
    fn inline_depth_limit() {
        // a x = b x; b x = a x  (mutual recursion via non-self references)
        // These are NOT detected as self-recursive but the depth limit prevents infinite expansion
        let a_def = make_def("a", lambda(1, "x", app(2, global(3, "b"), local(4, "x"))));
        let b_def = make_def("b", lambda(5, "x", app(6, global(7, "a"), local(8, "x"))));
        let main_def = make_def("main", app(10, global(11, "a"), lit_int(12)));
        let mut modules = vec![make_module("Main", vec![a_def, b_def, main_def])];
        // This should terminate thanks to MAX_INLINE_DEPTH
        inline_program(&mut modules);
        // Just verify it didn't hang
    }

    #[test]
    fn freshen_produces_unique_ids() {
        let mut body = binary(1, "+", local(2, "x"), local(3, "y"));
        let mut id_gen = IdGen::new(100);
        freshen_ids(&mut body, &mut id_gen);
        // All ids should be >= 100 and unique
        match &body {
            RustIrExpr::Binary {
                id, left, right, ..
            } => {
                let left_id = expr_id(left);
                let right_id = expr_id(right);
                assert!(*id >= 100);
                assert!(left_id >= 100);
                assert!(right_id >= 100);
                assert_ne!(*id, left_id);
                assert_ne!(*id, right_id);
                assert_ne!(left_id, right_id);
            }
            _ => panic!("expected Binary"),
        }
    }
}
