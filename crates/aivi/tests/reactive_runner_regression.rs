use std::path::Path;

use aivi::{
    check_modules, desugar_modules, elaborate_expected_coercions, embedded_stdlib_modules,
    parse_modules, resolve_import_names, run_test_suite,
};

#[path = "test_support.rs"]
mod test_support;

fn filtered_errors(
    diags: Vec<aivi::diagnostics::FileDiagnostic>,
) -> Vec<aivi::diagnostics::FileDiagnostic> {
    diags
        .into_iter()
        .filter(|diag| {
            !diag.path.starts_with("<embedded:")
                && diag.diagnostic.severity == aivi::diagnostics::DiagnosticSeverity::Error
        })
        .collect()
}

fn hir_contains_binary_op(expr: &aivi::hir::HirExpr, target: &str) -> bool {
    use aivi::hir::{HirBlockItem, HirExpr, HirTextPart};

    match expr {
        HirExpr::Binary {
            op, left, right, ..
        } => {
            op == target
                || hir_contains_binary_op(left, target)
                || hir_contains_binary_op(right, target)
        }
        HirExpr::Lambda { body, .. } => hir_contains_binary_op(body, target),
        HirExpr::App { func, arg, .. } | HirExpr::Pipe { func, arg, .. } => {
            hir_contains_binary_op(func, target) || hir_contains_binary_op(arg, target)
        }
        HirExpr::Call { func, args, .. } => {
            hir_contains_binary_op(func, target)
                || args.iter().any(|arg| hir_contains_binary_op(arg, target))
        }
        HirExpr::DebugFn { body, .. } => hir_contains_binary_op(body, target),
        HirExpr::TextInterpolate { parts, .. } => parts.iter().any(|part| match part {
            HirTextPart::Text { .. } => false,
            HirTextPart::Expr { expr } => hir_contains_binary_op(expr, target),
        }),
        HirExpr::List { items, .. } => items
            .iter()
            .any(|item| hir_contains_binary_op(&item.expr, target)),
        HirExpr::Tuple { items, .. } => items
            .iter()
            .any(|item| hir_contains_binary_op(item, target)),
        HirExpr::Record { fields, .. } => fields
            .iter()
            .any(|field| hir_contains_binary_op(&field.value, target)),
        HirExpr::Patch {
            target: base,
            fields,
            ..
        } => {
            hir_contains_binary_op(base, target)
                || fields
                    .iter()
                    .any(|field| hir_contains_binary_op(&field.value, target))
        }
        HirExpr::FieldAccess { base, .. } => hir_contains_binary_op(base, target),
        HirExpr::Index { base, index, .. } => {
            hir_contains_binary_op(base, target) || hir_contains_binary_op(index, target)
        }
        HirExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            hir_contains_binary_op(cond, target)
                || hir_contains_binary_op(then_branch, target)
                || hir_contains_binary_op(else_branch, target)
        }
        HirExpr::Match {
            scrutinee, arms, ..
        } => {
            hir_contains_binary_op(scrutinee, target)
                || arms.iter().any(|arm| {
                    hir_contains_binary_op(&arm.body, target)
                        || arm
                            .guard
                            .as_ref()
                            .is_some_and(|guard| hir_contains_binary_op(guard, target))
                })
        }
        HirExpr::Block { items, .. } => items.iter().any(|item| match item {
            HirBlockItem::Bind { expr, .. }
            | HirBlockItem::Filter { expr }
            | HirBlockItem::Yield { expr }
            | HirBlockItem::Recurse { expr }
            | HirBlockItem::Expr { expr } => hir_contains_binary_op(expr, target),
        }),
        HirExpr::Var { .. }
        | HirExpr::LitNumber { .. }
        | HirExpr::LitString { .. }
        | HirExpr::LitSigil { .. }
        | HirExpr::LitBool { .. }
        | HirExpr::LitDateTime { .. }
        | HirExpr::Mock { .. }
        | HirExpr::Raw { .. } => false,
    }
}

#[test]
fn run_test_suite_executes_block_local_signal_sugar() {
    let source = r#"
@no_prelude
module integrationTests.stdlib.aivi.reactive.tests

use aivi
use aivi.reactive
use aivi.testing

ShellState = AiSettingsSection | SearchSection

@test "signal patch operator updates scalar signals"
signal_patch_operator_updates_scalar = {
  count = signal 1
  assertEq (count <<- (_ + 3)) Unit
  assertEq (get count) 4
}

@test "signal pipe derives a new signal from the current value"
signal_pipe_derives_from_source = {
  count = signal 2
  doubled = count ->> (_ * 2)
  assertEq (get doubled) 4
  assertEq (set count 5) Unit
  assertEq (get doubled) 10
}

@test "signal pipe accepts bare matcher blocks"
signal_pipe_accepts_bare_matcher_blocks = {
  shellState = signal (Some SearchSection)
  aiSettingsOpen = shellState ->>
    | Some AiSettingsSection => True
    | _                      => False
  assertEq (get aiSettingsOpen) False
  assertEq (set shellState (Some AiSettingsSection)) Unit
  assertEq (get aiSettingsOpen) True
  assertEq (set shellState None) Unit
  assertEq (get aiSettingsOpen) False
}
"#;

    let (mut modules, diags) = parse_modules(Path::new("test.aivi"), source);
    let parse_errors = filtered_errors(diags);
    assert!(
        parse_errors.is_empty(),
        "unexpected parse diagnostics: {parse_errors:?}"
    );

    let mut all_modules = embedded_stdlib_modules();
    all_modules.append(&mut modules);
    resolve_import_names(&mut all_modules);

    let mut diags = check_modules(&all_modules);
    if !test_support::file_diagnostics_have_non_embedded_errors(&diags) {
        diags.extend(elaborate_expected_coercions(&mut all_modules));
    }
    let diags = filtered_errors(diags);
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");

    let program = desugar_modules(&all_modules);
    let main_module = program
        .modules
        .iter()
        .find(|module| module.name == "integrationTests.stdlib.aivi.reactive.tests")
        .expect("expected reactive test module");
    for def_name in [
        "signal_patch_operator_updates_scalar",
        "signal_pipe_derives_from_source",
        "signal_pipe_accepts_bare_matcher_blocks",
    ] {
        let def = main_module
            .defs
            .iter()
            .find(|def| def.name == def_name || def.name.ends_with(&format!(".{def_name}")))
            .unwrap_or_else(|| {
                panic!(
                    "expected {def_name} def, available defs: {:?}",
                    main_module
                        .defs
                        .iter()
                        .map(|def| def.name.as_str())
                        .collect::<Vec<_>>()
                )
            });
        assert!(
            !hir_contains_binary_op(&def.expr, "<<-"),
            "expected {def_name} HIR to eliminate `<<-`: {:#?}",
            def.expr
        );
        assert!(
            !hir_contains_binary_op(&def.expr, "->>"),
            "expected {def_name} HIR to eliminate `->>`: {:#?}",
            def.expr
        );
    }
    let tests = test_support::collect_test_entries(&all_modules);
    let report = run_test_suite(program, &tests, &all_modules, false, None)
        .expect("run_test_suite succeeds");

    assert_eq!(report.failed, 0, "unexpected test failures: {report:#?}");
    assert_eq!(report.passed, 3, "expected all three tests to pass");
}
