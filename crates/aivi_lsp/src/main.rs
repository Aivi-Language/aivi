use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use aivi::{
    infer_value_types, lex_cst, parse_modules, BlockItem, ClassDecl, CstToken, Def, DomainDecl,
    DomainItem, Expr, InstanceDecl, ListItem, Literal, MatchArm, Module, ModuleItem, PathSegment,
    Pattern, RecordField, RecordPatternField, Span, SpannedName, TypeAlias, TypeCtor, TypeDecl,
    TypeExpr, UseDecl,
};
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, CodeActionProviderCapability,
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse, DeclarationCapability,
    Diagnostic, DiagnosticSeverity, DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverContents, HoverParams,
    HoverProviderCapability, ImplementationProviderCapability, InitializeParams, InitializeResult,
    InitializedParams, Location, MarkupContent, MarkupKind, NumberOrString, OneOf, Position, Range,
    ReferenceParams, RenameParams, SemanticToken, SemanticTokenType, SemanticTokens,
    SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensOptions, SemanticTokensParams,
    SemanticTokensResult, SemanticTokensServerCapabilities, ServerCapabilities, SignatureHelp,
    SignatureHelpOptions, SignatureHelpParams, SignatureInformation, SymbolKind,
    TextDocumentPositionParams, TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit, Url,
    WorkspaceEdit,
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
    workspace_root: Option<PathBuf>,
    open_modules_by_uri: HashMap<Url, Vec<String>>,
    open_module_index: HashMap<String, IndexedModule>,
    disk_module_index: HashMap<String, IndexedModule>,
    disk_index_root: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct IndexedModule {
    uri: Url,
    module: Module,
}

struct CallInfo<'a> {
    func: &'a Expr,
    active_parameter: usize,
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

    const SEM_TOKEN_KEYWORD: u32 = 0;
    const SEM_TOKEN_TYPE: u32 = 1;
    const SEM_TOKEN_FUNCTION: u32 = 2;
    const SEM_TOKEN_VARIABLE: u32 = 3;
    const SEM_TOKEN_NUMBER: u32 = 4;
    const SEM_TOKEN_STRING: u32 = 5;
    const SEM_TOKEN_COMMENT: u32 = 6;
    const SEM_TOKEN_OPERATOR: u32 = 7;
    const SEM_TOKEN_DECORATOR: u32 = 8;

    fn semantic_tokens_legend() -> SemanticTokensLegend {
        SemanticTokensLegend {
            token_types: vec![
                SemanticTokenType::KEYWORD,
                SemanticTokenType::TYPE,
                SemanticTokenType::FUNCTION,
                SemanticTokenType::VARIABLE,
                SemanticTokenType::NUMBER,
                SemanticTokenType::STRING,
                SemanticTokenType::COMMENT,
                SemanticTokenType::OPERATOR,
                SemanticTokenType::DECORATOR,
            ],
            token_modifiers: Vec::new(),
        }
    }

    fn is_operator_symbol(symbol: &str) -> bool {
        matches!(
            symbol,
            "=" | "==" | "!=" | "<" | ">" | "<=" | ">=" | "&&" | "||" | "!" | "?"
                | "??" | "+" | "-" | "*" | "/" | "%" | "->" | "=>" | "|>" | "<|"
                | "::" | ".." | "..." | ":" | "."
        )
    }

    fn classify_semantic_token(
        prev: Option<&CstToken>,
        token: &CstToken,
        next: Option<&CstToken>,
    ) -> Option<u32> {
        match token.kind.as_str() {
            "comment" => Some(Self::SEM_TOKEN_COMMENT),
            "string" => Some(Self::SEM_TOKEN_STRING),
            "sigil" => Some(Self::SEM_TOKEN_STRING),
            "number" => Some(Self::SEM_TOKEN_NUMBER),
            "symbol" => {
                if token.text == "@" {
                    Some(Self::SEM_TOKEN_DECORATOR)
                } else if Self::is_operator_symbol(&token.text) {
                    Some(Self::SEM_TOKEN_OPERATOR)
                } else {
                    None
                }
            }
            "ident" => {
                if Self::KEYWORDS.contains(&token.text.as_str()) {
                    return Some(Self::SEM_TOKEN_KEYWORD);
                }
                if prev.is_some_and(|prev| prev.kind == "symbol" && prev.text == "@") {
                    return Some(Self::SEM_TOKEN_DECORATOR);
                }
                if matches!(next, Some(next) if next.kind == "symbol" && (next.text == ":" || next.text == "="))
                {
                    return Some(Self::SEM_TOKEN_FUNCTION);
                }
                if token.text.chars().next().is_some_and(|ch| ch.is_ascii_uppercase()) {
                    Some(Self::SEM_TOKEN_TYPE)
                } else {
                    Some(Self::SEM_TOKEN_VARIABLE)
                }
            }
            _ => None,
        }
    }

    fn build_semantic_tokens(text: &str) -> SemanticTokens {
        let (tokens, _) = lex_cst(text);
        let significant: Vec<usize> = tokens
            .iter()
            .enumerate()
            .filter(|(_, token)| token.kind != "whitespace")
            .map(|(idx, _)| idx)
            .collect();

        let mut data = Vec::new();
        let mut last_line = 0u32;
        let mut last_start = 0u32;

        for (position, token_index) in significant.iter().copied().enumerate() {
            let token = &tokens[token_index];
            let prev = position
                .checked_sub(1)
                .and_then(|prev| significant.get(prev))
                .map(|idx| &tokens[*idx]);
            let next = significant
                .get(position + 1)
                .map(|idx| &tokens[*idx]);

            let Some(token_type) = Self::classify_semantic_token(prev, token, next) else {
                continue;
            };

            let line = token.span.start.line.saturating_sub(1) as u32;
            let start = token.span.start.column.saturating_sub(1) as u32;
            let len = token
                .span
                .end
                .column
                .saturating_sub(token.span.start.column)
                .saturating_add(1) as u32;
            if len == 0 {
                continue;
            }

            let delta_line = line.saturating_sub(last_line);
            let delta_start = if delta_line == 0 {
                start.saturating_sub(last_start)
            } else {
                start
            };

            data.push(SemanticToken {
                delta_line,
                delta_start,
                length: len,
                token_type,
                token_modifiers_bitset: 0,
            });

            last_line = line;
            last_start = start;
        }

        SemanticTokens {
            result_id: None,
            data,
        }
    }

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

    fn build_definition_with_workspace(
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
            let imported = use_decl.wildcard
                || use_decl.items.iter().any(|item| item.name == ident);
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

    fn build_hover(text: &str, uri: &Url, position: Position) -> Option<Hover> {
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

    fn build_hover_with_workspace(
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
        }

        let doc = Self::doc_for_ident(text, current_module, &ident);
        let inferred_current = inferred.get(&current_module.name.name);
        if let Some(contents) =
            Self::hover_contents_for_module(current_module, &ident, inferred_current, doc.as_deref())
        {
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: contents,
                }),
                range: None,
            });
        }

        for use_decl in current_module.uses.iter() {
            let imported = use_decl.wildcard
                || use_decl.items.iter().any(|item| item.name == ident);
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

    fn build_references_with_workspace(
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

        let origin_module = if Self::module_member_definition_range(current_module, &ident).is_some() {
            Some(current_module.name.name.clone())
        } else {
            current_module
                .uses
                .iter()
                .find(|use_decl| use_decl.wildcard || use_decl.items.iter().any(|item| item.name == ident))
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

    fn build_rename_with_workspace(
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
        let Some(first) = chars.next() else {
            return None;
        };
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

    fn doc_block_above(text: &str, line: usize) -> Option<String> {
        let lines: Vec<&str> = text.lines().collect();
        let Some(mut index) = line.checked_sub(2) else {
            return None;
        };
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

    fn doc_for_ident(text: &str, module: &Module, ident: &str) -> Option<String> {
        let line = Self::decl_line_for_ident(module, ident)?;
        Self::doc_block_above(text, line)
    }

    fn hover_contents_for_module(
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
                    format!("`{}` : `{}`", sig.name.name, Self::type_expr_to_string(&sig.ty)),
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
            for domain in module
                .items
                .iter()
                .filter_map(|item| match item {
                    ModuleItem::DomainDecl(domain) => Some(domain),
                    _ => None,
                })
            {
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
                    format!("`{}` : `{}`", sig.name.name, Self::type_expr_to_string(&sig.ty)),
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

    fn build_signature_help_with_workspace(
        text: &str,
        uri: &Url,
        position: Position,
        workspace_modules: &HashMap<String, IndexedModule>,
    ) -> Option<SignatureHelp> {
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        let current_module = Self::module_at_position(&modules, position)?;

        let call = current_module
            .items
            .iter()
            .find_map(|item| Self::call_info_in_item(item, position))?;

        let callee_name = Self::callee_ident_name(call.func)?;
        let signature_label =
            Self::resolve_type_signature_label(current_module, &callee_name, workspace_modules)?;

        Some(SignatureHelp {
            signatures: vec![SignatureInformation {
                label: signature_label,
                documentation: None,
                parameters: None,
                active_parameter: None,
            }],
            active_signature: Some(0),
            active_parameter: Some(call.active_parameter as u32),
        })
    }

    fn resolve_type_signature_label(
        current_module: &Module,
        ident: &str,
        workspace_modules: &HashMap<String, IndexedModule>,
    ) -> Option<String> {
        if let Some(label) = Self::type_signature_label_in_module(current_module, ident) {
            return Some(label);
        }

        for use_decl in current_module.uses.iter() {
            let imported = use_decl.wildcard
                || use_decl.items.iter().any(|item| item.name == ident);
            if !imported {
                continue;
            }
            let Some(indexed) = workspace_modules.get(&use_decl.module.name) else {
                continue;
            };
            if let Some(label) = Self::type_signature_label_in_module(&indexed.module, ident) {
                return Some(label);
            }
        }

        None
    }

    fn type_signature_label_in_module(module: &Module, ident: &str) -> Option<String> {
        for item in module.items.iter() {
            if let ModuleItem::TypeSig(sig) = item {
                if sig.name.name == ident {
                    return Some(format!(
                        "`{}` : `{}`",
                        sig.name.name,
                        Self::type_expr_to_string(&sig.ty)
                    ));
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

    fn module_member_definition_range(module: &Module, ident: &str) -> Option<Range> {
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

    fn module_at_position<'a>(modules: &'a [Module], position: Position) -> Option<&'a Module> {
        modules.iter().find(|module| {
            let range = Self::span_to_range(module.span.clone());
            Self::range_contains_position(&range, position)
        })
    }

    fn range_contains_position(range: &Range, position: Position) -> bool {
        let after_start = position.line > range.start.line
            || (position.line == range.start.line && position.character >= range.start.character);
        let before_end = position.line < range.end.line
            || (position.line == range.end.line && position.character < range.end.character);
        after_start && before_end
    }

    fn build_workspace_index(root: &Path) -> HashMap<String, IndexedModule> {
        let mut modules = HashMap::new();
        for path in Self::collect_aivi_paths(root) {
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let (file_modules, _) = parse_modules(&path, &text);
            let Ok(uri) = Url::from_file_path(&path) else {
                continue;
            };
            for module in file_modules {
                modules
                    .entry(module.name.name.clone())
                    .or_insert_with(|| IndexedModule {
                        uri: uri.clone(),
                        module,
                    });
            }
        }

        modules
    }

    fn collect_aivi_paths(root: &Path) -> Vec<PathBuf> {
        fn should_skip_dir(name: &str) -> bool {
            matches!(
                name,
                ".git"
                    | "target"
                    | "node_modules"
                    | "dist"
                    | "out"
                    | ".idea"
                    | ".junie"
                    | ".gemini"
            )
        }

        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            let Ok(entries) = fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                        if should_skip_dir(name) {
                            continue;
                        }
                    }
                    walk(&path, out);
                    continue;
                }
                if path.extension().and_then(|e| e.to_str()) == Some("aivi") {
                    out.push(path);
                }
            }
        }

        let mut out = Vec::new();
        walk(root, &mut out);
        out.sort();
        out
    }

    async fn workspace_modules_for(&self, uri: &Url) -> HashMap<String, IndexedModule> {
        let (workspace_root, open_modules, disk_modules, disk_root) = {
            let state = self.state.lock().await;
            (
                state.workspace_root.clone(),
                state.open_module_index.clone(),
                state.disk_module_index.clone(),
                state.disk_index_root.clone(),
            )
        };

        let fallback_root = PathBuf::from(Self::path_from_uri(uri))
            .parent()
            .map(|p| p.to_path_buf());
        let root = workspace_root.or(fallback_root);

        let disk_modules = if let Some(root) = root {
            let needs_rebuild = disk_root.as_ref() != Some(&root) || disk_modules.is_empty();
            if needs_rebuild {
                let indexed = Self::build_workspace_index(&root);
                let mut state = self.state.lock().await;
                state.disk_index_root = Some(root.clone());
                state.disk_module_index = indexed.clone();
                indexed
            } else {
                disk_modules
            }
        } else {
            disk_modules
        };

        let mut merged = disk_modules;
        merged.extend(open_modules);
        merged
    }

    async fn update_document(&self, uri: Url, text: String, version: Option<i32>) {
        let path = PathBuf::from(Self::path_from_uri(&uri));
        let (modules, _) = parse_modules(&path, &text);

        let mut state = self.state.lock().await;

        if let Some(existing) = state.open_modules_by_uri.remove(&uri) {
            for module_name in existing {
                state.open_module_index.remove(&module_name);
            }
        }

        let mut module_names = Vec::new();
        for module in modules {
            module_names.push(module.name.name.clone());
            state.open_module_index.insert(
                module.name.name.clone(),
                IndexedModule {
                    uri: uri.clone(),
                    module,
                },
            );
        }
        state.open_modules_by_uri.insert(uri.clone(), module_names);
        state.documents.insert(uri, DocumentState { text, version });
    }

    async fn remove_document(&self, uri: &Url) {
        let mut state = self.state.lock().await;
        state.documents.remove(uri);
        if let Some(existing) = state.open_modules_by_uri.remove(uri) {
            for module_name in existing {
                state.open_module_index.remove(&module_name);
            }
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

    fn call_info_in_item(item: &ModuleItem, position: Position) -> Option<CallInfo<'_>> {
        match item {
            ModuleItem::Def(def) => Self::find_call_info(&def.expr, position),
            ModuleItem::InstanceDecl(instance_decl) => instance_decl
                .defs
                .iter()
                .find_map(|def| Self::find_call_info(&def.expr, position)),
            ModuleItem::DomainDecl(domain_decl) => domain_decl.items.iter().find_map(|domain_item| {
                match domain_item {
                    DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                        Self::find_call_info(&def.expr, position)
                    }
                    DomainItem::TypeAlias(_) | DomainItem::TypeSig(_) => None,
                }
            }),
            _ => None,
        }
    }

    fn callee_ident_name(expr: &Expr) -> Option<String> {
        match expr {
            Expr::Ident(name) => Some(name.name.clone()),
            Expr::FieldAccess { field, .. } => Some(field.name.clone()),
            _ => None,
        }
    }

    fn find_call_info(expr: &Expr, position: Position) -> Option<CallInfo<'_>> {
        if !Self::expr_contains_position(expr, position) {
            return None;
        }

        match expr {
            Expr::Call { func, args, span: _ } => {
                if let Some(inner) = Self::find_call_info(func, position) {
                    return Some(inner);
                }
                for arg in args.iter() {
                    if let Some(inner) = Self::find_call_info(arg, position) {
                        return Some(inner);
                    }
                }
                let active_parameter = Self::active_call_parameter(args, position);
                Some(CallInfo {
                    func: func.as_ref(),
                    active_parameter,
                })
            }
            Expr::List { items, .. } => items
                .iter()
                .find_map(|item| Self::find_call_info(&item.expr, position)),
            Expr::Tuple { items, .. } => items.iter().find_map(|item| Self::find_call_info(item, position)),
            Expr::Record { fields, .. } => fields
                .iter()
                .find_map(|field| Self::find_call_info(&field.value, position)),
            Expr::FieldAccess { base, .. } => Self::find_call_info(base, position),
            Expr::Index { base, index, .. } => Self::find_call_info(base, position)
                .or_else(|| Self::find_call_info(index, position)),
            Expr::Lambda { body, .. } => Self::find_call_info(body, position),
            Expr::Match { scrutinee, arms, .. } => {
                if let Some(scrutinee) = scrutinee {
                    if let Some(inner) = Self::find_call_info(scrutinee, position) {
                        return Some(inner);
                    }
                }
                for arm in arms.iter() {
                    if let Some(guard) = arm.guard.as_ref() {
                        if let Some(inner) = Self::find_call_info(guard, position) {
                            return Some(inner);
                        }
                    }
                    if let Some(inner) = Self::find_call_info(&arm.body, position) {
                        return Some(inner);
                    }
                }
                None
            }
            Expr::If { cond, then_branch, else_branch, .. } => {
                Self::find_call_info(cond, position)
                    .or_else(|| Self::find_call_info(then_branch, position))
                    .or_else(|| Self::find_call_info(else_branch, position))
            }
            Expr::Binary { left, right, .. } => Self::find_call_info(left, position)
                .or_else(|| Self::find_call_info(right, position)),
            Expr::Block { items, .. } => items.iter().find_map(|item| match item {
                BlockItem::Bind { expr, .. } => Self::find_call_info(expr, position),
                BlockItem::Filter { expr, .. }
                | BlockItem::Yield { expr, .. }
                | BlockItem::Recurse { expr, .. }
                | BlockItem::Expr { expr, .. } => Self::find_call_info(expr, position),
            }),
            Expr::Ident(_)
            | Expr::Literal(_)
            | Expr::FieldSection { .. }
            | Expr::Raw { .. } => None,
        }
    }

    fn active_call_parameter(args: &[Expr], position: Position) -> usize {
        if args.is_empty() {
            return 0;
        }

        for (index, arg) in args.iter().enumerate() {
            if Self::expr_contains_position(arg, position) {
                return index;
            }
        }

        let ended_before = args
            .iter()
            .filter(|arg| Self::expr_ends_before_position(arg, position))
            .count();
        ended_before.min(args.len().saturating_sub(1))
    }

    fn expr_contains_position(expr: &Expr, position: Position) -> bool {
        let range = Self::span_to_range(Self::expr_span(expr).clone());
        Self::range_contains_position(&range, position)
    }

    fn expr_ends_before_position(expr: &Expr, position: Position) -> bool {
        let range = Self::span_to_range(Self::expr_span(expr).clone());
        position.line > range.end.line
            || (position.line == range.end.line && position.character >= range.end.character)
    }

    fn expr_span(expr: &Expr) -> &Span {
        match expr {
            Expr::Ident(name) => &name.span,
            Expr::Literal(lit) => match lit {
                Literal::Number { span, .. }
                | Literal::String { span, .. }
                | Literal::Sigil { span, .. }
                | Literal::Bool { span, .. }
                | Literal::DateTime { span, .. } => span,
            },
            Expr::List { span, .. }
            | Expr::Tuple { span, .. }
            | Expr::Record { span, .. }
            | Expr::FieldAccess { span, .. }
            | Expr::FieldSection { span, .. }
            | Expr::Index { span, .. }
            | Expr::Call { span, .. }
            | Expr::Lambda { span, .. }
            | Expr::Match { span, .. }
            | Expr::If { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Block { span, .. }
            | Expr::Raw { span, .. } => span,
        }
    }

    fn build_diagnostics(text: &str, uri: &Url) -> Vec<Diagnostic> {
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (_, diagnostics) = parse_modules(&path, text);
        diagnostics
            .into_iter()
            .map(|file_diag| Diagnostic {
                range: Self::span_to_range(file_diag.diagnostic.span),
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(NumberOrString::String(file_diag.diagnostic.code)),
                code_description: None,
                source: Some("aivi".to_string()),
                message: file_diag.diagnostic.message,
                related_information: None,
                tags: None,
                data: None,
            })
            .collect()
    }

    fn end_position(text: &str) -> Position {
        let mut line = 0u32;
        let mut column = 0u32;
        for ch in text.chars() {
            if ch == '\n' {
                line += 1;
                column = 0;
            } else {
                column += 1;
            }
        }
        Position::new(line, column)
    }

    fn end_of_line_position(text: &str, line: u32) -> Position {
        let parts: Vec<&str> = text.split('\n').collect();
        let column = parts
            .get(line as usize)
            .map(|line| line.chars().count() as u32)
            .unwrap_or(0);
        Position::new(line, column)
    }

    fn closing_for(open: char) -> Option<char> {
        match open {
            '{' => Some('}'),
            '(' => Some(')'),
            '[' => Some(']'),
            _ => None,
        }
    }

    fn unclosed_open_delimiter(message: &str) -> Option<char> {
        let start = message.find('\'')?;
        let rest = &message[start + 1..];
        let mut chars = rest.chars();
        let open = chars.next()?;
        let end = chars.next()?;
        (end == '\'').then_some(open)
    }

    fn build_code_actions(
        text: &str,
        uri: &Url,
        diagnostics: &[Diagnostic],
    ) -> Vec<CodeActionOrCommand> {
        let mut out = Vec::new();
        for diagnostic in diagnostics {
            let code = match diagnostic.code.as_ref() {
                Some(NumberOrString::String(code)) => code.as_str(),
                Some(NumberOrString::Number(_)) => continue,
                None => continue,
            };

            match code {
                "E1004" => {
                    let Some(open) = Self::unclosed_open_delimiter(&diagnostic.message) else {
                        continue;
                    };
                    let Some(close) = Self::closing_for(open) else {
                        continue;
                    };
                    let position = Self::end_position(text);
                    let range = Range::new(position, position);
                    let edit = TextEdit {
                        range,
                        new_text: close.to_string(),
                    };
                    out.push(CodeActionOrCommand::CodeAction(CodeAction {
                        title: format!("Insert missing '{close}'"),
                        kind: Some(CodeActionKind::QUICKFIX),
                        diagnostics: Some(vec![diagnostic.clone()]),
                        edit: Some(WorkspaceEdit {
                            changes: Some(HashMap::from([(uri.clone(), vec![edit])])),
                            document_changes: None,
                            change_annotations: None,
                        }),
                        command: None,
                        is_preferred: Some(true),
                        disabled: None,
                        data: None,
                    }));
                }
                "E1002" => {
                    out.push(CodeActionOrCommand::CodeAction(CodeAction {
                        title: "Remove unmatched closing delimiter".to_string(),
                        kind: Some(CodeActionKind::QUICKFIX),
                        diagnostics: Some(vec![diagnostic.clone()]),
                        edit: Some(WorkspaceEdit {
                            changes: Some(HashMap::from([(
                                uri.clone(),
                                vec![TextEdit {
                                    range: diagnostic.range,
                                    new_text: String::new(),
                                }],
                            )])),
                            document_changes: None,
                            change_annotations: None,
                        }),
                        command: None,
                        is_preferred: Some(true),
                        disabled: None,
                        data: None,
                    }));
                }
                "E1001" => {
                    let position = Self::end_of_line_position(text, diagnostic.range.end.line);
                    let range = Range::new(position, position);
                    out.push(CodeActionOrCommand::CodeAction(CodeAction {
                        title: "Insert missing closing quote".to_string(),
                        kind: Some(CodeActionKind::QUICKFIX),
                        diagnostics: Some(vec![diagnostic.clone()]),
                        edit: Some(WorkspaceEdit {
                            changes: Some(HashMap::from([(
                                uri.clone(),
                                vec![TextEdit {
                                    range,
                                    new_text: "\"".to_string(),
                                }],
                            )])),
                            document_changes: None,
                            change_annotations: None,
                        }),
                        command: None,
                        is_preferred: Some(true),
                        disabled: None,
                        data: None,
                    }));
                }
                _ => {}
            }
        }
        out
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
                        DomainItem::TypeSig(sig) => {
                            let range = Self::span_to_range(sig.span);
                            children.push(DocumentSymbol {
                                name: sig.name.name,
                                detail: Some("domain sig".to_string()),
                                kind: SymbolKind::FUNCTION,
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
            for item in module.items.iter() {
                if let Some(span) = match item {
                    ModuleItem::Def(def) if def.name.name == name => Some(def.name.span.clone()),
                    ModuleItem::TypeSig(sig) if sig.name.name == name => Some(sig.name.span.clone()),
                    ModuleItem::TypeDecl(decl) if decl.name.name == name => Some(decl.name.span.clone()),
                    ModuleItem::TypeAlias(alias) if alias.name.name == name => Some(alias.name.span.clone()),
                    ModuleItem::ClassDecl(class_decl) if class_decl.name.name == name => {
                        Some(class_decl.name.span.clone())
                    }
                    ModuleItem::InstanceDecl(instance_decl) if instance_decl.name.name == name => {
                        Some(instance_decl.name.span.clone())
                    }
                    ModuleItem::DomainDecl(domain_decl) if domain_decl.name.name == name => {
                        Some(domain_decl.name.span.clone())
                    }
                    _ => None,
                } {
                    return span;
                }
            }
            for export in module.exports.iter() {
                if export.name == name {
                    return export.span.clone();
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
    fn build_definition_resolves_def_across_files_via_use() {
        let math_text = r#"@no_prelude
module examples.compiler.math = {
  export add
  add = x y => x + y
}
"#;
        let app_text = r#"@no_prelude
module examples.compiler.app = {
  export run
  use examples.compiler.math (add)
  run = add 1 2
}
"#;

        let math_uri = Url::parse("file:///math.aivi").expect("valid uri");
        let app_uri = Url::parse("file:///app.aivi").expect("valid uri");

        let mut workspace = HashMap::new();
        let math_path = PathBuf::from("math.aivi");
        let (math_modules, _) = parse_modules(&math_path, math_text);
        for module in math_modules {
            workspace.insert(
                module.name.name.clone(),
                IndexedModule {
                    uri: math_uri.clone(),
                    module,
                },
            );
        }

        let position = position_for(app_text, "add 1 2");
        let location = Backend::build_definition_with_workspace(
            app_text,
            &app_uri,
            position,
            &workspace,
        )
        .expect("definition found");

        let expected_span = find_symbol_span(math_text, "add");
        let expected_range = Backend::span_to_range(expected_span);
        assert_eq!(location.uri, math_uri);
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
    fn build_hover_reports_type_signature_across_files_via_use() {
        let math_text = r#"@no_prelude
module examples.compiler.math = {
  export add
  add : Number -> Number -> Number
  add = x y => x + y
}
"#;
        let app_text = r#"@no_prelude
module examples.compiler.app = {
  export run
  use examples.compiler.math (add)
  run = add 1 2
}
"#;

        let math_uri = Url::parse("file:///math.aivi").expect("valid uri");
        let app_uri = Url::parse("file:///app.aivi").expect("valid uri");

        let mut workspace = HashMap::new();
        let math_path = PathBuf::from("math.aivi");
        let (math_modules, _) = parse_modules(&math_path, math_text);
        for module in math_modules {
            workspace.insert(
                module.name.name.clone(),
                IndexedModule {
                    uri: math_uri.clone(),
                    module,
                },
            );
        }

        let position = position_for(app_text, "add 1 2");
        let hover = Backend::build_hover_with_workspace(app_text, &app_uri, position, &workspace)
            .expect("hover found");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup hover");
        };
        assert!(markup.value.contains("`add`"));
        assert!(markup.value.contains("Number"));
    }

    #[test]
    fn build_hover_includes_docs_and_inferred_types() {
        let text = r#"@no_prelude
module examples.docs = {
  // Identity function.
  id = x => x

  run = id 1
}
"#;
        let uri = sample_uri();
        let position = position_for(text, "id 1");
        let hover = Backend::build_hover(text, &uri, position).expect("hover found");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup hover");
        };
        assert!(markup.value.contains("`id`"));
        assert!(markup.value.contains(":"));
        assert!(markup.value.contains("Identity function."));
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
    fn build_signature_help_resolves_imported_type_sig() {
        let math_text = r#"@no_prelude
module examples.compiler.math = {
  export add
  add : Number -> Number -> Number
  add = x y => x + y
}
"#;
        let app_text = r#"@no_prelude
module examples.compiler.app = {
  export run
  use examples.compiler.math (add)
  run = add 1 2
}
"#;

        let math_uri = Url::parse("file:///math.aivi").expect("valid uri");
        let app_uri = Url::parse("file:///app.aivi").expect("valid uri");

        let mut workspace = HashMap::new();
        let math_path = PathBuf::from("math.aivi");
        let (math_modules, _) = parse_modules(&math_path, math_text);
        for module in math_modules {
            workspace.insert(
                module.name.name.clone(),
                IndexedModule {
                    uri: math_uri.clone(),
                    module,
                },
            );
        }

        let position = position_for(app_text, "1 2");
        let help = Backend::build_signature_help_with_workspace(
            app_text,
            &app_uri,
            position,
            &workspace,
        )
        .expect("signature help");

        assert_eq!(help.active_signature, Some(0));
        assert_eq!(help.active_parameter, Some(0));
        assert_eq!(help.signatures.len(), 1);
        assert!(help.signatures[0].label.contains("`add`"));
        assert!(help.signatures[0].label.contains("Number"));
    }

    #[test]
    fn build_references_searches_across_modules() {
        let math_text = r#"@no_prelude
module examples.compiler.math = {
  export add
  add : Number -> Number -> Number
  add = x y => x + y
}
"#;
        let app_text = r#"@no_prelude
module examples.compiler.app = {
  export run
  use examples.compiler.math (add)
  run = add 1 2
}
"#;

        let math_uri = Url::parse("file:///math.aivi").expect("valid uri");
        let app_uri = Url::parse("file:///app.aivi").expect("valid uri");

        let mut workspace = HashMap::new();
        let math_path = PathBuf::from("math.aivi");
        let (math_modules, _) = parse_modules(&math_path, math_text);
        for module in math_modules {
            workspace.insert(
                module.name.name.clone(),
                IndexedModule {
                    uri: math_uri.clone(),
                    module,
                },
            );
        }
        let app_path = PathBuf::from("app.aivi");
        let (app_modules, _) = parse_modules(&app_path, app_text);
        for module in app_modules {
            workspace.insert(
                module.name.name.clone(),
                IndexedModule {
                    uri: app_uri.clone(),
                    module,
                },
            );
        }

        let position = position_for(app_text, "add 1 2");
        let locations = Backend::build_references_with_workspace(
            app_text,
            &app_uri,
            position,
            true,
            &workspace,
        );

        assert!(locations.iter().any(|loc| loc.uri == math_uri));
        assert!(locations.iter().any(|loc| loc.uri == app_uri));
    }

    #[test]
    fn build_rename_edits_across_modules() {
        let math_text = r#"@no_prelude
module examples.compiler.math = {
  export add
  add : Number -> Number -> Number
  add = x y => x + y
}
"#;
        let app_text = r#"@no_prelude
module examples.compiler.app = {
  export run
  use examples.compiler.math (add)
  run = add 1 2
}
"#;

        let math_uri = Url::parse("file:///math.aivi").expect("valid uri");
        let app_uri = Url::parse("file:///app.aivi").expect("valid uri");

        let mut workspace = HashMap::new();
        let math_path = PathBuf::from("math.aivi");
        let (math_modules, _) = parse_modules(&math_path, math_text);
        for module in math_modules {
            workspace.insert(
                module.name.name.clone(),
                IndexedModule {
                    uri: math_uri.clone(),
                    module,
                },
            );
        }
        let app_path = PathBuf::from("app.aivi");
        let (app_modules, _) = parse_modules(&app_path, app_text);
        for module in app_modules {
            workspace.insert(
                module.name.name.clone(),
                IndexedModule {
                    uri: app_uri.clone(),
                    module,
                },
            );
        }

        let position = position_for(app_text, "add 1 2");
        let edit = Backend::build_rename_with_workspace(
            app_text,
            &app_uri,
            position,
            "sum",
            &workspace,
        )
        .expect("rename edit");

        let changes = edit.changes.expect("changes");
        assert!(changes.contains_key(&math_uri));
        assert!(changes.contains_key(&app_uri));
        assert!(changes
            .values()
            .flatten()
            .all(|edit| edit.new_text == "sum"));
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
    fn code_actions_offer_quick_fix_for_unclosed_delimiter() {
        let text = "module broken = {";
        let uri = sample_uri();
        let diagnostics = Backend::build_diagnostics(text, &uri);
        let actions = Backend::build_code_actions(text, &uri, &diagnostics);
        let expected_pos = Backend::end_position(text);

        let mut saw_fix = false;
        for action in actions {
            let CodeActionOrCommand::CodeAction(action) = action else {
                continue;
            };
            if !action.title.contains("Insert missing") {
                continue;
            }
            let Some(edit) = action.edit else {
                continue;
            };
            let Some(changes) = edit.changes else {
                continue;
            };
            let Some(edits) = changes.get(&uri) else {
                continue;
            };
            if edits.iter().any(|edit| {
                edit.new_text == "}"
                    && edit.range.start == expected_pos
                    && edit.range.end == expected_pos
            }) {
                saw_fix = true;
                break;
            }
        }

        assert!(saw_fix);
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

    #[test]
    fn semantic_tokens_highlight_keywords_types_and_literals() {
        let text = sample_text();
        let tokens = Backend::build_semantic_tokens(text);
        let lines: Vec<&str> = text.lines().collect();

        let mut abs_line = 0u32;
        let mut abs_start = 0u32;
        let mut seen_module_keyword = false;
        let mut seen_type_name = false;
        let mut seen_number = false;
        let mut seen_decorator = false;

        for token in tokens.data.iter() {
            abs_line += token.delta_line;
            if token.delta_line == 0 {
                abs_start += token.delta_start;
            } else {
                abs_start = token.delta_start;
            }
            let line = lines.get(abs_line as usize).copied().unwrap_or_default();
            let text: String = line
                .chars()
                .skip(abs_start as usize)
                .take(token.length as usize)
                .collect();

            match (token.token_type, text.as_str()) {
                (Backend::SEM_TOKEN_KEYWORD, "module") => seen_module_keyword = true,
                (Backend::SEM_TOKEN_TYPE, "Number") => seen_type_name = true,
                (Backend::SEM_TOKEN_NUMBER, "1") => seen_number = true,
                (Backend::SEM_TOKEN_DECORATOR, "@") => seen_decorator = true,
                _ => {}
            }
        }

        assert!(seen_module_keyword);
        assert!(seen_type_name);
        assert!(seen_number);
        assert!(seen_decorator);
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let root = params
            .root_uri
            .and_then(|uri| uri.to_file_path().ok())
            .or_else(|| {
                params
                    .workspace_folders
                    .as_ref()
                    .and_then(|folders| folders.first())
                    .and_then(|folder| folder.uri.to_file_path().ok())
            });
        if root.is_some() {
            let mut state = self.state.lock().await;
            state.workspace_root = root;
            state.disk_module_index.clear();
            state.disk_index_root = None;
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
                document_symbol_provider: Some(OneOf::Left(true)),
                definition_provider: Some(OneOf::Left(true)),
                declaration_provider: Some(DeclarationCapability::Simple(true)),
                implementation_provider: Some(ImplementationProviderCapability::Simple(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec![" ".to_string()]),
                    retrigger_characters: None,
                    work_done_progress_options: Default::default(),
                }),
                references_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Left(true)),
                semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
                    SemanticTokensOptions {
                        legend: Self::semantic_tokens_legend(),
                        full: Some(SemanticTokensFullOptions::Bool(true)),
                        range: None,
                        work_done_progress_options: Default::default(),
                    },
                )),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
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
        self.remove_document(&uri).await;
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
        let location = match self.with_document_text(&uri, |content| content.to_string()).await {
            Some(text) => {
                let workspace = self.workspace_modules_for(&uri).await;
                Self::build_definition_with_workspace(&text, &uri, position, &workspace)
            }
            None => None,
        };
        Ok(location.map(GotoDefinitionResponse::Scalar))
    }

    async fn goto_declaration(
        &self,
        params: GotoDeclarationParams,
    ) -> Result<Option<GotoDeclarationResponse>> {
        let TextDocumentPositionParams { text_document, position } = params.text_document_position_params;
        let uri = text_document.uri;
        let location = match self.with_document_text(&uri, |content| content.to_string()).await {
            Some(text) => {
                let workspace = self.workspace_modules_for(&uri).await;
                Self::build_definition_with_workspace(&text, &uri, position, &workspace)
            }
            None => None,
        };
        Ok(location.map(GotoDeclarationResponse::Scalar))
    }

    async fn goto_implementation(
        &self,
        params: GotoImplementationParams,
    ) -> Result<Option<GotoImplementationResponse>> {
        let TextDocumentPositionParams { text_document, position } = params.text_document_position_params;
        let uri = text_document.uri;
        let location = match self.with_document_text(&uri, |content| content.to_string()).await {
            Some(text) => {
                let workspace = self.workspace_modules_for(&uri).await;
                Self::build_definition_with_workspace(&text, &uri, position, &workspace)
            }
            None => None,
        };
        Ok(location.map(GotoImplementationResponse::Scalar))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let TextDocumentPositionParams { text_document, position } = params.text_document_position_params;
        let uri = text_document.uri;
        let hover = match self.with_document_text(&uri, |content| content.to_string()).await {
            Some(text) => {
                let workspace = self.workspace_modules_for(&uri).await;
                Self::build_hover_with_workspace(&text, &uri, position, &workspace)
                    .or_else(|| Self::build_hover(&text, &uri, position))
            }
            None => None,
        };
        Ok(hover)
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let TextDocumentPositionParams {
            text_document,
            position,
        } = params.text_document_position_params;
        let uri = text_document.uri;
        let help = match self.with_document_text(&uri, |content| content.to_string()).await {
            Some(text) => {
                let workspace = self.workspace_modules_for(&uri).await;
                Self::build_signature_help_with_workspace(&text, &uri, position, &workspace)
            }
            None => None,
        };
        Ok(help)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let TextDocumentPositionParams { text_document, position } = params.text_document_position;
        let uri = text_document.uri;
        let include_declaration = params.context.include_declaration;
        let locations = match self.with_document_text(&uri, |content| content.to_string()).await {
            Some(text) => {
                let workspace = self.workspace_modules_for(&uri).await;
                Self::build_references_with_workspace(
                    &text,
                    &uri,
                    position,
                    include_declaration,
                    &workspace,
                )
            }
            None => Vec::new(),
        };
        Ok(Some(locations))
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let new_name = params.new_name;
        let edit = match self.with_document_text(&uri, |content| content.to_string()).await {
            Some(text) => {
                let workspace = self.workspace_modules_for(&uri).await;
                Self::build_rename_with_workspace(&text, &uri, position, &new_name, &workspace)
            }
            None => None,
        };
        Ok(edit)
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<Vec<CodeActionOrCommand>>> {
        let uri = params.text_document.uri;
        let diagnostics = params.context.diagnostics;
        let actions = self
            .with_document_text(&uri, |content| Self::build_code_actions(content, &uri, &diagnostics))
            .await
            .unwrap_or_default();
        Ok(Some(actions))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let tokens = self
            .with_document_text(&uri, |content| Self::build_semantic_tokens(content))
            .await;
        Ok(tokens.map(SemanticTokensResult::Tokens))
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
