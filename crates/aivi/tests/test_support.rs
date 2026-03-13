use std::path::PathBuf;

use aivi::{diagnostics::DiagnosticSeverity, Expr, FileDiagnostic, Literal, Module, ModuleItem};

#[allow(dead_code)]
pub fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

pub fn collect_test_entries(modules: &[Module]) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    for module in modules {
        if module.name.name.starts_with("aivi.") || module.name.name == "aivi" {
            continue;
        }
        for item in &module.items {
            let ModuleItem::Def(def) = item else {
                continue;
            };
            if let Some(dec) = def.decorators.iter().find(|d| d.name.name == "test") {
                let name = format!("{}.{}", module.name.name, def.name.name);
                let description = match &dec.arg {
                    Some(Expr::Literal(Literal::String { text, .. })) => text.clone(),
                    _ => name.clone(),
                };
                entries.push((name, description));
            }
        }
    }
    entries.sort();
    entries.dedup();
    entries
}

pub fn file_diagnostics_have_non_embedded_errors(diags: &[FileDiagnostic]) -> bool {
    diags.iter().any(|diag| {
        !diag.path.starts_with("<embedded:")
            && diag.diagnostic.severity == DiagnosticSeverity::Error
    })
}

#[allow(dead_code)]
pub fn configured_test_threads(env_var: &str, default_cap: usize) -> usize {
    std::env::var(env_var)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|count| *count > 0)
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|count| count.get().min(default_cap))
                .unwrap_or(1)
        })
}
