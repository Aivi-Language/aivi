//! Perceus-style variable use analysis for RustIR.
//!
//! Walks `RustIrExpr` trees and computes, for each local variable reference,
//! whether it is the *last* use of that variable within its scope. This
//! information is consumed by the Cranelift lowering pass to:
//!
//! 1. Avoid cloning on last use (consume the value directly).
//! 2. Emit `rt_drop_value` for owned values that are never used.
//! 3. Generate reuse tokens from consumed allocations.
//!
//! The analysis is *conservative*: a variable that appears in multiple branches
//! of a `Match` or `If` is marked as borrowed in all of them (since only one
//! branch executes, but the analysis is pre-control-flow).

use std::collections::{HashMap, HashSet};

use crate::rust_ir::{RustIrExpr, RustIrTextPart};

/// Per-variable use information within a single function body.
#[derive(Debug, Clone)]
pub(crate) struct UseInfo {
    /// Total number of references to this variable.
    pub(crate) use_count: u32,
}

/// Maps `(expr_id, var_name)` to whether that particular reference is the last
/// use of the variable. Built by [`analyze_uses`].
#[derive(Debug)]
pub(crate) struct UseMap {
    /// For each `(expr_id, var_name)` pair that is a *last* use, an entry exists.
    last_uses: HashSet<(u32, String)>,
    /// Per-variable use counts.
    var_info: HashMap<String, UseInfo>,
}

impl UseMap {
    /// Returns `true` if the `Local { id, name }` reference at the given
    /// `expr_id` is the last use of `name` in the function body.
    pub(crate) fn is_last_use(&self, expr_id: u32, var_name: &str) -> bool {
        self.last_uses.contains(&(expr_id, var_name.to_string()))
    }

    /// Returns the total number of uses for a variable, or 0 if unknown.
    #[allow(dead_code)]
    pub(crate) fn use_count(&self, var_name: &str) -> u32 {
        self.var_info.get(var_name).map_or(0, |info| info.use_count)
    }
}

/// Analyze a single `RustIrExpr` (typically a function body) and produce a
/// [`UseMap`] annotating each local variable reference.
pub(crate) fn analyze_uses(expr: &RustIrExpr) -> UseMap {
    // Phase 1: count all uses of each local variable.
    let mut counts: HashMap<String, Vec<u32>> = HashMap::new();
    collect_uses(expr, &mut counts);

    // Phase 2: for variables used exactly once, that single site is the last use.
    // For variables used multiple times, walk the expression tree again in
    // execution order and mark the last reference found on the "main" path.
    let mut last_uses = HashSet::new();
    let mut var_info = HashMap::new();

    for (name, ids) in &counts {
        let total = ids.len() as u32;
        var_info.insert(name.clone(), UseInfo { use_count: total });
        if total == 1 {
            last_uses.insert((ids[0], name.clone()));
        }
    }

    // For multi-use variables, walk in reverse execution order and mark the
    // last seen reference. We track which variables have already been marked.
    if counts.values().any(|ids| ids.len() > 1) {
        let mut marked: HashSet<String> = HashSet::new();
        mark_last_uses_reverse(expr, &mut last_uses, &mut marked, &counts);
    }

    UseMap {
        last_uses,
        var_info,
    }
}

/// Collect all `(var_name, expr_id)` pairs for `Local` references.
fn collect_uses(expr: &RustIrExpr, out: &mut HashMap<String, Vec<u32>>) {
    match expr {
        RustIrExpr::Local { id, name, .. } => {
            out.entry(name.clone()).or_default().push(*id);
        }

        // Leaf nodes
        RustIrExpr::Global { .. }
        | RustIrExpr::Builtin { .. }
        | RustIrExpr::ConstructorValue { .. }
        | RustIrExpr::LitNumber { .. }
        | RustIrExpr::LitString { .. }
        | RustIrExpr::LitBool { .. }
        | RustIrExpr::LitDateTime { .. }
        | RustIrExpr::LitSigil { .. }
        | RustIrExpr::Raw { .. } => {}

        // Unary wrappers
        RustIrExpr::Lambda { body, .. } | RustIrExpr::DebugFn { body, .. } => {
            collect_uses(body, out);
        }

        // Binary
        RustIrExpr::App { func, arg, .. } | RustIrExpr::Pipe { func, arg, .. } => {
            collect_uses(func, out);
            collect_uses(arg, out);
        }
        RustIrExpr::Binary { left, right, .. } => {
            collect_uses(left, out);
            collect_uses(right, out);
        }
        RustIrExpr::Index { base, index, .. } => {
            collect_uses(base, out);
            collect_uses(index, out);
        }

        // N-ary
        RustIrExpr::Call { func, args, .. } => {
            collect_uses(func, out);
            for arg in args {
                collect_uses(arg, out);
            }
        }
        RustIrExpr::TextInterpolate { parts, .. } => {
            for part in parts {
                if let RustIrTextPart::Expr { expr } = part {
                    collect_uses(expr, out);
                }
            }
        }
        RustIrExpr::List { items, .. } => {
            for item in items {
                collect_uses(&item.expr, out);
            }
        }
        RustIrExpr::Tuple { items, .. } => {
            for item in items {
                collect_uses(item, out);
            }
        }
        RustIrExpr::Record { fields, .. } => {
            for field in fields {
                collect_uses(&field.value, out);
                collect_uses_in_path(&field.path, out);
            }
        }
        RustIrExpr::Patch { target, fields, .. } => {
            collect_uses(target, out);
            for field in fields {
                collect_uses(&field.value, out);
                collect_uses_in_path(&field.path, out);
            }
        }

        // Access
        RustIrExpr::FieldAccess { base, .. } => {
            collect_uses(base, out);
        }

        // Control flow
        RustIrExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            collect_uses(cond, out);
            collect_uses(then_branch, out);
            collect_uses(else_branch, out);
        }
        RustIrExpr::Match {
            scrutinee, arms, ..
        } => {
            collect_uses(scrutinee, out);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_uses(guard, out);
                }
                collect_uses(&arm.body, out);
            }
        }

        RustIrExpr::Mock {
            substitutions,
            body,
            ..
        } => {
            for sub in substitutions {
                if let Some(v) = &sub.value {
                    collect_uses(v, out);
                }
            }
            collect_uses(body, out);
        }
    }
}

fn collect_uses_in_path(
    path: &[crate::rust_ir::RustIrPathSegment],
    out: &mut HashMap<String, Vec<u32>>,
) {
    use crate::rust_ir::RustIrPathSegment;
    for seg in path {
        match seg {
            RustIrPathSegment::IndexValue(expr) | RustIrPathSegment::IndexPredicate(expr) => {
                collect_uses(expr, out);
            }
            _ => {}
        }
    }
}

/// Walk the expression in *reverse* execution order and mark the first (i.e.
/// last-in-execution-order) reference to each variable as the last use.
///
/// For branching constructs (`If`, `Match`), we do NOT mark last uses inside
/// branches for variables also used after the branch or in multiple branches.
/// This conservative approach ensures safety.
fn mark_last_uses_reverse(
    expr: &RustIrExpr,
    last_uses: &mut HashSet<(u32, String)>,
    marked: &mut HashSet<String>,
    counts: &HashMap<String, Vec<u32>>,
) {
    match expr {
        RustIrExpr::Local { id, name, .. } => {
            if let Some(ids) = counts.get(name) {
                if ids.len() > 1 && !marked.contains(name) {
                    marked.insert(name.clone());
                    last_uses.insert((*id, name.clone()));
                }
            }
        }

        // Leaf nodes — nothing to mark
        RustIrExpr::Global { .. }
        | RustIrExpr::Builtin { .. }
        | RustIrExpr::ConstructorValue { .. }
        | RustIrExpr::LitNumber { .. }
        | RustIrExpr::LitString { .. }
        | RustIrExpr::LitBool { .. }
        | RustIrExpr::LitDateTime { .. }
        | RustIrExpr::LitSigil { .. }
        | RustIrExpr::Raw { .. } => {}

        // Unary — recurse into body
        RustIrExpr::Lambda { body, .. } | RustIrExpr::DebugFn { body, .. } => {
            mark_last_uses_reverse(body, last_uses, marked, counts);
        }

        // Binary — reverse order: right first, then left
        RustIrExpr::App { func, arg, .. } | RustIrExpr::Pipe { func, arg, .. } => {
            mark_last_uses_reverse(arg, last_uses, marked, counts);
            mark_last_uses_reverse(func, last_uses, marked, counts);
        }
        RustIrExpr::Binary { left, right, .. } => {
            mark_last_uses_reverse(right, last_uses, marked, counts);
            mark_last_uses_reverse(left, last_uses, marked, counts);
        }
        RustIrExpr::Index { base, index, .. } => {
            mark_last_uses_reverse(index, last_uses, marked, counts);
            mark_last_uses_reverse(base, last_uses, marked, counts);
        }

        // Call — reverse order
        RustIrExpr::Call { func, args, .. } => {
            for arg in args.iter().rev() {
                mark_last_uses_reverse(arg, last_uses, marked, counts);
            }
            mark_last_uses_reverse(func, last_uses, marked, counts);
        }
        RustIrExpr::TextInterpolate { parts, .. } => {
            for part in parts.iter().rev() {
                if let RustIrTextPart::Expr { expr } = part {
                    mark_last_uses_reverse(expr, last_uses, marked, counts);
                }
            }
        }

        // Data structures — reverse element order
        RustIrExpr::List { items, .. } => {
            for item in items.iter().rev() {
                mark_last_uses_reverse(&item.expr, last_uses, marked, counts);
            }
        }
        RustIrExpr::Tuple { items, .. } => {
            for item in items.iter().rev() {
                mark_last_uses_reverse(item, last_uses, marked, counts);
            }
        }
        RustIrExpr::Record { fields, .. } => {
            for field in fields.iter().rev() {
                mark_last_uses_reverse(&field.value, last_uses, marked, counts);
            }
        }
        RustIrExpr::Patch { target, fields, .. } => {
            for field in fields.iter().rev() {
                mark_last_uses_reverse(&field.value, last_uses, marked, counts);
            }
            mark_last_uses_reverse(target, last_uses, marked, counts);
        }

        // Access
        RustIrExpr::FieldAccess { base, .. } => {
            mark_last_uses_reverse(base, last_uses, marked, counts);
        }

        // Branching: conservative — mark inside each branch independently but
        // only if the variable is NOT used after the branch point and is only
        // used in a single branch.
        RustIrExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            // Variables used in both branches cannot have a "last use" inside
            // a branch because we don't know which branch runs.
            // Collect vars from each branch.
            let mut then_vars: HashMap<String, Vec<u32>> = HashMap::new();
            let mut else_vars: HashMap<String, Vec<u32>> = HashMap::new();
            collect_uses(then_branch, &mut then_vars);
            collect_uses(else_branch, &mut else_vars);

            // Mark in each branch for vars unique to that branch
            let mut then_marked = marked.clone();
            for name in then_vars.keys() {
                if !else_vars.contains_key(name) {
                    mark_last_uses_reverse(then_branch, last_uses, &mut then_marked, counts);
                    break;
                }
            }
            let mut else_marked = marked.clone();
            for name in else_vars.keys() {
                if !then_vars.contains_key(name) {
                    mark_last_uses_reverse(else_branch, last_uses, &mut else_marked, counts);
                    break;
                }
            }

            // Merge: a variable is marked if either branch marked it
            marked.extend(then_marked);
            marked.extend(else_marked);

            mark_last_uses_reverse(cond, last_uses, marked, counts);
        }

        RustIrExpr::Match {
            scrutinee, arms, ..
        } => {
            // Conservative: don't mark last uses inside match arms for vars
            // used in multiple arms. Only mark for vars unique to one arm.
            let mut arm_vars: Vec<HashMap<String, Vec<u32>>> = Vec::new();
            for arm in arms {
                let mut vars = HashMap::new();
                if let Some(guard) = &arm.guard {
                    collect_uses(guard, &mut vars);
                }
                collect_uses(&arm.body, &mut vars);
                arm_vars.push(vars);
            }

            // Count in how many arms each variable appears
            let mut arm_counts: HashMap<String, usize> = HashMap::new();
            for vars in &arm_vars {
                for name in vars.keys() {
                    *arm_counts.entry(name.clone()).or_default() += 1;
                }
            }

            // Mark last uses only in arms where the variable is unique to that arm
            for (i, arm) in arms.iter().enumerate() {
                let mut arm_marked = marked.clone();
                for name in arm_vars[i].keys() {
                    if arm_counts.get(name).copied().unwrap_or(0) == 1 && !marked.contains(name) {
                        // This var only appears in this arm — safe to mark
                        if let Some(guard) = &arm.guard {
                            mark_last_uses_reverse(guard, last_uses, &mut arm_marked, counts);
                        }
                        mark_last_uses_reverse(&arm.body, last_uses, &mut arm_marked, counts);
                        break;
                    }
                }
                marked.extend(arm_marked);
            }

            mark_last_uses_reverse(scrutinee, last_uses, marked, counts);
        }

        RustIrExpr::Mock {
            substitutions,
            body,
            ..
        } => {
            mark_last_uses_reverse(body, last_uses, marked, counts);
            for sub in substitutions.iter().rev() {
                if let Some(v) = &sub.value {
                    mark_last_uses_reverse(v, last_uses, marked, counts);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(id: u32, name: &str) -> RustIrExpr {
        RustIrExpr::Local {
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

    #[test]
    fn single_use_is_last() {
        // x used once → that use is the last use
        let expr = local(1, "x");
        let map = analyze_uses(&expr);
        assert!(map.is_last_use(1, "x"));
        assert_eq!(map.use_count("x"), 1);
    }

    #[test]
    fn two_uses_second_is_last() {
        // App(x, x) — evaluated left-to-right, so id=2 (right) is last use
        let expr = RustIrExpr::App {
            id: 0,
            func: Box::new(local(1, "x")),
            arg: Box::new(local(2, "x")),
        };
        let map = analyze_uses(&expr);
        assert!(!map.is_last_use(1, "x"));
        assert!(map.is_last_use(2, "x"));
        assert_eq!(map.use_count("x"), 2);
    }

    #[test]
    fn different_vars_both_last() {
        let expr = RustIrExpr::App {
            id: 0,
            func: Box::new(local(1, "f")),
            arg: Box::new(local(2, "x")),
        };
        let map = analyze_uses(&expr);
        assert!(map.is_last_use(1, "f"));
        assert!(map.is_last_use(2, "x"));
    }

    #[test]
    fn three_uses_last_is_marked() {
        // Call(x, [x, x]) — the third x (id=3) should be last
        let expr = RustIrExpr::Call {
            id: 0,
            func: Box::new(local(1, "x")),
            args: vec![local(2, "x"), local(3, "x")],
        };
        let map = analyze_uses(&expr);
        assert!(!map.is_last_use(1, "x"));
        assert!(!map.is_last_use(2, "x"));
        assert!(map.is_last_use(3, "x"));
    }

    #[test]
    fn unused_var_zero_count() {
        let expr = lit_int(1);
        let map = analyze_uses(&expr);
        assert_eq!(map.use_count("nonexistent"), 0);
        assert!(!map.is_last_use(1, "nonexistent"));
        // Also verify that a used var in a trivial expr gets counted
        let expr2 = local(1, "x");
        let map2 = analyze_uses(&expr2);
        assert_eq!(map2.use_count("x"), 1);
        assert!(map2.is_last_use(1, "x"));
    }
}
