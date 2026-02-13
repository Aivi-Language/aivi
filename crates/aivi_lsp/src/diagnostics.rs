use std::collections::HashMap;
use std::path::PathBuf;

use aivi::{check_modules, check_types, embedded_stdlib_modules, parse_modules};
use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, Diagnostic, DiagnosticRelatedInformation,
    DiagnosticSeverity, Location, NumberOrString, Position, Range, TextEdit, Url, WorkspaceEdit,
};

use crate::backend::Backend;
use crate::state::IndexedModule;

impl Backend {
    #[cfg(test)]
    pub(super) fn build_diagnostics(text: &str, uri: &Url) -> Vec<Diagnostic> {
        Self::build_diagnostics_with_workspace(text, uri, &HashMap::new())
    }

    pub(super) fn build_diagnostics_with_workspace(
        text: &str,
        uri: &Url,
        workspace_modules: &HashMap<String, IndexedModule>,
    ) -> Vec<Diagnostic> {
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (file_modules, parse_diags) = parse_modules(&path, text);

        // Always surface lex/parse diagnostics first; semantic checking on malformed syntax is
        // best-effort and must never crash the server.
        let mut out: Vec<Diagnostic> = parse_diags
            .into_iter()
            .map(|file_diag| Self::file_diag_to_lsp(uri, file_diag))
            .collect();

        // Build a module set for resolver + typechecker: workspace modules + this file's modules.
        let mut module_map = HashMap::new();
        // Include embedded stdlib so imports/prelude/classes resolve for user code, but keep
        // diagnostics scoped to the current file (below) to avoid surfacing stdlib churn.
        for module in embedded_stdlib_modules() {
            module_map.insert(module.name.name.clone(), module);
        }
        for indexed in workspace_modules.values() {
            module_map.insert(indexed.module.name.name.clone(), indexed.module.clone());
        }
        for module in file_modules {
            module_map.insert(module.name.name.clone(), module);
        }
        let modules: Vec<aivi::Module> = module_map.into_values().collect();

        let semantic_diags = std::panic::catch_unwind(|| {
            let mut diags = check_modules(&modules);
            diags.extend(check_types(&modules));
            diags
        })
        .unwrap_or_default();

        for file_diag in semantic_diags {
            // LSP publishes per-document diagnostics; keep only the ones for this file.
            if PathBuf::from(&file_diag.path) != path {
                continue;
            }
            out.push(Self::file_diag_to_lsp(uri, file_diag));
        }

        out
    }

    fn file_diag_to_lsp(uri: &Url, file_diag: aivi::FileDiagnostic) -> Diagnostic {
        let related_information = (!file_diag.diagnostic.labels.is_empty()).then(|| {
            file_diag
                .diagnostic
                .labels
                .into_iter()
                .map(|label| DiagnosticRelatedInformation {
                    location: Location {
                        uri: uri.clone(),
                        range: Self::span_to_range(label.span),
                    },
                    message: label.message,
                })
                .collect()
        });

        Diagnostic {
            range: Self::span_to_range(file_diag.diagnostic.span),
            severity: Some(match file_diag.diagnostic.severity {
                aivi::DiagnosticSeverity::Error => DiagnosticSeverity::ERROR,
                aivi::DiagnosticSeverity::Warning => DiagnosticSeverity::WARNING,
            }),
            code: Some(NumberOrString::String(file_diag.diagnostic.code)),
            code_description: None,
            source: Some("aivi".to_string()),
            message: file_diag.diagnostic.message,
            related_information,
            tags: None,
            data: None,
        }
    }

    pub(super) fn end_position(text: &str) -> Position {
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

    pub(super) fn build_code_actions(
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
}
