use std::{fs, path::PathBuf};

use aivi_base::SourceDatabase;
use aivi_syntax::parse_module;

use crate::{Item, ItemId, Module, lower_module};

pub fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("frontend")
}

pub fn lower_text(path: &str, text: &str) -> crate::LoweringResult {
    let mut sources = SourceDatabase::new();
    let file_id = sources.add_file(path, text);
    let parsed = parse_module(&sources[file_id]);
    assert!(
        !parsed.has_errors(),
        "fixture {path} should parse before HIR lowering: {:?}",
        parsed.all_diagnostics().collect::<Vec<_>>()
    );
    lower_module(&parsed.module)
}

pub fn lower_fixture(path: &str) -> crate::LoweringResult {
    let text = fs::read_to_string(fixture_root().join(path)).expect("fixture should be readable");
    lower_text(path, &text)
}

pub fn item_name(module: &Module, item_id: ItemId) -> &str {
    match &module.items()[item_id] {
        Item::Type(item) => item.name.text(),
        Item::Value(item) => item.name.text(),
        Item::Function(item) => item.name.text(),
        Item::Signal(item) => item.name.text(),
        Item::Class(item) => item.name.text(),
        Item::Domain(item) => item.name.text(),
        Item::SourceProviderContract(_) | Item::Instance(_) | Item::Use(_) | Item::Export(_) => {
            "<anonymous>"
        }
    }
}
