use aivi::{parse_modules, DiagnosticSeverity, ModuleItem};
use std::collections::HashSet;
use std::path::Path;

fn error_lines(diagnostics: &[aivi::FileDiagnostic]) -> HashSet<usize> {
    diagnostics
        .iter()
        .filter(|diag| diag.diagnostic.severity == DiagnosticSeverity::Error)
        .map(|diag| diag.diagnostic.span.start.line)
        .collect()
}

#[test]
fn parser_recovers_after_multiple_errors() {
    let src = "module parse.recovery\n\n\
good = 1\n\n\
badIf = if True then 1 else\n\
badOp = 1 +\n\
badTuple = (1, 2\n\n\
stillGood = 42\n";

    let (modules, diagnostics) = parse_modules(Path::new("recovery.aivi"), src);
    let lines = error_lines(&diagnostics);

    assert!(lines.contains(&5), "expected error on line 5, got {lines:?}");
    assert!(lines.contains(&6), "expected error on line 6, got {lines:?}");
    assert!(lines.contains(&7), "expected error on line 7, got {lines:?}");

    let module = modules.first().expect("parsed module");
    let recovered = module.items.iter().any(|item| match item {
        ModuleItem::Def(def) => def.name.name == "stillGood",
        _ => false,
    });

    assert!(recovered, "expected parser to recover and parse later defs");
}
