use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use aivi::{parse_modules, ModuleItem};
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, Url};

use crate::backend::Backend;
use crate::state::IndexedModule;

impl Backend {
    pub(super) fn build_completion_items(
        text: &str,
        uri: &Url,
        workspace_modules: &HashMap<String, IndexedModule>,
    ) -> Vec<CompletionItem> {
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        let mut items = Vec::new();
        let mut seen = HashSet::new();
        let mut push_item = |label: String, kind: CompletionItemKind| {
            let key = format!("{label}:{kind:?}");
            if seen.insert(key) {
                items.push(CompletionItem {
                    label,
                    kind: Some(kind),
                    ..CompletionItem::default()
                });
            }
        };
        for keyword in Self::KEYWORDS {
            push_item(keyword.to_string(), CompletionItemKind::KEYWORD);
        }
        for sigil in Self::SIGILS {
            push_item(sigil.to_string(), CompletionItemKind::SNIPPET);
        }

        let mut module_list = Vec::new();
        let mut seen_modules = HashSet::new();
        for module in modules {
            seen_modules.insert(module.name.name.clone());
            module_list.push(module);
        }
        for indexed in workspace_modules.values() {
            if seen_modules.insert(indexed.module.name.name.clone()) {
                module_list.push(indexed.module.clone());
            }
        }

        for module in module_list {
            push_item(module.name.name.clone(), CompletionItemKind::MODULE);
            for export in module.exports {
                push_item(export.name, CompletionItemKind::PROPERTY);
            }
            for item in module.items {
                if let Some((label, kind)) = Self::completion_from_item(item) {
                    push_item(label, kind);
                }
            }
        }
        items
    }

    fn completion_from_item(item: ModuleItem) -> Option<(String, CompletionItemKind)> {
        match item {
            ModuleItem::Def(def) => Some((def.name.name, CompletionItemKind::FUNCTION)),
            ModuleItem::TypeSig(sig) => Some((sig.name.name, CompletionItemKind::FUNCTION)),
            ModuleItem::TypeDecl(decl) => Some((decl.name.name, CompletionItemKind::STRUCT)),
            ModuleItem::TypeAlias(alias) => {
                Some((alias.name.name, CompletionItemKind::TYPE_PARAMETER))
            }
            ModuleItem::ClassDecl(class_decl) => {
                Some((class_decl.name.name, CompletionItemKind::CLASS))
            }
            ModuleItem::InstanceDecl(instance_decl) => {
                Some((instance_decl.name.name, CompletionItemKind::VARIABLE))
            }
            ModuleItem::DomainDecl(domain_decl) => {
                Some((domain_decl.name.name, CompletionItemKind::MODULE))
            }
        }
    }
}
