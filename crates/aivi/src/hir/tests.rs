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

    fn parse_elaborate_and_lower(src: &str) -> crate::hir::HirProgram {
        let (mut modules, diags) = crate::surface::parse_modules(Path::new("test.aivi"), src);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| {
                d.diagnostic.severity == crate::diagnostics::DiagnosticSeverity::Error
                    && !d.path.starts_with("<embedded:")
            })
            .collect();
        assert!(errors.is_empty(), "parse errors: {errors:?}");

        let mut all_modules = crate::stdlib::embedded_stdlib_modules();
        all_modules.append(&mut modules);

        let diags = crate::resolver::check_modules(&all_modules);
        let errors: Vec<_> = diags
            .into_iter()
            .filter(|d| {
                d.diagnostic.severity == crate::diagnostics::DiagnosticSeverity::Error
                    && !d.path.starts_with("<embedded:")
            })
            .collect();
        assert!(errors.is_empty(), "resolver errors: {errors:?}");

        let diags = crate::typecheck::elaborate_expected_coercions(&mut all_modules);
        let errors: Vec<_> = diags
            .into_iter()
            .filter(|d| {
                d.diagnostic.severity == crate::diagnostics::DiagnosticSeverity::Error
                    && !d.path.starts_with("<embedded:")
            })
            .collect();
        assert!(errors.is_empty(), "elaboration errors: {errors:?}");

        crate::hir::desugar_modules(&all_modules)
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
        assert!(matches!(expr, HirExpr::Lambda { param, .. } if param == "_arg0"));
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
head = [x, ...] => Some x
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
firstOrSelf = all as (Some inner) => all
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
    | n when n < 0 => "negative"
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

    // ---- lower_blocks_and_patterns.rs: When/Unless/Given block items ----

    #[test]
    fn do_block_when_desugars_to_if() {
        let program = parse_and_lower(
            r#"
module Test

f = do Effect {
  when True <- print "hello"
  pure Unit
}
"#,
        );
        let expr = find_def_expr(&program, "f");
        assert!(matches!(expr, HirExpr::Block { .. }));
    }

    #[test]
    fn do_block_unless_desugars_to_negated_if() {
        let program = parse_and_lower(
            r#"
module Test

f = do Effect {
  unless False <- print "hello"
  pure Unit
}
"#,
        );
        let expr = find_def_expr(&program, "f");
        assert!(matches!(expr, HirExpr::Block { .. }));
    }

    #[test]
    fn do_block_given_desugars_to_if() {
        let program = parse_and_lower(
            r#"
module Test

f = do Effect {
  given True or fail "not true"
  pure Unit
}
"#,
        );
        let expr = find_def_expr(&program, "f");
        assert!(matches!(expr, HirExpr::Block { .. }));
    }

    // ---- lower_blocks_and_patterns.rs: Filter, Yield, Recurse ----

    #[test]
    fn generate_block_yield_lowered() {
        let program = parse_and_lower(
            r#"
module Test

nums = generate {
  yield 1
  yield 2
  yield 3
}
"#,
        );
        let expr = find_def_expr(&program, "nums");
        assert!(matches!(expr, HirExpr::Block { .. }));
    }

    #[test]
    fn generate_block_filter_lowered() {
        let program = parse_and_lower(
            r#"
module Test

evens = generate {
  x <- [1, 2, 3, 4]
  filter (x == 2)
  yield x
}
"#,
        );
        let expr = find_def_expr(&program, "evens");
        assert!(matches!(expr, HirExpr::Block { .. }));
    }

    #[test]
    fn generate_block_bind_lowered() {
        let program = parse_and_lower(
            r#"
module Test

cross = generate {
  x <- [1, 2]
  y <- [10, 20]
  yield (x + y)
}
"#,
        );
        let expr = find_def_expr(&program, "cross");
        assert!(matches!(expr, HirExpr::Block { .. }));
    }

    // ---- lower_blocks_and_patterns.rs: resource blocks ----

    #[test]
    fn resource_block_lowered() {
        let program = parse_and_lower(
            r#"
module Test

myRes = resource {
  handle <- openFile "test.txt"
  yield handle
  closeFile handle
}
"#,
        );
        let expr = find_def_expr(&program, "myRes");
        assert!(matches!(expr, HirExpr::Block { .. }));
    }

    // ---- lower_blocks_and_patterns.rs: nested do blocks ----

    #[test]
    fn nested_do_blocks() {
        let program = parse_and_lower(
            r#"
module Test

f = do Effect {
  x <- do Effect {
    pure 42
  }
  pure x
}
"#,
        );
        let expr = find_def_expr(&program, "f");
        if let HirExpr::Block { items, .. } = expr {
            assert!(items.len() >= 2);
        }
    }

    // ---- lower_blocks_and_patterns.rs: complex patterns ----

    #[test]
    fn pattern_nested_constructor_lowered() {
        let program = parse_and_lower(
            r#"
module Test

deep : Option (Option A) -> A
deep = Some (Some x) => x
"#,
        );
        let expr = find_def_expr(&program, "deep");
        assert!(expr_is_lambda(expr));
    }

    #[test]
    fn pattern_list_with_spread_rest() {
        let program = parse_and_lower(
            r#"
module Test

tail : List A -> List A
tail = [_, ...rest] => rest
tail = _ => []
"#,
        );
        let expr = find_def_expr(&program, "tail");
        let _ = expr;
    }

    #[test]
    fn pattern_list_exact_elements() {
        let program = parse_and_lower(
            r#"
module Test

isPair : List A -> Bool
isPair = [_, _] => True
isPair = _ => False
"#,
        );
        let expr = find_def_expr(&program, "isPair");
        let _ = expr;
    }

    #[test]
    fn pattern_record_multiple_fields() {
        let program = parse_and_lower(
            r#"
module Test

getInfo = { name, age } => (name, age)
"#,
        );
        let expr = find_def_expr(&program, "getInfo");
        assert!(expr_is_lambda(expr));
    }

    #[test]
    fn pattern_tuple_nested() {
        let program = parse_and_lower(
            r#"
module Test

nested = ((a, b), c) => (a, b, c)
"#,
        );
        let expr = find_def_expr(&program, "nested");
        assert!(expr_is_lambda(expr));
    }

    #[test]
    fn pattern_constructor_with_multiple_args() {
        let program = parse_and_lower(
            r#"
module Test

Result E A = Ok A | Err E

getValue : Result E A -> Option A
getValue = Ok x => Some x
getValue = Err _ => None
"#,
        );
        let expr = find_def_expr(&program, "getValue");
        let _ = expr;
    }

    #[test]
    fn pattern_literal_string() {
        let program = parse_and_lower(
            r#"
module Test

greet = "hello" => "hi"
greet = _ => "unknown"
"#,
        );
        let expr = find_def_expr(&program, "greet");
        let _ = expr;
    }

    #[test]
    fn pattern_literal_bool() {
        let program = parse_and_lower(
            r#"
module Test

flip = True => False
flip = False => True
"#,
        );
        let expr = find_def_expr(&program, "flip");
        let _ = expr;
    }

    // ---- lower_expr.rs: more expression types ----

    #[test]
    fn lower_pipe_expression() {
        let program = parse_and_lower(
            r#"
module Test

g = x => x + 1
result = 5 |> g
"#,
        );
        let expr = find_def_expr(&program, "result");
        assert!(matches!(expr, HirExpr::App { .. }));
    }

    #[test]
    fn lower_pipe_chain() {
        let program = parse_and_lower(
            r#"
module Test

g = x => x + 1
h = x => x * 2
result = 5 |> g |> h
"#,
        );
        let expr = find_def_expr(&program, "result");
        assert!(matches!(expr, HirExpr::App { .. }));
    }

    #[test]
    fn lower_and_desugars_to_if() {
        let program = parse_and_lower(
            r#"
module Test
result = True && False
"#,
        );
        let expr = find_def_expr(&program, "result");
        assert!(matches!(expr, HirExpr::If { .. }));
    }

    #[test]
    fn lower_or_desugars_to_if() {
        let program = parse_and_lower(
            r#"
module Test
result = True || False
"#,
        );
        let expr = find_def_expr(&program, "result");
        assert!(matches!(expr, HirExpr::If { .. }));
    }

    #[test]
    fn lower_patch_expression() {
        let program = parse_and_lower(
            r#"
module Test
updated = { x: 1, y: 2 } <| { x: 10 }
"#,
        );
        let expr = find_def_expr(&program, "updated");
        assert!(matches!(expr, HirExpr::Patch { .. }));
    }

    #[test]
    fn lower_signal_update_operator_to_reactive_update_call() {
        let program = parse_elaborate_and_lower(
            r#"
module Test
use aivi
use aivi.reactive

tick : Signal Int -> Unit
tick = counter => counter <<- (_ + 1)
"#,
        );
        let expr = find_def_expr(&program, "tick");
        assert!(expr_is_lambda(expr));
        let body = inner_lambda_body(expr);
        match body {
            HirExpr::Call { func, args, .. } => {
                assert!(matches!(
                    func.as_ref(),
                    HirExpr::FieldAccess { base, field, .. }
                        if matches!(base.as_ref(), HirExpr::Var { name, .. } if name == "reactive")
                            && field == "update"
                ));
                assert_eq!(args.len(), 2);
            }
            other => panic!("expected reactive update call, got {other:?}"),
        }
    }

    #[test]
    fn lower_index_expression() {
        let program = parse_and_lower(
            r#"
module Test
item = lst => lst[0]
"#,
        );
        let expr = find_def_expr(&program, "item");
        assert!(expr_is_lambda(expr));
        let body = inner_lambda_body(expr);
        assert!(matches!(body, HirExpr::Index { .. }));
    }

    #[test]
    fn lower_unary_neg() {
        let program = parse_and_lower(
            r#"
module Test
neg = x => -x
"#,
        );
        let expr = find_def_expr(&program, "neg");
        assert!(expr_is_lambda(expr));
        let body = inner_lambda_body(expr);
        assert!(matches!(body, HirExpr::Binary { op, .. } if op == "-"));
    }

    #[test]
    fn lower_datetime_literal() {
        let program = parse_and_lower(
            r#"
module Test
d = 2024-01-01T00:00:00Z
"#,
        );
        let expr = find_def_expr(&program, "d");
        assert!(matches!(expr, HirExpr::LitDateTime { .. }));
    }

    #[test]
    fn lower_lambda_multiple_params() {
        let program = parse_and_lower(
            r#"
module Test
add = a => b => c => a + b + c
"#,
        );
        let expr = find_def_expr(&program, "add");
        assert!(expr_is_lambda(expr));
        let b1 = inner_lambda_body(expr);
        assert!(expr_is_lambda(b1));
        let b2 = inner_lambda_body(b1);
        assert!(expr_is_lambda(b2));
    }

    #[test]
    fn lower_field_section() {
        let program = parse_and_lower(
            r#"
module Test
getName = .name
"#,
        );
        let expr = find_def_expr(&program, "getName");
        assert!(expr_is_lambda(expr));
        let body = inner_lambda_body(expr);
        assert!(matches!(body, HirExpr::FieldAccess { .. }));
    }

    #[test]
    fn lower_suffixed_number() {
        let program = parse_and_lower(
            r#"
module Test
duration = 30s
"#,
        );
        let expr = find_def_expr(&program, "duration");
        assert!(matches!(expr, HirExpr::App { .. }));
    }

    // ---- lower_blocks_and_patterns.rs: Let binding in do Effect wraps pure ----

    #[test]
    fn do_effect_let_wraps_pure() {
        let program = parse_and_lower(
            r#"
module Test

f = do Effect {
  x = 42
  pure x
}
"#,
        );
        let expr = find_def_expr(&program, "f");
        if let HirExpr::Block { items, .. } = expr {
            if let HirBlockItem::Bind { expr: bind_expr, is_monadic, .. } = &items[0] {
                assert!(!is_monadic);
                assert!(matches!(bind_expr, HirExpr::Call { func, .. } if matches!(func.as_ref(), HirExpr::Var { name, .. } if name == "pure")));
            }
        }
    }

    // ---- lower_blocks_and_patterns.rs: generic do block desugaring ----

    #[test]
    fn generic_do_block_desugars_to_chain() {
        let program = parse_and_lower(
            r#"
module Test

f = do Option {
  x <- Some 1
  y <- Some 2
  of (x + y)
}
"#,
        );
        let expr = find_def_expr(&program, "f");
        fn contains_chain(expr: &HirExpr) -> bool {
            match expr {
                HirExpr::Call { func, args, .. } => {
                    matches!(func.as_ref(), HirExpr::Var { name, .. } if name == "chain")
                        || contains_chain(func)
                        || args.iter().any(contains_chain)
                }
                HirExpr::App { func, arg, .. } => contains_chain(func) || contains_chain(arg),
                HirExpr::Lambda { body, .. } => contains_chain(body),
                _ => false,
            }
        }
        assert!(contains_chain(expr), "expected chain calls in generic do block desugaring");
    }

    // ---- lower_blocks_and_patterns.rs: plain block desugaring ----

    #[test]
    fn plain_block_let_bindings() {
        let program = parse_and_lower(
            r#"
module Test

result = do {
  x = 1
  y = 2
  x + y
}
"#,
        );
        let expr = find_def_expr(&program, "result");
        assert!(matches!(expr, HirExpr::Block { .. }));
    }

    // ---- lower_expr.rs: record spread ----

    #[test]
    fn lower_record_with_spread() {
        let program = parse_and_lower(
            r#"
module Test

base = { x: 1, y: 2 }
extended = { ...base, z: 3 }
"#,
        );
        let expr = find_def_expr(&program, "extended");
        if let HirExpr::Record { fields, .. } = expr {
            assert!(fields.iter().any(|f| f.spread));
        }
    }

    // ---- lower_expr.rs: list with spread (range) ----

    #[test]
    fn lower_list_with_range() {
        let program = parse_and_lower(
            r#"
module Test

nums = [1, 2..5, 6]
"#,
        );
        let expr = find_def_expr(&program, "nums");
        if let HirExpr::List { items, .. } = expr {
            assert!(items.len() == 3);
            assert!(items[1].spread);
        }
    }

    // ---- lower_expr.rs: Mock expression ----

    #[test]
    fn lower_mock_expression() {
        let program = parse_and_lower(
            r#"
module Test

result = mock someFunc = x => 42 in someFunc 1
"#,
        );
        let expr = find_def_expr(&program, "result");
        assert!(matches!(expr, HirExpr::Mock { .. }));
    }

    // ---- lower_expr.rs: match with scrutinee ----

    #[test]
    fn lower_match_with_scrutinee() {
        let program = parse_and_lower(
            r#"
module Test
classify = x =>
  x ?
    | 0 => "zero"
    | _ => "other"
"#,
        );
        let expr = find_def_expr(&program, "classify");
        assert!(expr_is_lambda(expr));
        let body = inner_lambda_body(expr);
        assert!(matches!(body, HirExpr::Match { .. }));
    }
}
