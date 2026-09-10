use std::sync::Arc;

use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, CodeActionResponse,
    NumberOrString, Position, Range, TextEdit, WorkspaceEdit,
};

use crate::{analysis::FileAnalysis, state::ServerState};

/// Produce code actions for the requested range.
///
/// Currently emits a "Remove unused symbol" quickfix for every
/// `aivi/unused-symbol` diagnostic that overlaps the request range.
pub fn code_actions(
    params: CodeActionParams,
    state: Arc<ServerState>,
) -> Option<CodeActionResponse> {
    let uri = &params.text_document.uri;
    let file = state.file(uri)?;
    let analysis = FileAnalysis::load(&state.db, file);
    let hir = aivi_query::hir_module(&state.db, file);

    // Generate the unused-symbol diagnostics from the LSP layer.
    let unused_diags = crate::unused::collect_unused_diagnostics(hir.module(), &analysis.source);

    let request_range = params.range;
    let mut actions: Vec<CodeActionOrCommand> = Vec::new();

    actions.extend(crate::type_annotations::build_type_annotation_code_actions(
        uri,
        analysis.typed_declarations.as_ref(),
        analysis.source.as_ref(),
        request_range,
    ));

    for diag in &unused_diags {
        if diag.code != Some(NumberOrString::String("aivi/unused-symbol".to_owned())) {
            continue;
        }

        // Include this action if the diagnostic range overlaps the requested range.
        if !ranges_overlap(diag.range, request_range) {
            continue;
        }

        // Remove the complete parsed declaration, never just its name line.
        let Some(line_edit) =
            delete_declaration_edit(&state.db, file, &analysis.source, diag.range)
        else {
            continue;
        };

        let mut changes = std::collections::HashMap::new();
        changes.insert(uri.clone(), vec![line_edit]);

        actions.push(CodeActionOrCommand::CodeAction(CodeAction {
            title: "Remove unused symbol".to_owned(),
            kind: Some(CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![diag.clone()]),
            edit: Some(WorkspaceEdit {
                changes: Some(changes),
                ..Default::default()
            }),
            ..Default::default()
        }));
    }

    if actions.is_empty() {
        None
    } else {
        Some(actions)
    }
}

fn ranges_overlap(a: Range, b: Range) -> bool {
    a.start <= b.end && b.start <= a.end
}

/// Delete the parsed declaration, including its attached annotation/decorators.
fn delete_declaration_edit(
    db: &aivi_query::RootDatabase,
    file: aivi_query::SourceFile,
    source: &aivi_base::SourceFile,
    range: Range,
) -> Option<TextEdit> {
    let cursor = source.lsp_position_to_offset(aivi_base::LspPosition {
        line: range.start.line,
        character: range.start.character,
    })?;
    let parsed = aivi_query::parsed_file(db, file);
    let item = parsed
        .cst()
        .items()
        .iter()
        .find(|item| item.span().span().contains(cursor))?;
    let span = item.span().span();
    let hir = aivi_query::hir_module(db, file);
    let declarations = crate::type_annotations::collect_typed_declaration_summaries(
        hir.module(),
        parsed.cst(),
        source,
    );
    let mut start = span.start();
    if let Some(annotation) = declarations
        .iter()
        .find(|declaration| declaration.header_span == item.span())
        .and_then(|declaration| declaration.annotation.as_ref())
    {
        start = start.min(annotation.full_span.span().start());
    }
    for decorator in &item.base().decorators {
        start = start.min(decorator.span.span().start());
    }
    let start = source.offset_to_lsp_position(start);
    let end = source.offset_to_lsp_position(span.end());
    Some(TextEdit {
        range: Range {
            start: Position {
                line: start.line,
                character: start.character,
            },
            end: Position {
                line: end.line,
                character: end.character,
            },
        },
        new_text: String::new(),
    })
}
