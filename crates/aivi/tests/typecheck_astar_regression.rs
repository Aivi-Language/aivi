use std::path::Path;

use aivi::{check_modules, check_types, file_diagnostics_have_errors, parse_modules};

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
