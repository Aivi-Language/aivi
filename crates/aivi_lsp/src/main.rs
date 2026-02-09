use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use aivi::{
    parse_modules, BlockItem, ClassDecl, Def, DomainDecl, DomainItem, Expr, InstanceDecl, ListItem,
    MatchArm, Module, ModuleItem, PathSegment, Pattern, RecordField, RecordPatternField, Span,
    SpannedName, TypeAlias, TypeCtor, TypeDecl, TypeExpr, UseDecl,
};
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse, DeclarationCapability,
    Diagnostic, DiagnosticSeverity, DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverContents, HoverParams,
    HoverProviderCapability, ImplementationProviderCapability, InitializeParams, InitializeResult,
    InitializedParams, Location, MarkupContent, MarkupKind, OneOf, Position, Range,
    ReferenceParams, ServerCapabilities, SymbolKind, TextDocumentPositionParams,
    TextDocumentSyncCapability, TextDocumentSyncKind, Url,
};
use tower_lsp::lsp_types::request::{GotoDeclarationParams, GotoDeclarationResponse, GotoImplementationParams, GotoImplementationResponse};
use tower_lsp::{Client, LanguageServer, LspService, Server};

#[derive(Default)]
struct DocumentState {
    text: String,
    version: Option<i32>,
}

#[derive(Default)]
struct BackendState {
    documents: HashMap<Url, DocumentState>,
}

struct Backend {
    client: Client,
    state: Arc<Mutex<BackendState>>,
}

impl Backend {
    const KEYWORDS: [&'static str; 19] = [
        "module", "export", "use", "type", "alias", "class", "instance", "domain",
        "def", "let", "if", "else", "match", "case", "when", "true", "false", "in",
        "where",
    ];
    const SIGILS: [&'static str; 4] = ["~r//", "~u()", "~d()", "~dt()"];

    fn span_to_range(span: Span) -> Range {
        let start_line = span.start.line.saturating_sub(1) as u32;
        let start_char = span.start.column.saturating_sub(1) as u32;
        let end_line = span.end.line.saturating_sub(1) as u32;
        let end_char = span.end.column as u32;
        Range::new(Position::new(start_line, start_char), Position::new(end_line, end_char))
    }

    fn offset_at(text: &str, position: Position) -> usize {
        let mut offset = 0usize;
        let mut line = 0u32;
        for chunk in text.split_inclusive('\n') {
            if line == position.line {
                let char_offset = position.character as usize;
                return offset + chunk.chars().take(char_offset).map(|c| c.len_utf8()).sum::<usize>();
            }
            offset += chunk.len();
            line += 1;
        }
        offset
    }

    fn extract_identifier(text: &str, position: Position) -> Option<String> {
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

    fn build_definition(text: &str, uri: &Url, position: Position) -> Option<Location> {
        let ident = Self::extract_identifier(text, position)?;
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        for module in modules {
            if module.name.name == ident {
                let range = Self::span_to_range(module.name.span);
                return Some(Location::new(uri.clone(), range));
            }
            for export in module.exports.iter() {
                if export.name == ident {
                    let range = Self::span_to_range(export.span.clone());
                    return Some(Location::new(uri.clone(), range));
                }
            }
            for item in module.items.iter() {
                if let Some(range) = Self::item_definition_range(item, &ident) {
                    return Some(Location::new(uri.clone(), range));
                }
            }
        }
        None
    }

    fn build_hover(text: &str, uri: &Url, position: Position) -> Option<Hover> {
        let ident = Self::extract_identifier(text, position)?;
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        for module in modules {
            if let Some(contents) = Self::hover_contents_for_module(&module, &ident) {
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

    fn build_references(
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

    fn hover_contents_for_module(module: &Module, ident: &str) -> Option<String> {
        if module.name.name == ident {
            return Some(format!("module `{}`", module.name.name));
        }
        let mut type_signatures = HashMap::new();
        for item in module.items.iter() {
            if let ModuleItem::TypeSig(sig) = item {
                type_signatures.insert(
                    sig.name.name.clone(),
                    format!("`{}` : `{}`", sig.name.name, Self::type_expr_to_string(&sig.ty)),
                );
            }
        }
        if let Some(sig) = type_signatures.get(ident) {
            return Some(sig.clone());
        }
        for item in module.items.iter() {
            if let Some(contents) = Self::hover_contents_for_item(item, ident, &type_signatures) {
                return Some(contents);
            }
        }
        for domain in module
            .items
            .iter()
            .filter_map(|item| match item {
                ModuleItem::DomainDecl(domain) => Some(domain),
                _ => None,
            })
        {
            if let Some(contents) = Self::hover_contents_for_domain(domain, ident) {
                return Some(contents);
            }
        }
        None
    }

    fn hover_contents_for_item(
        item: &ModuleItem,
        ident: &str,
        type_signatures: &HashMap<String, String>,
    ) -> Option<String> {
        match item {
            ModuleItem::Def(def) => {
                if def.name.name == ident {
                    let fallback = format!("`{}`", def.name.name);
                    return Some(type_signatures.get(ident).cloned().unwrap_or(fallback));
                }
            }
            ModuleItem::TypeSig(sig) => {
                if sig.name.name == ident {
                    return Some(format!("`{}` : `{}`", sig.name.name, Self::type_expr_to_string(&sig.ty)));
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

    fn hover_contents_for_domain(domain_decl: &DomainDecl, ident: &str) -> Option<String> {
        for item in domain_decl.items.iter() {
            match item {
                DomainItem::TypeAlias(type_decl) => {
                    if type_decl.name.name == ident {
                        return Some(format!("`{}`", Self::format_type_decl(type_decl)));
                    }
                }
                DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                    if def.name.name == ident {
                        return Some(format!("`{}`", def.name.name));
                    }
                }
            }
        }
        None
    }

    fn collect_module_references(
        module: &Module,
        ident: &str,
        uri: &Url,
        include_declaration: bool,
        locations: &mut Vec<Location>,
    ) {
        if include_declaration && module.name.name == ident {
            locations.push(Location::new(uri.clone(), Self::span_to_range(module.name.span.clone())));
        }
        for export in module.exports.iter() {
            if export.name == ident {
                locations.push(Location::new(uri.clone(), Self::span_to_range(export.span.clone())));
            }
        }
        for annotation in module.annotations.iter() {
            if annotation.name == ident {
                locations.push(Location::new(uri.clone(), Self::span_to_range(annotation.span.clone())));
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
            locations.push(Location::new(uri.clone(), Self::span_to_range(use_decl.module.span.clone())));
        }
        for item in use_decl.items.iter() {
            if item.name == ident {
                locations.push(Location::new(uri.clone(), Self::span_to_range(item.span.clone())));
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
                    locations.push(Location::new(uri.clone(), Self::span_to_range(sig.name.span.clone())));
                }
                Self::collect_type_expr_references(&sig.ty, ident, uri, locations);
            }
            ModuleItem::TypeDecl(decl) => {
                Self::collect_type_decl_references(decl, ident, uri, include_declaration, locations);
            }
            ModuleItem::TypeAlias(alias) => {
                Self::collect_type_alias_references(alias, ident, uri, include_declaration, locations);
            }
            ModuleItem::ClassDecl(class_decl) => {
                Self::collect_class_references(class_decl, ident, uri, include_declaration, locations);
            }
            ModuleItem::InstanceDecl(instance_decl) => {
                Self::collect_instance_references(instance_decl, ident, uri, include_declaration, locations);
            }
            ModuleItem::DomainDecl(domain_decl) => {
                Self::collect_domain_references(domain_decl, ident, uri, include_declaration, locations);
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
            locations.push(Location::new(uri.clone(), Self::span_to_range(def.name.span.clone())));
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
            locations.push(Location::new(uri.clone(), Self::span_to_range(decl.name.span.clone())));
        }
        for param in decl.params.iter() {
            if param.name == ident {
                locations.push(Location::new(uri.clone(), Self::span_to_range(param.span.clone())));
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
            locations.push(Location::new(uri.clone(), Self::span_to_range(alias.name.span.clone())));
        }
        for param in alias.params.iter() {
            if param.name == ident {
                locations.push(Location::new(uri.clone(), Self::span_to_range(param.span.clone())));
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
            locations.push(Location::new(uri.clone(), Self::span_to_range(class_decl.name.span.clone())));
        }
        for param in class_decl.params.iter() {
            Self::collect_type_expr_references(param, ident, uri, locations);
        }
        for member in class_decl.members.iter() {
            if include_declaration && member.name.name == ident {
                locations.push(Location::new(uri.clone(), Self::span_to_range(member.name.span.clone())));
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
            locations.push(Location::new(uri.clone(), Self::span_to_range(instance_decl.name.span.clone())));
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
            locations.push(Location::new(uri.clone(), Self::span_to_range(domain_decl.name.span.clone())));
        }
        Self::collect_type_expr_references(&domain_decl.over, ident, uri, locations);
        for item in domain_decl.items.iter() {
            match item {
                DomainItem::TypeAlias(decl) => {
                    Self::collect_type_decl_references(decl, ident, uri, include_declaration, locations);
                }
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
            locations.push(Location::new(uri.clone(), Self::span_to_range(ctor.name.span.clone())));
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
                    locations.push(Location::new(uri.clone(), Self::span_to_range(name.span.clone())));
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
                        locations.push(Location::new(uri.clone(), Self::span_to_range(name.span.clone())));
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
                    locations.push(Location::new(uri.clone(), Self::span_to_range(name.span.clone())));
                }
            }
            Pattern::Constructor { name, args, .. } => {
                if name.name == ident {
                    locations.push(Location::new(uri.clone(), Self::span_to_range(name.span.clone())));
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
                locations.push(Location::new(uri.clone(), Self::span_to_range(segment.span.clone())));
            }
        }
        Self::collect_pattern_references(&field.pattern, ident, uri, locations);
    }

    fn collect_expr_references(
        expr: &Expr,
        ident: &str,
        uri: &Url,
        locations: &mut Vec<Location>,
    ) {
        match expr {
            Expr::Ident(name) => {
                if name.name == ident {
                    locations.push(Location::new(uri.clone(), Self::span_to_range(name.span.clone())));
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
                    locations.push(Location::new(uri.clone(), Self::span_to_range(field.span.clone())));
                }
            }
            Expr::FieldSection { field, .. } => {
                if field.name == ident {
                    locations.push(Location::new(uri.clone(), Self::span_to_range(field.span.clone())));
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
            Expr::Match { scrutinee, arms, .. } => {
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
                    locations.push(Location::new(uri.clone(), Self::span_to_range(name.span.clone())));
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
            format!(" {}", params.iter().map(|param| param.name.clone()).collect::<Vec<_>>().join(" "))
        }
    }

    fn type_expr_to_string(expr: &TypeExpr) -> String {
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

    fn item_definition_range(item: &ModuleItem, ident: &str) -> Option<Range> {
        match item {
            ModuleItem::Def(def) if def.name.name == ident => Some(Self::span_to_range(def.name.span.clone())),
            ModuleItem::TypeSig(sig) if sig.name.name == ident => Some(Self::span_to_range(sig.name.span.clone())),
            ModuleItem::TypeDecl(decl) if decl.name.name == ident => Some(Self::span_to_range(decl.name.span.clone())),
            ModuleItem::TypeAlias(alias) if alias.name.name == ident => Some(Self::span_to_range(alias.name.span.clone())),
            ModuleItem::ClassDecl(class_decl) if class_decl.name.name == ident => {
                Some(Self::span_to_range(class_decl.name.span.clone()))
            }
            ModuleItem::InstanceDecl(instance_decl) if instance_decl.name.name == ident => {
                Some(Self::span_to_range(instance_decl.name.span.clone()))
            }
            ModuleItem::DomainDecl(domain_decl) if domain_decl.name.name == ident => {
                Some(Self::span_to_range(domain_decl.name.span.clone()))
            }
            _ => None,
        }
    }

    fn build_completion_items(text: &str, uri: &Url) -> Vec<CompletionItem> {
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        let mut items = Vec::new();
        for keyword in Self::KEYWORDS {
            items.push(CompletionItem {
                label: keyword.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                ..CompletionItem::default()
            });
        }
        for sigil in Self::SIGILS {
            items.push(CompletionItem {
                label: sigil.to_string(),
                kind: Some(CompletionItemKind::SNIPPET),
                ..CompletionItem::default()
            });
        }
        for module in modules {
            items.push(CompletionItem {
                label: module.name.name.clone(),
                kind: Some(CompletionItemKind::MODULE),
                ..CompletionItem::default()
            });
            for export in module.exports {
                items.push(CompletionItem {
                    label: export.name,
                    kind: Some(CompletionItemKind::PROPERTY),
                    ..CompletionItem::default()
                });
            }
            for item in module.items {
                if let Some((label, kind)) = Self::completion_from_item(item) {
                    items.push(CompletionItem {
                        label,
                        kind: Some(kind),
                        ..CompletionItem::default()
                    });
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
            ModuleItem::TypeAlias(alias) => Some((alias.name.name, CompletionItemKind::TYPE_PARAMETER)),
            ModuleItem::ClassDecl(class_decl) => Some((class_decl.name.name, CompletionItemKind::CLASS)),
            ModuleItem::InstanceDecl(instance_decl) => Some((instance_decl.name.name, CompletionItemKind::VARIABLE)),
            ModuleItem::DomainDecl(domain_decl) => Some((domain_decl.name.name, CompletionItemKind::MODULE)),
        }
    }

    fn path_from_uri(uri: &Url) -> String {
        uri.to_file_path()
            .unwrap_or_else(|_| PathBuf::from(uri.to_string()))
            .display()
            .to_string()
    }

    fn build_diagnostics(text: &str, uri: &Url) -> Vec<Diagnostic> {
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (_, diagnostics) = parse_modules(&path, text);
        diagnostics
            .into_iter()
            .map(|file_diag| Diagnostic {
                range: Self::span_to_range(file_diag.diagnostic.span),
                severity: Some(DiagnosticSeverity::ERROR),
                code: None,
                code_description: None,
                source: Some("aivi".to_string()),
                message: file_diag.diagnostic.message,
                related_information: None,
                tags: None,
                data: None,
            })
            .collect()
    }

    fn build_document_symbols(text: &str, uri: &Url) -> Vec<DocumentSymbol> {
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        modules
            .into_iter()
            .map(|module| {
                let mut children = Vec::new();
                for export in module.exports {
                    let range = Self::span_to_range(export.span);
                    children.push(DocumentSymbol {
                        name: export.name,
                        detail: Some("export".to_string()),
                        kind: SymbolKind::PROPERTY,
                        tags: None,
                        deprecated: None,
                        range,
                        selection_range: range,
                        children: None,
                    });
                }
                for item in module.items {
                    if let Some(symbol) = Self::symbol_from_item(item) {
                        children.push(symbol);
                    }
                }
                let range = Self::span_to_range(module.span);
                DocumentSymbol {
                    name: module.name.name,
                    detail: Some("module".to_string()),
                    kind: SymbolKind::MODULE,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range: range,
                    children: Some(children),
                }
            })
            .collect()
    }

    fn symbol_from_item(item: ModuleItem) -> Option<DocumentSymbol> {
        match item {
            ModuleItem::Def(def) => {
                let range = Self::span_to_range(def.span);
                Some(DocumentSymbol {
                    name: def.name.name,
                    detail: None,
                    kind: SymbolKind::FUNCTION,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range: range,
                    children: None,
                })
            }
            ModuleItem::TypeSig(sig) => {
                let range = Self::span_to_range(sig.span);
                Some(DocumentSymbol {
                    name: sig.name.name,
                    detail: Some("type signature".to_string()),
                    kind: SymbolKind::FUNCTION,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range: range,
                    children: None,
                })
            }
            ModuleItem::TypeDecl(decl) => {
                let range = Self::span_to_range(decl.span);
                Some(DocumentSymbol {
                    name: decl.name.name,
                    detail: Some("type".to_string()),
                    kind: SymbolKind::STRUCT,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range: range,
                    children: None,
                })
            }
            ModuleItem::TypeAlias(alias) => {
                let range = Self::span_to_range(alias.span);
                Some(DocumentSymbol {
                    name: alias.name.name,
                    detail: Some("type alias".to_string()),
                    kind: SymbolKind::TYPE_PARAMETER,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range: range,
                    children: None,
                })
            }
            ModuleItem::ClassDecl(class_decl) => {
                let range = Self::span_to_range(class_decl.span);
                Some(DocumentSymbol {
                    name: class_decl.name.name,
                    detail: Some("class".to_string()),
                    kind: SymbolKind::CLASS,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range: range,
                    children: None,
                })
            }
            ModuleItem::InstanceDecl(instance_decl) => {
                let range = Self::span_to_range(instance_decl.span);
                Some(DocumentSymbol {
                    name: instance_decl.name.name,
                    detail: Some("instance".to_string()),
                    kind: SymbolKind::OBJECT,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range: range,
                    children: None,
                })
            }
            ModuleItem::DomainDecl(domain_decl) => {
                let mut children = Vec::new();
                for domain_item in domain_decl.items {
                    match domain_item {
                        DomainItem::TypeAlias(type_alias) => {
                            let range = Self::span_to_range(type_alias.span);
                            children.push(DocumentSymbol {
                                name: type_alias.name.name,
                                detail: Some("domain type".to_string()),
                                kind: SymbolKind::TYPE_PARAMETER,
                                tags: None,
                                deprecated: None,
                                range,
                                selection_range: range,
                                children: None,
                            });
                        }
                        DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                            let range = Self::span_to_range(def.span);
                            children.push(DocumentSymbol {
                                name: def.name.name,
                                detail: Some("domain def".to_string()),
                                kind: SymbolKind::FUNCTION,
                                tags: None,
                                deprecated: None,
                                range,
                                selection_range: range,
                                children: None,
                            });
                        }
                    }
                }
                let range = Self::span_to_range(domain_decl.span);
                Some(DocumentSymbol {
                    name: domain_decl.name.name,
                    detail: Some("domain".to_string()),
                    kind: SymbolKind::NAMESPACE,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range: range,
                    children: Some(children),
                })
            }
        }
    }

    async fn update_document(&self, uri: Url, text: String, version: Option<i32>) {
        let mut state = self.state.lock().await;
        state.documents.insert(uri, DocumentState { text, version });
    }

    async fn with_document_text<F, R>(&self, uri: &Url, f: F) -> Option<R>
    where
        F: FnOnce(&str) -> R,
    {
        let state = self.state.lock().await;
        state.documents.get(uri).map(|doc| f(&doc.text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_text() -> &'static str {
        r#"@no_prelude
module examples.compiler.math = {
  export add, sub

  add : Number -> Number -> Number
  sub : Number -> Number -> Number

  add = x y => x + y
  sub = x y => x - y
}

module examples.compiler.app = {
  export run

  use examples.compiler.math (add)

  run = add 1 2
}
"#
    }

    fn sample_uri() -> Url {
        Url::parse("file:///test.aivi").expect("valid test uri")
    }

    fn position_for(text: &str, needle: &str) -> Position {
        let offset = text.find(needle).expect("needle exists");
        let mut line = 0u32;
        let mut column = 0u32;
        for (idx, ch) in text.char_indices() {
            if idx == offset {
                break;
            }
            if ch == '\n' {
                line += 1;
                column = 0;
            } else {
                column += 1;
            }
        }
        Position::new(line, column)
    }

    fn find_symbol_span(text: &str, name: &str) -> Span {
        let path = PathBuf::from("test.aivi");
        let (modules, _) = parse_modules(&path, text);
        for module in modules {
            for export in module.exports.iter() {
                if export.name == name {
                    return export.span.clone();
                }
            }
            for item in module.items {
                if let ModuleItem::Def(def) = item {
                    if def.name.name == name {
                        return def.name.span;
                    }
                }
            }
        }
        panic!("symbol not found: {name}");
    }

    #[test]
    fn completion_items_include_keywords_and_defs() {
        let text = sample_text();
        let uri = sample_uri();
        let items = Backend::build_completion_items(text, &uri);
        let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
        assert!(labels.contains(&"module"));
        assert!(labels.contains(&"examples.compiler.math"));
        assert!(labels.contains(&"add"));
    }

    #[test]
    fn build_definition_resolves_def() {
        let text = sample_text();
        let uri = sample_uri();
        let position = position_for(text, "add 1 2");
        let location = Backend::build_definition(text, &uri, position).expect("definition found");
        let expected_span = find_symbol_span(text, "add");
        let expected_range = Backend::span_to_range(expected_span);
        assert_eq!(location.range, expected_range);
    }

    #[test]
    fn build_hover_reports_type_signature() {
        let text = sample_text();
        let uri = sample_uri();
        let position = position_for(text, "add 1 2");
        let hover = Backend::build_hover(text, &uri, position).expect("hover found");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup hover");
        };
        assert!(markup.value.contains("`add`"));
        assert!(markup.value.contains(":"));
    }

    #[test]
    fn build_references_finds_symbol_mentions() {
        let text = sample_text();
        let uri = sample_uri();
        let position = position_for(text, "add 1 2");
        let locations = Backend::build_references(text, &uri, position, true);
        let expected_span = find_symbol_span(text, "add");
        let expected_range = Backend::span_to_range(expected_span);
        assert!(locations.iter().any(|location| location.range == expected_range));
        assert!(locations.len() >= 2);
    }

    #[test]
    fn build_diagnostics_reports_error() {
        let text = "module broken = {";
        let uri = sample_uri();
        let diagnostics = Backend::build_diagnostics(text, &uri);
        assert!(!diagnostics.is_empty());
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(diagnostics[0].source.as_deref(), Some("aivi"));
    }

    #[test]
    fn document_symbols_include_module_and_children() {
        let text = sample_text();
        let uri = sample_uri();
        let symbols = Backend::build_document_symbols(text, &uri);
        let module = symbols
            .iter()
            .find(|symbol| symbol.name == "examples.compiler.math")
            .expect("module symbol exists");
        let children = module.children.as_ref().expect("module has children");
        let child_names: Vec<&str> = children.iter().map(|child| child.name.as_str()).collect();
        assert!(child_names.contains(&"add"));
        assert!(child_names.contains(&"sub"));
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _params: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
                document_symbol_provider: Some(OneOf::Left(true)),
                definition_provider: Some(OneOf::Left(true)),
                declaration_provider: Some(DeclarationCapability::Simple(true)),
                implementation_provider: Some(ImplementationProviderCapability::Simple(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                references_provider: Some(OneOf::Left(true)),
                completion_provider: Some(tower_lsp::lsp_types::CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: None,
                    ..tower_lsp::lsp_types::CompletionOptions::default()
                }),
                ..ServerCapabilities::default()
            },
            server_info: Some(tower_lsp::lsp_types::ServerInfo {
                name: "aivi-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        self.client
            .log_message(tower_lsp::lsp_types::MessageType::INFO, "aivi-lsp initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: tower_lsp::lsp_types::DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        let version = params.text_document.version;
        self.update_document(uri.clone(), text, Some(version)).await;
        if let Some(diagnostics) = self.with_document_text(&uri, |content| {
            Self::build_diagnostics(content, &uri)
        }).await {
            self.client.publish_diagnostics(uri, diagnostics, Some(version)).await;
        }
    }

    async fn did_change(&self, params: tower_lsp::lsp_types::DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        if let Some(change) = params.content_changes.into_iter().next() {
            self.update_document(uri.clone(), change.text, Some(version)).await;
            if let Some(diagnostics) = self.with_document_text(&uri, |content| {
                Self::build_diagnostics(content, &uri)
            }).await {
                self.client.publish_diagnostics(uri, diagnostics, Some(version)).await;
            }
        }
    }

    async fn did_close(&self, params: tower_lsp::lsp_types::DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        let mut state = self.state.lock().await;
        state.documents.remove(&uri);
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn document_symbol(&self, params: DocumentSymbolParams) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let symbols = self
            .with_document_text(&uri, |content| Self::build_document_symbols(content, &uri))
            .await
            .unwrap_or_default();
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let TextDocumentPositionParams { text_document, position } = params.text_document_position_params;
        let uri = text_document.uri;
        let location = self
            .with_document_text(&uri, |content| Self::build_definition(content, &uri, position))
            .await
            .flatten();
        Ok(location.map(GotoDefinitionResponse::Scalar))
    }

    async fn goto_declaration(
        &self,
        params: GotoDeclarationParams,
    ) -> Result<Option<GotoDeclarationResponse>> {
        let TextDocumentPositionParams { text_document, position } = params.text_document_position_params;
        let uri = text_document.uri;
        let location = self
            .with_document_text(&uri, |content| Self::build_definition(content, &uri, position))
            .await
            .flatten();
        Ok(location.map(GotoDeclarationResponse::Scalar))
    }

    async fn goto_implementation(
        &self,
        params: GotoImplementationParams,
    ) -> Result<Option<GotoImplementationResponse>> {
        let TextDocumentPositionParams { text_document, position } = params.text_document_position_params;
        let uri = text_document.uri;
        let location = self
            .with_document_text(&uri, |content| Self::build_definition(content, &uri, position))
            .await
            .flatten();
        Ok(location.map(GotoImplementationResponse::Scalar))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let TextDocumentPositionParams { text_document, position } = params.text_document_position_params;
        let uri = text_document.uri;
        let hover = self
            .with_document_text(&uri, |content| Self::build_hover(content, &uri, position))
            .await
            .flatten();
        Ok(hover)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let TextDocumentPositionParams { text_document, position } = params.text_document_position;
        let uri = text_document.uri;
        let include_declaration = params.context.include_declaration;
        let locations = self
            .with_document_text(&uri, |content| {
                Self::build_references(content, &uri, position, include_declaration)
            })
            .await
            .unwrap_or_default();
        Ok(Some(locations))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let items = self
            .with_document_text(&uri, |content| Self::build_completion_items(content, &uri))
            .await
            .unwrap_or_default();
        Ok(Some(CompletionResponse::Array(items)))
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(|client| Backend {
        client,
        state: Arc::new(Mutex::new(BackendState::default())),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
