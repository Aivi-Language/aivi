#![cfg(feature = "insta")]

use aivi::{
    file_diagnostics_have_errors, format_text, hir, kernel, lower_rust_ir, parse_modules,
    FileDiagnostic,
};
use serde::Serialize;
use std::path::Path;

fn pretty_json<T: Serialize>(value: &T) -> String {
    let mut out = serde_json::to_string_pretty(value).expect("serialize json");
    out.push('\n');
    out
}

fn normalize_newlines(mut s: String) -> String {
    s = s.replace("\r\n", "\n");
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

fn diagnostics_snapshot(diags: &[FileDiagnostic]) -> String {
    let mut lines: Vec<String> = diags
        .iter()
        .map(|diag| {
            let span = &diag.diagnostic.span.start;
            format!(
                "{}:{}:{}:{}:{} {}",
                diag.path,
                span.line,
                span.column,
                diag.diagnostic.code,
                match diag.diagnostic.severity {
                    aivi::DiagnosticSeverity::Error => "error",
                    aivi::DiagnosticSeverity::Warning => "warning",
                },
                diag.diagnostic.message
            )
        })
        .collect();
    lines.sort();
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

fn snapshot_case(name: &str, src: &str) {
    let path = Path::new(&format!("{name}.aivi"));
    let (modules, diags) = parse_modules(path, src);
    assert!(
        !file_diagnostics_have_errors(&diags),
        "parse errors in {name}: {diags:?}"
    );

    insta::assert_snapshot!(
        format!("{name}_surface_ast"),
        format!("{:#?}", modules)
    );

    insta::assert_snapshot!(
        format!("{name}_formatted"),
        normalize_newlines(format_text(src))
    );

    let hir_program = hir::desugar_modules(&modules);
    insta::assert_snapshot!(format!("{name}_hir"), pretty_json(&hir_program));

    let kernel_program = kernel::lower_hir(hir_program.clone());
    insta::assert_snapshot!(format!("{name}_kernel"), pretty_json(&kernel_program));

    let rust_ir = lower_rust_ir(kernel_program).expect("lower rust ir");
    insta::assert_snapshot!(format!("{name}_rust_ir"), pretty_json(&rust_ir));
}

#[test]
fn snapshot_basic_program() {
    let src = "module snapshots.basic\n\n\
add = a b => a + b\n\
inc = add 1\n\
answer = if True then add 20 22 else 0\n\
tupled = (inc 1, inc 2)\n";

    snapshot_case("snapshots_basic", src);
}

#[test]
fn snapshot_match_program() {
    let src = "module snapshots.matching\n\n\
describe = value => value match\n\
  | 0 => \"zero\"\n\
  | 1 => \"one\"\n\
  | _ => \"many\"\n\n\
flip = pair => pair match\n\
  | (a, b) => (b, a)\n";

    snapshot_case("snapshots_matching", src);
}

#[test]
fn snapshot_parse_diagnostics() {
    let src = "module snapshots.errors\n\n\
badIf = if True then 1 else\n\
badOp = 1 +\n\
stillGood = 5\n";

    let (_modules, diags) = parse_modules(Path::new("snapshots.errors.aivi"), src);
    assert!(file_diagnostics_have_errors(&diags));

    insta::assert_snapshot!("snapshots_errors_diagnostics", diagnostics_snapshot(&diags));
}
