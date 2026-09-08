use std::{collections::HashMap, sync::Arc};

use aivi_base::{Diagnostic, FileId, LabelStyle, LspRange, Severity};
use tower_lsp::lsp_types::{
    self as lsp, DiagnosticRelatedInformation, DiagnosticSeverity, Location, NumberOrString,
    Position, Range, Url,
};

/// Convert an aivi_base::LspRange to a tower-lsp Range.
pub fn lsp_range(r: LspRange) -> Range {
    Range {
        start: Position {
            line: r.start.line,
            character: r.start.character,
        },
        end: Position {
            line: r.end.line,
            character: r.end.character,
        },
    }
}

/// Collect all diagnostics for a file and convert to LSP format.
pub fn collect_lsp_diagnostics(
    db: &aivi_query::RootDatabase,
    file: aivi_query::SourceFile,
    uri: &Url,
) -> Vec<lsp::Diagnostic> {
    let analysis = crate::analysis::FileAnalysis::load(db, file);
    let related_sources = analysis
        .diagnostics
        .iter()
        .any(|diagnostic| {
            diagnostic
                .labels
                .iter()
                .any(|label| label.style == LabelStyle::Secondary)
        })
        .then(|| {
            db.files()
                .into_iter()
                .filter_map(|related_file| {
                    let source = related_file.source(db);
                    let uri = Url::from_file_path(related_file.path(db)).ok()?;
                    Some((source.id(), (uri, source)))
                })
                .collect::<HashMap<_, _>>()
        });

    let mut diagnostics: Vec<lsp::Diagnostic> = analysis
        .diagnostics
        .iter()
        .map(|diagnostic| {
            convert_diagnostic(
                diagnostic,
                analysis.source.as_ref(),
                related_sources.as_ref(),
                uri,
            )
        })
        .collect();

    diagnostics.extend(
        crate::type_annotations::collect_type_annotation_diagnostics(
            analysis.typed_declarations.as_ref(),
            analysis.source.as_ref(),
        ),
    );

    // Append unused-symbol hints only when the file has no errors, to avoid
    // false positives while the user is actively editing.
    let has_errors = analysis
        .diagnostics
        .iter()
        .any(|d| d.severity == aivi_base::Severity::Error);
    if !has_errors {
        let hir = aivi_query::hir_module(db, file);
        diagnostics.extend(crate::unused::collect_unused_diagnostics(
            hir.module(),
            analysis.source.as_ref(),
        ));
    }

    diagnostics.sort_by(|left, right| {
        left.range
            .start
            .cmp(&right.range.start)
            .then_with(|| left.range.end.cmp(&right.range.end))
            .then_with(|| diagnostic_severity_rank(left).cmp(&diagnostic_severity_rank(right)))
            .then_with(|| compare_diagnostic_codes(left, right))
            .then_with(|| left.message.cmp(&right.message))
    });
    diagnostics
}

type RelatedSources = HashMap<FileId, (Url, Arc<aivi_base::SourceFile>)>;

fn convert_diagnostic(
    d: &Diagnostic,
    source_file: &aivi_base::SourceFile,
    related_sources: Option<&RelatedSources>,
    file_uri: &Url,
) -> lsp::Diagnostic {
    let severity = match d.severity {
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
        Severity::Note => DiagnosticSeverity::INFORMATION,
        Severity::Help => DiagnosticSeverity::HINT,
    };

    let range = d
        .labels
        .iter()
        .find(|l| l.style == aivi_base::LabelStyle::Primary)
        .or_else(|| d.labels.first())
        .map(|label| {
            let lsp_r = source_file.span_to_lsp_range(label.span.span());
            lsp_range(lsp_r)
        })
        .unwrap_or_else(|| Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 0,
            },
        });

    let code = d.code.map(|c| NumberOrString::String(c.to_string()));

    // Convert secondary labels to LSP DiagnosticRelatedInformation entries so
    // editors can navigate to additional context spans referenced by the
    // diagnostic (e.g. a previous definition site).
    let related_information: Vec<DiagnosticRelatedInformation> = d
        .labels
        .iter()
        .filter(|l| l.style == LabelStyle::Secondary)
        .map(|label| {
            let label_file_id = label.span.file();
            // Prefer looking up the URI from the database so cross-file
            // secondary labels resolve to the correct document URI.  Fall back
            // to the current file's URI when the file cannot be located.
            let (label_uri, label_source) = related_sources
                .and_then(|sources| sources.get(&label_file_id))
                .map(|(uri, source)| (uri.clone(), source.as_ref()))
                .unwrap_or_else(|| (file_uri.clone(), source_file));

            let lsp_r = label_source.span_to_lsp_range(label.span.span());
            let label_range = lsp_range(lsp_r);
            DiagnosticRelatedInformation {
                location: Location {
                    uri: label_uri,
                    range: label_range,
                },
                message: label.message.clone(),
            }
        })
        .collect();

    lsp::Diagnostic {
        range,
        severity: Some(severity),
        code,
        code_description: None,
        source: Some("aivi".to_owned()),
        message: d.message.clone(),
        related_information: if related_information.is_empty() {
            None
        } else {
            Some(related_information)
        },
        tags: None,
        data: None,
    }
}

fn diagnostic_severity_rank(diagnostic: &lsp::Diagnostic) -> u8 {
    match diagnostic.severity {
        Some(DiagnosticSeverity::ERROR) => 0,
        Some(DiagnosticSeverity::WARNING) => 1,
        Some(DiagnosticSeverity::INFORMATION) => 2,
        Some(DiagnosticSeverity::HINT) => 3,
        Some(_) => 4,
        None => 5,
    }
}

fn compare_diagnostic_codes(left: &lsp::Diagnostic, right: &lsp::Diagnostic) -> std::cmp::Ordering {
    match (left.code.as_ref(), right.code.as_ref()) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (Some(_), None) => std::cmp::Ordering::Greater,
        (Some(NumberOrString::Number(left)), Some(NumberOrString::Number(right))) => {
            left.cmp(right)
        }
        (Some(NumberOrString::Number(_)), Some(NumberOrString::String(_))) => {
            std::cmp::Ordering::Less
        }
        (Some(NumberOrString::String(_)), Some(NumberOrString::Number(_))) => {
            std::cmp::Ordering::Greater
        }
        (Some(NumberOrString::String(left)), Some(NumberOrString::String(right))) => {
            left.cmp(right)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use aivi_base::{DiagnosticCode, DiagnosticRenderer, SourceDatabase};

    use super::*;

    #[test]
    fn native_diagnostic_preserves_cli_and_lsp_identity_and_locations() {
        let mut sources = SourceDatabase::new();
        let file_id = sources.add_file(
            "/diagnostic-parity/main.aivi",
            "value first = 0\nvalue 🦀icon = 1\n",
        );
        let source = &sources[file_id];
        let primary_start = source.text().find("icon").expect("fixture contains icon");
        let secondary_start = source.text().find("first").expect("fixture contains first");
        let diagnostic = Diagnostic::warning("duplicate declaration")
            .with_code(DiagnosticCode::new("hir", "duplicate-term-name"))
            .with_primary_label(
                source.source_span(primary_start..primary_start + "icon".len()),
                "duplicate is declared here",
            )
            .with_secondary_label(
                source.source_span(secondary_start..secondary_start + "first".len()),
                "first declaration is here",
            );

        // `aivi check` renders this same native diagnostic model. Keep its
        // stable identity and secondary context aligned with the LSP view.
        let cli = DiagnosticRenderer::plain().render(&diagnostic, &sources);
        assert!(cli.contains("warning[hir::duplicate-term-name]: duplicate declaration"));
        assert!(cli.contains("duplicate is declared here"));
        assert!(cli.contains("first declaration is here"));

        let uri = Url::from_file_path(source.path()).expect("absolute fixture URI");
        let related_sources = HashMap::from([(file_id, (uri.clone(), Arc::new(source.clone())))]);
        let lsp = convert_diagnostic(&diagnostic, source, Some(&related_sources), &uri);

        assert_eq!(lsp.severity, Some(DiagnosticSeverity::WARNING));
        assert_eq!(
            lsp.code,
            Some(NumberOrString::String(
                "hir::duplicate-term-name".to_owned()
            ))
        );
        assert_eq!(lsp.message, "duplicate declaration");
        assert_eq!(lsp.range.start, Position::new(1, 8));
        assert_eq!(lsp.range.end, Position::new(1, 12));

        let related = lsp
            .related_information
            .expect("secondary label becomes related information");
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].location.uri, uri);
        assert_eq!(related[0].location.range.start, Position::new(0, 6));
        assert_eq!(related[0].location.range.end, Position::new(0, 11));
        assert_eq!(related[0].message, "first declaration is here");
    }
}
