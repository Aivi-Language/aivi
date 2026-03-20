use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use aivi::{parse_modules, Module};
use tower_lsp::lsp_types::{Location, Position, Url};

use crate::backend::Backend;
use crate::state::IndexedModule;

use super::{resolve_import_name, resolve_module_alias};

impl Backend {
    fn identifier_span_near_offset(
        text: &str,
        offset: usize,
        allow_dot: bool,
    ) -> Option<(usize, usize)> {
        if text.is_empty() {
            return None;
        }

        let is_ident_char = |c: char| c.is_alphanumeric() || c == '_' || (allow_dot && c == '.');
        let ch_at = (offset < text.len())
            .then(|| text[offset..].chars().next())
            .flatten();
        let ch_before = (offset > 0)
            .then(|| text[..offset].chars().last())
            .flatten();
        let on_ident = ch_at.is_some_and(is_ident_char) || ch_before.is_some_and(is_ident_char);
        if !on_ident {
            return None;
        }

        let mut start = offset;
        while start > 0 {
            let ch = text[..start].chars().last().unwrap();
            if is_ident_char(ch) {
                start -= ch.len_utf8();
            } else {
                break;
            }
        }

        let mut end = offset;
        while end < text.len() {
            let ch = text[end..].chars().next().unwrap();
            if is_ident_char(ch) {
                end += ch.len_utf8();
            } else {
                break;
            }
        }

        Some((start, end))
    }

    fn identifier_span_at_position(
        text: &str,
        position: Position,
        allow_dot: bool,
    ) -> Option<(usize, usize)> {
        let offset = Self::offset_at(text, position).min(text.len());
        Self::identifier_span_near_offset(text, offset, allow_dot)
    }

    fn cursor_on_final_identifier_segment(text: &str, position: Position) -> bool {
        let Some((_, full_end)) = Self::identifier_span_at_position(text, position, true) else {
            return false;
        };
        let Some((_, segment_end)) = Self::identifier_span_at_position(text, position, false)
        else {
            return false;
        };
        segment_end == full_end
    }

    fn resolve_dotted_definition_target<'a>(
        ident: &'a str,
        current_module: &'a Module,
        workspace_modules: &'a HashMap<String, IndexedModule>,
    ) -> Option<(&'a IndexedModule, &'a str)> {
        let (qualifier, member) = ident.rsplit_once('.')?;
        let module_name =
            resolve_module_alias(&current_module.uses, qualifier).unwrap_or(qualifier);
        workspace_modules
            .get(module_name)
            .map(|indexed| (indexed, member))
    }

    fn tag_name_at_position(text: &str, position: Position) -> Option<String> {
        let offset = Self::offset_at(text, position).min(text.len());
        let ch_at = (offset < text.len())
            .then(|| text[offset..].chars().next())
            .flatten();
        let ch_before = (offset > 0)
            .then(|| text[..offset].chars().last())
            .flatten();

        let mut candidate_offsets = vec![offset];
        if ch_at == Some('<') {
            candidate_offsets.push(offset + '<'.len_utf8());
        }
        if matches!(ch_before, Some('<') | Some('/')) {
            candidate_offsets.push(offset);
        }

        for candidate_offset in candidate_offsets {
            let Some((start, end)) =
                Self::identifier_span_near_offset(text, candidate_offset, true)
            else {
                continue;
            };
            let Some(before) = text[..start].chars().last() else {
                continue;
            };
            if before == '<' {
                return Some(text[start..end].to_string());
            }
            if before == '/' {
                let slash_start = start.saturating_sub('/'.len_utf8());
                if slash_start > 0 && text[..slash_start].ends_with('<') {
                    return Some(text[start..end].to_string());
                }
            }
        }

        None
    }

    fn lowered_tag_call_ident_in_expr(expr: &aivi::Expr, position: Position) -> Option<String> {
        let range = Self::span_to_range(Self::expr_span(expr).clone());
        if !Self::range_contains_position(&range, position) {
            return None;
        }

        match expr {
            aivi::Expr::Call { func, args, .. } => {
                if let aivi::Expr::Ident(name) = func.as_ref() {
                    let func_range = Self::span_to_range(name.span.clone());
                    if Self::range_contains_position(&func_range, position) {
                        return Some(name.name.clone());
                    }
                }
                if let Some(name) = Self::lowered_tag_call_ident_in_expr(func, position) {
                    return Some(name);
                }
                for arg in args.iter() {
                    if let Some(name) = Self::lowered_tag_call_ident_in_expr(arg, position) {
                        return Some(name);
                    }
                }
                None
            }
            aivi::Expr::UnaryNeg { expr, .. } | aivi::Expr::Suffixed { base: expr, .. } => {
                Self::lowered_tag_call_ident_in_expr(expr, position)
            }
            aivi::Expr::TextInterpolate { parts, .. } => parts.iter().find_map(|part| match part {
                aivi::TextPart::Expr { expr, .. } => {
                    Self::lowered_tag_call_ident_in_expr(expr, position)
                }
                _ => None,
            }),
            aivi::Expr::List { items, .. } => items
                .iter()
                .find_map(|item| Self::lowered_tag_call_ident_in_expr(&item.expr, position)),
            aivi::Expr::Tuple { items, .. } => items
                .iter()
                .find_map(|item| Self::lowered_tag_call_ident_in_expr(item, position)),
            aivi::Expr::Record { fields, .. } | aivi::Expr::PatchLit { fields, .. } => fields
                .iter()
                .find_map(|field| Self::lowered_tag_call_ident_in_expr(&field.value, position)),
            aivi::Expr::FieldAccess { base, .. } => {
                Self::lowered_tag_call_ident_in_expr(base, position)
            }
            aivi::Expr::Index { base, index, .. } => {
                Self::lowered_tag_call_ident_in_expr(base, position)
                    .or_else(|| Self::lowered_tag_call_ident_in_expr(index, position))
            }
            aivi::Expr::Lambda { body, .. } => Self::lowered_tag_call_ident_in_expr(body, position),
            aivi::Expr::Match {
                scrutinee, arms, ..
            } => scrutinee
                .as_deref()
                .and_then(|expr| Self::lowered_tag_call_ident_in_expr(expr, position))
                .or_else(|| {
                    arms.iter().find_map(|arm| {
                        arm.guard
                            .as_ref()
                            .and_then(|guard| Self::lowered_tag_call_ident_in_expr(guard, position))
                            .or_else(|| Self::lowered_tag_call_ident_in_expr(&arm.body, position))
                    })
                }),
            aivi::Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => Self::lowered_tag_call_ident_in_expr(cond, position)
                .or_else(|| Self::lowered_tag_call_ident_in_expr(then_branch, position))
                .or_else(|| Self::lowered_tag_call_ident_in_expr(else_branch, position)),
            aivi::Expr::Binary { left, right, .. } => {
                Self::lowered_tag_call_ident_in_expr(left, position)
                    .or_else(|| Self::lowered_tag_call_ident_in_expr(right, position))
            }
            aivi::Expr::Block { items, .. } => items.iter().find_map(|item| match item {
                aivi::BlockItem::Expr { expr, .. } => {
                    Self::lowered_tag_call_ident_in_expr(expr, position)
                }
                aivi::BlockItem::Let { expr, .. }
                | aivi::BlockItem::Bind { expr, .. }
                | aivi::BlockItem::Filter { expr, .. }
                | aivi::BlockItem::Yield { expr, .. }
                | aivi::BlockItem::Recurse { expr, .. } => {
                    Self::lowered_tag_call_ident_in_expr(expr, position)
                }
                aivi::BlockItem::When { cond, effect, .. } => {
                    Self::lowered_tag_call_ident_in_expr(cond, position)
                        .or_else(|| Self::lowered_tag_call_ident_in_expr(effect, position))
                }
                aivi::BlockItem::Unless { cond, effect, .. } => {
                    Self::lowered_tag_call_ident_in_expr(cond, position)
                        .or_else(|| Self::lowered_tag_call_ident_in_expr(effect, position))
                }
                aivi::BlockItem::Given {
                    cond, fail_expr, ..
                } => Self::lowered_tag_call_ident_in_expr(cond, position)
                    .or_else(|| Self::lowered_tag_call_ident_in_expr(fail_expr, position)),
            }),
            aivi::Expr::Mock {
                substitutions,
                body,
                ..
            } => substitutions
                .iter()
                .find_map(|sub| {
                    sub.value
                        .as_ref()
                        .and_then(|expr| Self::lowered_tag_call_ident_in_expr(expr, position))
                })
                .or_else(|| Self::lowered_tag_call_ident_in_expr(body, position)),
            aivi::Expr::Flow { root, .. } => Self::lowered_tag_call_ident_in_expr(root, position),
            aivi::Expr::Ident(_)
            | aivi::Expr::Literal(_)
            | aivi::Expr::FieldSection { .. }
            | aivi::Expr::Raw { .. } => None,
        }
    }

    fn lowered_tag_call_ident_at_position(
        modules: &[Module],
        position: Position,
    ) -> Option<String> {
        fn from_def(def: &aivi::Def, position: Position) -> Option<String> {
            Backend::lowered_tag_call_ident_in_expr(&def.expr, position)
        }

        modules.iter().find_map(|module| {
            module.items.iter().find_map(|item| match item {
                aivi::ModuleItem::Def(def) => from_def(def, position),
                aivi::ModuleItem::InstanceDecl(instance) => {
                    instance.defs.iter().find_map(|def| from_def(def, position))
                }
                aivi::ModuleItem::DomainDecl(domain) => {
                    domain.items.iter().find_map(|item| match item {
                        aivi::DomainItem::Def(def) | aivi::DomainItem::LiteralDef(def) => {
                            from_def(def, position)
                        }
                        aivi::DomainItem::TypeAlias(_) | aivi::DomainItem::TypeSig(_) => None,
                    })
                }
                aivi::ModuleItem::TypeSig(_)
                | aivi::ModuleItem::TypeDecl(_)
                | aivi::ModuleItem::TypeAlias(_)
                | aivi::ModuleItem::ClassDecl(_) => None,
            })
        })
    }

    fn definition_lookup_candidates(
        text: &str,
        position: Position,
        modules: &[Module],
    ) -> Vec<String> {
        let mut candidates = Vec::new();
        let mut push_candidate = |ident: String| {
            if !ident.is_empty() && !candidates.iter().any(|candidate| candidate == &ident) {
                candidates.push(ident);
            }
        };

        if let Some(tag_name) = Self::tag_name_at_position(text, position) {
            push_candidate(tag_name.clone());
            if let Some(lowered) = Self::lowered_tag_call_ident_at_position(modules, position) {
                push_candidate(lowered);
            }
        }

        if let Some(ident) = Self::extract_identifier(text, position) {
            push_candidate(ident);
        }

        candidates
    }

    fn build_local_definition_for_ident(
        modules: &[Module],
        uri: &Url,
        ident: &str,
    ) -> Option<Location> {
        for module in modules {
            if module.name.name == ident {
                let range = Self::span_to_range(module.name.span.clone());
                return Some(Location::new(uri.clone(), range));
            }
            if let Some(range) = Self::module_member_definition_range(module, ident) {
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

    fn record_field_definition_range_for_type(
        module: &Module,
        ty: &aivi::TypeExpr,
        field_name: &str,
    ) -> Option<tower_lsp::lsp_types::Range> {
        use aivi::TypeExpr;

        match ty {
            TypeExpr::Record { fields, .. } => fields.iter().rev().find_map(|field| match field {
                aivi::RecordTypeField::Named { name, .. } if name.name == field_name => {
                    Some(Self::span_to_range(name.span.clone()))
                }
                aivi::RecordTypeField::Spread { ty, .. } => {
                    Self::record_field_definition_range_for_type(module, ty, field_name)
                }
                _ => None,
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

    pub(crate) fn build_definition(text: &str, uri: &Url, position: Position) -> Option<Location> {
        if let Some(location) = Self::build_record_field_definition(text, uri, position) {
            return Some(location);
        }

        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        for ident in Self::definition_lookup_candidates(text, position, &modules) {
            if let Some(location) = Self::build_local_definition_for_ident(&modules, uri, &ident) {
                return Some(location);
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
        if workspace_modules.is_empty() {
            return Self::build_definition(text, uri, position);
        }

        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        let lookup_candidates = Self::definition_lookup_candidates(text, position, &modules);
        if lookup_candidates.is_empty() {
            return None;
        }
        for ident in lookup_candidates.iter() {
            if let Some(location) = Self::build_local_definition_for_ident(&modules, uri, ident) {
                return Some(location);
            }
        }
        let current_module = Self::module_at_position(&modules, position)?;

        for ident in lookup_candidates.iter() {
            if ident.contains('.') {
                if let Some(indexed) = workspace_modules.get(ident) {
                    let range = Self::span_to_range(indexed.module.name.span.clone());
                    return Some(Location::new(indexed.uri.clone(), range));
                }

                if let Some((indexed, member)) =
                    Self::resolve_dotted_definition_target(ident, current_module, workspace_modules)
                {
                    if Self::cursor_on_final_identifier_segment(text, position) {
                        if let Some(range) =
                            Self::module_member_definition_range(&indexed.module, member)
                        {
                            return Some(Location::new(indexed.uri.clone(), range));
                        }
                    } else {
                        let range = Self::span_to_range(indexed.module.name.span.clone());
                        return Some(Location::new(indexed.uri.clone(), range));
                    }
                }
            }

            if let Some(module_name) = resolve_module_alias(&current_module.uses, ident) {
                if let Some(indexed) = workspace_modules.get(module_name) {
                    let range = Self::span_to_range(indexed.module.name.span.clone());
                    return Some(Location::new(indexed.uri.clone(), range));
                }
            }

            for use_decl in current_module.uses.iter() {
                let original = resolve_import_name(&use_decl.items, ident);
                let imported = use_decl.wildcard || original.is_some();
                if !imported {
                    continue;
                }

                let lookup = original.unwrap_or(ident);
                let Some(indexed) = workspace_modules.get(&use_decl.module.name) else {
                    continue;
                };
                if let Some(range) = Self::module_member_definition_range(&indexed.module, lookup) {
                    return Some(Location::new(indexed.uri.clone(), range));
                }
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
