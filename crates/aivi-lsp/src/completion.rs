use std::sync::Arc;

use aivi_base::LspPosition;
use aivi_hir::LspSymbolKind;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse,
};

use crate::{analysis::FileAnalysis, state::ServerState};

pub fn completion(params: CompletionParams, state: Arc<ServerState>) -> Option<CompletionResponse> {
    let uri = &params.text_document_position.text_document.uri;
    let lsp_pos = params.text_document_position.position;

    let file = state.file(uri)?;
    let current_analysis = FileAnalysis::load(&state.db, file);

    // Reject out-of-range cursor positions before returning any items.
    current_analysis
        .source
        .lsp_position_to_offset(LspPosition {
            line: lsp_pos.line,
            character: lsp_pos.character,
        })?;

    let hir = aivi_query::hir_module(&state.db, file);
    let module = hir.module();
    let cursor = current_analysis
        .source
        .lsp_position_to_offset(LspPosition {
            line: lsp_pos.line,
            character: lsp_pos.character,
        })?;
    let before = &current_analysis.source.text()[..cursor.as_usize()];
    let word = before
        .rsplit(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.'))
        .next()
        .unwrap_or("");
    if let Some((base, _prefix)) = word.rsplit_once('.') {
        return Some(CompletionResponse::Array(record_members(
            module, cursor, base,
        )));
    }
    let mut items = std::collections::BTreeMap::new();
    for symbol in current_analysis.symbols.iter() {
        items.insert(
            symbol.name.clone(),
            CompletionItem {
                label: symbol.name.clone(),
                kind: Some(lsp_symbol_kind_to_completion_kind(symbol.kind)),
                detail: symbol.detail.clone(),
                ..Default::default()
            },
        );
    }
    for (_, import) in module.imports().iter() {
        let name = import.local_name.text();
        if name.starts_with("__") || name.contains('#') {
            continue;
        }
        items
            .entry(name.to_owned())
            .or_insert_with(|| CompletionItem {
                label: name.to_owned(),
                ..Default::default()
            });
    }
    // Lexical parameters take precedence over module/import names. Nested
    // lambdas are visited from outermost to innermost scope.
    let mut scopes = Vec::new();
    for (_, item) in module.items().iter() {
        if let aivi_hir::Item::Function(function) = item
            && function.header.span.span().contains(cursor)
        {
            scopes.push((function.header.span.span().len(), &function.parameters));
        }
    }
    for (_, expr) in module.exprs().iter() {
        if let aivi_hir::ExprKind::Lambda(lambda) = &expr.kind
            && expr.span.span().contains(cursor)
        {
            scopes.push((expr.span.span().len(), &lambda.parameters));
        }
    }
    scopes.sort_by_key(|(length, _)| std::cmp::Reverse(*length));
    for (_, parameters) in scopes {
        for parameter in parameters {
            let name = module.bindings()[parameter.binding].name.text();
            items.insert(
                name.to_owned(),
                CompletionItem {
                    label: name.to_owned(),
                    kind: Some(CompletionItemKind::VARIABLE),
                    ..Default::default()
                },
            );
        }
    }
    for (_, expression) in module.exprs().iter() {
        let aivi_hir::ExprKind::Pipe(pipe) = &expression.kind else {
            continue;
        };
        for stage in pipe.stages.iter() {
            let aivi_hir::PipeStageKind::Case { pattern, body } = stage.kind else {
                continue;
            };
            if !module.exprs()[body].span.span().contains(cursor) {
                continue;
            }
            let mut patterns = vec![pattern];
            while let Some(pattern) = patterns.pop() {
                match &module.patterns()[pattern].kind {
                    aivi_hir::PatternKind::Binding(binding) => {
                        let name = binding.name.text();
                        items.insert(
                            name.to_owned(),
                            CompletionItem {
                                label: name.to_owned(),
                                kind: Some(CompletionItemKind::VARIABLE),
                                ..Default::default()
                            },
                        );
                    }
                    aivi_hir::PatternKind::Tuple(elements) => {
                        patterns.extend(elements.iter().copied())
                    }
                    aivi_hir::PatternKind::List { elements, rest } => {
                        patterns.extend(elements);
                        patterns.extend(rest);
                    }
                    aivi_hir::PatternKind::Record(fields) => {
                        patterns.extend(fields.iter().map(|field| field.pattern))
                    }
                    aivi_hir::PatternKind::Constructor { arguments, .. } => {
                        patterns.extend(arguments)
                    }
                    _ => {}
                }
            }
        }
    }
    Some(CompletionResponse::Array(items.into_values().collect()))
}

fn record_members(
    module: &aivi_hir::Module,
    cursor: aivi_base::ByteIndex,
    base: &str,
) -> Vec<CompletionItem> {
    use aivi_hir::{GateType, GeneralExprOutcome, Item};
    let mut parts = base.split('.');
    let Some(name) = parts.next() else {
        return Vec::new();
    };
    let reports = aivi_hir::elaborate_general_expressions(module);
    let mut ty = None;
    for report in reports.items() {
        let owner = &module.items()[report.owner];
        if owner.span().span().contains(cursor)
            && let Some(parameter) = report
                .parameters
                .iter()
                .find(|parameter| parameter.name.as_ref() == name)
        {
            ty = Some(parameter.ty.clone());
            break;
        }
        if matches!(owner, Item::Value(value) if value.name.text() == name)
            && let GeneralExprOutcome::Lowered(body) = &report.outcome
        {
            ty = Some(body.ty.clone());
        }
    }
    let Some(mut ty) = ty else {
        return Vec::new();
    };
    for part in parts {
        let GateType::Record(fields) = ty else {
            return Vec::new();
        };
        let Some(field) = fields.into_iter().find(|field| field.name == part) else {
            return Vec::new();
        };
        ty = field.ty;
    }
    let GateType::Record(fields) = ty else {
        return Vec::new();
    };
    fields
        .into_iter()
        .map(|field| CompletionItem {
            label: field.name,
            kind: Some(CompletionItemKind::FIELD),
            detail: Some(field.ty.to_string()),
            ..Default::default()
        })
        .collect()
}

fn lsp_symbol_kind_to_completion_kind(kind: LspSymbolKind) -> CompletionItemKind {
    match kind {
        LspSymbolKind::Function | LspSymbolKind::Method => CompletionItemKind::FUNCTION,
        LspSymbolKind::Variable | LspSymbolKind::Constant => CompletionItemKind::VARIABLE,
        LspSymbolKind::Field | LspSymbolKind::Property => CompletionItemKind::FIELD,
        LspSymbolKind::Enum => CompletionItemKind::ENUM,
        LspSymbolKind::EnumMember => CompletionItemKind::ENUM_MEMBER,
        LspSymbolKind::Struct => CompletionItemKind::STRUCT,
        LspSymbolKind::Class | LspSymbolKind::Interface => CompletionItemKind::CLASS,
        LspSymbolKind::Module | LspSymbolKind::Namespace | LspSymbolKind::Package => {
            CompletionItemKind::MODULE
        }
        LspSymbolKind::Constructor => CompletionItemKind::CONSTRUCTOR,
        LspSymbolKind::TypeParameter => CompletionItemKind::TYPE_PARAMETER,
        LspSymbolKind::Operator => CompletionItemKind::OPERATOR,
        _ => CompletionItemKind::TEXT,
    }
}
