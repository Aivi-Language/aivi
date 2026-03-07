use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use aivi::{parse_modules, Module};
use tower_lsp::lsp_types::{Location, Position, Url};

use crate::backend::Backend;
use crate::state::IndexedModule;

use super::resolve_import_name;

impl Backend {
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
            TypeExpr::CapabilityClause { base, .. } => {
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

    pub(crate) fn build_definition(text: &str, uri: &Url, position: Position) -> Option<Location> {
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

    pub(crate) fn build_definition_with_workspace(
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
            let original = resolve_import_name(&use_decl.items, &ident);
            let imported = use_decl.wildcard || original.is_some();
            if !imported {
                continue;
            }

            let lookup = original.unwrap_or(&ident);
            let Some(indexed) = workspace_modules.get(&use_decl.module.name) else {
                continue;
            };
            if let Some(range) = Self::module_member_definition_range(&indexed.module, lookup) {
                return Some(Location::new(indexed.uri.clone(), range));
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
        let mut direct_imports = Vec::new();
        for use_decl in current_module.uses.iter() {
            let module_name = &use_decl.module.name;
            if seen.insert(module_name.clone()) {
                if let Some(indexed) = workspace_modules.get(module_name) {
                    result.push(indexed.module.clone());
                    direct_imports.push(indexed.module.clone());
                }
            }
        }

        // Add 2nd-level imports (imports of directly imported modules) so type
        // inference can resolve transitive dependencies for hover.
        for imported_module in &direct_imports {
            for use_decl in imported_module.uses.iter() {
                let module_name = &use_decl.module.name;
                if seen.insert(module_name.clone()) {
                    if let Some(indexed) = workspace_modules.get(module_name) {
                        result.push(indexed.module.clone());
                    }
                }
            }
        }

        result
    }
}
