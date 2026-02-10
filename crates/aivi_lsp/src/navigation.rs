use super::*;

impl Backend {
    pub(super) fn build_definition(text: &str, uri: &Url, position: Position) -> Option<Location> {
        let ident = Self::extract_identifier(text, position)?;
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        for module in modules {
            if module.name.name == ident {
                let range = Self::span_to_range(module.name.span);
                return Some(Location::new(uri.clone(), range));
            }
            if let Some(range) = Self::module_member_definition_range(&module, &ident) {
                return Some(Location::new(uri.clone(), range));
            }
            for export in module.exports.iter() {
                if export.name == ident {
                    let range = Self::span_to_range(export.span.clone());
                    return Some(Location::new(uri.clone(), range));
                }
            }
        }
        None
    }

    pub(super) fn build_definition_with_workspace(
        text: &str,
        uri: &Url,
        position: Position,
        workspace_modules: &HashMap<String, IndexedModule>,
    ) -> Option<Location> {
        let ident = Self::extract_identifier(text, position)?;

        if let Some(location) = Self::build_definition(text, uri, position) {
            return Some(location);
        }

        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        let current_module = Self::module_at_position(&modules, position)?;

        if ident.contains('.') {
            if let Some(indexed) = workspace_modules.get(&ident) {
                let range = Self::span_to_range(indexed.module.name.span.clone());
                return Some(Location::new(indexed.uri.clone(), range));
            }
        }

        for use_decl in current_module.uses.iter() {
            let imported =
                use_decl.wildcard || use_decl.items.iter().any(|item| item.name == ident);
            if !imported {
                continue;
            }

            let Some(indexed) = workspace_modules.get(&use_decl.module.name) else {
                continue;
            };
            if let Some(range) = Self::module_member_definition_range(&indexed.module, &ident) {
                return Some(Location::new(indexed.uri.clone(), range));
            }
        }

        None
    }

    pub(super) fn build_hover(text: &str, uri: &Url, position: Position) -> Option<Hover> {
        let ident = Self::extract_identifier(text, position)?;
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        let (_, inferred) = infer_value_types(&modules);
        for module in modules.iter() {
            let doc = Self::doc_for_ident(text, module, &ident);
            let inferred = inferred.get(&module.name.name);
            if let Some(contents) =
                Self::hover_contents_for_module(module, &ident, inferred, doc.as_deref())
            {
                return Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: contents,
                    }),
                    range: None,
                });
            }
        }
        None
    }

    pub(super) fn build_hover_with_workspace(
        text: &str,
        uri: &Url,
        position: Position,
        workspace_modules: &HashMap<String, IndexedModule>,
    ) -> Option<Hover> {
        let ident = Self::extract_identifier(text, position)?;
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        let current_module = Self::module_at_position(&modules, position)?;

        let workspace_module_list: Vec<Module> = workspace_modules
            .values()
            .map(|indexed| indexed.module.clone())
            .collect();
        let (_, inferred) = infer_value_types(&workspace_module_list);

        if ident.contains('.') {
            if let Some(indexed) = workspace_modules.get(&ident) {
                let doc_text = indexed
                    .uri
                    .to_file_path()
                    .ok()
                    .and_then(|path| fs::read_to_string(path).ok());
                let doc = doc_text
                    .as_deref()
                    .and_then(|text| Self::doc_for_ident(text, &indexed.module, &ident));
                let inferred = inferred.get(&indexed.module.name.name);
                if let Some(contents) = Self::hover_contents_for_module(
                    &indexed.module,
                    &ident,
                    inferred,
                    doc.as_deref(),
                ) {
                    return Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: contents,
                        }),
                        range: None,
                    });
                }
            }
        }

        let doc = Self::doc_for_ident(text, current_module, &ident);
        let inferred_current = inferred.get(&current_module.name.name);
        if let Some(contents) = Self::hover_contents_for_module(
            current_module,
            &ident,
            inferred_current,
            doc.as_deref(),
        ) {
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: contents,
                }),
                range: None,
            });
        }

        for use_decl in current_module.uses.iter() {
            let imported =
                use_decl.wildcard || use_decl.items.iter().any(|item| item.name == ident);
            if !imported {
                continue;
            }
            let Some(indexed) = workspace_modules.get(&use_decl.module.name) else {
                continue;
            };
            let doc_text = indexed
                .uri
                .to_file_path()
                .ok()
                .and_then(|path| fs::read_to_string(path).ok());
            let doc = doc_text
                .as_deref()
                .and_then(|text| Self::doc_for_ident(text, &indexed.module, &ident));
            let inferred = inferred.get(&indexed.module.name.name);
            if let Some(contents) =
                Self::hover_contents_for_module(&indexed.module, &ident, inferred, doc.as_deref())
            {
                return Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: contents,
                    }),
                    range: None,
                });
            }
        }

        None
    }

    pub(super) fn build_references(
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
                uri,
                include_declaration,
                &mut locations,
            );
        }
        locations
    }

    pub(super) fn build_references_with_workspace(
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
                        use_decl.wildcard || use_decl.items.iter().any(|item| item.name == ident)
                    })
                    .map(|use_decl| use_decl.module.name.clone())
            };

        let Some(origin_module) = origin_module else {
            return Self::build_references(text, uri, position, include_declaration);
        };

        let mut locations = Vec::new();
        for (module_name, indexed) in workspace_modules.iter() {
            let should_search = module_name == &origin_module
                || indexed.module.uses.iter().any(|use_decl| {
                    use_decl.module.name == origin_module
                        && (use_decl.wildcard
                            || use_decl.items.iter().any(|item| item.name == ident))
                });
            if !should_search {
                continue;
            }

            let include_decl_here = include_declaration && module_name == &origin_module;
            Self::collect_module_references(
                &indexed.module,
                &ident,
                &indexed.uri,
                include_decl_here,
                &mut locations,
            );
        }

        locations
    }

    pub(super) fn build_rename_with_workspace(
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
