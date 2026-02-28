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
