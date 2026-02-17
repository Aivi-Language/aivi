use std::path::Path;

use aivi::{
    check_modules, check_types, file_diagnostics_have_errors, load_module_diagnostics,
    load_modules, parse_modules,
};

fn check_ok(source: &str) {
    let (modules, diagnostics) = parse_modules(Path::new("test.aivi"), source);
    assert!(
        !file_diagnostics_have_errors(&diagnostics),
        "parse errors: {diagnostics:?}"
    );

    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(
        !file_diagnostics_have_errors(&module_diags),
        "unexpected errors: {module_diags:?}"
    );
}

fn check_target_ok(target: &str) {
    let mut diagnostics =
        load_module_diagnostics(target).unwrap_or_else(|e| panic!("load diags: {e}"));
    let modules = load_modules(target).unwrap_or_else(|e| panic!("load modules: {e}"));

    diagnostics.extend(check_modules(&modules));
    if !file_diagnostics_have_errors(&diagnostics) {
        diagnostics.extend(check_types(&modules));
    }

    // Match `aivi check` behavior: ignore embedded stdlib errors (v0.1 stdlib is allowed to be
    // incomplete), but ensure the user file has no type errors.
    diagnostics.retain(|diag| !diag.path.starts_with("<embedded:"));

    assert!(
        !file_diagnostics_have_errors(&diagnostics),
        "unexpected errors: {diagnostics:?}"
    );
}

#[test]
fn typecheck_map_get_match_then_record_update_does_not_corrupt_types() {
    // Regression test for a type inference corruption bug seen in `integration-tests/complex/aStar.aivi`:
    // a `Map.get ... ?` match in the same block as Float arithmetic and a record update could
    // incorrectly unify the map value type to `Int`.
    let source = r#"
@no_prelude
module test.typecheck.astarRegression

State = {
  queue: Heap (Float, Int, Float)
  scores: Map Int Float
}

infinity : Float
infinity = 1000000.0

step : State -> { to: Int, weight: Float } -> State
step = state edge => {
  currentG = 1.0
  tentative = currentG + edge.weight
  prevScore = Map.get edge.to state.scores ?
    | Some v => v
    | None   => infinity
  if tentative < prevScore then {
    queue: Heap.push (tentative + 1.0, edge.to, tentative) state.queue
    scores: Map.insert edge.to tentative state.scores
  }
  else
    state
}
"#;
    check_ok(source);
}

#[test]
fn typecheck_astar_no_ambiguous_vec2_minus() {
    // Regression for `integration-tests/complex/aStar.aivi`:
    // `magnitude (target - current)` used to fail with an ambiguous `(-)` inside a match arm.
    check_target_ok("integration-tests/complex/aStar.aivi");
}
