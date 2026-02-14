use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use aivi::{
    check_modules, check_types, file_diagnostics_have_errors, format_text, parse_modules,
    parse_target, CstBundle, Diagnostic, DiagnosticLabel, DiagnosticSeverity, FileDiagnostic, Span,
};
use walkdir::WalkDir;

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn set_workspace_root() -> PathBuf {
    let root = workspace_root();
    std::env::set_current_dir(&root).expect("set cwd");
    root
}

fn bless_enabled() -> bool {
    std::env::var("AIVI_BLESS").is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n")
}

fn write_blessed(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create golden parent");
    }
    fs::write(path, contents).expect("write blessed golden");
}

fn read_expected(path: &Path) -> String {
    normalize_newlines(&fs::read_to_string(path).expect("read golden"))
}

fn canonical_json(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Null => serde_json::Value::Null,
        serde_json::Value::Bool(b) => serde_json::Value::Bool(*b),
        serde_json::Value::Number(n) => serde_json::Value::Number(n.clone()),
        serde_json::Value::String(s) => serde_json::Value::String(s.clone()),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(canonical_json).collect())
        }
        serde_json::Value::Object(map) => {
            let mut keys: Vec<_> = map.keys().cloned().collect();
            keys.sort();
            let mut out = serde_json::Map::new();
            for k in keys {
                out.insert(k.clone(), canonical_json(&map[&k]));
            }
            serde_json::Value::Object(out)
        }
    }
}

fn canonical_pretty_json(value: &serde_json::Value) -> String {
    let value = canonical_json(value);
    let mut s = serde_json::to_string_pretty(&value).expect("pretty json");
    s.push('\n');
    s
}

fn severity_str(sev: DiagnosticSeverity) -> &'static str {
    match sev {
        DiagnosticSeverity::Error => "error",
        DiagnosticSeverity::Warning => "warning",
    }
}

fn diag_label_json(label: &DiagnosticLabel) -> serde_json::Value {
    serde_json::json!({
        "message": label.message,
        "span": span_json(&label.span),
    })
}

fn span_json(span: &Span) -> serde_json::Value {
    serde_json::json!({
        "start": { "line": span.start.line, "column": span.start.column },
        "end": { "line": span.end.line, "column": span.end.column },
    })
}

fn diag_json(path: &str, diag: &Diagnostic) -> serde_json::Value {
    let mut labels = diag.labels.clone();
    labels.sort_by(|a, b| {
        a.span
            .start
            .line
            .cmp(&b.span.start.line)
            .then(a.span.start.column.cmp(&b.span.start.column))
            .then(a.message.cmp(&b.message))
    });
    serde_json::json!({
        "path": path,
        "code": diag.code,
        "severity": severity_str(diag.severity),
        "message": diag.message,
        "span": span_json(&diag.span),
        "labels": labels.iter().map(diag_label_json).collect::<Vec<_>>(),
    })
}

fn diagnostics_snapshot(diags: &[FileDiagnostic]) -> String {
    let mut items: Vec<serde_json::Value> = diags
        .iter()
        .map(|fd| diag_json(&fd.path, &fd.diagnostic))
        .collect();
    items.sort_by(|a, b| {
        let ap = a.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let bp = b.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let ac = a.get("code").and_then(|v| v.as_str()).unwrap_or("");
        let bc = b.get("code").and_then(|v| v.as_str()).unwrap_or("");
        let am = a.get("message").and_then(|v| v.as_str()).unwrap_or("");
        let bm = b.get("message").and_then(|v| v.as_str()).unwrap_or("");
        ap.cmp(bp).then(ac.cmp(bc)).then(am.cmp(bm))
    });
    canonical_pretty_json(&serde_json::Value::Array(items))
}

fn cst_bundle_snapshot(bundle: &CstBundle) -> String {
    let value = serde_json::to_value(bundle).expect("bundle to json");
    canonical_pretty_json(&value)
}

fn fmt_snapshot(input: &str) -> String {
    let mut out = format_text(input);
    out = normalize_newlines(&out);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn list_cases(root: &Path) -> Vec<PathBuf> {
    let mut cases = Vec::new();
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.file_name() == OsStr::new("input.aivi"))
    {
        cases.push(entry.path().parent().expect("case dir").to_path_buf());
    }
    cases.sort();
    cases
}

fn run_case(case_dir: &Path) {
    let root = set_workspace_root();

    let input_path = case_dir.join("input.aivi");
    let rel_input = input_path.strip_prefix(&root).unwrap_or(&input_path);
    let input_src = normalize_newlines(&fs::read_to_string(&input_path).expect("read input"));

    // 1) Parse CST bundle snapshot (lexer + parser diagnostics included).
    let parse_expected_path = case_dir.join("parse.cst.json");
    let bundle = parse_target(&rel_input.to_string_lossy()).expect("parse target");
    let actual_parse = cst_bundle_snapshot(&bundle);
    if bless_enabled() {
        write_blessed(&parse_expected_path, &actual_parse);
    } else {
        let expected = read_expected(&parse_expected_path);
        assert_eq!(
            actual_parse.trim_end(),
            expected.trim_end(),
            "parse snapshot mismatch for {}",
            case_dir.display()
        );
    }

    // 2) Check/typecheck diagnostics snapshot (only if the golden exists).
    let check_expected_path = case_dir.join("check.diagnostics.json");
    if bless_enabled() || check_expected_path.exists() {
        let (modules, parse_diags) = parse_modules(rel_input, &input_src);
        assert!(
            !file_diagnostics_have_errors(&parse_diags),
            "parse produced errors for {}, cannot check: {parse_diags:?}",
            case_dir.display(),
        );

        let mut diags = check_modules(&modules);
        if !file_diagnostics_have_errors(&diags) {
            diags.extend(check_types(&modules));
        }
        let actual = diagnostics_snapshot(&diags);
        if bless_enabled() {
            write_blessed(&check_expected_path, &actual);
        } else {
            let expected = read_expected(&check_expected_path);
            assert_eq!(
                actual.trim_end(),
                expected.trim_end(),
                "diagnostics snapshot mismatch for {}",
                case_dir.display()
            );
        }
    }

    // 3) Formatter snapshot + idempotence (only if the golden exists).
    let fmt_expected_path = case_dir.join("fmt.aivi");
    if bless_enabled() || fmt_expected_path.exists() {
        let formatted1 = fmt_snapshot(&input_src);
        let formatted2 = fmt_snapshot(&formatted1);
        assert_eq!(
            formatted1,
            formatted2,
            "fmt not idempotent for {}",
            case_dir.display()
        );

        if bless_enabled() {
            write_blessed(&fmt_expected_path, &formatted1);
        } else {
            let expected = read_expected(&fmt_expected_path);
            assert_eq!(
                formatted1.trim_end(),
                expected.trim_end(),
                "fmt snapshot mismatch for {}",
                case_dir.display()
            );
        }
    }
}

#[test]
fn goldens_are_up_to_date() {
    let root = set_workspace_root();
    let cases_root = root.join("crates/aivi/tests/goldens/cases");
    let cases = list_cases(&cases_root);
    assert!(
        !cases.is_empty(),
        "no golden cases found under {}",
        cases_root.display()
    );

    for case_dir in cases {
        run_case(&case_dir);
    }
}
