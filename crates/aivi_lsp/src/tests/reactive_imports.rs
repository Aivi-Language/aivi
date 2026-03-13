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
