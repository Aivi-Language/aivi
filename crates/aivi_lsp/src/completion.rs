use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use aivi::{parse_modules, ModuleItem};
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, Position, Url};

use crate::backend::Backend;
use crate::state::IndexedModule;

impl Backend {
    pub(super) fn build_completion_items(
        text: &str,
        uri: &Url,
        position: Position,
        workspace_modules: &HashMap<String, IndexedModule>,
    ) -> Vec<CompletionItem> {
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);

        let mut module_map = HashMap::new();
        for module in modules {
            module_map.insert(module.name.name.clone(), module);
        }
        for indexed in workspace_modules.values() {
            module_map
                .entry(indexed.module.name.name.clone())
                .or_insert_with(|| indexed.module.clone());
        }

        let mut seen = HashSet::new();
        let mut items = Vec::new();
        let mut push_item = |item: CompletionItem| {
            let kind_key = item.kind.unwrap_or(CompletionItemKind::TEXT);
            let key = format!(
                "{}:{kind_key:?}:{}",
                item.label,
                item.detail.as_deref().unwrap_or("")
            );
            if seen.insert(key) {
                items.push(item);
            }
        };

        let line_prefix = Self::line_prefix(text, position);

        if let Some(prefix) = Self::use_module_prefix(&line_prefix) {
            for name in module_map.keys() {
                if name.starts_with(prefix) {
                    push_item(CompletionItem {
                        label: name.clone(),
                        kind: Some(CompletionItemKind::MODULE),
                        ..CompletionItem::default()
                    });
                }
            }
            return items;
        }

        if let Some((module_name, already_imported, member_prefix)) =
            Self::use_exports_context(&line_prefix)
        {
            if let Some(module) = module_map.get(module_name) {
                for (label, kind, detail) in Self::module_export_completions(module) {
                    if already_imported.contains(&label) {
                        continue;
                    }
                    if !member_prefix.is_empty() && !label.starts_with(member_prefix) {
                        continue;
                    }
                    push_item(CompletionItem {
                        label,
                        kind: Some(kind),
                        detail,
                        ..CompletionItem::default()
                    });
                }
            }
            return items;
        }

        if let Some((path_prefix, member_prefix)) = Self::qualified_name_context(&line_prefix) {
            let mut produced_any = false;
            let mut module_segments = HashSet::new();
            let dotted = format!("{path_prefix}.");
            for name in module_map.keys() {
                if let Some(rest) = name.strip_prefix(&dotted) {
                    let seg = rest.split('.').next().unwrap_or(rest);
                    if seg.starts_with(&member_prefix) {
                        module_segments.insert(seg.to_string());
                    }
                }
            }
            for seg in module_segments {
                push_item(CompletionItem {
                    label: seg,
                    kind: Some(CompletionItemKind::MODULE),
                    ..CompletionItem::default()
                });
                produced_any = true;
            }

            if let Some(module) = module_map.get(&path_prefix) {
                for (label, kind, detail) in Self::module_export_completions(module) {
                    if !member_prefix.is_empty() && !label.starts_with(&member_prefix) {
                        continue;
                    }
                    push_item(CompletionItem {
                        label,
                        kind: Some(kind),
                        detail,
                        ..CompletionItem::default()
                    });
                    produced_any = true;
                }
            }

            if produced_any {
                return items;
            }
        }

        for keyword in Self::KEYWORDS {
            push_item(CompletionItem {
                label: keyword.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                ..CompletionItem::default()
            });
        }
        for sigil in Self::SIGILS {
            push_item(CompletionItem {
                label: sigil.to_string(),
                kind: Some(CompletionItemKind::SNIPPET),
                ..CompletionItem::default()
            });
        }

        for module in module_map.values() {
            push_item(CompletionItem {
                label: module.name.name.clone(),
                kind: Some(CompletionItemKind::MODULE),
                ..CompletionItem::default()
            });

            for (label, kind, detail) in Self::module_export_completions(module) {
                push_item(CompletionItem {
                    label,
                    kind: Some(kind),
                    detail,
                    ..CompletionItem::default()
                });
            }
        }

        items
    }

    fn completion_from_item(item: ModuleItem) -> Option<(String, CompletionItemKind)> {
        match item {
            ModuleItem::Def(def) => Some((def.name.name, CompletionItemKind::FUNCTION)),
            ModuleItem::TypeSig(sig) => Some((sig.name.name, CompletionItemKind::FUNCTION)),
            ModuleItem::TypeDecl(decl) => Some((decl.name.name, CompletionItemKind::STRUCT)),
            ModuleItem::TypeAlias(alias) => Some((alias.name.name, CompletionItemKind::INTERFACE)),
            ModuleItem::ClassDecl(class_decl) => {
                Some((class_decl.name.name, CompletionItemKind::CLASS))
            }
            ModuleItem::InstanceDecl(instance_decl) => {
                Some((instance_decl.name.name, CompletionItemKind::STRUCT))
            }
            ModuleItem::DomainDecl(domain_decl) => {
                Some((domain_decl.name.name, CompletionItemKind::MODULE))
            }
            ModuleItem::MachineDecl(_) => None,
        }
    }

    fn line_prefix(text: &str, position: Position) -> String {
        let offset = Self::offset_at(text, position).min(text.len());
        let line_start = text[..offset].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
        text[line_start..offset].to_string()
    }

    fn use_module_prefix(line_prefix: &str) -> Option<&str> {
        // `use <prefix>`
        let trimmed = line_prefix.trim_start();
        let rest = trimmed.strip_prefix("use ")?;
        if rest.contains('(') {
            return None;
        }
        if rest.contains(' ') || rest.contains('\t') {
            return None;
        }
        Some(rest)
    }

    fn use_exports_context(line_prefix: &str) -> Option<(&str, HashSet<String>, &str)> {
        // `use Mod (a, b, <prefix>`
        let trimmed = line_prefix.trim_start();
        let rest = trimmed.strip_prefix("use ")?;
        let (module_name, after_module) = rest.split_once('(')?;
        let module_name = module_name.trim_end();
        if module_name.is_empty() {
            return None;
        }
        let inside = after_module;
        let mut imported = HashSet::new();
        let parts: Vec<&str> = inside.split(',').collect();
        let prefix_part = parts.last().copied().unwrap_or("");
        for part in parts.iter().take(parts.len().saturating_sub(1)) {
            let name = part.trim();
            if name.is_empty() {
                continue;
            }
            // Only handle basic `use Mod (name, ...)` items for now.
            if name
                .chars()
                .all(|ch| ch.is_alphanumeric() || ch == '_' || ch == '.')
            {
                imported.insert(name.to_string());
            }
        }
        let member_prefix = prefix_part.trim_start().trim();
        Some((module_name, imported, member_prefix))
    }

    fn qualified_name_context(line_prefix: &str) -> Option<(String, String)> {
        // If the user is typing a dotted identifier, suggest either sub-modules or members.
        let suffix = line_prefix
            .chars()
            .rev()
            .take_while(|ch| ch.is_alphanumeric() || *ch == '_' || *ch == '.')
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        if !suffix.contains('.') {
            return None;
        }
        let (path_prefix, member_prefix) = suffix.rsplit_once('.')?;
        Some((path_prefix.to_string(), member_prefix.to_string()))
    }

    fn module_export_completions(
        module: &aivi::Module,
    ) -> Vec<(String, CompletionItemKind, Option<String>)> {
        let mut kind_by_name: HashMap<String, CompletionItemKind> = HashMap::new();
        let mut type_sig_by_name: HashMap<String, String> = HashMap::new();

        for item in module.items.iter().cloned() {
            match item {
                ModuleItem::TypeDecl(decl) => {
                    kind_by_name.insert(decl.name.name.clone(), CompletionItemKind::STRUCT);
                    for ctor in decl.constructors {
                        kind_by_name
                            .entry(ctor.name.name)
                            .or_insert(CompletionItemKind::CONSTRUCTOR);
                    }
                }
                ModuleItem::TypeSig(sig) => {
                    kind_by_name
                        .entry(sig.name.name.clone())
                        .or_insert(CompletionItemKind::FUNCTION);
                    type_sig_by_name.insert(
                        sig.name.name,
                        format!(": {}", Self::type_expr_to_string(&sig.ty)),
                    );
                }
                other => {
                    if let Some((label, kind)) = Self::completion_from_item(other) {
                        kind_by_name.entry(label).or_insert(kind);
                    }
                }
            }
        }

        let mut out = Vec::new();
        if !module.exports.is_empty() {
            for export in module.exports.iter() {
                let label = export.name.name.clone();
                let kind = kind_by_name
                    .get(&label)
                    .copied()
                    .unwrap_or(CompletionItemKind::PROPERTY);
                let detail = type_sig_by_name.get(&label).cloned();
                out.push((label, kind, detail));
            }
        } else {
            for (label, kind) in kind_by_name {
                let detail = type_sig_by_name.get(&label).cloned();
                out.push((label, kind, detail));
            }
        }
        out
    }
}
