#[test]
fn diagnostics_resolve_selective_embedded_reactive_imports() {
    let text = r#"module examples.reactive
use aivi
use aivi.reactive (Signal, signal, set, update, derive)

state : Signal { count: Int }
state = signal { count: 0 }

title = derive state .count
increment = _ => update state (patch { count: _ + 1 })
reset = _ => set state { count: 0 }
"#;
    let diagnostics = Backend::build_diagnostics(text, &sample_uri());
    assert!(
        !diagnostics.iter().any(|diag| {
            matches!(
                diag.code.as_ref(),
                Some(NumberOrString::String(code)) if code == "E2005"
            )
        }),
        "unexpected unknown-name diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn diagnostics_with_session_accept_unannotated_signal_writes() {
    let temp_dir =
        std::env::temp_dir().join(format!("aivi-lsp-signal-session-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");

    let path = temp_dir.join("signal_app.aivi");
    let uri = Url::from_file_path(&path).expect("uri");
    let text = r#"module test.signal_app

use aivi
use aivi.reactive

state = signal 1
write = _ => state <<- 2
"#;

    let mut workspace = HashMap::new();
    let (modules, parse_diags) = parse_modules(&path, text);
    assert!(
        parse_diags.is_empty(),
        "unexpected parse diagnostics for {}: {parse_diags:?}",
        path.display()
    );
    for module in modules {
        workspace.insert(
            module.name.name.clone(),
            IndexedModule {
                uri: uri.clone(),
                module,
                text: Some(text.to_string()),
            },
        );
    }

    let session = std::sync::Mutex::new(aivi_driver::WorkspaceSession::new());
    {
        let mut guard = session.lock().expect("lock session");
        guard.upsert_source(path.clone(), text.to_string());
    }

    let diagnostics = Backend::build_diagnostics_with_session(
        text,
        &uri,
        &workspace,
        false,
        &crate::strict::StrictConfig::default(),
        &session,
    );
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|diag| diag.severity == Some(DiagnosticSeverity::ERROR))
        .collect();
    assert!(
        errors.is_empty(),
        "valid unannotated signal writes should not produce LSP errors: {errors:?}"
    );
    assert!(
        !diagnostics.iter().any(|diag| {
            matches!(
                diag.code.as_ref(),
                Some(NumberOrString::String(code)) if code == "E3000"
            )
        }),
        "unexpected E3000 diagnostics: {diagnostics:#?}"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}
