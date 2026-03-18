use std::collections::HashMap;

use crate::rust_ir::{
    RustIrExpr, RustIrListItem, RustIrMatchArm, RustIrMockSubstitution, RustIrModule,
    RustIrRecordField, RustIrTextPart,
};

use super::substitute::{freshen_ids, substitute, substitute_many};
use super::InlineCandidate;
use super::MAX_INLINE_DEPTH;

/// Counter for generating fresh expression ids.
pub(super) struct IdGen(u32);

impl IdGen {
    pub(super) fn new(start: u32) -> Self {
        Self(start)
    }
    pub(super) fn next(&mut self) -> u32 {
        let id = self.0;
        self.0 += 1;
        id
    }
}

/// Find the maximum expression id across all modules (for freshening).
pub(super) fn max_expr_id(modules: &[RustIrModule]) -> u32 {
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

pub(super) fn expr_id(expr: &RustIrExpr) -> u32 {
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
        | RustIrExpr::Raw { id, .. }
        | RustIrExpr::Mock { id, .. } => *id,
    }
}

pub(super) fn set_expr_id(expr: &mut RustIrExpr, new_id: u32) {
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
        | RustIrExpr::Raw { id, .. }
        | RustIrExpr::Mock { id, .. } => *id = new_id,
    }
}

/// Inline call sites in a single expression. Returns the rewritten expression.
pub(super) fn inline_expr(
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
                        // Use simultaneous substitution to avoid variable capture
                        // when an argument expression contains a variable that
                        // shares a name with another parameter.
                        let bindings: Vec<(&str, &RustIrExpr)> = candidate
                            .params
                            .iter()
                            .zip(args.iter())
                            .map(|(p, a)| (p.as_str(), a))
                            .collect();
                        substitute_many(&mut body, &bindings);
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

        RustIrExpr::Lambda {
            id,
            param,
            body,
            location,
        } => RustIrExpr::Lambda {
            id,
            param,
            body: Box::new(inline_expr(*body, candidates, id_gen, depth)),
            location,
        },

        RustIrExpr::App {
            id,
            func,
            arg,
            location,
        } => RustIrExpr::App {
            id,
            func: Box::new(inline_expr(*func, candidates, id_gen, depth)),
            arg: Box::new(inline_expr(*arg, candidates, id_gen, depth)),
            location,
        },

        RustIrExpr::Call {
            id,
            func,
            args,
            location,
        } => RustIrExpr::Call {
            id,
            func: Box::new(inline_expr(*func, candidates, id_gen, depth)),
            args: args
                .into_iter()
                .map(|a| inline_expr(a, candidates, id_gen, depth))
                .collect(),
            location,
        },

        RustIrExpr::Pipe {
            id,
            pipe_id,
            step,
            label,
            log_time,
            func,
            arg,
            location,
        } => RustIrExpr::Pipe {
            id,
            pipe_id,
            step,
            label,
            log_time,
            func: Box::new(inline_expr(*func, candidates, id_gen, depth)),
            arg: Box::new(inline_expr(*arg, candidates, id_gen, depth)),
            location,
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

        RustIrExpr::FieldAccess {
            id,
            base,
            field,
            location,
        } => RustIrExpr::FieldAccess {
            id,
            base: Box::new(inline_expr(*base, candidates, id_gen, depth)),
            field,
            location,
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
            location,
        } => RustIrExpr::Match {
            id,
            scrutinee: Box::new(inline_expr(*scrutinee, candidates, id_gen, depth)),
            arms: arms
                .into_iter()
                .map(|a| RustIrMatchArm {
                    pattern: a.pattern,
                    guard: a.guard.map(|g| inline_expr(g, candidates, id_gen, depth)),
                    guard_negated: a.guard_negated,
                    body: inline_expr(a.body, candidates, id_gen, depth),
                })
                .collect(),
            location,
        },

        RustIrExpr::If {
            id,
            cond,
            then_branch,
            else_branch,
            location,
        } => RustIrExpr::If {
            id,
            cond: Box::new(inline_expr(*cond, candidates, id_gen, depth)),
            then_branch: Box::new(inline_expr(*then_branch, candidates, id_gen, depth)),
            else_branch: Box::new(inline_expr(*else_branch, candidates, id_gen, depth)),
            location,
        },

        RustIrExpr::Binary {
            id,
            op,
            left,
            right,
            location,
        } => RustIrExpr::Binary {
            id,
            op,
            left: Box::new(inline_expr(*left, candidates, id_gen, depth)),
            right: Box::new(inline_expr(*right, candidates, id_gen, depth)),
            location,
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
