use std::path::PathBuf;

use aivi::{Expr, Literal, Module, ModuleItem};

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
