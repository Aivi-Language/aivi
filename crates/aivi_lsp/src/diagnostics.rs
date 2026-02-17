use std::collections::HashMap;
use std::path::{Path, PathBuf};

use aivi::{check_modules, check_types, embedded_stdlib_modules, parse_modules};
use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, Diagnostic, DiagnosticRelatedInformation,
    DiagnosticSeverity, Location, NumberOrString, Position, Range, TextEdit, Url, WorkspaceEdit,
};

use crate::backend::Backend;
use crate::state::IndexedModule;
use crate::strict::{build_strict_diagnostics, StrictConfig};

impl Backend {
    fn is_specs_snippet_path(path: &Path) -> bool {
        let mut comps = path.components().map(|c| c.as_os_str());
        while let Some(comp) = comps.next() {
            if comp == "specs" {
                return comps.any(|c| c == "snippets");
            }
        }
        false
    }

    #[cfg(test)]
    pub(super) fn build_diagnostics(text: &str, uri: &Url) -> Vec<Diagnostic> {
        Self::build_diagnostics_with_workspace(
            text,
            uri,
            &HashMap::new(),
            false,
            &StrictConfig::default(),
        )
    }

    #[cfg(test)]
    pub(super) fn build_diagnostics_strict(
        text: &str,
        uri: &Url,
        strict: &StrictConfig,
    ) -> Vec<Diagnostic> {
        Self::build_diagnostics_with_workspace(text, uri, &HashMap::new(), false, strict)
    }

    pub(super) fn build_diagnostics_with_workspace(
        text: &str,
        uri: &Url,
        workspace_modules: &HashMap<String, IndexedModule>,
        include_specs_snippets: bool,
        strict: &StrictConfig,
    ) -> Vec<Diagnostic> {
        let path = PathBuf::from(Self::path_from_uri(uri));
        if !include_specs_snippets && Self::is_specs_snippet_path(&path) {
            // `specs/snippets/**/*.aivi` contains documentation fragments, not necessarily complete
            // modules. Avoid surfacing diagnostics as "nags" when authoring specs.
            return Vec::new();
        }
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

        // Strict-mode diagnostics are an additive overlay. They must not affect parsing,
        // name resolution, or typing; they only provide additional validation and quick fixes.
        out.extend(build_strict_diagnostics(
            text,
            uri,
            &path,
            strict,
            workspace_modules,
        ));

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

        let code = file_diag.diagnostic.code.clone();
        Diagnostic {
            range: Self::span_to_range(file_diag.diagnostic.span),
            severity: Some(match file_diag.diagnostic.severity {
                aivi::DiagnosticSeverity::Error => DiagnosticSeverity::ERROR,
                aivi::DiagnosticSeverity::Warning => DiagnosticSeverity::WARNING,
            }),
            code: Some(NumberOrString::String(code.clone())),
            code_description: None,
            source: Some(format!("aivi.{}", category_for_code(&code))),
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

    fn unknown_name_from_message(message: &str) -> Option<String> {
        // Compiler diagnostic format: "unknown name 'x'".
        let start = message.find('\'')?;
        let rest = &message[start + 1..];
        let end = rest.find('\'')?;
        let name = &rest[..end];
        if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        }
    }

    fn import_insertion_position(text: &str) -> Position {
        // Modules are file-scoped and the `module` declaration must appear first (after optional
        // decorators). We insert after the last contiguous `use ...` line, or directly after the
        // module declaration when there are no uses.
        let lines: Vec<&str> = text.split('\n').collect();
        let mut i: usize = 0;

        // Skip leading empty lines and module decorators.
        while i < lines.len() {
            let trimmed = lines[i].trim();
            if trimmed.is_empty() || trimmed.starts_with('@') {
                i += 1;
                continue;
            }
            break;
        }

        // Find `module` line.
        while i < lines.len() {
            let trimmed = lines[i].trim_start();
            if trimmed.starts_with("module ") {
                break;
            }
            // If we didn't find a module line, fall back to start of document.
            return Position::new(0, 0);
        }
        if i >= lines.len() {
            return Position::new(0, 0);
        }

        let module_line = i;
        let mut last_use_line: Option<usize> = None;
        i = module_line + 1;
        while i < lines.len() {
            let trimmed = lines[i].trim_start();
            if trimmed.starts_with("use ") {
                last_use_line = Some(i);
                i += 1;
                continue;
            }
            // Stop on the first non-use line (including a blank line).
            break;
        }

        let insert_line = last_use_line.map(|l| l + 1).unwrap_or(module_line + 1);
        Position::new(insert_line as u32, 0)
    }

    fn import_quickfixes_for_unknown_name(
        text: &str,
        uri: &Url,
        diagnostic: &Diagnostic,
        workspace_modules: &HashMap<String, IndexedModule>,
    ) -> Vec<CodeActionOrCommand> {
        let Some(name) = Self::unknown_name_from_message(&diagnostic.message) else {
            return Vec::new();
        };

        let mut providers: Vec<String> = Vec::new();
        for (module_name, indexed) in workspace_modules {
            if indexed
                .module
                .exports
                .iter()
                .any(|e| matches!(e.kind, aivi::ScopeItemKind::Value) && e.name.name == name)
            {
                providers.push(module_name.clone());
            }
        }

        providers.sort();
        providers.dedup();

        // Heuristic: keep the list small to avoid spamming the user.
        const MAX_ACTIONS: usize = 8;
        if providers.is_empty() {
            return Vec::new();
        }

        let insert_at = Self::import_insertion_position(text);
        let range = Range::new(insert_at, insert_at);

        let mut out = Vec::new();
        let preferred = providers.len() == 1;
        for (idx, module_name) in providers.into_iter().take(MAX_ACTIONS).enumerate() {
            let title = format!("Add `use {module_name} ({name})`");
            let edit = TextEdit {
                range,
                new_text: format!("use {module_name} ({name})\n"),
            };
            out.push(CodeActionOrCommand::CodeAction(CodeAction {
                title,
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(WorkspaceEdit {
                    changes: Some(HashMap::from([(uri.clone(), vec![edit])])),
                    document_changes: None,
                    change_annotations: None,
                }),
                command: None,
                is_preferred: Some(preferred && idx == 0),
                disabled: None,
                data: None,
            }));
        }
        out
    }

    pub(super) fn build_code_actions_with_workspace(
        text: &str,
        uri: &Url,
        diagnostics: &[Diagnostic],
        workspace_modules: &HashMap<String, IndexedModule>,
    ) -> Vec<CodeActionOrCommand> {
        let mut out = Vec::new();
        for diagnostic in diagnostics {
            // Generic strict-mode (and future) quickfix embedding: Diagnostics may carry a
            // serialized `TextEdit` list in `Diagnostic.data`.
            if let Some(actions) = quickfixes_from_diagnostic_data(uri, diagnostic) {
                out.extend(actions);
            }

            let code = match diagnostic.code.as_ref() {
                Some(NumberOrString::String(code)) => code.as_str(),
                Some(NumberOrString::Number(_)) => continue,
                None => continue,
            };

            match code {
                "E3000" => {
                    out.extend(Self::import_quickfixes_for_unknown_name(
                        text,
                        uri,
                        diagnostic,
                        workspace_modules,
                    ));
                }
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

fn category_for_code(code: &str) -> &'static str {
    // Keep the mapping coarse and stable; strict-mode uses its own source.
    if code.starts_with('E') {
        match &code.get(1..2) {
            Some("1") => "Syntax",
            Some("2") => "NameResolution",
            Some("3") => "Type",
            _ => "Syntax",
        }
    } else if code.starts_with('W') {
        "Style"
    } else if code.starts_with("AIVI-S") {
        "Strict"
    } else {
        "Syntax"
    }
}

fn quickfixes_from_diagnostic_data(
    uri: &Url,
    diagnostic: &Diagnostic,
) -> Option<Vec<CodeActionOrCommand>> {
    let data = diagnostic.data.as_ref()?;
    let obj = data.as_object()?;
    let fix = obj.get("aiviQuickFix")?;

    #[derive(serde::Deserialize)]
    struct FixPayload {
        title: String,
        #[serde(default)]
        is_preferred: bool,
        edits: Vec<TextEdit>,
    }

    let payload: FixPayload = serde_json::from_value::<FixPayload>(fix.clone()).ok()?;
    if payload.edits.is_empty() {
        return None;
    }

    Some(vec![CodeActionOrCommand::CodeAction(CodeAction {
        title: payload.title,
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diagnostic.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(HashMap::from([(uri.clone(), payload.edits)])),
            document_changes: None,
            change_annotations: None,
        }),
        command: None,
        is_preferred: Some(payload.is_preferred),
        disabled: None,
        data: None,
    })])
}
