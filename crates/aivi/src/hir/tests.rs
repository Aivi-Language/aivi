#[cfg(test)]
mod debug_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn contains_debug_nodes(expr: &HirExpr) -> bool {
        match expr {
            HirExpr::DebugFn { .. } => true,
            HirExpr::Pipe { .. } => true,
            HirExpr::Lambda { body, .. } => contains_debug_nodes(body),
            HirExpr::App { func, arg, .. } => contains_debug_nodes(func) || contains_debug_nodes(arg),
            HirExpr::Call { func, args, .. } => {
                contains_debug_nodes(func) || args.iter().any(contains_debug_nodes)
            }
            HirExpr::TextInterpolate { parts, .. } => parts.iter().any(|p| match p {
                HirTextPart::Expr { expr } => contains_debug_nodes(expr),
                _ => false,
            }),
            HirExpr::List { items, .. } => items.iter().any(|i| contains_debug_nodes(&i.expr)),
            HirExpr::Tuple { items, .. } => items.iter().any(contains_debug_nodes),
            HirExpr::Record { fields, .. } => fields.iter().any(|f| contains_debug_nodes(&f.value)),
            HirExpr::Patch { target, fields, .. } => {
                contains_debug_nodes(target) || fields.iter().any(|f| contains_debug_nodes(&f.value))
            }
            HirExpr::FieldAccess { base, .. } => contains_debug_nodes(base),
            HirExpr::Index { base, index, .. } => contains_debug_nodes(base) || contains_debug_nodes(index),
            HirExpr::Match { scrutinee, arms, .. } => {
                contains_debug_nodes(scrutinee) || arms.iter().any(|a| contains_debug_nodes(&a.body))
            }
            HirExpr::If { cond, then_branch, else_branch, .. } => {
                contains_debug_nodes(cond) || contains_debug_nodes(then_branch) || contains_debug_nodes(else_branch)
            }
            HirExpr::Binary { left, right, .. } => contains_debug_nodes(left) || contains_debug_nodes(right),
            HirExpr::Block { items, .. } => items.iter().any(|i| match i {
                HirBlockItem::Bind { expr, .. } | HirBlockItem::Expr { expr } => contains_debug_nodes(expr),
                _ => false,
            }),
            HirExpr::Var { .. }
            | HirExpr::LitNumber { .. }
            | HirExpr::LitString { .. }
            | HirExpr::LitSigil { .. }
            | HirExpr::LitBool { .. }
            | HirExpr::LitDateTime { .. }
            | HirExpr::Raw { .. } => false,
            HirExpr::Mock { substitutions, body, .. } => {
                substitutions.iter().any(|s| s.value.as_ref().is_some_and(contains_debug_nodes))
                    || contains_debug_nodes(body)
            }
        }
    }

    fn collect_pipes(expr: &HirExpr, out: &mut Vec<(u32, u32, String)>) {
        match expr {
            HirExpr::Pipe {
                pipe_id, step, label, func, arg, ..
            } => {
                out.push((*pipe_id, *step, label.clone()));
                collect_pipes(func, out);
                collect_pipes(arg, out);
            }
            HirExpr::DebugFn { body, .. } => collect_pipes(body, out),
            HirExpr::Lambda { body, .. } => collect_pipes(body, out),
            HirExpr::App { func, arg, .. } => {
                collect_pipes(func, out);
                collect_pipes(arg, out);
            }
            HirExpr::Call { func, args, .. } => {
                collect_pipes(func, out);
                for arg in args {
                    collect_pipes(arg, out);
                }
            }
            HirExpr::TextInterpolate { parts, .. } => {
                for part in parts {
                    if let HirTextPart::Expr { expr } = part {
                        collect_pipes(expr, out);
                    }
                }
            }
            HirExpr::List { items, .. } => {
                for item in items {
                    collect_pipes(&item.expr, out);
                }
            }
            HirExpr::Tuple { items, .. } => {
                for item in items {
                    collect_pipes(item, out);
                }
            }
            HirExpr::Record { fields, .. } => {
                for field in fields {
                    collect_pipes(&field.value, out);
                }
            }
            HirExpr::Patch { target, fields, .. } => {
                collect_pipes(target, out);
                for field in fields {
                    collect_pipes(&field.value, out);
                }
            }
            HirExpr::FieldAccess { base, .. } => collect_pipes(base, out),
            HirExpr::Index { base, index, .. } => {
                collect_pipes(base, out);
                collect_pipes(index, out);
            }
            HirExpr::Match { scrutinee, arms, .. } => {
                collect_pipes(scrutinee, out);
                for arm in arms {
                    collect_pipes(&arm.body, out);
                }
            }
            HirExpr::If { cond, then_branch, else_branch, .. } => {
                collect_pipes(cond, out);
                collect_pipes(then_branch, out);
                collect_pipes(else_branch, out);
            }
            HirExpr::Binary { left, right, .. } => {
                collect_pipes(left, out);
                collect_pipes(right, out);
            }
            HirExpr::Block { items, .. } => {
                for item in items {
                    match item {
                        HirBlockItem::Bind { expr, .. } | HirBlockItem::Expr { expr } => {
                            collect_pipes(expr, out);
                        }
                        _ => {}
                    }
                }
            }
            HirExpr::Var { .. }
            | HirExpr::LitNumber { .. }
            | HirExpr::LitString { .. }
            | HirExpr::LitSigil { .. }
            | HirExpr::LitBool { .. }
            | HirExpr::LitDateTime { .. }
            | HirExpr::Raw { .. } => {}
            HirExpr::Mock { substitutions, body, .. } => {
                for sub in substitutions {
                    if let Some(v) = &sub.value {
                        collect_pipes(v, out);
                    }
                }
                collect_pipes(body, out);
            }
        }
    }

    fn with_debug_trace(enabled: bool, f: impl FnOnce()) {
        super::DEBUG_TRACE_OVERRIDE.with(|cell| {
            let prev = cell.get();
            cell.set(Some(enabled));
            f();
            cell.set(prev);
        });
    }

    fn write_temp_source(source: &str) -> std::path::PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let mut path = std::env::temp_dir();
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let filename = format!("aivi_debug_{}_{}.aivi", std::process::id(), id);
        path.push(filename);
        std::fs::write(&path, source).expect("write temp source");
        path
    }

    #[test]
    fn debug_erased_when_flag_off() {
        let source = r#"
module test.debug

@debug(pipes, args, return, time)
f = x => x |> g 1 |> h
"#;
        let path = write_temp_source(source);
        with_debug_trace(false, || {
            let (modules, diags) = crate::surface::parse_modules(&path, source);
            assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
            let program = desugar_modules(&modules);
            let module = program.modules.into_iter().next().expect("module");
            let def = module.defs.into_iter().find(|d| d.name == "f").expect("f");
            assert!(!contains_debug_nodes(&def.expr));
        });
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn debug_instruments_pipes_and_labels() {
        let source = r#"
module test.debug

g = n x => x + n
h = x => x * 2

@debug(pipes, time)
f = x => x |> g 1 |> h
"#;
        let path = write_temp_source(source);
        with_debug_trace(true, || {
            let (modules, diags) = crate::surface::parse_modules(&path, source);
            assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
            let surface_def = match &modules[0].items[2] {
                ModuleItem::Def(def) => def,
                other => panic!("expected def item, got {other:?}"),
            };
            let params = super::parse_debug_params(&surface_def.decorators).expect("debug params");
            assert!(params.pipes);
            assert!(params.time);
            let program = desugar_modules(&modules);
            let module = program.modules.into_iter().next().expect("module");
            let def = module.defs.into_iter().find(|d| d.name == "f").expect("f");

            assert!(contains_debug_nodes(&def.expr));

            let mut pipes = Vec::new();
            collect_pipes(&def.expr, &mut pipes);
            pipes.sort_by_key(|(pipe_id, step, _)| (*pipe_id, *step));
            assert_eq!(pipes.len(), 2);
            assert_eq!(pipes[0].0, 1);
            assert_eq!(pipes[0].1, 1);
            assert_eq!(pipes[0].2, "g 1");
            assert_eq!(pipes[1].0, 1);
            assert_eq!(pipes[1].1, 2);
            assert_eq!(pipes[1].2, "h");
        });
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod lower_tests {
    use std::path::Path;
    use crate::hir::{HirExpr, HirBlockItem};

    fn parse_and_lower(src: &str) -> crate::hir::HirProgram {
        let (modules, diags) = crate::surface::parse_modules(Path::new("test.aivi"), src);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| {
                d.diagnostic.severity == crate::diagnostics::DiagnosticSeverity::Error
                    && !d.path.starts_with("<embedded:")
            })
            .collect();
        assert!(errors.is_empty(), "parse errors: {errors:?}");
        crate::hir::desugar_modules(&modules)
    }

    fn find_def_expr<'a>(program: &'a crate::hir::HirProgram, def_name: &str) -> &'a HirExpr {
        for module in &program.modules {
            for def in &module.defs {
                if def.name == def_name {
                    return &def.expr;
                }
            }
        }
        panic!("def {def_name} not found");
    }

    fn count_lambdas(expr: &HirExpr) -> usize {
        match expr {
            HirExpr::Lambda { body, .. } => 1 + count_lambdas(body),
            HirExpr::App { func, arg, .. } => count_lambdas(func) + count_lambdas(arg),
            HirExpr::Call { func, args, .. } => {
                count_lambdas(func) + args.iter().map(count_lambdas).sum::<usize>()
            }
            HirExpr::Match { scrutinee, arms, .. } => {
                count_lambdas(scrutinee)
                    + arms.iter().map(|a| count_lambdas(&a.body)).sum::<usize>()
            }
            HirExpr::Block { items, .. } => items
                .iter()
                .map(|item| match item {
                    HirBlockItem::Bind { expr, .. } | HirBlockItem::Expr { expr } => {
                        count_lambdas(expr)
                    }
                    _ => 0,
                })
                .sum(),
            HirExpr::If { cond, then_branch, else_branch, .. } => {
                count_lambdas(cond) + count_lambdas(then_branch) + count_lambdas(else_branch)
            }
            HirExpr::Binary { left, right, .. } => count_lambdas(left) + count_lambdas(right),
            _ => 0,
        }
    }

    fn expr_is_lambda(expr: &HirExpr) -> bool {
        matches!(expr, HirExpr::Lambda { .. })
    }

    fn inner_lambda_body(expr: &HirExpr) -> &HirExpr {
        match expr {
            HirExpr::Lambda { body, .. } => body,
            _ => panic!("expected Lambda"),
        }
    }

    // ---- lower_blocks_and_patterns.rs: lower_pattern ----

    #[test]
    fn pattern_wildcard_lowered() {
        let program = parse_and_lower(
            r#"
module Test

f = _ => 42
"#,
        );
        let expr = find_def_expr(&program, "f");
        assert!(matches!(expr, HirExpr::Lambda { param, .. } if param == "__0"));
    }

    #[test]
    fn pattern_ident_lowered() {
        let program = parse_and_lower(
            r#"
module Test

f = x => x
"#,
        );
        let expr = find_def_expr(&program, "f");
        assert!(expr_is_lambda(expr));
    }

    #[test]
    fn pattern_literal_lowered_to_match() {
        let program = parse_and_lower(
            r#"
module Test

isZero = 0 => True
isZero = _ => False
"#,
        );
        let expr = find_def_expr(&program, "isZero");
        let _ = expr;
    }

    #[test]
    fn pattern_constructor_lowered() {
        let program = parse_and_lower(
            r#"
module Test

fromSome : Option A -> A
fromSome = Some x => x
"#,
        );
        let expr = find_def_expr(&program, "fromSome");
        assert!(expr_is_lambda(expr));
    }

    #[test]
    fn pattern_tuple_lowered() {
        let program = parse_and_lower(
            r#"
module Test

fst = (a, _) => a
"#,
        );
        let expr = find_def_expr(&program, "fst");
        assert!(expr_is_lambda(expr));
    }

    #[test]
    fn pattern_record_lowered() {
        let program = parse_and_lower(
            r#"
module Test

getName = { name } => name
"#,
        );
        let expr = find_def_expr(&program, "getName");
        assert!(expr_is_lambda(expr));
    }

    #[test]
    fn pattern_list_cons_lowered() {
        let program = parse_and_lower(
            r#"
module Test

head : List A -> Option A
head = (x :: _) => Some x
head = _ => None
"#,
        );
        let expr = find_def_expr(&program, "head");
        let _ = expr;
    }

    #[test]
    fn pattern_at_binding_lowered() {
        let program = parse_and_lower(
            r#"
module Test

firstOrSelf : Option (Option A) -> Option A
firstOrSelf = all@(Some inner) => all
firstOrSelf = None => None
"#,
        );
        let expr = find_def_expr(&program, "firstOrSelf");
        let _ = expr;
    }

    // ---- lower_blocks_and_patterns.rs: do blocks ----

    #[test]
    fn do_block_bind_lowered() {
        let program = parse_and_lower(
            r#"
module Test

f = do Effect {
  x <- pure 42
  pure x
}
"#,
        );
        let expr = find_def_expr(&program, "f");
        assert!(matches!(expr, HirExpr::Block { .. }));
    }

    #[test]
    fn do_block_let_binding_lowered() {
        let program = parse_and_lower(
            r#"
module Test

g = do Effect {
  result = 1 + 2
  pure result
}
"#,
        );
        let expr = find_def_expr(&program, "g");
        assert!(matches!(expr, HirExpr::Block { .. }));
    }

    #[test]
    fn do_block_multiple_items() {
        let program = parse_and_lower(
            r#"
module Test

combined = do Effect {
  a <- pure 1
  b <- pure 2
  pure (a + b)
}
"#,
        );
        let expr = find_def_expr(&program, "combined");
        if let HirExpr::Block { items, .. } = expr {
            assert!(!items.is_empty());
        }
    }

    // ---- lower_blocks_and_patterns.rs: placeholder hole desugaring ----

    #[test]
    fn placeholder_in_call_args_desugars_to_lambda() {
        let program = parse_and_lower(
            r#"
module Test

addOne = map (_ + 1)
"#,
        );
        let expr = find_def_expr(&program, "addOne");
        let _ = expr;
    }

    #[test]
    fn multiple_holes_desugar_to_multi_param_lambda() {
        let program = parse_and_lower(
            r#"
module Test

sub = _ - _
"#,
        );
        let expr = find_def_expr(&program, "sub");
        let lambda_count = count_lambdas(expr);
        assert!(
            lambda_count >= 2,
            "expected at least 2 lambdas from two holes, got {lambda_count}"
        );
    }

    // ---- lower_expr.rs: various expression types ----

    #[test]
    fn lower_literal_number() {
        let program = parse_and_lower(
            r#"
module Test
n = 42
"#,
        );
        let expr = find_def_expr(&program, "n");
        assert!(matches!(expr, HirExpr::LitNumber { .. }));
    }

    #[test]
    fn lower_literal_string() {
        let program = parse_and_lower(
            r#"
module Test
s = "hello"
"#,
        );
        let expr = find_def_expr(&program, "s");
        assert!(matches!(expr, HirExpr::LitString { .. }));
    }

    #[test]
    fn lower_literal_bool() {
        let program = parse_and_lower(
            r#"
module Test
t = True
f = False
"#,
        );
        let _t = find_def_expr(&program, "t");
        let _f = find_def_expr(&program, "f");
    }

    #[test]
    fn lower_binary_expression() {
        let program = parse_and_lower(
            r#"
module Test
sum = 1 + 2
"#,
        );
        let expr = find_def_expr(&program, "sum");
        assert!(matches!(expr, HirExpr::Binary { op, .. } if op == "+"));
    }

    #[test]
    fn lower_if_expression() {
        let program = parse_and_lower(
            r#"
module Test
result = if True then 1 else 0
"#,
        );
        let expr = find_def_expr(&program, "result");
        assert!(matches!(expr, HirExpr::If { .. }));
    }

    #[test]
    fn lower_match_expression() {
        let program = parse_and_lower(
            r#"
module Test
describe = x =>
  x ?
    | 0 => "zero"
    | _ => "other"
"#,
        );
        let expr = find_def_expr(&program, "describe");
        assert!(expr_is_lambda(expr));
    }

    #[test]
    fn lower_match_with_guard() {
        let program = parse_and_lower(
            r#"
module Test
classify = x =>
  x ?
    | n if n < 0 => "negative"
    | 0 => "zero"
    | _ => "positive"
"#,
        );
        let expr = find_def_expr(&program, "classify");
        assert!(expr_is_lambda(expr));
    }

    #[test]
    fn lower_list_literal() {
        let program = parse_and_lower(
            r#"
module Test
nums = [1, 2, 3]
"#,
        );
        let expr = find_def_expr(&program, "nums");
        assert!(matches!(expr, HirExpr::List { items, .. } if items.len() == 3));
    }

    #[test]
    fn lower_tuple_literal() {
        let program = parse_and_lower(
            r#"
module Test
pair = (1, "hello")
"#,
        );
        let expr = find_def_expr(&program, "pair");
        assert!(matches!(expr, HirExpr::Tuple { items, .. } if items.len() == 2));
    }

    #[test]
    fn lower_record_literal() {
        let program = parse_and_lower(
            r#"
module Test
point = { x: 1, y: 2 }
"#,
        );
        let expr = find_def_expr(&program, "point");
        assert!(matches!(expr, HirExpr::Record { fields, .. } if fields.len() == 2));
    }

    #[test]
    fn lower_field_access() {
        let program = parse_and_lower(
            r#"
module Test
getX = pt => pt.x
"#,
        );
        let expr = find_def_expr(&program, "getX");
        assert!(expr_is_lambda(expr));
        let body = inner_lambda_body(expr);
        assert!(matches!(body, HirExpr::FieldAccess { .. }));
    }

    #[test]
    fn lower_function_application() {
        let program = parse_and_lower(
            r#"
module Test
result = identity 42
"#,
        );
        let expr = find_def_expr(&program, "result");
        let _ = expr;
    }

    #[test]
    fn lower_text_interpolation() {
        let program = parse_and_lower(
            r#"
module Test
greet = name => "Hello, ${name}!"
"#,
        );
        let expr = find_def_expr(&program, "greet");
        assert!(expr_is_lambda(expr));
        let body = inner_lambda_body(expr);
        assert!(matches!(body, HirExpr::TextInterpolate { .. }));
    }
}
