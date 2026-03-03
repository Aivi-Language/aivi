use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use aivi::parse_modules;
use tower_lsp::lsp_types::{Location, Position, TextEdit, Url, WorkspaceEdit};

use crate::backend::Backend;
use crate::state::IndexedModule;

use super::resolve_import_name;

impl Backend {
    pub(crate) fn build_references(
        text: &str,
        uri: &Url,
        position: Position,
        include_declaration: bool,
    ) -> Vec<Location> {
        let Some(ident) = Self::extract_identifier(text, position) else {
            return Vec::new();
        };
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        let mut locations = Vec::new();
        for module in modules {
            Self::collect_module_references(
                &module,
                &ident,
                text,
                uri,
                include_declaration,
                &mut locations,
            );
        }
        locations
    }

    pub(crate) fn build_references_with_workspace(
        text: &str,
        uri: &Url,
        position: Position,
        include_declaration: bool,
        workspace_modules: &HashMap<String, IndexedModule>,
    ) -> Vec<Location> {
        let Some(ident) = Self::extract_identifier(text, position) else {
            return Vec::new();
        };

        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        let Some(current_module) = Self::module_at_position(&modules, position) else {
            return Self::build_references(text, uri, position, include_declaration);
        };

        let origin_module =
            if Self::module_member_definition_range(current_module, &ident).is_some() {
                Some(current_module.name.name.clone())
            } else {
                current_module
                    .uses
                    .iter()
                    .find(|use_decl| {
                        use_decl.wildcard || resolve_import_name(&use_decl.items, &ident).is_some()
                    })
                    .map(|use_decl| use_decl.module.name.clone())
            };

        let Some(origin_module) = origin_module else {
            return Self::build_references(text, uri, position, include_declaration);
        };

        // Resolve the original (exported) name for cross-module reference search.
        let original_name = current_module
            .uses
            .iter()
            .find_map(|use_decl| resolve_import_name(&use_decl.items, &ident))
            .unwrap_or(&ident);

        let mut locations = Vec::new();
        for (module_name, indexed) in workspace_modules.iter() {
            let should_search = module_name == &origin_module
                || indexed.module.uses.iter().any(|use_decl| {
                    use_decl.module.name == origin_module
                        && (use_decl.wildcard
                            || resolve_import_name(&use_decl.items, original_name).is_some())
                });
            if !should_search {
                continue;
            }

            let include_decl_here = include_declaration && module_name == &origin_module;

            let module_text = if let Some(t) = &indexed.text {
                Some(t.clone())
            } else {
                indexed
                    .uri
                    .to_file_path()
                    .ok()
                    .and_then(|path| fs::read_to_string(path).ok())
            };

            if let Some(module_text) = module_text {
                Self::collect_module_references(
                    &indexed.module,
                    &ident,
                    &module_text,
                    &indexed.uri,
                    include_decl_here,
                    &mut locations,
                );
            }
        }

        locations
    }

    pub(crate) fn build_rename_with_workspace(
        text: &str,
        uri: &Url,
        position: Position,
        new_name: &str,
        workspace_modules: &HashMap<String, IndexedModule>,
    ) -> Option<WorkspaceEdit> {
        let _ident = Self::extract_identifier(text, position)?;

        if new_name.is_empty() || new_name.contains('.') {
            return None;
        }
        let mut chars = new_name.chars();
        let first = chars.next()?;
        if !(first.is_ascii_alphabetic() || first == '_') {
            return None;
        }
        if !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
            return None;
        }

        let locations =
            Self::build_references_with_workspace(text, uri, position, true, workspace_modules);
        if locations.is_empty() {
            return None;
        }

        let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
        for location in locations {
            changes.entry(location.uri).or_default().push(TextEdit {
                range: location.range,
                new_text: new_name.to_string(),
            });
        }

        Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        })
    }
}
