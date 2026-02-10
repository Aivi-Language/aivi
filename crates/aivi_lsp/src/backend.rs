use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use aivi::{
    BlockItem, ClassDecl, Def, DomainDecl, DomainItem, Expr, InstanceDecl, ListItem, MatchArm,
    Module, ModuleItem, PathSegment, Pattern, RecordField, RecordPatternField, Span, SpannedName,
    TypeAlias, TypeCtor, TypeDecl, TypeExpr, UseDecl,
};
use tokio::sync::Mutex;
use tower_lsp::lsp_types::{Location, Position, Range, Url};
use tower_lsp::Client;

use crate::state::BackendState;

pub(super) struct Backend {
    pub(super) client: Client,
    pub(super) state: Arc<Mutex<BackendState>>,
}

impl Backend {
    pub(super) fn span_to_range(span: Span) -> Range {
        let start_line = span.start.line.saturating_sub(1) as u32;
        let start_char = span.start.column.saturating_sub(1) as u32;
        let end_line = span.end.line.saturating_sub(1) as u32;
        let end_char = span.end.column as u32;
        Range::new(
            Position::new(start_line, start_char),
            Position::new(end_line, end_char),
        )
    }

    pub(super) fn offset_at(text: &str, position: Position) -> usize {
        let mut offset = 0usize;
        for (line, chunk) in text.split_inclusive('\n').enumerate() {
            if line as u32 == position.line {
                let char_offset = position.character as usize;
                return offset
                    + chunk
                        .chars()
                        .take(char_offset)
                        .map(|c| c.len_utf8())
                        .sum::<usize>();
            }
            offset += chunk.len();
        }
        offset
    }

    pub(super) fn extract_identifier(text: &str, position: Position) -> Option<String> {
        let offset = Self::offset_at(text, position).min(text.len());
        let bytes = text.as_bytes();
        if bytes.is_empty() {
            return None;
        }
        let mut start = offset.min(bytes.len());
        while start > 0 {
            let ch = text[start - 1..].chars().next()?;
            if ch.is_alphanumeric() || ch == '_' || ch == '.' {
                start -= ch.len_utf8();
            } else {
                break;
            }
        }
        let mut end = offset.min(bytes.len());
        while end < bytes.len() {
            let ch = text[end..].chars().next()?;
            if ch.is_alphanumeric() || ch == '_' || ch == '.' {
                end += ch.len_utf8();
            } else {
                break;
            }
        }
        let ident = text[start..end].trim();
        if ident.is_empty() {
            None
        } else {
            Some(ident.to_string())
        }
    }

    fn doc_block_above(text: &str, line: usize) -> Option<String> {
        let lines: Vec<&str> = text.lines().collect();
        let mut index = line.checked_sub(2)?;
        let mut docs = Vec::new();
        loop {
            let current = lines.get(index)?.trim_start();
            if current.is_empty() {
                break;
            }
            let Some(body) = current.strip_prefix("//") else {
                break;
            };
            docs.push(body.strip_prefix(' ').unwrap_or(body).to_string());
            if index == 0 {
                break;
            }
            index -= 1;
        }
        docs.reverse();
        (!docs.is_empty()).then_some(docs.join("\n"))
    }

    fn decl_line_for_ident(module: &Module, ident: &str) -> Option<usize> {
        if module.name.name == ident {
            return Some(module.name.span.start.line);
        }
        for item in module.items.iter() {
            match item {
                ModuleItem::Def(def) if def.name.name == ident => {
                    return Some(def.name.span.start.line);
                }
                ModuleItem::TypeSig(sig) if sig.name.name == ident => {
                    return Some(sig.name.span.start.line);
                }
                ModuleItem::TypeDecl(decl) if decl.name.name == ident => {
                    return Some(decl.name.span.start.line);
                }
                ModuleItem::TypeAlias(alias) if alias.name.name == ident => {
                    return Some(alias.name.span.start.line);
                }
                ModuleItem::ClassDecl(class_decl) if class_decl.name.name == ident => {
                    return Some(class_decl.name.span.start.line);
                }
                ModuleItem::InstanceDecl(instance_decl) if instance_decl.name.name == ident => {
                    return Some(instance_decl.name.span.start.line);
                }
                ModuleItem::DomainDecl(domain_decl) if domain_decl.name.name == ident => {
                    return Some(domain_decl.name.span.start.line);
                }
                ModuleItem::DomainDecl(domain_decl) => {
                    for domain_item in domain_decl.items.iter() {
                        match domain_item {
                            DomainItem::TypeAlias(type_decl) if type_decl.name.name == ident => {
                                return Some(type_decl.name.span.start.line);
                            }
                            DomainItem::TypeSig(sig) if sig.name.name == ident => {
                                return Some(sig.name.span.start.line);
                            }
                            DomainItem::Def(def) | DomainItem::LiteralDef(def)
                                if def.name.name == ident =>
                            {
                                return Some(def.name.span.start.line);
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub(super) fn doc_for_ident(text: &str, module: &Module, ident: &str) -> Option<String> {
        let line = Self::decl_line_for_ident(module, ident)?;
        Self::doc_block_above(text, line)
    }

    pub(super) fn hover_contents_for_module(
        module: &Module,
        ident: &str,
        inferred: Option<&HashMap<String, String>>,
        doc: Option<&str>,
    ) -> Option<String> {
        let mut base = None;
        if module.name.name == ident {
            base = Some(format!("module `{}`", module.name.name));
        }
        let mut type_signatures = HashMap::new();
        for item in module.items.iter() {
            if let ModuleItem::TypeSig(sig) = item {
                type_signatures.insert(
                    sig.name.name.clone(),
                    format!(
                        "`{}` : `{}`",
                        sig.name.name,
                        Self::type_expr_to_string(&sig.ty)
                    ),
                );
            }
        }
        if base.is_none() {
            if let Some(sig) = type_signatures.get(ident) {
                base = Some(sig.clone());
            }
        }
        if base.is_none() {
            for item in module.items.iter() {
                if let Some(contents) =
                    Self::hover_contents_for_item(item, ident, &type_signatures, inferred)
                {
                    base = Some(contents);
                    break;
                }
            }
        }
        if base.is_none() {
            for domain in module.items.iter().filter_map(|item| match item {
                ModuleItem::DomainDecl(domain) => Some(domain),
                _ => None,
            }) {
                if let Some(contents) = Self::hover_contents_for_domain(domain, ident, inferred) {
                    base = Some(contents);
                    break;
                }
            }
        }

        let mut base = base?;
        if let Some(doc) = doc {
            let doc = doc.trim();
            if !doc.is_empty() {
                base.push_str("\n\n");
                base.push_str(doc);
            }
        }
        Some(base)
    }

    fn hover_contents_for_item(
        item: &ModuleItem,
        ident: &str,
        type_signatures: &HashMap<String, String>,
        inferred: Option<&HashMap<String, String>>,
    ) -> Option<String> {
        match item {
            ModuleItem::Def(def) => {
                if def.name.name == ident {
                    if let Some(sig) = type_signatures.get(ident) {
                        return Some(sig.clone());
                    }
                    if let Some(ty) = inferred.and_then(|types| types.get(ident)) {
                        return Some(format!("`{}` : `{}`", def.name.name, ty));
                    }
                    return Some(format!("`{}`", def.name.name));
                }
            }
            ModuleItem::TypeSig(sig) => {
                if sig.name.name == ident {
                    return Some(format!(
                        "`{}` : `{}`",
                        sig.name.name,
                        Self::type_expr_to_string(&sig.ty)
                    ));
                }
            }
            ModuleItem::TypeDecl(decl) => {
                if decl.name.name == ident {
                    return Some(format!("`{}`", Self::format_type_decl(decl)));
                }
            }
            ModuleItem::TypeAlias(alias) => {
                if alias.name.name == ident {
                    return Some(format!("`{}`", Self::format_type_alias(alias)));
                }
            }
            ModuleItem::ClassDecl(class_decl) => {
                if class_decl.name.name == ident {
                    return Some(format!("`{}`", Self::format_class_decl(class_decl)));
                }
                for member in class_decl.members.iter() {
                    if member.name.name == ident {
                        return Some(format!(
                            "`{}` : `{}`",
                            member.name.name,
                            Self::type_expr_to_string(&member.ty)
                        ));
                    }
                }
            }
            ModuleItem::InstanceDecl(instance_decl) => {
                if instance_decl.name.name == ident {
                    return Some(format!("`{}`", Self::format_instance_decl(instance_decl)));
                }
            }
            ModuleItem::DomainDecl(domain_decl) => {
                if domain_decl.name.name == ident {
                    return Some(format!(
                        "`domain {}` over `{}`",
                        domain_decl.name.name,
                        Self::type_expr_to_string(&domain_decl.over)
                    ));
                }
            }
        }
        None
    }

    fn hover_contents_for_domain(
        domain_decl: &DomainDecl,
        ident: &str,
        inferred: Option<&HashMap<String, String>>,
    ) -> Option<String> {
        let mut type_signatures = HashMap::new();
        for item in domain_decl.items.iter() {
            if let DomainItem::TypeSig(sig) = item {
                type_signatures.insert(
                    sig.name.name.clone(),
                    format!(
                        "`{}` : `{}`",
                        sig.name.name,
                        Self::type_expr_to_string(&sig.ty)
                    ),
                );
            }
        }
        if let Some(sig) = type_signatures.get(ident) {
            return Some(sig.clone());
        }
        for item in domain_decl.items.iter() {
            match item {
                DomainItem::TypeAlias(type_decl) => {
                    if type_decl.name.name == ident {
                        return Some(format!("`{}`", Self::format_type_decl(type_decl)));
                    }
                }
                DomainItem::TypeSig(_) => {}
                DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                    if def.name.name == ident {
                        if let Some(sig) = type_signatures.get(ident) {
                            return Some(sig.clone());
                        }
                        if let Some(ty) = inferred.and_then(|types| types.get(ident)) {
                            return Some(format!("`{}` : `{}`", def.name.name, ty));
                        }
                        return Some(format!("`{}`", def.name.name));
                    }
                }
            }
        }
        None
    }

    pub(super) fn collect_module_references(
        module: &Module,
        ident: &str,
        uri: &Url,
        include_declaration: bool,
        locations: &mut Vec<Location>,
    ) {
        if include_declaration && module.name.name == ident {
            locations.push(Location::new(
                uri.clone(),
                Self::span_to_range(module.name.span.clone()),
            ));
        }
        for export in module.exports.iter() {
            if export.name == ident {
                locations.push(Location::new(
                    uri.clone(),
                    Self::span_to_range(export.span.clone()),
                ));
            }
        }
        for annotation in module.annotations.iter() {
            if annotation.name == ident {
                locations.push(Location::new(
                    uri.clone(),
                    Self::span_to_range(annotation.span.clone()),
                ));
            }
        }
        for use_decl in module.uses.iter() {
            Self::collect_use_references(use_decl, ident, uri, locations);
        }
        for item in module.items.iter() {
            Self::collect_item_references(item, ident, uri, include_declaration, locations);
        }
    }

    fn collect_use_references(
        use_decl: &UseDecl,
        ident: &str,
        uri: &Url,
        locations: &mut Vec<Location>,
    ) {
        if use_decl.module.name == ident {
            locations.push(Location::new(
                uri.clone(),
                Self::span_to_range(use_decl.module.span.clone()),
            ));
        }
        for item in use_decl.items.iter() {
            if item.name == ident {
                locations.push(Location::new(
                    uri.clone(),
                    Self::span_to_range(item.span.clone()),
                ));
            }
        }
    }

    fn collect_item_references(
        item: &ModuleItem,
        ident: &str,
        uri: &Url,
        include_declaration: bool,
        locations: &mut Vec<Location>,
    ) {
        match item {
            ModuleItem::Def(def) => {
                Self::collect_def_references(def, ident, uri, include_declaration, locations);
            }
            ModuleItem::TypeSig(sig) => {
                if include_declaration && sig.name.name == ident {
                    locations.push(Location::new(
                        uri.clone(),
                        Self::span_to_range(sig.name.span.clone()),
                    ));
                }
                Self::collect_type_expr_references(&sig.ty, ident, uri, locations);
            }
            ModuleItem::TypeDecl(decl) => {
                Self::collect_type_decl_references(
                    decl,
                    ident,
                    uri,
                    include_declaration,
                    locations,
                );
            }
            ModuleItem::TypeAlias(alias) => {
                Self::collect_type_alias_references(alias, ident, uri, include_declaration, locations);
            }
            ModuleItem::ClassDecl(class_decl) => {
                Self::collect_class_references(class_decl, ident, uri, include_declaration, locations);
            }
            ModuleItem::InstanceDecl(instance_decl) => {
                Self::collect_instance_references(
                    instance_decl,
                    ident,
                    uri,
                    include_declaration,
                    locations,
                );
            }
            ModuleItem::DomainDecl(domain_decl) => {
                Self::collect_domain_references(
                    domain_decl,
                    ident,
                    uri,
                    include_declaration,
                    locations,
                );
            }
        }
    }

    fn collect_def_references(
        def: &Def,
        ident: &str,
        uri: &Url,
        include_declaration: bool,
        locations: &mut Vec<Location>,
    ) {
        if include_declaration && def.name.name == ident {
            locations.push(Location::new(
                uri.clone(),
                Self::span_to_range(def.name.span.clone()),
            ));
        }
        for param in def.params.iter() {
            Self::collect_pattern_references(param, ident, uri, locations);
        }
        Self::collect_expr_references(&def.expr, ident, uri, locations);
    }

    fn collect_type_decl_references(
        decl: &TypeDecl,
        ident: &str,
        uri: &Url,
        include_declaration: bool,
        locations: &mut Vec<Location>,
    ) {
        if include_declaration && decl.name.name == ident {
            locations.push(Location::new(
                uri.clone(),
                Self::span_to_range(decl.name.span.clone()),
            ));
        }
        for param in decl.params.iter() {
            if param.name == ident {
                locations.push(Location::new(
                    uri.clone(),
                    Self::span_to_range(param.span.clone()),
                ));
            }
        }
        for ctor in decl.constructors.iter() {
            Self::collect_type_ctor_references(ctor, ident, uri, include_declaration, locations);
        }
    }

    fn collect_type_alias_references(
        alias: &TypeAlias,
        ident: &str,
        uri: &Url,
        include_declaration: bool,
        locations: &mut Vec<Location>,
    ) {
        if include_declaration && alias.name.name == ident {
            locations.push(Location::new(
                uri.clone(),
                Self::span_to_range(alias.name.span.clone()),
            ));
        }
        for param in alias.params.iter() {
            if param.name == ident {
                locations.push(Location::new(
                    uri.clone(),
                    Self::span_to_range(param.span.clone()),
                ));
            }
        }
        Self::collect_type_expr_references(&alias.aliased, ident, uri, locations);
    }

    fn collect_class_references(
        class_decl: &ClassDecl,
        ident: &str,
        uri: &Url,
        include_declaration: bool,
        locations: &mut Vec<Location>,
    ) {
        if include_declaration && class_decl.name.name == ident {
            locations.push(Location::new(
                uri.clone(),
                Self::span_to_range(class_decl.name.span.clone()),
            ));
        }
        for param in class_decl.params.iter() {
            Self::collect_type_expr_references(param, ident, uri, locations);
        }
        for member in class_decl.members.iter() {
            if include_declaration && member.name.name == ident {
                locations.push(Location::new(
                    uri.clone(),
                    Self::span_to_range(member.name.span.clone()),
                ));
            }
            Self::collect_type_expr_references(&member.ty, ident, uri, locations);
        }
    }

    fn collect_instance_references(
        instance_decl: &InstanceDecl,
        ident: &str,
        uri: &Url,
        include_declaration: bool,
        locations: &mut Vec<Location>,
    ) {
        if include_declaration && instance_decl.name.name == ident {
            locations.push(Location::new(
                uri.clone(),
                Self::span_to_range(instance_decl.name.span.clone()),
            ));
        }
        for param in instance_decl.params.iter() {
            Self::collect_type_expr_references(param, ident, uri, locations);
        }
        for def in instance_decl.defs.iter() {
            Self::collect_def_references(def, ident, uri, include_declaration, locations);
        }
    }

    fn collect_domain_references(
        domain_decl: &DomainDecl,
        ident: &str,
        uri: &Url,
        include_declaration: bool,
        locations: &mut Vec<Location>,
    ) {
        if include_declaration && domain_decl.name.name == ident {
            locations.push(Location::new(
                uri.clone(),
                Self::span_to_range(domain_decl.name.span.clone()),
            ));
        }
        Self::collect_type_expr_references(&domain_decl.over, ident, uri, locations);
        for item in domain_decl.items.iter() {
            match item {
                DomainItem::TypeAlias(decl) => {
                    Self::collect_type_decl_references(
                        decl,
                        ident,
                        uri,
                        include_declaration,
                        locations,
                    );
                }
                DomainItem::TypeSig(_) => {}
                DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                    Self::collect_def_references(def, ident, uri, include_declaration, locations);
                }
            }
        }
    }

    fn collect_type_ctor_references(
        ctor: &TypeCtor,
        ident: &str,
        uri: &Url,
        include_declaration: bool,
        locations: &mut Vec<Location>,
    ) {
        if include_declaration && ctor.name.name == ident {
            locations.push(Location::new(
                uri.clone(),
                Self::span_to_range(ctor.name.span.clone()),
            ));
        }
        for arg in ctor.args.iter() {
            Self::collect_type_expr_references(arg, ident, uri, locations);
        }
    }

    fn collect_type_expr_references(
        expr: &TypeExpr,
        ident: &str,
        uri: &Url,
        locations: &mut Vec<Location>,
    ) {
        match expr {
            TypeExpr::Name(name) => {
                if name.name == ident {
                    locations.push(Location::new(
                        uri.clone(),
                        Self::span_to_range(name.span.clone()),
                    ));
                }
            }
            TypeExpr::Apply { base, args, .. } => {
                Self::collect_type_expr_references(base, ident, uri, locations);
                for arg in args.iter() {
                    Self::collect_type_expr_references(arg, ident, uri, locations);
                }
            }
            TypeExpr::Func { params, result, .. } => {
                for param in params.iter() {
                    Self::collect_type_expr_references(param, ident, uri, locations);
                }
                Self::collect_type_expr_references(result, ident, uri, locations);
            }
            TypeExpr::Record { fields, .. } => {
                for (name, ty) in fields.iter() {
                    if name.name == ident {
                        locations.push(Location::new(
                            uri.clone(),
                            Self::span_to_range(name.span.clone()),
                        ));
                    }
                    Self::collect_type_expr_references(ty, ident, uri, locations);
                }
            }
            TypeExpr::Tuple { items, .. } => {
                for item in items.iter() {
                    Self::collect_type_expr_references(item, ident, uri, locations);
                }
            }
            TypeExpr::Star { .. } | TypeExpr::Unknown { .. } => {}
        }
    }

    fn collect_pattern_references(
        pattern: &Pattern,
        ident: &str,
        uri: &Url,
        locations: &mut Vec<Location>,
    ) {
        match pattern {
            Pattern::Ident(name) => {
                if name.name == ident {
                    locations.push(Location::new(
                        uri.clone(),
                        Self::span_to_range(name.span.clone()),
                    ));
                }
            }
            Pattern::Constructor { name, args, .. } => {
                if name.name == ident {
                    locations.push(Location::new(
                        uri.clone(),
                        Self::span_to_range(name.span.clone()),
                    ));
                }
                for arg in args.iter() {
                    Self::collect_pattern_references(arg, ident, uri, locations);
                }
            }
            Pattern::Tuple { items, .. } => {
                for item in items.iter() {
                    Self::collect_pattern_references(item, ident, uri, locations);
                }
            }
            Pattern::List { items, rest, .. } => {
                for item in items.iter() {
                    Self::collect_pattern_references(item, ident, uri, locations);
                }
                if let Some(rest) = rest {
                    Self::collect_pattern_references(rest, ident, uri, locations);
                }
            }
            Pattern::Record { fields, .. } => {
                for field in fields.iter() {
                    Self::collect_record_pattern_references(field, ident, uri, locations);
                }
            }
            Pattern::Wildcard(_) | Pattern::Literal(_) => {}
        }
    }

    fn collect_record_pattern_references(
        field: &RecordPatternField,
        ident: &str,
        uri: &Url,
        locations: &mut Vec<Location>,
    ) {
        for segment in field.path.iter() {
            if segment.name == ident {
                locations.push(Location::new(
                    uri.clone(),
                    Self::span_to_range(segment.span.clone()),
                ));
            }
        }
        Self::collect_pattern_references(&field.pattern, ident, uri, locations);
    }

    fn collect_expr_references(expr: &Expr, ident: &str, uri: &Url, locations: &mut Vec<Location>) {
        match expr {
            Expr::TextInterpolate { parts, .. } => {
                for part in parts {
                    if let aivi::TextPart::Expr { expr, .. } = part {
                        Self::collect_expr_references(expr, ident, uri, locations);
                    }
                }
            }
            Expr::Ident(name) => {
                if name.name == ident {
                    locations.push(Location::new(
                        uri.clone(),
                        Self::span_to_range(name.span.clone()),
                    ));
                }
            }
            Expr::Literal(_) => {}
            Expr::List { items, .. } => {
                for item in items.iter() {
                    Self::collect_list_item_references(item, ident, uri, locations);
                }
            }
            Expr::Tuple { items, .. } => {
                for item in items.iter() {
                    Self::collect_expr_references(item, ident, uri, locations);
                }
            }
            Expr::Record { fields, .. } => {
                for field in fields.iter() {
                    Self::collect_record_field_references(field, ident, uri, locations);
                }
            }
            Expr::FieldAccess { base, field, .. } => {
                Self::collect_expr_references(base, ident, uri, locations);
                if field.name == ident {
                    locations.push(Location::new(
                        uri.clone(),
                        Self::span_to_range(field.span.clone()),
                    ));
                }
            }
            Expr::FieldSection { field, .. } => {
                if field.name == ident {
                    locations.push(Location::new(
                        uri.clone(),
                        Self::span_to_range(field.span.clone()),
                    ));
                }
            }
            Expr::Index { base, index, .. } => {
                Self::collect_expr_references(base, ident, uri, locations);
                Self::collect_expr_references(index, ident, uri, locations);
            }
            Expr::Call { func, args, .. } => {
                Self::collect_expr_references(func, ident, uri, locations);
                for arg in args.iter() {
                    Self::collect_expr_references(arg, ident, uri, locations);
                }
            }
            Expr::Lambda { params, body, .. } => {
                for param in params.iter() {
                    Self::collect_pattern_references(param, ident, uri, locations);
                }
                Self::collect_expr_references(body, ident, uri, locations);
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                if let Some(scrutinee) = scrutinee {
                    Self::collect_expr_references(scrutinee, ident, uri, locations);
                }
                for arm in arms.iter() {
                    Self::collect_match_arm_references(arm, ident, uri, locations);
                }
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                Self::collect_expr_references(cond, ident, uri, locations);
                Self::collect_expr_references(then_branch, ident, uri, locations);
                Self::collect_expr_references(else_branch, ident, uri, locations);
            }
            Expr::Binary { left, right, .. } => {
                Self::collect_expr_references(left, ident, uri, locations);
                Self::collect_expr_references(right, ident, uri, locations);
            }
            Expr::Block { items, .. } => {
                for item in items.iter() {
                    Self::collect_block_item_references(item, ident, uri, locations);
                }
            }
            Expr::Raw { .. } => {}
        }
    }

    fn collect_list_item_references(
        item: &ListItem,
        ident: &str,
        uri: &Url,
        locations: &mut Vec<Location>,
    ) {
        Self::collect_expr_references(&item.expr, ident, uri, locations);
    }

    fn collect_record_field_references(
        field: &RecordField,
        ident: &str,
        uri: &Url,
        locations: &mut Vec<Location>,
    ) {
        for segment in field.path.iter() {
            Self::collect_path_segment_references(segment, ident, uri, locations);
        }
        Self::collect_expr_references(&field.value, ident, uri, locations);
    }

    fn collect_path_segment_references(
        segment: &PathSegment,
        ident: &str,
        uri: &Url,
        locations: &mut Vec<Location>,
    ) {
        match segment {
            PathSegment::Field(name) => {
                if name.name == ident {
                    locations.push(Location::new(
                        uri.clone(),
                        Self::span_to_range(name.span.clone()),
                    ));
                }
            }
            PathSegment::Index(expr, _) => {
                Self::collect_expr_references(expr, ident, uri, locations);
            }
        }
    }

    fn collect_match_arm_references(
        arm: &MatchArm,
        ident: &str,
        uri: &Url,
        locations: &mut Vec<Location>,
    ) {
        Self::collect_pattern_references(&arm.pattern, ident, uri, locations);
        if let Some(guard) = &arm.guard {
            Self::collect_expr_references(guard, ident, uri, locations);
        }
        Self::collect_expr_references(&arm.body, ident, uri, locations);
    }

    fn collect_block_item_references(
        item: &BlockItem,
        ident: &str,
        uri: &Url,
        locations: &mut Vec<Location>,
    ) {
        match item {
            BlockItem::Bind { pattern, expr, .. } => {
                Self::collect_pattern_references(pattern, ident, uri, locations);
                Self::collect_expr_references(expr, ident, uri, locations);
            }
            BlockItem::Filter { expr, .. }
            | BlockItem::Yield { expr, .. }
            | BlockItem::Recurse { expr, .. }
            | BlockItem::Expr { expr, .. } => {
                Self::collect_expr_references(expr, ident, uri, locations);
            }
        }
    }

    fn format_type_decl(decl: &TypeDecl) -> String {
        let params = Self::format_type_params(&decl.params);
        let ctors = decl
            .constructors
            .iter()
            .map(Self::format_type_ctor)
            .collect::<Vec<_>>()
            .join(" | ");
        if ctors.is_empty() {
            format!("type {}{}", decl.name.name, params)
        } else {
            format!("type {}{} = {}", decl.name.name, params, ctors)
        }
    }

    fn format_type_alias(alias: &TypeAlias) -> String {
        let params = Self::format_type_params(&alias.params);
        let aliased = Self::type_expr_to_string(&alias.aliased);
        format!("type {}{} = {}", alias.name.name, params, aliased)
    }

    fn format_class_decl(class_decl: &ClassDecl) -> String {
        let params = class_decl
            .params
            .iter()
            .map(Self::type_expr_to_string)
            .collect::<Vec<_>>()
            .join(" ");
        if params.is_empty() {
            format!("class {}", class_decl.name.name)
        } else {
            format!("class {} {}", class_decl.name.name, params)
        }
    }

    fn format_instance_decl(instance_decl: &InstanceDecl) -> String {
        let params = instance_decl
            .params
            .iter()
            .map(Self::type_expr_to_string)
            .collect::<Vec<_>>()
            .join(" ");
        if params.is_empty() {
            format!("instance {}", instance_decl.name.name)
        } else {
            format!("instance {} {}", instance_decl.name.name, params)
        }
    }

    fn format_type_ctor(ctor: &TypeCtor) -> String {
        let args = ctor
            .args
            .iter()
            .map(Self::type_expr_to_string)
            .collect::<Vec<_>>()
            .join(" ");
        if args.is_empty() {
            ctor.name.name.clone()
        } else {
            format!("{} {}", ctor.name.name, args)
        }
    }

    fn format_type_params(params: &[SpannedName]) -> String {
        if params.is_empty() {
            String::new()
        } else {
            format!(
                " {}",
                params
                    .iter()
                    .map(|param| param.name.clone())
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        }
    }

    pub(super) fn type_expr_to_string(expr: &TypeExpr) -> String {
        match expr {
            TypeExpr::Name(name) => name.name.clone(),
            TypeExpr::Apply { base, args, .. } => {
                let base_str = match **base {
                    TypeExpr::Func { .. } => format!("({})", Self::type_expr_to_string(base)),
                    _ => Self::type_expr_to_string(base),
                };
                if args.is_empty() {
                    base_str
                } else {
                    let args_str = args
                        .iter()
                        .map(Self::type_expr_to_string)
                        .collect::<Vec<_>>()
                        .join(" ");
                    format!("{} {}", base_str, args_str)
                }
            }
            TypeExpr::Func { params, result, .. } => {
                let params_str = params
                    .iter()
                    .map(|param| match param {
                        TypeExpr::Func { .. } => format!("({})", Self::type_expr_to_string(param)),
                        _ => Self::type_expr_to_string(param),
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                let result_str = Self::type_expr_to_string(result);
                if params_str.is_empty() {
                    format!("-> {}", result_str)
                } else {
                    format!("{} -> {}", params_str, result_str)
                }
            }
            TypeExpr::Record { fields, .. } => {
                let fields_str = fields
                    .iter()
                    .map(|(name, ty)| format!("{}: {}", name.name, Self::type_expr_to_string(ty)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{{}}}", fields_str)
            }
            TypeExpr::Tuple { items, .. } => {
                let items_str = items
                    .iter()
                    .map(Self::type_expr_to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({})", items_str)
            }
            TypeExpr::Star { .. } | TypeExpr::Unknown { .. } => "*".to_string(),
        }
    }

    pub(super) fn module_member_definition_range(module: &Module, ident: &str) -> Option<Range> {
        for item in module.items.iter() {
            match item {
                ModuleItem::Def(def) => {
                    if def.name.name == ident {
                        return Some(Self::span_to_range(def.name.span.clone()));
                    }
                }
                ModuleItem::TypeSig(sig) => {
                    if sig.name.name == ident {
                        return Some(Self::span_to_range(sig.name.span.clone()));
                    }
                }
                ModuleItem::TypeDecl(decl) => {
                    if decl.name.name == ident {
                        return Some(Self::span_to_range(decl.name.span.clone()));
                    }
                    for ctor in decl.constructors.iter() {
                        if ctor.name.name == ident {
                            return Some(Self::span_to_range(ctor.name.span.clone()));
                        }
                    }
                }
                ModuleItem::TypeAlias(alias) => {
                    if alias.name.name == ident {
                        return Some(Self::span_to_range(alias.name.span.clone()));
                    }
                }
                ModuleItem::ClassDecl(class_decl) => {
                    if class_decl.name.name == ident {
                        return Some(Self::span_to_range(class_decl.name.span.clone()));
                    }
                    for member in class_decl.members.iter() {
                        if member.name.name == ident {
                            return Some(Self::span_to_range(member.name.span.clone()));
                        }
                    }
                }
                ModuleItem::InstanceDecl(instance_decl) => {
                    if instance_decl.name.name == ident {
                        return Some(Self::span_to_range(instance_decl.name.span.clone()));
                    }
                    for def in instance_decl.defs.iter() {
                        if def.name.name == ident {
                            return Some(Self::span_to_range(def.name.span.clone()));
                        }
                    }
                }
                ModuleItem::DomainDecl(domain_decl) => {
                    if domain_decl.name.name == ident {
                        return Some(Self::span_to_range(domain_decl.name.span.clone()));
                    }
                    for domain_item in domain_decl.items.iter() {
                        match domain_item {
                            DomainItem::TypeAlias(type_decl) => {
                                if type_decl.name.name == ident {
                                    return Some(Self::span_to_range(type_decl.name.span.clone()));
                                }
                                for ctor in type_decl.constructors.iter() {
                                    if ctor.name.name == ident {
                                        return Some(Self::span_to_range(ctor.name.span.clone()));
                                    }
                                }
                            }
                            DomainItem::TypeSig(_) => {}
                            DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                                if def.name.name == ident {
                                    return Some(Self::span_to_range(def.name.span.clone()));
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub(super) fn module_at_position(modules: &[Module], position: Position) -> Option<&Module> {
        modules.iter().find(|module| {
            let range = Self::span_to_range(module.span.clone());
            Self::range_contains_position(&range, position)
        })
    }

    pub(super) fn range_contains_position(range: &Range, position: Position) -> bool {
        let after_start = position.line > range.start.line
            || (position.line == range.start.line && position.character >= range.start.character);
        let before_end = position.line < range.end.line
            || (position.line == range.end.line && position.character < range.end.character);
        after_start && before_end
    }

    pub(super) fn path_from_uri(uri: &Url) -> String {
        uri.to_file_path()
            .unwrap_or_else(|_| PathBuf::from(uri.to_string()))
            .display()
            .to_string()
    }

    pub(super) fn stdlib_uri(name: &str) -> Url {
        Url::parse(&format!("aivi://stdlib/{name}"))
            .expect("stdlib uri should be valid")
    }
}
