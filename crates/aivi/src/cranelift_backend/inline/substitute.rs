use crate::rust_ir::{RustIrExpr, RustIrPattern, RustIrRecordField, RustIrTextPart};

use super::transform::{set_expr_id, IdGen};

/// Replace all occurrences of multiple params simultaneously.
/// This avoids variable capture when an argument expression contains a variable
/// that shares a name with another parameter being substituted.
pub(super) fn substitute_many(expr: &mut RustIrExpr, bindings: &[(&str, &RustIrExpr)]) {
    if bindings.is_empty() {
        return;
    }
    if bindings.len() == 1 {
        substitute(expr, bindings[0].0, bindings[0].1);
        return;
    }
    match expr {
        RustIrExpr::Local { name, .. } => {
            for &(param, arg) in bindings {
                if name.as_str() == param {
                    *expr = arg.clone();
                    return;
                }
            }
        }

        // Leaves
        RustIrExpr::Global { .. }
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
                    substitute_many(expr, bindings);
                }
            }
        }

        RustIrExpr::Lambda {
            param: lam_param,
            body,
            ..
        } => {
            // Filter out any bindings shadowed by this lambda param
            let filtered: Vec<_> = bindings
                .iter()
                .filter(|&&(p, _)| p != lam_param.as_str())
                .copied()
                .collect();
            substitute_many(body, &filtered);
        }

        RustIrExpr::App { func, arg: a, .. } | RustIrExpr::Pipe { func, arg: a, .. } => {
            substitute_many(func, bindings);
            substitute_many(a, bindings);
        }

        RustIrExpr::Call { func, args, .. } => {
            substitute_many(func, bindings);
            for a in args {
                substitute_many(a, bindings);
            }
        }

        RustIrExpr::DebugFn { body, .. } => {
            substitute_many(body, bindings);
        }

        RustIrExpr::List { items, .. } => {
            for item in items {
                substitute_many(&mut item.expr, bindings);
            }
        }

        RustIrExpr::Tuple { items, .. } => {
            for item in items {
                substitute_many(item, bindings);
            }
        }

        RustIrExpr::Record { fields, .. } => {
            substitute_many_in_record_fields(fields, bindings);
        }

        RustIrExpr::Patch { target, fields, .. } => {
            substitute_many(target, bindings);
            substitute_many_in_record_fields(fields, bindings);
        }

        RustIrExpr::FieldAccess { base, .. } => {
            substitute_many(base, bindings);
        }

        RustIrExpr::Index { base, index, .. } => {
            substitute_many(base, bindings);
            substitute_many(index, bindings);
        }

        RustIrExpr::Match {
            scrutinee, arms, ..
        } => {
            substitute_many(scrutinee, bindings);
            for arm in arms {
                let filtered: Vec<_> = bindings
                    .iter()
                    .filter(|&&(p, _)| !pattern_binds(&arm.pattern, p))
                    .copied()
                    .collect();
                if let Some(g) = &mut arm.guard {
                    substitute_many(g, &filtered);
                }
                substitute_many(&mut arm.body, &filtered);
            }
        }

        RustIrExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            substitute_many(cond, bindings);
            substitute_many(then_branch, bindings);
            substitute_many(else_branch, bindings);
        }

        RustIrExpr::Binary { left, right, .. } => {
            substitute_many(left, bindings);
            substitute_many(right, bindings);
        }

        RustIrExpr::Mock {
            substitutions,
            body,
            ..
        } => {
            for sub in substitutions {
                if let Some(v) = &mut sub.value {
                    substitute_many(v, bindings);
                }
            }
            substitute_many(body, bindings);
        }
    }
}

fn substitute_many_in_record_fields(
    fields: &mut [RustIrRecordField],
    bindings: &[(&str, &RustIrExpr)],
) {
    for f in fields {
        substitute_many(&mut f.value, bindings);
        for seg in &mut f.path {
            match seg {
                crate::rust_ir::RustIrPathSegment::IndexValue(e)
                | crate::rust_ir::RustIrPathSegment::IndexPredicate(e) => {
                    substitute_many(e, bindings);
                }
                _ => {}
            }
        }
    }
}

/// Replace all occurrences of `Local { name == param }` with a clone of `arg`.
pub(super) fn substitute(expr: &mut RustIrExpr, param: &str, arg: &RustIrExpr) {
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
pub(super) fn freshen_ids(expr: &mut RustIrExpr, id_gen: &mut IdGen) {
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::super::transform::{expr_id, IdGen};
    use super::super::{collect_candidates, expr_cost, inline_program};
    use super::*;
    use crate::rust_ir::{RustIrDef, RustIrModule};

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
        // Verify the output is still a valid program with a main definition
        let main = modules[0]
            .defs
            .iter()
            .find(|d| d.name == "main")
            .expect("main def should still exist after inlining");
        assert!(
            !matches!(main.expr, RustIrExpr::Global { .. }),
            "main body should have been at least partially inlined"
        );
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
