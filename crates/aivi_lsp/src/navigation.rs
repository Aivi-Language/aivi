use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use aivi::{infer_value_types, parse_modules, Module, ModuleItem};
use tower_lsp::lsp_types::{
    Hover, HoverContents, Location, MarkupContent, MarkupKind, Position, TextEdit, Url,
    WorkspaceEdit,
};

use crate::backend::Backend;
use crate::doc_index::DocIndex;
use crate::state::IndexedModule;

impl Backend {
    fn find_record_field_name_at_position(
        expr: &aivi::Expr,
        position: Position,
    ) -> Option<&aivi::SpannedName> {
        use aivi::Expr;
        match expr {
            Expr::Suffixed { base, .. } => Self::find_record_field_name_at_position(base, position),
            Expr::UnaryNeg { expr, .. } => Self::find_record_field_name_at_position(expr, position),
            Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } => {
                for field in fields.iter() {
                    for segment in field.path.iter() {
                        if let aivi::PathSegment::Field(name) = segment {
                            let range = Self::span_to_range(name.span.clone());
                            if Self::range_contains_position(&range, position) {
                                return Some(name);
                            }
                        }
                    }
                    if let Some(found) =
                        Self::find_record_field_name_at_position(&field.value, position)
                    {
                        return Some(found);
                    }
                }
                None
            }
            Expr::FieldAccess { base, field, .. } => {
                let range = Self::span_to_range(field.span.clone());
                if Self::range_contains_position(&range, position) {
                    return Some(field);
                }
                Self::find_record_field_name_at_position(base, position)
            }
            Expr::FieldSection { field, .. } => {
                let range = Self::span_to_range(field.span.clone());
                if Self::range_contains_position(&range, position) {
                    return Some(field);
                }
                None
            }
            Expr::Ident(_) | Expr::Literal(_) | Expr::Raw { .. } => None,
            Expr::TextInterpolate { parts, .. } => parts.iter().find_map(|part| match part {
                aivi::TextPart::Text { .. } => None,
                aivi::TextPart::Expr { expr, .. } => {
                    Self::find_record_field_name_at_position(expr, position)
                }
            }),
            Expr::List { items, .. } => items
                .iter()
                .find_map(|item| Self::find_record_field_name_at_position(&item.expr, position)),
            Expr::Tuple { items, .. } => items
                .iter()
                .find_map(|item| Self::find_record_field_name_at_position(item, position)),
            Expr::Index { base, index, .. } => {
                Self::find_record_field_name_at_position(base, position)
                    .or_else(|| Self::find_record_field_name_at_position(index, position))
            }
            Expr::Call { func, args, .. } => {
                Self::find_record_field_name_at_position(func, position).or_else(|| {
                    args.iter()
                        .find_map(|arg| Self::find_record_field_name_at_position(arg, position))
                })
            }
            Expr::Lambda {
                params: _, body, ..
            } => Self::find_record_field_name_at_position(body, position),
            Expr::Match {
                scrutinee, arms, ..
            } => scrutinee
                .as_ref()
                .and_then(|expr| Self::find_record_field_name_at_position(expr, position))
                .or_else(|| {
                    arms.iter().find_map(|arm| {
                        Self::find_record_field_name_at_position(&arm.body, position).or_else(
                            || {
                                arm.guard.as_ref().and_then(|guard| {
                                    Self::find_record_field_name_at_position(guard, position)
                                })
                            },
                        )
                    })
                }),
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => Self::find_record_field_name_at_position(cond, position)
                .or_else(|| Self::find_record_field_name_at_position(then_branch, position))
                .or_else(|| Self::find_record_field_name_at_position(else_branch, position)),
            Expr::Binary { left, right, .. } => {
                Self::find_record_field_name_at_position(left, position)
                    .or_else(|| Self::find_record_field_name_at_position(right, position))
            }
            Expr::Block { items, .. } => items.iter().find_map(|item| match item {
                aivi::BlockItem::Bind { expr, .. }
                | aivi::BlockItem::Let { expr, .. }
                | aivi::BlockItem::Filter { expr, .. }
                | aivi::BlockItem::Yield { expr, .. }
                | aivi::BlockItem::Recurse { expr, .. }
                | aivi::BlockItem::Expr { expr, .. } => {
                    Self::find_record_field_name_at_position(expr, position)
                }
                aivi::BlockItem::When { cond, effect, .. }
                | aivi::BlockItem::Unless { cond, effect, .. } => {
                    Self::find_record_field_name_at_position(cond, position)
                        .or_else(|| Self::find_record_field_name_at_position(effect, position))
                }
                aivi::BlockItem::Given { cond, fail_expr, .. } => {
                    Self::find_record_field_name_at_position(cond, position)
                        .or_else(|| Self::find_record_field_name_at_position(fail_expr, position))
                }
                aivi::BlockItem::On { transition, handler, .. } => {
                    Self::find_record_field_name_at_position(transition, position)
                        .or_else(|| Self::find_record_field_name_at_position(handler, position))
                }
            }),
        }
    }

    fn type_sig_for_value<'a>(module: &'a Module, value_name: &str) -> Option<&'a aivi::TypeSig> {
        for item in module.items.iter() {
            match item {
                aivi::ModuleItem::TypeSig(sig) if sig.name.name == value_name => return Some(sig),
                aivi::ModuleItem::DomainDecl(domain) => {
                    for domain_item in domain.items.iter() {
                        if let aivi::DomainItem::TypeSig(sig) = domain_item {
                            if sig.name.name == value_name {
                                return Some(sig);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn type_alias_named<'a>(module: &'a Module, type_name: &str) -> Option<&'a aivi::TypeAlias> {
        for item in module.items.iter() {
            match item {
                aivi::ModuleItem::TypeAlias(alias) if alias.name.name == type_name => {
                    return Some(alias);
                }
                _ => {}
            }
        }
        None
    }

    fn record_field_definition_range_for_type(
        module: &Module,
        ty: &aivi::TypeExpr,
        field_name: &str,
    ) -> Option<tower_lsp::lsp_types::Range> {
        use aivi::TypeExpr;

        match ty {
            TypeExpr::Record { fields, .. } => fields.iter().find_map(|(name, _)| {
                if name.name == field_name {
                    Some(Self::span_to_range(name.span.clone()))
                } else {
                    None
                }
            }),
            TypeExpr::Name(name) => {
                let bare = name.name.rsplit('.').next().unwrap_or(&name.name);
                let alias = Self::type_alias_named(module, bare)?;
                Self::record_field_definition_range_for_type(module, &alias.aliased, field_name)
            }
            TypeExpr::Apply { base, .. } => {
                // For `Foo A B`, field declarations live on `Foo` if it's a record alias.
                Self::record_field_definition_range_for_type(module, base, field_name)
            }
            TypeExpr::And { .. }
            | TypeExpr::Func { .. }
            | TypeExpr::Tuple { .. }
            | TypeExpr::Star { .. }
            | TypeExpr::Unknown { .. } => None,
        }
    }

    fn build_record_field_definition(
        text: &str,
        uri: &Url,
        position: Position,
    ) -> Option<Location> {
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        let module = Self::module_at_position(&modules, position)?;

        // Find the containing def so we can use its type signature to resolve the record type.
        for item in module.items.iter() {
            let aivi::ModuleItem::Def(def) = item else {
                continue;
            };
            let def_range = Self::span_to_range(Self::expr_span(&def.expr).clone());
            if !Self::range_contains_position(&def_range, position) {
                continue;
            }
            let field = Self::find_record_field_name_at_position(&def.expr, position)?;
            let sig = Self::type_sig_for_value(module, &def.name.name)?;
            let range = Self::record_field_definition_range_for_type(module, &sig.ty, &field.name)?;
            return Some(Location::new(uri.clone(), range));
        }

        None
    }

    pub(super) fn build_definition(text: &str, uri: &Url, position: Position) -> Option<Location> {
        if let Some(location) = Self::build_record_field_definition(text, uri, position) {
            return Some(location);
        }

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
                if export.name.name == ident {
                    let range = Self::span_to_range(export.name.span.clone());
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
        // Try local record-field navigation first (it relies on local type signatures and aliases).
        if let Some(location) = Self::build_record_field_definition(text, uri, position) {
            return Some(location);
        }

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
                use_decl.wildcard || use_decl.items.iter().any(|item| item.name.name == ident);
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

    pub(super) fn build_hover(
        text: &str,
        uri: &Url,
        position: Position,
        doc_index: &DocIndex,
    ) -> Option<Hover> {
        let ident = Self::extract_identifier(text, position)?;
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        let (_, inferred) = infer_value_types(&modules);
        for module in modules.iter() {
            let doc = Self::doc_for_ident(text, module, &ident);
            let inferred = inferred.get(&module.name.name);
            if let Some(contents) =
                Self::hover_contents_for_module(module, &ident, inferred, doc.as_deref(), doc_index)
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

    /// Collect only the modules relevant for type inference: the current file's
    /// modules plus directly imported modules. This avoids running `infer_value_types`
    /// on the entire workspace (which is too slow for interactive hover).
    pub(crate) fn collect_relevant_modules(
        file_modules: &[Module],
        current_module: &Module,
        workspace_modules: &HashMap<String, IndexedModule>,
    ) -> Vec<Module> {
        let mut seen = HashSet::new();
        let mut result = Vec::new();

        // Add all modules from the current file.
        for m in file_modules {
            if seen.insert(m.name.name.clone()) {
                result.push(m.clone());
            }
        }

        // Add directly imported modules (via `use` declarations).
        for use_decl in current_module.uses.iter() {
            let module_name = &use_decl.module.name;
            if seen.insert(module_name.clone()) {
                if let Some(indexed) = workspace_modules.get(module_name) {
                    result.push(indexed.module.clone());
                }
            }
        }

        result
    }

    /// Resolve hover for dotted member access like `Heap.push`, `Map.empty`,
    /// `MutableMap.create` — looks up the prefix as a type/domain name in imported
    /// modules and then finds the member in that module.
    fn hover_for_dotted_member(
        ident: &str,
        current_module: &Module,
        workspace_modules: &HashMap<String, IndexedModule>,
        inferred: &HashMap<String, HashMap<String, String>>,
        doc_index: &DocIndex,
    ) -> Option<Hover> {
        let dot_pos = ident.find('.')?;
        let prefix = &ident[..dot_pos];
        let member = &ident[dot_pos + 1..];
        if prefix.is_empty() || member.is_empty() {
            return None;
        }

        // Look through imported modules for one that exports or defines the prefix
        // as a type, domain, or type alias. Then look up the member in that module.
        let modules_to_search = Self::find_modules_exporting(
            prefix,
            current_module,
            workspace_modules,
        );

        #[cfg(test)]
        eprintln!("hover_for_dotted_member: prefix={prefix:?}, member={member:?}, modules_found={}", modules_to_search.len());

        for indexed in &modules_to_search {
            let inf = inferred.get(&indexed.module.name.name);
            let doc_text = indexed
                .uri
                .to_file_path()
                .ok()
                .and_then(|p| fs::read_to_string(p).ok());
            let doc = doc_text
                .as_deref()
                .and_then(|text| Self::doc_for_ident(text, &indexed.module, member));

            #[cfg(test)]
            eprintln!("  checking module={}, has_inferred={}", indexed.module.name.name, inf.is_some());

            // Check domain members with the member name.
            if let Some(contents) = Self::hover_contents_for_module(
                &indexed.module,
                member,
                inf,
                doc.as_deref(),
                doc_index,
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

        // Also check the current module itself (the prefix might be defined locally).
        let doc = Self::doc_for_ident("", current_module, member);
        let inf = inferred.get(&current_module.name.name);
        if let Some(contents) = Self::hover_contents_for_module(
            current_module,
            member,
            inf,
            doc.as_deref(),
            doc_index,
        ) {
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: contents,
                }),
                range: None,
            });
        }

        None
    }

    /// Find modules that export or define a name (type, domain, type alias, etc.)
    /// matching the given prefix. Searches the current module's `use` imports.
    fn find_modules_exporting<'a>(
        name: &str,
        current_module: &Module,
        workspace_modules: &'a HashMap<String, IndexedModule>,
    ) -> Vec<&'a IndexedModule> {
        let mut result = Vec::new();
        for use_decl in current_module.uses.iter() {
            let imports_name = use_decl.wildcard
                || use_decl.items.iter().any(|item| item.name.name == name);
            if !imports_name {
                continue;
            }
            if let Some(indexed) = workspace_modules.get(&use_decl.module.name) {
                result.push(indexed);
            }
        }

        // Also check modules imported without item lists (bare `use aivi.collections`)
        // where the module itself may export the name.
        for use_decl in current_module.uses.iter() {
            if !use_decl.items.is_empty() || use_decl.wildcard {
                continue;
            }
            if let Some(indexed) = workspace_modules.get(&use_decl.module.name) {
                // Check if this module exports the name.
                let exports_name = indexed.module.exports.iter().any(|e| e.name.name == name);
                if exports_name && !result.iter().any(|r| r.uri == indexed.uri) {
                    result.push(indexed);
                }
            }
        }

        // Also check the prelude module if present.
        if let Some(prelude) = workspace_modules.get("aivi.prelude") {
            let exports_name = prelude.module.exports.iter().any(|e| e.name.name == name);
            if exports_name && !result.iter().any(|r| r.uri == prelude.uri) {
                result.push(prelude);
            }
        }

        // Finally check all workspace modules that define this as a domain/type,
        // since the name could come from the core module (e.g. `Heap` from `aivi`).
        if result.is_empty() {
            for indexed in workspace_modules.values() {
                let defines_name = indexed.module.items.iter().any(|item| match item {
                    ModuleItem::TypeDecl(decl) => decl.name.name == name,
                    ModuleItem::DomainDecl(domain) => {
                        // Domain's `over` type might match (e.g. domain MinHeap over Heap a)
                        domain.name.name == name
                    }
                    ModuleItem::TypeAlias(alias) => alias.name.name == name,
                    _ => false,
                });
                if defines_name {
                    result.push(indexed);
                }
            }
        }

        result
    }

    pub(super) fn build_hover_with_workspace(
        text: &str,
        uri: &Url,
        position: Position,
        workspace_modules: &HashMap<String, IndexedModule>,
        doc_index: &DocIndex,
    ) -> Option<Hover> {
        let ident = Self::extract_identifier(text, position);
        #[cfg(test)]
        eprintln!("build_hover_ws: ident={ident:?}");
        let ident = ident?;
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        #[cfg(test)]
        eprintln!("build_hover_ws: modules={}", modules.len());
        let current_module = Self::module_at_position(&modules, position);
        #[cfg(test)]
        eprintln!("build_hover_ws: current_module={}", current_module.map(|m| m.name.name.as_str()).unwrap_or("None"));
        let current_module = current_module?;

        // Only infer types for the current file's modules + direct imports (not the
        // entire workspace) to keep hover responsive in large projects.
        let relevant_modules = Self::collect_relevant_modules(
            &modules,
            current_module,
            workspace_modules,
        );
        let (_, inferred) = infer_value_types(&relevant_modules);

        // Handle dotted identifiers: first check if it's a full module name (e.g.
        // "aivi.collections"), then check Domain.method / Type.constructor patterns.
        if ident.contains('.') {
            // 1. Exact module name match.
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
                    doc_index,
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

            // 2. Domain.method or Type.constructor (e.g. "Heap.push", "Map.empty").
            if let Some(hover) = Self::hover_for_dotted_member(
                &ident,
                current_module,
                workspace_modules,
                &inferred,
                doc_index,
            ) {
                return Some(hover);
            }
        }

        let doc = Self::doc_for_ident(text, current_module, &ident);
        let inferred_current = inferred.get(&current_module.name.name);
        if let Some(contents) = Self::hover_contents_for_module(
            current_module,
            &ident,
            inferred_current,
            doc.as_deref(),
            doc_index,
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
                use_decl.wildcard || use_decl.items.iter().any(|item| item.name.name == ident);
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
            if let Some(contents) = Self::hover_contents_for_module(
                &indexed.module,
                &ident,
                inferred,
                doc.as_deref(),
                doc_index,
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
                text,
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

        let origin_module = if Self::module_member_definition_range(current_module, &ident)
            .is_some()
        {
            Some(current_module.name.name.clone())
        } else {
            current_module
                .uses
                .iter()
                .find(|use_decl| {
                    use_decl.wildcard || use_decl.items.iter().any(|item| item.name.name == ident)
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
                            || use_decl.items.iter().any(|item| item.name.name == ident))
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
