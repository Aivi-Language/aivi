use std::collections::HashSet;

use crate::rust_ir::{RustIrBlockItem, RustIrExpr, RustIrPathSegment, RustIrPattern};

pub(super) fn collect_pattern_vars(pattern: &RustIrPattern, out: &mut Vec<String>) {
    match pattern {
        RustIrPattern::Wildcard { .. } => {}
        RustIrPattern::Var { name, .. } => out.push(name.clone()),
        RustIrPattern::Literal { .. } => {}
        RustIrPattern::At { name, pattern, .. } => {
            out.push(name.clone());
            collect_pattern_vars(pattern, out);
        }
        RustIrPattern::Constructor { args, .. } => {
            for arg in args {
                collect_pattern_vars(arg, out);
            }
        }
        RustIrPattern::Tuple { items, .. } => {
            for item in items {
                collect_pattern_vars(item, out);
            }
        }
        RustIrPattern::List { items, rest, .. } => {
            for item in items {
                collect_pattern_vars(item, out);
            }
            if let Some(rest) = rest.as_deref() {
                collect_pattern_vars(rest, out);
            }
        }
        RustIrPattern::Record { fields, .. } => {
            for field in fields {
                collect_pattern_vars(&field.pattern, out);
            }
        }
    }
}

pub(super) fn collect_free_locals_in_items(items: &[RustIrBlockItem]) -> Vec<String> {
    let mut bound: Vec<String> = Vec::new();
    let mut out: HashSet<String> = HashSet::new();

    for item in items {
        match item {
            RustIrBlockItem::Bind { pattern, expr } => {
                // Pre-add compiler-generated (`__`-prefixed) binders so that
                // self-referential bindings (e.g. `__loop0` from loop/recurse
                // desugaring) are not incorrectly reported as free.  This
                // mirrors the same logic in `lower_block_item`.
                let mut binders = Vec::new();
                collect_pattern_vars(pattern, &mut binders);
                let pre_added: Vec<String> = binders
                    .iter()
                    .filter(|n| n.starts_with("__"))
                    .cloned()
                    .collect();
                for name in &pre_added {
                    bound.push(name.clone());
                }
                collect_free_locals_in_expr(expr, &mut bound, &mut out);
                for binder in binders {
                    if !pre_added.contains(&binder) {
                        bound.push(binder);
                    }
                }
            }
            RustIrBlockItem::Filter { expr }
            | RustIrBlockItem::Yield { expr }
            | RustIrBlockItem::Recurse { expr }
            | RustIrBlockItem::Expr { expr } => {
                collect_free_locals_in_expr(expr, &mut bound, &mut out);
            }
        }
    }

    let mut out = out.into_iter().collect::<Vec<_>>();
    out.sort();
    out
}

pub(super) fn collect_free_locals_in_expr(
    expr: &RustIrExpr,
    bound: &mut Vec<String>,
    out: &mut HashSet<String>,
) {
    match expr {
        RustIrExpr::Local { name, .. } => {
            if !bound.iter().rev().any(|b| b == name) {
                out.insert(name.clone());
            }
        }
        RustIrExpr::Global { .. }
        | RustIrExpr::Builtin { .. }
        | RustIrExpr::ConstructorValue { .. }
        | RustIrExpr::LitNumber { .. }
        | RustIrExpr::LitString { .. }
        | RustIrExpr::LitSigil { .. }
        | RustIrExpr::LitBool { .. }
        | RustIrExpr::LitDateTime { .. }
        | RustIrExpr::Raw { .. } => {}
        RustIrExpr::TextInterpolate { parts, .. } => {
            for part in parts {
                if let crate::rust_ir::RustIrTextPart::Expr { expr } = part {
                    collect_free_locals_in_expr(expr, bound, out);
                }
            }
        }
        RustIrExpr::Lambda { param, body, .. } => {
            bound.push(param.clone());
            collect_free_locals_in_expr(body, bound, out);
            bound.pop();
        }
        RustIrExpr::App { func, arg, .. } => {
            collect_free_locals_in_expr(func, bound, out);
            collect_free_locals_in_expr(arg, bound, out);
        }
        RustIrExpr::Call { func, args, .. } => {
            collect_free_locals_in_expr(func, bound, out);
            for arg in args {
                collect_free_locals_in_expr(arg, bound, out);
            }
        }
        RustIrExpr::DebugFn { arg_vars, body, .. } => {
            for name in arg_vars {
                if !bound.iter().rev().any(|b| b == name) {
                    out.insert(name.clone());
                }
            }
            collect_free_locals_in_expr(body, bound, out);
        }
        RustIrExpr::Pipe { func, arg, .. } => {
            collect_free_locals_in_expr(func, bound, out);
            collect_free_locals_in_expr(arg, bound, out);
        }
        RustIrExpr::List { items, .. } => {
            for item in items {
                collect_free_locals_in_expr(&item.expr, bound, out);
            }
        }
        RustIrExpr::Tuple { items, .. } => {
            for item in items {
                collect_free_locals_in_expr(item, bound, out);
            }
        }
        RustIrExpr::Record { fields, .. } | RustIrExpr::Patch { fields, .. } => {
            for field in fields {
                for seg in &field.path {
                    match seg {
                        RustIrPathSegment::IndexValue(expr)
                        | RustIrPathSegment::IndexPredicate(expr) => {
                            collect_free_locals_in_expr(expr, bound, out);
                        }
                        RustIrPathSegment::Field(_)
                        | RustIrPathSegment::IndexFieldBool(_)
                        | RustIrPathSegment::IndexAll => {}
                    }
                }
                collect_free_locals_in_expr(&field.value, bound, out);
            }
            if let RustIrExpr::Patch { target, .. } = expr {
                collect_free_locals_in_expr(target, bound, out);
            }
        }
        RustIrExpr::FieldAccess { base, .. } => collect_free_locals_in_expr(base, bound, out),
        RustIrExpr::Index { base, index, .. } => {
            collect_free_locals_in_expr(base, bound, out);
            collect_free_locals_in_expr(index, bound, out);
        }
        RustIrExpr::Match {
            scrutinee, arms, ..
        } => {
            collect_free_locals_in_expr(scrutinee, bound, out);
            for arm in arms {
                let mut binders = Vec::new();
                collect_pattern_vars(&arm.pattern, &mut binders);
                bound.extend(binders.iter().cloned());
                if let Some(guard) = &arm.guard {
                    collect_free_locals_in_expr(guard, bound, out);
                }
                collect_free_locals_in_expr(&arm.body, bound, out);
                for _ in 0..binders.len() {
                    bound.pop();
                }
            }
        }
        RustIrExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            collect_free_locals_in_expr(cond, bound, out);
            collect_free_locals_in_expr(then_branch, bound, out);
            collect_free_locals_in_expr(else_branch, bound, out);
        }
        RustIrExpr::Binary { left, right, .. } => {
            collect_free_locals_in_expr(left, bound, out);
            collect_free_locals_in_expr(right, bound, out);
        }
        RustIrExpr::Block { items, .. } => {
            let before = bound.len();
            for item in items {
                match item {
                    RustIrBlockItem::Bind { pattern, expr } => {
                        // Pre-add `__`-prefixed binders (e.g. `__loop0`) so
                        // self-referential bindings inside nested blocks are
                        // not incorrectly reported as free.
                        let mut binders = Vec::new();
                        collect_pattern_vars(pattern, &mut binders);
                        let pre_added: Vec<String> = binders
                            .iter()
                            .filter(|n| n.starts_with("__"))
                            .cloned()
                            .collect();
                        for name in &pre_added {
                            bound.push(name.clone());
                        }
                        collect_free_locals_in_expr(expr, bound, out);
                        for binder in binders {
                            if !pre_added.contains(&binder) {
                                bound.push(binder);
                            }
                        }
                    }
                    RustIrBlockItem::Filter { expr }
                    | RustIrBlockItem::Yield { expr }
                    | RustIrBlockItem::Recurse { expr }
                    | RustIrBlockItem::Expr { expr } => {
                        collect_free_locals_in_expr(expr, bound, out);
                    }
                }
            }
            bound.truncate(before);
        }
    }
}

pub(super) fn rust_local_name(name: &str) -> String {
    let mut s = sanitize_ident(name);
    if s.is_empty() {
        s = "_".to_string();
    }
    if is_rust_keyword(&s) {
        s = format!("v_{s}");
    }
    s
}

pub(super) fn rust_global_fn_name(name: &str) -> String {
    let base = rust_local_name(name);
    let hash = fnv1a64(name);
    format!("def_{base}__{hash:016x}")
}

fn fnv1a64(value: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in value.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn sanitize_ident(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        let ok = ch == '_' || ch.is_ascii_alphanumeric();
        if ok {
            if i == 0 && ch.is_ascii_digit() {
                out.push('_');
            }
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    out
}

fn is_rust_keyword(ident: &str) -> bool {
    matches!(
        ident,
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "gen"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_ident_replaces_non_ascii_alnum() {
        assert_eq!(sanitize_ident("hello"), "hello");
        assert_eq!(sanitize_ident("hello-world"), "hello_world");
        assert_eq!(sanitize_ident("123abc"), "_123abc");
        assert_eq!(sanitize_ident(""), "");
    }

    #[test]
    fn rust_local_name_avoids_keywords_and_empty() {
        assert_eq!(rust_local_name("match"), "v_match");
        assert_eq!(rust_local_name("gen"), "v_gen");
        assert_eq!(rust_local_name(""), "_");
        assert_eq!(rust_local_name("9lives"), "_9lives");
    }

    #[test]
    fn rust_global_fn_name_has_stable_shape() {
        let name = rust_global_fn_name("main");
        assert!(name.starts_with("def_main__"));
        let suffix = name.strip_prefix("def_main__").unwrap();
        assert_eq!(suffix.len(), 16);
        assert!(suffix.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
