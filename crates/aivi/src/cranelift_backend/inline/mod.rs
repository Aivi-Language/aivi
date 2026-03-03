//! RustIR-level function inlining pass.
//!
//! Runs after monomorphization and before Cranelift lowering. Replaces call
//! sites of small, non-recursive functions with the callee's body, eliminating
//! call overhead and exposing more code to Cranelift's local optimisations.
//!
//! Auto-inlines functions whose body cost is below `INLINE_THRESHOLD`.

mod substitute;
mod transform;

use std::collections::HashMap;

use crate::rust_ir::{RustIrExpr, RustIrModule, RustIrPattern, RustIrRecordField, RustIrTextPart};

use transform::{inline_expr, max_expr_id, IdGen};

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
/// Short (bare) names are only registered when they are unambiguous across all
/// modules.  This prevents cross-module name collisions where a call to a
/// local `sum` could be wrongly inlined with the stdlib's `sum`.
fn collect_candidates(modules: &[RustIrModule]) -> HashMap<String, InlineCandidate> {
    // Count how many defs share each bare name across all modules.
    // Names that appear more than once must NOT be registered as short-name
    // candidates because Global("sum") in module A may mean a different
    // function than Global("sum") in module B.
    let mut name_count: HashMap<&str, usize> = HashMap::new();
    for module in modules {
        let module_dot = format!("{}.", module.name);
        for def in &module.defs {
            if def.name.starts_with(&module_dot) {
                continue;
            }
            *name_count.entry(&def.name).or_insert(0) += 1;
        }
    }

    let mut candidates = HashMap::new();

    for module in modules {
        let module_dot = format!("{}.", module.name);
        for def in &module.defs {
            // Skip qualified aliases emitted by the Kernel
            if def.name.starts_with(&module_dot) {
                continue;
            }
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
                // Only register under short name when there is exactly one def
                // with this name across all modules – otherwise the short name
                // is ambiguous and inlining through it would pick the wrong body.
                if name_count.get(def.name.as_str()).copied().unwrap_or(0) == 1 {
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
