use std::sync::Arc;

use aivi_base::{ByteIndex, LspPosition};
use aivi_hir::{ExprId, ExprKind, LspSymbol, LspSymbolKind};
use tower_lsp::lsp_types::{
    ParameterInformation, ParameterLabel, SignatureHelp, SignatureHelpParams, SignatureInformation,
};

use crate::{
    navigation::{NavigationAnalysis, NavigationLookup},
    state::ServerState,
};

pub fn signature_help(
    params: SignatureHelpParams,
    state: Arc<ServerState>,
) -> Option<SignatureHelp> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;
    let file = state.file(uri)?;
    let hir = aivi_query::hir_module(&state.db, file);
    let cursor = hir.source().lsp_position_to_offset(LspPosition {
        line: position.line,
        character: position.character,
    })?;
    let (callee, arguments) = tightest_application(hir.module(), cursor)?;
    let callee_expr = &hir.module().exprs()[callee];
    let callee_position = hir
        .source()
        .offset_to_lsp_position(callee_expr.span.span().start());
    let navigation = NavigationAnalysis::load(&state.db, file);
    let targets = match navigation.definition_targets_at_lsp_position(&state.db, callee_position) {
        NavigationLookup::Targets(targets) => targets,
        NavigationLookup::NoSite | NavigationLookup::NoTargets => return None,
    };
    let symbol = targets
        .iter()
        .find_map(|target| target.find_symbol_at_target(&state.db))
        .filter(|symbol| matches!(symbol.kind, LspSymbolKind::Function | LspSymbolKind::Method))?;
    Some(signature_for_symbol(
        &symbol,
        active_argument(hir.module(), &arguments, cursor),
    ))
}

fn tightest_application(
    module: &aivi_hir::Module,
    cursor: ByteIndex,
) -> Option<(ExprId, Vec<ExprId>)> {
    let mut best: Option<(u32, ExprId, Vec<ExprId>)> = None;
    for (_, expression) in module.exprs().iter() {
        let ExprKind::Apply { callee, arguments } = &expression.kind else {
            continue;
        };
        let span = expression.span.span();
        if cursor < span.start() || cursor > span.end() {
            continue;
        }
        let length = span.len();
        if best
            .as_ref()
            .is_none_or(|(best_length, _, _)| length < *best_length)
        {
            best = Some((length, *callee, arguments.iter().copied().collect()));
        }
    }
    best.map(|(_, callee, arguments)| (callee, arguments))
}

fn active_argument(module: &aivi_hir::Module, arguments: &[ExprId], cursor: ByteIndex) -> usize {
    arguments
        .iter()
        .position(|argument| cursor <= module.exprs()[*argument].span.span().end())
        .unwrap_or_else(|| arguments.len().saturating_sub(1))
}

fn signature_for_symbol(symbol: &LspSymbol, active_argument: usize) -> SignatureHelp {
    let parameter_labels = symbol
        .children
        .iter()
        .filter(|child| child.kind == LspSymbolKind::Variable)
        .map(|parameter| match &parameter.detail {
            Some(detail) => format!("{}: {detail}", parameter.name),
            None => parameter.name.clone(),
        })
        .collect::<Vec<_>>();
    let label = match &symbol.detail {
        Some(detail) if detail.starts_with('(') => format!("{}{detail}", symbol.name),
        Some(detail) if parameter_labels.is_empty() => format!("{}: {detail}", symbol.name),
        Some(detail) => format!(
            "{}({}) -> {detail}",
            symbol.name,
            parameter_labels.join(", ")
        ),
        None => format!("{}({})", symbol.name, parameter_labels.join(", ")),
    };
    let active_parameter = (!parameter_labels.is_empty())
        .then(|| active_argument.min(parameter_labels.len() - 1) as u32);
    let parameters = (!parameter_labels.is_empty()).then(|| {
        parameter_labels
            .into_iter()
            .map(|label| ParameterInformation {
                label: ParameterLabel::Simple(label),
                documentation: None,
            })
            .collect()
    });
    SignatureHelp {
        signatures: vec![SignatureInformation {
            label,
            documentation: None,
            parameters,
            active_parameter,
        }],
        active_signature: Some(0),
        active_parameter,
    }
}
