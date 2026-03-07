use std::collections::{HashMap, HashSet};
use std::path::Path;

use aivi::{embedded_stdlib_modules, lex_cst, parse_modules, Module};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, TextEdit, Url};

use crate::backend::Backend;

mod imports_and_domains;
mod lexical;
mod pattern_discipline;
mod pipe_discipline;
mod record_syntax;
mod tuple_intent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub(crate) enum StrictLevel {
    #[default]
    Off = 0,
    LexicalStructural = 1,
    NamesImports = 2,
    TypesDomains = 3,
    NoImplicitCoercions = 4,
    Pedantic = 5,
}

impl StrictLevel {
    pub(crate) fn from_u8(raw: u8) -> Self {
        match raw {
            0 => StrictLevel::Off,
            1 => StrictLevel::LexicalStructural,
            2 => StrictLevel::NamesImports,
            3 => StrictLevel::TypesDomains,
            4 => StrictLevel::NoImplicitCoercions,
            _ => StrictLevel::Pedantic,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StrictConfig {
    #[serde(default)]
    pub(crate) level: StrictLevel,
    /// When enabled at level >= 4, treat inferred/inserted expected-type coercions as errors.
    #[serde(default)]
    pub(crate) forbid_implicit_coercions: bool,
    /// Elevate AIVI warning diagnostics (e.g. unused imports) to errors.
    #[serde(default)]
    pub(crate) warnings_as_errors: bool,
}

impl Default for StrictConfig {
    fn default() -> Self {
        Self {
            level: StrictLevel::Off,
            forbid_implicit_coercions: false,
            warnings_as_errors: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StrictCategory {
    Syntax,
    Import,
    Pipe,
    Pattern,
    Effect,
    Generator,
    Style,
    Kernel,
    Domain,
    Type,
}

impl StrictCategory {
    fn as_str(self) -> &'static str {
        match self {
            StrictCategory::Syntax => "Syntax",
            StrictCategory::Import => "Import",
            StrictCategory::Pipe => "Pipe",
            StrictCategory::Pattern => "Pattern",
            StrictCategory::Effect => "Effect",
            StrictCategory::Generator => "Generator",
            StrictCategory::Style => "Style",
            StrictCategory::Kernel => "Kernel",
            StrictCategory::Domain => "Domain",
            StrictCategory::Type => "Type",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct StrictFix {
    pub(super) title: String,
    pub(super) edits: Vec<TextEdit>,
    pub(super) is_preferred: bool,
}

pub(super) fn expr_span(expr: &aivi::Expr) -> aivi::Span {
    match expr {
        aivi::Expr::Ident(n) => n.span.clone(),
        aivi::Expr::Literal(lit) => match lit {
            aivi::Literal::Number { span, .. }
            | aivi::Literal::String { span, .. }
            | aivi::Literal::Sigil { span, .. }
            | aivi::Literal::Bool { span, .. }
            | aivi::Literal::DateTime { span, .. } => span.clone(),
        },
        aivi::Expr::UnaryNeg { span, .. } => span.clone(),
        aivi::Expr::Suffixed { span, .. }
        | aivi::Expr::TextInterpolate { span, .. }
        | aivi::Expr::List { span, .. }
        | aivi::Expr::Tuple { span, .. }
        | aivi::Expr::Record { span, .. }
        | aivi::Expr::PatchLit { span, .. }
        | aivi::Expr::FieldAccess { span, .. }
        | aivi::Expr::FieldSection { span, .. }
        | aivi::Expr::Index { span, .. }
        | aivi::Expr::Call { span, .. }
        | aivi::Expr::Lambda { span, .. }
        | aivi::Expr::Match { span, .. }
        | aivi::Expr::If { span, .. }
        | aivi::Expr::Binary { span, .. }
        | aivi::Expr::CapabilityScope { span, .. }
        | aivi::Expr::Block { span, .. }
        | aivi::Expr::Raw { span, .. }
        | aivi::Expr::Mock { span, .. } => span.clone(),
    }
}

pub(super) fn diag_with_fix(
    code: &'static str,
    category: StrictCategory,
    severity: DiagnosticSeverity,
    message: String,
    range: tower_lsp::lsp_types::Range,
    fix: Option<StrictFix>,
) -> Diagnostic {
    Diagnostic {
        range,
        severity: Some(severity),
        code: Some(NumberOrString::String(code.to_string())),
        code_description: None,
        source: Some(format!("aivi.strict.{}", category.as_str())),
        message,
        related_information: None,
        tags: None,
        data: fix.map(|fix| {
            json!({
                "aiviQuickFix": {
                    "title": fix.title,
                    "isPreferred": fix.is_preferred,
                    "edits": fix.edits,
                }
            })
        }),
    }
}

pub(super) fn push_simple(
    out: &mut Vec<Diagnostic>,
    code: &'static str,
    category: StrictCategory,
    severity: DiagnosticSeverity,
    message: String,
    span: aivi::Span,
) {
    out.push(diag_with_fix(
        code,
        category,
        severity,
        message,
        Backend::span_to_range(span),
        None,
    ));
}

pub(crate) fn keywords_v01() -> HashSet<&'static str> {
    HashSet::from_iter(aivi::syntax::KEYWORDS_ALL.iter().copied())
}

pub(crate) fn is_invisible_unicode(ch: char) -> bool {
    // Keep this intentionally small and explicit; we want deterministic behavior without
    // a heavy Unicode dependency in the LSP.
    matches!(
        ch,
        '\u{00AD}' // soft hyphen
            | '\u{034F}' // combining grapheme joiner
            | '\u{061C}' // arabic letter mark
            | '\u{200B}' // zero width space
            | '\u{200C}' // zero width non-joiner
            | '\u{200D}' // zero width joiner
            | '\u{200E}' // lrm
            | '\u{200F}' // rlm
            | '\u{202A}'..='\u{202E}' // bidi embedding/override
            | '\u{2060}' // word joiner
            | '\u{2066}'..='\u{2069}' // bidi isolate controls
            | '\u{FEFF}' // zero width no-break space (bom)
    )
}

pub(crate) fn build_strict_diagnostics(
    text: &str,
    _uri: &Url,
    path: &Path,
    config: &StrictConfig,
    workspace_modules: &HashMap<String, crate::state::IndexedModule>,
) -> Vec<Diagnostic> {
    if config.level == StrictLevel::Off {
        return Vec::new();
    }

    // Strict diagnostics are a best-effort overlay; never let them crash the server.
    std::panic::catch_unwind(|| {
        let (file_modules, _parse_diags) = parse_modules(path, text);
        let all_modules = {
            let mut module_map: HashMap<String, Module> = HashMap::new();
            for module in embedded_stdlib_modules() {
                module_map.insert(module.name.name.clone(), module);
            }
            for indexed in workspace_modules.values() {
                module_map.insert(indexed.module.name.name.clone(), indexed.module.clone());
            }
            for module in file_modules.iter().cloned() {
                module_map.insert(module.name.name.clone(), module);
            }
            module_map.into_values().collect::<Vec<_>>()
        };

        let mut out = Vec::new();
        let (cst_tokens, _lex_diags) = lex_cst(text);

        if config.level as u8 >= StrictLevel::LexicalStructural as u8 {
            lexical::strict_lexical_and_structural(text, &cst_tokens, &mut out);
            record_syntax::strict_record_syntax_cst(&cst_tokens, &mut out);
            tuple_intent::strict_tuple_intent(&file_modules, &mut out);
            pattern_discipline::strict_block_shape(&file_modules, &mut out);
            pipe_discipline::strict_record_field_access(&file_modules, &mut out);
            pipe_discipline::strict_pipe_discipline(&file_modules, &mut out);
            pattern_discipline::strict_pattern_discipline(&file_modules, &mut out);
        }

        if config.level as u8 >= StrictLevel::NamesImports as u8 {
            imports_and_domains::strict_import_hygiene(&file_modules, &mut out);
            imports_and_domains::strict_missing_import_suggestions(
                &file_modules,
                &all_modules,
                &mut out,
            );
        }

        if config.level as u8 >= StrictLevel::TypesDomains as u8 {
            imports_and_domains::strict_domain_operator_heuristics(&file_modules, &mut out);
        }

        if config.level as u8 >= StrictLevel::NoImplicitCoercions as u8 {
            let file_module_names: HashSet<String> =
                file_modules.iter().map(|m| m.name.name.clone()).collect();
            imports_and_domains::strict_expected_type_coercions(
                &file_module_names,
                &all_modules,
                config,
                &mut out,
            );
        }

        if config.level as u8 >= StrictLevel::Pedantic as u8 {
            let span_hint =
                file_modules
                    .first()
                    .map(|m| m.name.span.clone())
                    .unwrap_or(aivi::Span {
                        start: aivi::Position { line: 1, column: 1 },
                        end: aivi::Position { line: 1, column: 1 },
                    });
            imports_and_domains::strict_kernel_consistency(&all_modules, span_hint, &mut out);
        }

        // Allow config to elevate warnings -> errors for strict-only diags.
        if config.warnings_as_errors {
            for diag in &mut out {
                if diag.severity == Some(DiagnosticSeverity::WARNING) {
                    diag.severity = Some(DiagnosticSeverity::ERROR);
                }
            }
        }

        out
    })
    .unwrap_or_default()
}
