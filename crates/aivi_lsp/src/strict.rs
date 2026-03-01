use std::collections::{HashMap, HashSet};
use std::path::Path;

use aivi::{
    elaborate_expected_coercions, embedded_stdlib_modules, lex_cst, lower_kernel, parse_modules,
    BlockKind, KernelExpr, KernelTextPart, Module,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, TextEdit, Url};

use crate::backend::Backend;

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
struct StrictFix {
    title: String,
    edits: Vec<TextEdit>,
    is_preferred: bool,
}

fn expr_span(expr: &aivi::Expr) -> aivi::Span {
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
        | aivi::Expr::Block { span, .. }
        | aivi::Expr::Raw { span, .. }
        | aivi::Expr::Mock { span, .. } => span.clone(),
    }
}

fn diag_with_fix(
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

fn push_simple(
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

fn keywords_v01() -> HashSet<&'static str> {
    HashSet::from_iter(aivi::syntax::KEYWORDS_ALL.iter().copied())
}

fn is_invisible_unicode(ch: char) -> bool {
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
            strict_lexical_and_structural(text, &cst_tokens, &mut out);
            strict_record_syntax_cst(&cst_tokens, &mut out);
            strict_tuple_intent(&file_modules, &mut out);
            strict_block_shape(&file_modules, &mut out);
            strict_record_field_access(&file_modules, &mut out);
            strict_pipe_discipline(&file_modules, &mut out);
            strict_pattern_discipline(&file_modules, &mut out);
        }

        if config.level as u8 >= StrictLevel::NamesImports as u8 {
            strict_import_hygiene(&file_modules, &mut out);
            strict_missing_import_suggestions(&file_modules, &all_modules, &mut out);
        }

        if config.level as u8 >= StrictLevel::TypesDomains as u8 {
            strict_domain_operator_heuristics(&file_modules, &mut out);
        }

        if config.level as u8 >= StrictLevel::NoImplicitCoercions as u8 {
            let file_module_names: HashSet<String> =
                file_modules.iter().map(|m| m.name.name.clone()).collect();
            strict_expected_type_coercions(&file_module_names, &all_modules, config, &mut out);
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
            strict_kernel_consistency(&all_modules, span_hint, &mut out);
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

fn strict_lexical_and_structural(
    text: &str,
    cst_tokens: &[aivi::CstToken],
    out: &mut Vec<Diagnostic>,
) {
    let keywords = keywords_v01();

    // 1) Invisible Unicode (whole-file scan; spans approximate per line/column).
    for (line_index, line) in text.lines().enumerate() {
        for (col_index, ch) in line.chars().enumerate() {
            if !is_invisible_unicode(ch) {
                continue;
            }
            let span = aivi::Span {
                start: aivi::Position {
                    line: line_index + 1,
                    column: col_index + 1,
                },
                end: aivi::Position {
                    line: line_index + 1,
                    column: col_index + 1,
                },
            };
            push_simple(
                out,
                "AIVI-S001",
                StrictCategory::Syntax,
                DiagnosticSeverity::ERROR,
                format!(
                    "AIVI-S001 [{}]\nInvisible Unicode character.\nFix: Remove the invisible character.",
                    StrictCategory::Syntax.as_str(),
                ),
                span,
            );
        }
    }

    // 2) Split arrow / split pipe tokens (`= >`, `| >`).
    // These are intentionally lexical: they should fire even if recovery continues.
    let mut i = 0usize;
    while i + 2 < cst_tokens.len() {
        let a = &cst_tokens[i];
        if a.kind != "symbol" || (a.text != "=" && a.text != "|") {
            i += 1;
            continue;
        }
        let ws = &cst_tokens[i + 1];
        let b = &cst_tokens[i + 2];
        if ws.kind != "whitespace" || b.kind != "symbol" || b.text != ">" {
            i += 1;
            continue;
        }
        let combined = if a.text == "=" { "=>" } else { "|>" };
        let code = if a.text == "=" {
            "AIVI-S014"
        } else {
            "AIVI-S015"
        };
        let category = StrictCategory::Syntax;
        let severity = DiagnosticSeverity::ERROR;
        let message = format!(
            "{code} [{}]\nMisplaced token.\nFound: \"{}{}{}\"\nExpected: \"{combined}\"\nFix: Replace with \"{combined}\".",
            category.as_str(),
            a.text,
            ws.text,
            b.text
        );
        let span = aivi::Span {
            start: a.span.start.clone(),
            end: b.span.end.clone(),
        };
        let edit = TextEdit {
            range: Backend::span_to_range(span.clone()),
            new_text: combined.to_string(),
        };
        out.push(diag_with_fix(
            code,
            category,
            severity,
            message,
            Backend::span_to_range(span),
            Some(StrictFix {
                title: format!("Replace with \"{combined}\""),
                edits: vec![edit],
                is_preferred: true,
            }),
        ));
        i += 3;
    }

    // 3) Identifier hygiene from lexical tokens (best-effort).
    for tok in cst_tokens {
        if tok.kind != "ident" {
            continue;
        }
        let name = tok.text.as_str();
        if keywords.contains(name) {
            push_simple(
                out,
                "AIVI-S002",
                StrictCategory::Syntax,
                DiagnosticSeverity::ERROR,
                format!(
                    "AIVI-S002 [{}]\nKeyword used as identifier.\nFound: \"{name}\"\nFix: Rename to a non-keyword identifier.",
                    StrictCategory::Syntax.as_str()
                ),
                tok.span.clone(),
            );
        }
        if name.contains("__") {
            push_simple(
                out,
                "AIVI-S003",
                StrictCategory::Style,
                DiagnosticSeverity::WARNING,
                format!(
                    "AIVI-S003 [{}]\nIdentifier contains \"__\".\nFound: \"{name}\"\nFix: Use a single '_' separator.",
                    StrictCategory::Style.as_str()
                ),
                tok.span.clone(),
            );
        }
        if name.starts_with('_') {
            push_simple(
                out,
                "AIVI-S004",
                StrictCategory::Style,
                DiagnosticSeverity::WARNING,
                format!(
                    "AIVI-S004 [{}]\nIdentifier starts with '_'.\nFound: \"{name}\"\nFix: Rename to start with a letter (values: a-z, types/modules: A-Z).",
                    StrictCategory::Style.as_str()
                ),
                tok.span.clone(),
            );
        }
        if name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            push_simple(
                out,
                "AIVI-S005",
                StrictCategory::Syntax,
                DiagnosticSeverity::ERROR,
                format!(
                    "AIVI-S005 [{}]\nIdentifier starts with a digit.\nFound: \"{name}\"\nFix: Rename to start with a letter.",
                    StrictCategory::Syntax.as_str()
                ),
                tok.span.clone(),
            );
        }
    }

    // 4) Tuple whitespace policy: no whitespace directly before ',' or ')' inside tuple parens.
    // This is a style restriction (not syntax) and only applies once we know the parens are a tuple.
    let mut paren_stack: Vec<(usize, bool)> = Vec::new(); // (index of '(', saw_comma_at_depth1)
    let mut depth = 0usize;
    for (idx, tok) in cst_tokens.iter().enumerate() {
        if tok.kind == "symbol" && tok.text == "(" {
            depth += 1;
            paren_stack.push((idx, false));
            continue;
        }
        if tok.kind == "symbol" && tok.text == ")" {
            if depth > 0 {
                depth -= 1;
                paren_stack.pop();
            }
            continue;
        }
        if tok.kind == "symbol" && tok.text == "," {
            if let Some((_open_idx, saw_comma)) = paren_stack.last_mut() {
                *saw_comma = true;
            }
            continue;
        }

        if tok.kind != "whitespace" {
            continue;
        }
        let Some((_open_idx, saw_comma)) = paren_stack.last().copied() else {
            continue;
        };
        if !saw_comma {
            continue;
        }
        let _prev = cst_tokens[..idx]
            .iter()
            .rfind(|t| t.kind != "whitespace" && t.kind != "comment");
        let next = cst_tokens[idx + 1..]
            .iter()
            .find(|t| t.kind != "whitespace" && t.kind != "comment");
        let (Some(_prev), Some(next)) = (_prev, next) else {
            continue;
        };
        if next.kind == "symbol" && (next.text == "," || next.text == ")") {
            push_simple(
                out,
                "AIVI-S006",
                StrictCategory::Style,
                DiagnosticSeverity::WARNING,
                format!(
                    "AIVI-S006 [{}]\nTrailing whitespace in tuple.\nFix: Remove whitespace before \"{}\".",
                    StrictCategory::Style.as_str(),
                    next.text
                ),
                tok.span.clone(),
            );
        }
    }
}

/// CST-level check for common record-literal mistakes such as using `=` instead
/// of `:` for field separators, or stray tokens inside `{ }`.
fn strict_record_syntax_cst(cst_tokens: &[aivi::CstToken], out: &mut Vec<Diagnostic>) {
    // Walk tokens tracking `{` / `}` depth.  Inside a `{ }` block that looks
    // like a record literal (has at least one `name :` pattern), flag:
    //   • `ident =` where `ident :` is expected  (AIVI-S016)
    //   • `ident / expr` and other operator-only tokens between commas/newlines
    //     that are not valid record-field syntax (AIVI-S017)
    //
    // We keep this deliberately conservative: only flag things that are
    // *unambiguously* wrong according to spec.  (`effect { }` / `generate { }`
    // blocks are not records and are excluded.)

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum BraceKind {
        Record,
        Other,
    }

    // Lightweight pre-scan to classify each `{` as record-like or other.
    // A brace is "record-like" when, at that nesting depth, we see at least one
    // `ident :` sequence that is *not* preceded by `effect`/`generate`/`resource`.
    let mut classified: Vec<(usize, usize, BraceKind)> = Vec::new(); // (open_idx, close_idx, kind)
    {
        #[derive(Clone)]
        struct Frame {
            open_idx: usize,
            saw_colon_field: bool,
            is_block_keyword: bool,
        }
        let mut stack: Vec<Frame> = Vec::new();
        let mut i = 0usize;
        while i < cst_tokens.len() {
            let tok = &cst_tokens[i];
            if tok.kind == "symbol" && tok.text == "{" {
                let is_block_kw = i > 0 && {
                    let prev = cst_tokens[..i]
                        .iter()
                        .rfind(|t| t.kind != "whitespace" && t.kind != "newline");
                    prev.is_some_and(|p| {
                        matches!(
                            p.text.as_str(),
                            "effect" | "generate" | "resource" | "=>" | "=" | "->" | "<-"
                        )
                    })
                };
                stack.push(Frame {
                    open_idx: i,
                    saw_colon_field: false,
                    is_block_keyword: is_block_kw,
                });
            } else if tok.kind == "symbol" && tok.text == "}" {
                if let Some(frame) = stack.pop() {
                    let kind = if frame.saw_colon_field && !frame.is_block_keyword {
                        BraceKind::Record
                    } else {
                        BraceKind::Other
                    };
                    classified.push((frame.open_idx, i, kind));
                }
            } else if tok.kind == "symbol" && tok.text == ":" {
                // Check if preceded by an ident at the same nesting depth.
                if let Some(frame) = stack.last_mut() {
                    let prev = cst_tokens[..i]
                        .iter()
                        .rfind(|t| t.kind != "whitespace" && t.kind != "newline");
                    if prev.is_some_and(|p| p.kind == "ident") {
                        frame.saw_colon_field = true;
                    }
                }
            }
            i += 1;
        }
    }

    // Build a set of record-brace ranges.
    let record_ranges: Vec<(usize, usize)> = classified
        .iter()
        .filter(|(_, _, k)| *k == BraceKind::Record)
        .map(|(o, c, _)| (*o, *c))
        .collect();

    // For each record range, look for `ident =` (wrong separator) and stray tokens.
    for &(open_idx, close_idx) in &record_ranges {
        let mut depth = 0isize;
        let mut j = open_idx + 1;
        while j < close_idx {
            let tok = &cst_tokens[j];
            if tok.kind == "whitespace" || tok.kind == "newline" || tok.kind == "comment" {
                j += 1;
                continue;
            }
            if tok.kind == "symbol" && tok.text == "{" {
                depth += 1;
                j += 1;
                continue;
            }
            if tok.kind == "symbol" && tok.text == "}" {
                depth -= 1;
                j += 1;
                continue;
            }
            // Only check at the top level of this record.
            if depth != 0 {
                j += 1;
                continue;
            }
            // Pattern: `ident =` at depth 0 → likely `name = value` instead of `name: value`.
            if tok.kind == "ident"
                && tok
                    .text
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_lowercase())
            {
                // Peek forward past whitespace for `=`.
                let next = cst_tokens[j + 1..close_idx]
                    .iter()
                    .find(|t| t.kind != "whitespace" && t.kind != "newline");
                if let Some(eq_tok) = next {
                    if eq_tok.kind == "symbol" && eq_tok.text == "=" {
                        // Confirm this is NOT part of a binding like `x = ...` at module level
                        // by checking that the `=` is not preceded by `:` on the same line.
                        // Inside a record brace, `name = value` is always wrong.
                        let span = aivi::Span {
                            start: tok.span.start.clone(),
                            end: eq_tok.span.end.clone(),
                        };
                        let field_name = &tok.text;
                        let edit = TextEdit {
                            range: Backend::span_to_range(eq_tok.span.clone()),
                            new_text: ":".to_string(),
                        };
                        out.push(diag_with_fix(
                            "AIVI-S016",
                            StrictCategory::Syntax,
                            DiagnosticSeverity::ERROR,
                            format!(
                                "AIVI-S016 [{}]\nInvalid record field separator.\nFound: `{field_name} =`\nExpected: `{field_name}: value`\nFix: Replace `=` with `:`.",
                                StrictCategory::Syntax.as_str()
                            ),
                            Backend::span_to_range(span),
                            Some(StrictFix {
                                title: "Replace `=` with `:`".to_string(),
                                edits: vec![edit],
                                is_preferred: true,
                            }),
                        ));
                    }
                }
            }
            j += 1;
        }
    }
}

fn strict_tuple_intent(file_modules: &[Module], out: &mut Vec<Diagnostic>) {
    fn walk_expr(expr: &aivi::Expr, out: &mut Vec<Diagnostic>) {
        match expr {
            aivi::Expr::Tuple { items, .. } => {
                for item in items {
                    // Strict: `(a b, c)` often means `(a, b, c)`; surface parse sees `a b` as a call.
                    if let aivi::Expr::Call { func, args, span } = item {
                        if matches!(&**func, aivi::Expr::Ident(_))
                            && args.len() == 1
                            && matches!(&args[0], aivi::Expr::Ident(_))
                        {
                            let func_span = expr_span(func);
                            let insert_at = aivi::Span {
                                start: func_span.end.clone(),
                                end: func_span.end.clone(),
                            };
                            let edit = TextEdit {
                                range: Backend::span_to_range(insert_at.clone()),
                                new_text: ",".to_string(),
                            };
                            let message = format!(
                                    "AIVI-S020 [{}]\nSuspicious tuple element.\nFound: function application inside a tuple element.\nHint: If you meant a 3-tuple, use commas.\nFix: Insert ',' after the first name.",
                                    StrictCategory::Syntax.as_str(),
                                );
                            out.push(diag_with_fix(
                                "AIVI-S020",
                                StrictCategory::Syntax,
                                DiagnosticSeverity::WARNING,
                                message,
                                Backend::span_to_range(span.clone()),
                                Some(StrictFix {
                                    title: "Insert missing comma".to_string(),
                                    edits: vec![edit],
                                    is_preferred: false,
                                }),
                            ));
                        }
                    }
                    walk_expr(item, out);
                }
            }
            aivi::Expr::List { items, .. } => {
                for item in items {
                    walk_expr(&item.expr, out);
                }
            }
            aivi::Expr::Record { fields, .. } | aivi::Expr::PatchLit { fields, .. } => {
                for field in fields {
                    walk_expr(&field.value, out);
                }
            }
            aivi::Expr::Call { func, args, .. } => {
                walk_expr(func, out);
                for arg in args {
                    walk_expr(arg, out);
                }
            }
            aivi::Expr::Lambda { body, .. } => walk_expr(body, out),
            aivi::Expr::Match {
                scrutinee, arms, ..
            } => {
                if let Some(scrutinee) = scrutinee {
                    walk_expr(scrutinee, out);
                }
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        walk_expr(guard, out);
                    }
                    walk_expr(&arm.body, out);
                }
            }
            aivi::Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                walk_expr(cond, out);
                walk_expr(then_branch, out);
                walk_expr(else_branch, out);
            }
            aivi::Expr::Binary { left, right, .. } => {
                walk_expr(left, out);
                walk_expr(right, out);
            }
            aivi::Expr::Block { items, .. } => {
                for item in items {
                    match item {
                        aivi::BlockItem::Bind { expr, .. }
                        | aivi::BlockItem::Let { expr, .. }
                        | aivi::BlockItem::Filter { expr, .. }
                        | aivi::BlockItem::Yield { expr, .. }
                        | aivi::BlockItem::Recurse { expr, .. }
                        | aivi::BlockItem::Expr { expr, .. } => walk_expr(expr, out),
                        aivi::BlockItem::When { cond, effect, .. }
                        | aivi::BlockItem::Unless { cond, effect, .. } => {
                            walk_expr(cond, out);
                            walk_expr(effect, out);
                        }
                        aivi::BlockItem::Given {
                            cond, fail_expr, ..
                        } => {
                            walk_expr(cond, out);
                            walk_expr(fail_expr, out);
                        }
                        aivi::BlockItem::On {
                            transition,
                            handler,
                            ..
                        } => {
                            walk_expr(transition, out);
                            walk_expr(handler, out);
                        }
                    }
                }
            }
            aivi::Expr::UnaryNeg { expr, .. } => walk_expr(expr, out),
            aivi::Expr::FieldAccess { base, .. }
            | aivi::Expr::Index { base, .. }
            | aivi::Expr::Suffixed { base, .. } => walk_expr(base, out),
            aivi::Expr::TextInterpolate { parts, .. } => {
                for part in parts {
                    if let aivi::TextPart::Expr { expr, .. } = part {
                        walk_expr(expr, out);
                    }
                }
            }
            aivi::Expr::Ident(_)
            | aivi::Expr::Literal(_)
            | aivi::Expr::FieldSection { .. }
            | aivi::Expr::Raw { .. } => {}
            aivi::Expr::Mock {
                substitutions,
                body,
                ..
            } => {
                for sub in substitutions {
                    if let Some(v) = &sub.value {
                        walk_expr(v, out);
                    }
                }
                walk_expr(body, out);
            }
        }
    }

    for module in file_modules {
        for item in &module.items {
            if let aivi::ModuleItem::Def(def) = item {
                walk_expr(&def.expr, out);
            }
            if let aivi::ModuleItem::DomainDecl(domain) = item {
                for d_item in &domain.items {
                    if let aivi::DomainItem::Def(def) | aivi::DomainItem::LiteralDef(def) = d_item {
                        walk_expr(&def.expr, out);
                    }
                }
            }
        }
    }
}

fn strict_pipe_discipline(file_modules: &[Module], out: &mut Vec<Diagnostic>) {
    fn walk_expr(expr: &aivi::Expr, out: &mut Vec<Diagnostic>) {
        match expr {
            aivi::Expr::Binary {
                op,
                left,
                right,
                span,
            } if op == "|>" => {
                // Rule: RHS should be "callable-ish" to avoid `x |> 1`-style mistakes.
                let rhs_callable = matches!(
                    &**right,
                    aivi::Expr::Ident(_)
                        | aivi::Expr::FieldAccess { .. }
                        | aivi::Expr::Lambda { .. }
                        | aivi::Expr::Call { .. }
                        | aivi::Expr::Match { .. }
                        | aivi::Expr::Block { .. }
                );
                if !rhs_callable {
                    push_simple(
                        out,
                        "AIVI-S100",
                        StrictCategory::Pipe,
                        DiagnosticSeverity::ERROR,
                        format!(
                            "AIVI-S100 [{}]\nPipe step is not callable.\nFix: Replace the right-hand side with a function (e.g. `x => ...`) or a function name.",
                            StrictCategory::Pipe.as_str()
                        ),
                        span.clone(),
                    );
                }
                // Rule: `x |> f a b` should usually be `x |> f _ a b` (explicit placeholder).
                if let aivi::Expr::Call { func, args, .. } = &**right {
                    if args.len() >= 2 && matches!(&**func, aivi::Expr::Ident(_)) {
                        let func_span = expr_span(func);
                        let insert_at = aivi::Span {
                            start: func_span.end.clone(),
                            end: func_span.end.clone(),
                        };
                        let edit = TextEdit {
                            range: Backend::span_to_range(insert_at.clone()),
                            new_text: " _".to_string(),
                        };
                        out.push(diag_with_fix(
                            "AIVI-S101",
                            StrictCategory::Pipe,
                            DiagnosticSeverity::WARNING,
                            format!(
                                "AIVI-S101 [{}]\nAmbiguous pipe step with multi-argument call.\nFound: a pipe step `f a b`.\nHint: Pipelines apply the left value as the final argument.\nFix: Insert `_` to make the intended argument position explicit.",
                                StrictCategory::Pipe.as_str()
                            ),
                            Backend::span_to_range(span.clone()),
                            Some(StrictFix {
                                title: "Insert `_` placeholder".to_string(),
                                edits: vec![edit],
                                is_preferred: false,
                            }),
                        ));
                    }
                }
                walk_expr(left, out);
                walk_expr(right, out);
            }
            _ => walk_expr_children(expr, out),
        }
    }

    fn walk_expr_children(expr: &aivi::Expr, out: &mut Vec<Diagnostic>) {
        match expr {
            aivi::Expr::Tuple { items, .. } => items.iter().for_each(|e| walk_expr(e, out)),
            aivi::Expr::List { items, .. } => items.iter().for_each(|i| walk_expr(&i.expr, out)),
            aivi::Expr::Record { fields, .. } | aivi::Expr::PatchLit { fields, .. } => {
                fields.iter().for_each(|f| walk_expr(&f.value, out))
            }
            aivi::Expr::Call { func, args, .. } => {
                walk_expr(func, out);
                args.iter().for_each(|a| walk_expr(a, out));
            }
            aivi::Expr::Lambda { body, .. } => walk_expr(body, out),
            aivi::Expr::Match {
                scrutinee, arms, ..
            } => {
                scrutinee.iter().for_each(|e| walk_expr(e, out));
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        walk_expr(guard, out);
                    }
                    walk_expr(&arm.body, out);
                }
            }
            aivi::Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                walk_expr(cond, out);
                walk_expr(then_branch, out);
                walk_expr(else_branch, out);
            }
            aivi::Expr::Binary { left, right, .. } => {
                walk_expr(left, out);
                walk_expr(right, out);
            }
            aivi::Expr::Block { items, .. } => {
                for item in items {
                    match item {
                        aivi::BlockItem::Bind { expr, .. }
                        | aivi::BlockItem::Let { expr, .. }
                        | aivi::BlockItem::Filter { expr, .. }
                        | aivi::BlockItem::Yield { expr, .. }
                        | aivi::BlockItem::Recurse { expr, .. }
                        | aivi::BlockItem::Expr { expr, .. } => walk_expr(expr, out),
                        aivi::BlockItem::When { cond, effect, .. }
                        | aivi::BlockItem::Unless { cond, effect, .. } => {
                            walk_expr(cond, out);
                            walk_expr(effect, out);
                        }
                        aivi::BlockItem::Given {
                            cond, fail_expr, ..
                        } => {
                            walk_expr(cond, out);
                            walk_expr(fail_expr, out);
                        }
                        aivi::BlockItem::On {
                            transition,
                            handler,
                            ..
                        } => {
                            walk_expr(transition, out);
                            walk_expr(handler, out);
                        }
                    }
                }
            }
            aivi::Expr::UnaryNeg { expr, .. } => walk_expr(expr, out),
            aivi::Expr::FieldAccess { base, .. }
            | aivi::Expr::Index { base, .. }
            | aivi::Expr::Suffixed { base, .. } => walk_expr(base, out),
            aivi::Expr::TextInterpolate { parts, .. } => {
                for part in parts {
                    if let aivi::TextPart::Expr { expr, .. } = part {
                        walk_expr(expr, out);
                    }
                }
            }
            aivi::Expr::Ident(_)
            | aivi::Expr::Literal(_)
            | aivi::Expr::FieldSection { .. }
            | aivi::Expr::Raw { .. } => {}
            aivi::Expr::Mock {
                substitutions,
                body,
                ..
            } => {
                for sub in substitutions {
                    if let Some(v) = &sub.value {
                        walk_expr(v, out);
                    }
                }
                walk_expr(body, out);
            }
        }
    }

    for module in file_modules {
        for item in &module.items {
            if let aivi::ModuleItem::Def(def) = item {
                walk_expr(&def.expr, out);
            }
            if let aivi::ModuleItem::DomainDecl(domain) = item {
                for d_item in &domain.items {
                    if let aivi::DomainItem::Def(def) | aivi::DomainItem::LiteralDef(def) = d_item {
                        walk_expr(&def.expr, out);
                    }
                }
            }
        }
    }
}

fn strict_record_field_access(file_modules: &[Module], out: &mut Vec<Diagnostic>) {
    fn walk_expr(expr: &aivi::Expr, out: &mut Vec<Diagnostic>) {
        match expr {
            aivi::Expr::FieldAccess { base, field, span } => {
                if let aivi::Expr::Record { fields, .. } = &**base {
                    let mut has = false;
                    for f in fields {
                        if let Some(aivi::PathSegment::Field(name)) = f.path.last() {
                            if name.name == field.name {
                                has = true;
                                break;
                            }
                        }
                    }
                    if !has {
                        push_simple(
                            out,
                            "AIVI-S140",
                            StrictCategory::Type,
                            DiagnosticSeverity::ERROR,
                            format!(
                                "AIVI-S140 [{}]\nUnknown field on record literal.\nFound: `.{}'\nFix: Use an existing field name or add the field to the record literal.",
                                StrictCategory::Type.as_str(),
                                field.name
                            ),
                            span.clone(),
                        );
                    }
                }
                walk_expr(base, out);
            }
            _ => walk_expr_children(expr, out),
        }
    }

    fn walk_expr_children(expr: &aivi::Expr, out: &mut Vec<Diagnostic>) {
        match expr {
            aivi::Expr::Tuple { items, .. } => items.iter().for_each(|e| walk_expr(e, out)),
            aivi::Expr::List { items, .. } => items.iter().for_each(|i| walk_expr(&i.expr, out)),
            aivi::Expr::Record { fields, .. } | aivi::Expr::PatchLit { fields, .. } => {
                if matches!(expr, aivi::Expr::PatchLit { .. }) && fields.is_empty() {
                    let span = match expr {
                        aivi::Expr::PatchLit { span, .. } => span.clone(),
                        _ => unreachable!(),
                    };
                    push_simple(
                        out,
                        "AIVI-S141",
                        StrictCategory::Style,
                        DiagnosticSeverity::WARNING,
                        format!(
                            "AIVI-S141 [{}]\nEmpty patch literal.\nFix: Remove it, or add at least one patch entry.",
                            StrictCategory::Style.as_str()
                        ),
                        span,
                    );
                }
                fields.iter().for_each(|f| walk_expr(&f.value, out))
            }
            aivi::Expr::Call { func, args, .. } => {
                walk_expr(func, out);
                args.iter().for_each(|a| walk_expr(a, out));
            }
            aivi::Expr::Lambda { body, .. } => walk_expr(body, out),
            aivi::Expr::Match {
                scrutinee, arms, ..
            } => {
                scrutinee.iter().for_each(|e| walk_expr(e, out));
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        walk_expr(guard, out);
                    }
                    walk_expr(&arm.body, out);
                }
            }
            aivi::Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                walk_expr(cond, out);
                walk_expr(then_branch, out);
                walk_expr(else_branch, out);
            }
            aivi::Expr::Binary { left, right, .. } => {
                walk_expr(left, out);
                walk_expr(right, out);
            }
            aivi::Expr::Block { items, .. } => {
                for item in items {
                    match item {
                        aivi::BlockItem::Bind { expr, .. }
                        | aivi::BlockItem::Let { expr, .. }
                        | aivi::BlockItem::Filter { expr, .. }
                        | aivi::BlockItem::Yield { expr, .. }
                        | aivi::BlockItem::Recurse { expr, .. }
                        | aivi::BlockItem::Expr { expr, .. } => walk_expr(expr, out),
                        aivi::BlockItem::When { cond, effect, .. }
                        | aivi::BlockItem::Unless { cond, effect, .. } => {
                            walk_expr(cond, out);
                            walk_expr(effect, out);
                        }
                        aivi::BlockItem::Given {
                            cond, fail_expr, ..
                        } => {
                            walk_expr(cond, out);
                            walk_expr(fail_expr, out);
                        }
                        aivi::BlockItem::On {
                            transition,
                            handler,
                            ..
                        } => {
                            walk_expr(transition, out);
                            walk_expr(handler, out);
                        }
                    }
                }
            }
            aivi::Expr::UnaryNeg { expr, .. } => walk_expr(expr, out),
            aivi::Expr::FieldAccess { base, .. }
            | aivi::Expr::Index { base, .. }
            | aivi::Expr::Suffixed { base, .. } => walk_expr(base, out),
            aivi::Expr::TextInterpolate { parts, .. } => {
                for part in parts {
                    if let aivi::TextPart::Expr { expr, .. } = part {
                        walk_expr(expr, out);
                    }
                }
            }
            aivi::Expr::Ident(_)
            | aivi::Expr::Literal(_)
            | aivi::Expr::FieldSection { .. }
            | aivi::Expr::Raw { .. } => {}
            aivi::Expr::Mock {
                substitutions,
                body,
                ..
            } => {
                for sub in substitutions {
                    if let Some(v) = &sub.value {
                        walk_expr(v, out);
                    }
                }
                walk_expr(body, out);
            }
        }
    }

    for module in file_modules {
        for item in &module.items {
            if let aivi::ModuleItem::Def(def) = item {
                walk_expr(&def.expr, out);
            }
            if let aivi::ModuleItem::DomainDecl(domain) = item {
                for d_item in &domain.items {
                    if let aivi::DomainItem::Def(def) | aivi::DomainItem::LiteralDef(def) = d_item {
                        walk_expr(&def.expr, out);
                    }
                }
            }
        }
    }
}

fn strict_pattern_discipline(file_modules: &[Module], out: &mut Vec<Diagnostic>) {
    fn pattern_binds_name(pat: &aivi::Pattern, name: &str) -> bool {
        match pat {
            aivi::Pattern::Ident(n) | aivi::Pattern::SubjectIdent(n) => n.name == name,
            aivi::Pattern::At {
                name: n, pattern, ..
            } => n.name == name || pattern_binds_name(pattern, name),
            aivi::Pattern::Tuple { items, .. } => items.iter().any(|p| pattern_binds_name(p, name)),
            aivi::Pattern::List { items, rest, .. } => {
                items.iter().any(|p| pattern_binds_name(p, name))
                    || rest.as_deref().is_some_and(|p| pattern_binds_name(p, name))
            }
            aivi::Pattern::Record { fields, .. } => {
                fields.iter().any(|f| pattern_binds_name(&f.pattern, name))
            }
            aivi::Pattern::Constructor { args, .. } => {
                args.iter().any(|p| pattern_binds_name(p, name))
            }
            aivi::Pattern::Wildcard(_) | aivi::Pattern::Literal(_) => false,
        }
    }

    fn collect_pattern_binders(pat: &aivi::Pattern, out: &mut Vec<aivi::SpannedName>) {
        match pat {
            aivi::Pattern::Ident(n) | aivi::Pattern::SubjectIdent(n) => out.push(n.clone()),
            aivi::Pattern::At { name, pattern, .. } => {
                out.push(name.clone());
                collect_pattern_binders(pattern, out);
            }
            aivi::Pattern::Tuple { items, .. } => {
                items.iter().for_each(|p| collect_pattern_binders(p, out))
            }
            aivi::Pattern::List { items, rest, .. } => {
                items.iter().for_each(|p| collect_pattern_binders(p, out));
                if let Some(rest) = rest.as_deref() {
                    collect_pattern_binders(rest, out);
                }
            }
            aivi::Pattern::Record { fields, .. } => fields
                .iter()
                .for_each(|f| collect_pattern_binders(&f.pattern, out)),
            aivi::Pattern::Constructor { args, .. } => {
                args.iter().for_each(|p| collect_pattern_binders(p, out))
            }
            aivi::Pattern::Wildcard(_) | aivi::Pattern::Literal(_) => {}
        }
    }

    fn expr_uses_name_free(expr: &aivi::Expr, name: &str) -> bool {
        match expr {
            aivi::Expr::Ident(n) => n.name == name,
            aivi::Expr::Tuple { items, .. } => items.iter().any(|e| expr_uses_name_free(e, name)),
            aivi::Expr::List { items, .. } => items
                .iter()
                .any(|item| expr_uses_name_free(&item.expr, name)),
            aivi::Expr::Record { fields, .. } | aivi::Expr::PatchLit { fields, .. } => {
                fields.iter().any(|f| expr_uses_name_free(&f.value, name))
            }
            aivi::Expr::Call { func, args, .. } => {
                expr_uses_name_free(func, name) || args.iter().any(|a| expr_uses_name_free(a, name))
            }
            aivi::Expr::Lambda { params, body, .. } => {
                if params.iter().any(|p| pattern_binds_name(p, name)) {
                    false
                } else {
                    expr_uses_name_free(body, name)
                }
            }
            aivi::Expr::Match {
                scrutinee, arms, ..
            } => {
                let scrutinee_uses = scrutinee
                    .as_ref()
                    .is_some_and(|e| expr_uses_name_free(e, name));
                if scrutinee_uses {
                    return true;
                }
                arms.iter().any(|arm| {
                    if pattern_binds_name(&arm.pattern, name) {
                        false
                    } else {
                        arm.guard
                            .as_ref()
                            .is_some_and(|g| expr_uses_name_free(g, name))
                            || expr_uses_name_free(&arm.body, name)
                    }
                })
            }
            aivi::Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                expr_uses_name_free(cond, name)
                    || expr_uses_name_free(then_branch, name)
                    || expr_uses_name_free(else_branch, name)
            }
            aivi::Expr::Binary { left, right, .. } => {
                expr_uses_name_free(left, name) || expr_uses_name_free(right, name)
            }
            aivi::Expr::Block { items, .. } => {
                let mut shadowed = false;
                for item in items {
                    match item {
                        aivi::BlockItem::Bind { pattern, expr, .. }
                        | aivi::BlockItem::Let { pattern, expr, .. } => {
                            if !shadowed && expr_uses_name_free(expr, name) {
                                return true;
                            }
                            if pattern_binds_name(pattern, name) {
                                shadowed = true;
                            }
                        }
                        aivi::BlockItem::Filter { expr, .. }
                        | aivi::BlockItem::Yield { expr, .. }
                        | aivi::BlockItem::Recurse { expr, .. }
                        | aivi::BlockItem::Expr { expr, .. } => {
                            if !shadowed && expr_uses_name_free(expr, name) {
                                return true;
                            }
                        }
                        aivi::BlockItem::When { cond, effect, .. }
                        | aivi::BlockItem::Unless { cond, effect, .. } => {
                            if !shadowed
                                && (expr_uses_name_free(cond, name)
                                    || expr_uses_name_free(effect, name))
                            {
                                return true;
                            }
                        }
                        aivi::BlockItem::Given {
                            cond, fail_expr, ..
                        } => {
                            if !shadowed
                                && (expr_uses_name_free(cond, name)
                                    || expr_uses_name_free(fail_expr, name))
                            {
                                return true;
                            }
                        }
                        aivi::BlockItem::On {
                            transition,
                            handler,
                            ..
                        } => {
                            if !shadowed
                                && (expr_uses_name_free(transition, name)
                                    || expr_uses_name_free(handler, name))
                            {
                                return true;
                            }
                        }
                    }
                }
                false
            }
            aivi::Expr::FieldAccess { base, .. }
            | aivi::Expr::Index { base, .. }
            | aivi::Expr::Suffixed { base, .. } => expr_uses_name_free(base, name),
            aivi::Expr::UnaryNeg { expr, .. } => expr_uses_name_free(expr, name),
            aivi::Expr::TextInterpolate { parts, .. } => parts.iter().any(|p| match p {
                aivi::TextPart::Text { .. } => false,
                aivi::TextPart::Expr { expr, .. } => expr_uses_name_free(expr, name),
            }),
            aivi::Expr::Literal(_) | aivi::Expr::FieldSection { .. } | aivi::Expr::Raw { .. } => {
                false
            }
            aivi::Expr::Mock {
                substitutions,
                body,
                ..
            } => {
                substitutions.iter().any(|sub| {
                    sub.value
                        .as_ref()
                        .is_some_and(|v| expr_uses_name_free(v, name))
                }) || expr_uses_name_free(body, name)
            }
        }
    }

    fn check_arms(arms: &[aivi::MatchArm], out: &mut Vec<Diagnostic>) {
        let mut saw_wildcard = false;
        for arm in arms {
            let mut binders = Vec::new();
            collect_pattern_binders(&arm.pattern, &mut binders);
            for binder in binders {
                if binder.name.starts_with('_') {
                    continue;
                }
                let used = arm
                    .guard
                    .as_ref()
                    .is_some_and(|g| expr_uses_name_free(g, &binder.name))
                    || expr_uses_name_free(&arm.body, &binder.name);
                if !used {
                    push_simple(
                        out,
                        "AIVI-S301",
                        StrictCategory::Pattern,
                        DiagnosticSeverity::WARNING,
                        format!(
                            "AIVI-S301 [{}]\nUnused pattern binding.\nFound: `{}`.\nFix: Use the value, or rename to `_`/`_name` to mark it intentionally unused. If you only want to assert the field exists, prefer matching with `_` (e.g. `age: _`).",
                            StrictCategory::Pattern.as_str(),
                            binder.name
                        ),
                        binder.span.clone(),
                    );
                }
            }
            if saw_wildcard {
                push_simple(
                    out,
                    "AIVI-S300",
                    StrictCategory::Pattern,
                    DiagnosticSeverity::WARNING,
                    format!(
                        "AIVI-S300 [{}]\nUnreachable match arm.\nReason: a previous arm is a wildcard `_`.\nFix: Move `_` arm to the end, or remove unreachable arms.",
                        StrictCategory::Pattern.as_str()
                    ),
                    arm.span.clone(),
                );
            }
            if matches!(arm.pattern, aivi::Pattern::Wildcard(_)) {
                saw_wildcard = true;
            }
        }
    }

    fn walk_expr(expr: &aivi::Expr, out: &mut Vec<Diagnostic>) {
        match expr {
            aivi::Expr::Match { arms, .. } => {
                check_arms(arms, out);
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        walk_expr(guard, out);
                    }
                    walk_expr(&arm.body, out);
                }
            }
            _ => walk_expr_children(expr, out),
        }
    }

    fn walk_expr_children(expr: &aivi::Expr, out: &mut Vec<Diagnostic>) {
        match expr {
            aivi::Expr::Tuple { items, .. } => items.iter().for_each(|e| walk_expr(e, out)),
            aivi::Expr::List { items, .. } => items.iter().for_each(|i| walk_expr(&i.expr, out)),
            aivi::Expr::Record { fields, .. } | aivi::Expr::PatchLit { fields, .. } => {
                fields.iter().for_each(|f| walk_expr(&f.value, out))
            }
            aivi::Expr::Call { func, args, .. } => {
                walk_expr(func, out);
                args.iter().for_each(|a| walk_expr(a, out));
            }
            aivi::Expr::Lambda { body, .. } => walk_expr(body, out),
            aivi::Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                walk_expr(cond, out);
                walk_expr(then_branch, out);
                walk_expr(else_branch, out);
            }
            aivi::Expr::Binary { left, right, .. } => {
                walk_expr(left, out);
                walk_expr(right, out);
            }
            aivi::Expr::Block { items, .. } => {
                for item in items {
                    match item {
                        aivi::BlockItem::Bind { expr, .. }
                        | aivi::BlockItem::Let { expr, .. }
                        | aivi::BlockItem::Filter { expr, .. }
                        | aivi::BlockItem::Yield { expr, .. }
                        | aivi::BlockItem::Recurse { expr, .. }
                        | aivi::BlockItem::Expr { expr, .. } => walk_expr(expr, out),
                        aivi::BlockItem::When { cond, effect, .. }
                        | aivi::BlockItem::Unless { cond, effect, .. } => {
                            walk_expr(cond, out);
                            walk_expr(effect, out);
                        }
                        aivi::BlockItem::Given {
                            cond, fail_expr, ..
                        } => {
                            walk_expr(cond, out);
                            walk_expr(fail_expr, out);
                        }
                        aivi::BlockItem::On {
                            transition,
                            handler,
                            ..
                        } => {
                            walk_expr(transition, out);
                            walk_expr(handler, out);
                        }
                    }
                }
            }
            aivi::Expr::UnaryNeg { expr, .. } => walk_expr(expr, out),
            aivi::Expr::FieldAccess { base, .. }
            | aivi::Expr::Index { base, .. }
            | aivi::Expr::Suffixed { base, .. } => walk_expr(base, out),
            aivi::Expr::TextInterpolate { parts, .. } => {
                for part in parts {
                    if let aivi::TextPart::Expr { expr, .. } = part {
                        walk_expr(expr, out);
                    }
                }
            }
            aivi::Expr::Ident(_)
            | aivi::Expr::Literal(_)
            | aivi::Expr::FieldSection { .. }
            | aivi::Expr::Raw { .. } => {}
            aivi::Expr::Mock {
                substitutions,
                body,
                ..
            } => {
                for sub in substitutions {
                    if let Some(v) = &sub.value {
                        walk_expr(v, out);
                    }
                }
                walk_expr(body, out);
            }
            aivi::Expr::Match { .. } => unreachable!(),
        }
    }

    for module in file_modules {
        for item in &module.items {
            if let aivi::ModuleItem::Def(def) = item {
                walk_expr(&def.expr, out);
            }
            if let aivi::ModuleItem::DomainDecl(domain) = item {
                for d_item in &domain.items {
                    if let aivi::DomainItem::Def(def) | aivi::DomainItem::LiteralDef(def) = d_item {
                        walk_expr(&def.expr, out);
                    }
                }
            }
        }
    }
}

fn strict_block_shape(file_modules: &[Module], out: &mut Vec<Diagnostic>) {
    fn names_in_pattern(pat: &aivi::Pattern, out: &mut Vec<String>) {
        match pat {
            aivi::Pattern::Ident(name) => out.push(name.name.clone()),
            aivi::Pattern::SubjectIdent(name) => out.push(name.name.clone()),
            aivi::Pattern::At { name, pattern, .. } => {
                out.push(name.name.clone());
                names_in_pattern(pattern, out);
            }
            aivi::Pattern::Tuple { items, .. } => {
                items.iter().for_each(|p| names_in_pattern(p, out))
            }
            aivi::Pattern::List { items, rest, .. } => {
                items.iter().for_each(|p| names_in_pattern(p, out));
                if let Some(rest) = rest.as_ref() {
                    names_in_pattern(rest, out);
                }
            }
            aivi::Pattern::Record { fields, .. } => fields
                .iter()
                .for_each(|f| names_in_pattern(&f.pattern, out)),
            aivi::Pattern::Constructor { args, .. } => {
                args.iter().for_each(|p| names_in_pattern(p, out))
            }
            aivi::Pattern::Wildcard(_) | aivi::Pattern::Literal(_) => {}
        }
    }

    fn expr_uses_name(expr: &aivi::Expr, name: &str) -> bool {
        match expr {
            aivi::Expr::Ident(n) => n.name == name,
            aivi::Expr::Tuple { items, .. } => items.iter().any(|e| expr_uses_name(e, name)),
            aivi::Expr::List { items, .. } => items.iter().any(|i| expr_uses_name(&i.expr, name)),
            aivi::Expr::Record { fields, .. } | aivi::Expr::PatchLit { fields, .. } => {
                fields.iter().any(|f| expr_uses_name(&f.value, name))
            }
            aivi::Expr::Call { func, args, .. } => {
                expr_uses_name(func, name) || args.iter().any(|a| expr_uses_name(a, name))
            }
            aivi::Expr::Lambda { params, body, .. } => {
                // Conservative: if the lambda binds the name, treat it as not used in body for outer scope.
                let mut bound = Vec::new();
                for p in params {
                    names_in_pattern(p, &mut bound);
                }
                if bound.iter().any(|b| b == name) {
                    false
                } else {
                    expr_uses_name(body, name)
                }
            }
            aivi::Expr::Match {
                scrutinee, arms, ..
            } => {
                scrutinee.as_ref().is_some_and(|e| expr_uses_name(e, name))
                    || arms.iter().any(|arm| {
                        expr_uses_name(&arm.body, name)
                            || arm.guard.as_ref().is_some_and(|g| expr_uses_name(g, name))
                    })
            }
            aivi::Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                expr_uses_name(cond, name)
                    || expr_uses_name(then_branch, name)
                    || expr_uses_name(else_branch, name)
            }
            aivi::Expr::Binary { left, right, .. } => {
                expr_uses_name(left, name) || expr_uses_name(right, name)
            }
            aivi::Expr::Block { items, .. } => items.iter().any(|item| match item {
                aivi::BlockItem::Bind { expr, .. }
                | aivi::BlockItem::Let { expr, .. }
                | aivi::BlockItem::Filter { expr, .. }
                | aivi::BlockItem::Yield { expr, .. }
                | aivi::BlockItem::Recurse { expr, .. }
                | aivi::BlockItem::Expr { expr, .. } => expr_uses_name(expr, name),
                aivi::BlockItem::When { cond, effect, .. }
                | aivi::BlockItem::Unless { cond, effect, .. } => {
                    expr_uses_name(cond, name) || expr_uses_name(effect, name)
                }
                aivi::BlockItem::Given {
                    cond, fail_expr, ..
                } => expr_uses_name(cond, name) || expr_uses_name(fail_expr, name),
                aivi::BlockItem::On {
                    transition,
                    handler,
                    ..
                } => expr_uses_name(transition, name) || expr_uses_name(handler, name),
            }),
            aivi::Expr::FieldAccess { base, .. }
            | aivi::Expr::Index { base, .. }
            | aivi::Expr::Suffixed { base, .. } => expr_uses_name(base, name),
            aivi::Expr::UnaryNeg { expr, .. } => expr_uses_name(expr, name),
            aivi::Expr::TextInterpolate { parts, .. } => parts.iter().any(|p| match p {
                aivi::TextPart::Text { .. } => false,
                aivi::TextPart::Expr { expr, .. } => expr_uses_name(expr, name),
            }),
            aivi::Expr::Literal(_) | aivi::Expr::FieldSection { .. } | aivi::Expr::Raw { .. } => {
                false
            }
            aivi::Expr::Mock {
                substitutions,
                body,
                ..
            } => {
                substitutions
                    .iter()
                    .any(|sub| sub.value.as_ref().is_some_and(|v| expr_uses_name(v, name)))
                    || expr_uses_name(body, name)
            }
        }
    }

    fn check_block(kind: BlockKind, items: &[aivi::BlockItem], out: &mut Vec<Diagnostic>) {
        // Rule: block last item should be an expression/yield, not a binding.
        if let Some(last) = items.last() {
            if matches!(
                last,
                aivi::BlockItem::Let { .. } | aivi::BlockItem::Bind { .. }
            ) {
                let span = match last {
                    aivi::BlockItem::Let { span, .. } | aivi::BlockItem::Bind { span, .. } => {
                        span.clone()
                    }
                    _ => unreachable!(),
                };
                let cat = match kind {
                    BlockKind::Do { .. } => StrictCategory::Effect,
                    BlockKind::Generate => StrictCategory::Generator,
                    _ => StrictCategory::Style,
                };
                push_simple(
                    out,
                    "AIVI-S220",
                    cat,
                    DiagnosticSeverity::WARNING,
                    format!(
                        "AIVI-S220 [{}]\nBlock ends with a binding.\nFix: Add a final expression (the block result), or convert the binding into a pure expression.",
                        cat.as_str()
                    ),
                    span,
                );
            }
        }

        // Rule: unused bound names inside a block (simple forward-use check).
        for (idx, item) in items.iter().enumerate() {
            let (pat, expr, span) = match item {
                aivi::BlockItem::Bind {
                    pattern,
                    expr,
                    span,
                } => (Some(pattern), Some(expr), Some(span)),
                aivi::BlockItem::Let {
                    pattern,
                    expr,
                    span,
                } => (Some(pattern), Some(expr), Some(span)),
                _ => (None, None, None),
            };
            let (Some(pat), Some(_expr), Some(span)) = (pat, expr, span) else {
                continue;
            };
            let mut bound = Vec::new();
            names_in_pattern(pat, &mut bound);
            if bound.is_empty() {
                continue;
            }
            let rest_items = &items[idx + 1..];
            for name in bound {
                let used_later = rest_items.iter().any(|it| match it {
                    aivi::BlockItem::Bind { expr, .. }
                    | aivi::BlockItem::Let { expr, .. }
                    | aivi::BlockItem::Filter { expr, .. }
                    | aivi::BlockItem::Yield { expr, .. }
                    | aivi::BlockItem::Recurse { expr, .. }
                    | aivi::BlockItem::Expr { expr, .. } => expr_uses_name(expr, &name),
                    aivi::BlockItem::When { cond, effect, .. }
                    | aivi::BlockItem::Unless { cond, effect, .. } => {
                        expr_uses_name(cond, &name) || expr_uses_name(effect, &name)
                    }
                    aivi::BlockItem::Given {
                        cond, fail_expr, ..
                    } => expr_uses_name(cond, &name) || expr_uses_name(fail_expr, &name),
                    aivi::BlockItem::On {
                        transition,
                        handler,
                        ..
                    } => expr_uses_name(transition, &name) || expr_uses_name(handler, &name),
                });
                if !used_later && !name.starts_with('_') {
                    push_simple(
                        out,
                        "AIVI-S221",
                        StrictCategory::Style,
                        DiagnosticSeverity::WARNING,
                        format!(
                            "AIVI-S221 [{}]\nUnused binding in block.\nFound: `{name}`.\nFix: Use the value, or rename to `_`/`_name` to mark it intentionally unused.",
                            StrictCategory::Style.as_str()
                        ),
                        span.clone(),
                    );
                }
            }
        }
    }

    fn walk_expr(expr: &aivi::Expr, out: &mut Vec<Diagnostic>) {
        if let aivi::Expr::Block { kind, items, .. } = expr {
            check_block(kind.clone(), items, out);
            for item in items {
                match item {
                    aivi::BlockItem::Bind { expr, .. }
                    | aivi::BlockItem::Let { expr, .. }
                    | aivi::BlockItem::Filter { expr, .. }
                    | aivi::BlockItem::Yield { expr, .. }
                    | aivi::BlockItem::Recurse { expr, .. }
                    | aivi::BlockItem::Expr { expr, .. } => walk_expr(expr, out),
                    aivi::BlockItem::When { cond, effect, .. }
                    | aivi::BlockItem::Unless { cond, effect, .. } => {
                        walk_expr(cond, out);
                        walk_expr(effect, out);
                    }
                    aivi::BlockItem::Given {
                        cond, fail_expr, ..
                    } => {
                        walk_expr(cond, out);
                        walk_expr(fail_expr, out);
                    }
                    aivi::BlockItem::On {
                        transition,
                        handler,
                        ..
                    } => {
                        walk_expr(transition, out);
                        walk_expr(handler, out);
                    }
                }
            }
        }
        // Keep it small: other passes already walk expressions.
    }

    for module in file_modules {
        for item in &module.items {
            if let aivi::ModuleItem::Def(def) = item {
                walk_expr(&def.expr, out);
            }
            if let aivi::ModuleItem::DomainDecl(domain) = item {
                for d_item in &domain.items {
                    if let aivi::DomainItem::Def(def) | aivi::DomainItem::LiteralDef(def) = d_item {
                        walk_expr(&def.expr, out);
                    }
                }
            }
        }
    }
}

fn strict_import_hygiene(file_modules: &[Module], out: &mut Vec<Diagnostic>) {
    for module in file_modules {
        let mut seen_use: HashSet<(&str, bool)> = HashSet::new();
        for use_decl in &module.uses {
            let key = (use_decl.module.name.as_str(), use_decl.wildcard);
            if !seen_use.insert(key) {
                push_simple(
                    out,
                    "AIVI-S200",
                    StrictCategory::Import,
                    DiagnosticSeverity::WARNING,
                    format!(
                        "AIVI-S200 [{}]\nDuplicate `use` declaration.\nFound: `use {}`.\nFix: Remove the duplicate import.",
                        StrictCategory::Import.as_str(),
                        use_decl.module.name
                    ),
                    use_decl.module.span.clone(),
                );
            }
        }
    }
}

fn strict_missing_import_suggestions(
    file_modules: &[Module],
    all_modules: &[Module],
    out: &mut Vec<Diagnostic>,
) {
    // Best-effort: if a name is used but not defined in the module, suggest a `use`.
    // We intentionally keep this heuristic simple and only fire when there's a single obvious provider.
    let mut providers: HashMap<&str, Vec<&str>> = HashMap::new();
    for m in all_modules {
        for export in &m.exports {
            providers
                .entry(export.name.name.as_str())
                .or_default()
                .push(m.name.name.as_str());
        }
    }

    fn collect_idents(expr: &aivi::Expr, out: &mut Vec<aivi::SpannedName>) {
        match expr {
            aivi::Expr::Ident(n) => out.push(n.clone()),
            aivi::Expr::Tuple { items, .. } => items.iter().for_each(|e| collect_idents(e, out)),
            aivi::Expr::List { items, .. } => {
                items.iter().for_each(|i| collect_idents(&i.expr, out))
            }
            aivi::Expr::Record { fields, .. } | aivi::Expr::PatchLit { fields, .. } => {
                fields.iter().for_each(|f| collect_idents(&f.value, out))
            }
            aivi::Expr::Call { func, args, .. } => {
                collect_idents(func, out);
                args.iter().for_each(|a| collect_idents(a, out));
            }
            aivi::Expr::Lambda { body, .. } => collect_idents(body, out),
            aivi::Expr::Match {
                scrutinee, arms, ..
            } => {
                if let Some(s) = scrutinee {
                    collect_idents(s, out);
                }
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        collect_idents(guard, out);
                    }
                    collect_idents(&arm.body, out);
                }
            }
            aivi::Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                collect_idents(cond, out);
                collect_idents(then_branch, out);
                collect_idents(else_branch, out);
            }
            aivi::Expr::Binary { left, right, .. } => {
                collect_idents(left, out);
                collect_idents(right, out);
            }
            aivi::Expr::Block { items, .. } => {
                for item in items {
                    match item {
                        aivi::BlockItem::Bind { expr, .. }
                        | aivi::BlockItem::Let { expr, .. }
                        | aivi::BlockItem::Filter { expr, .. }
                        | aivi::BlockItem::Yield { expr, .. }
                        | aivi::BlockItem::Recurse { expr, .. }
                        | aivi::BlockItem::Expr { expr, .. } => collect_idents(expr, out),
                        aivi::BlockItem::When { cond, effect, .. }
                        | aivi::BlockItem::Unless { cond, effect, .. } => {
                            collect_idents(cond, out);
                            collect_idents(effect, out);
                        }
                        aivi::BlockItem::Given {
                            cond, fail_expr, ..
                        } => {
                            collect_idents(cond, out);
                            collect_idents(fail_expr, out);
                        }
                        aivi::BlockItem::On {
                            transition,
                            handler,
                            ..
                        } => {
                            collect_idents(transition, out);
                            collect_idents(handler, out);
                        }
                    }
                }
            }
            aivi::Expr::UnaryNeg { expr, .. } => collect_idents(expr, out),
            aivi::Expr::FieldAccess { base, .. }
            | aivi::Expr::Index { base, .. }
            | aivi::Expr::Suffixed { base, .. } => collect_idents(base, out),
            aivi::Expr::TextInterpolate { parts, .. } => {
                for part in parts {
                    if let aivi::TextPart::Expr { expr, .. } = part {
                        collect_idents(expr, out);
                    }
                }
            }
            aivi::Expr::Literal(_) | aivi::Expr::FieldSection { .. } | aivi::Expr::Raw { .. } => {}
            aivi::Expr::Mock {
                substitutions,
                body,
                ..
            } => {
                for sub in substitutions {
                    if let Some(v) = &sub.value {
                        collect_idents(v, out);
                    }
                }
                collect_idents(body, out);
            }
        }
    }

    for module in file_modules {
        // Diagnostics are published per-document; only emit for modules defined in this file.

        let mut defined: HashSet<&str> = HashSet::new();
        for item in &module.items {
            match item {
                aivi::ModuleItem::Def(def) => {
                    defined.insert(def.name.name.as_str());
                }
                aivi::ModuleItem::TypeSig(sig) => {
                    defined.insert(sig.name.name.as_str());
                }
                _ => {}
            }
        }
        for use_decl in &module.uses {
            if use_decl.wildcard {
                // We treat wildcard as defining all exports; no suggestions.
                continue;
            }
            for it in &use_decl.items {
                defined.insert(it.name.name.as_str());
            }
        }

        for item in &module.items {
            let aivi::ModuleItem::Def(def) = item else {
                continue;
            };
            let mut used = Vec::new();
            collect_idents(&def.expr, &mut used);
            for name in used {
                if defined.contains(name.name.as_str()) {
                    continue;
                }
                // Prefer suggestions for UpperIdent constructors/types used as values.
                if !name
                    .name
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_uppercase())
                {
                    continue;
                }
                let Some(cands) = providers.get(name.name.as_str()) else {
                    continue;
                };
                if cands.len() != 1 {
                    continue;
                }
                let module_name = cands[0];
                let insert_span = module.name.span.clone();
                let insert_at = aivi::Span {
                    start: insert_span.start.clone(),
                    end: insert_span.start.clone(),
                };
                let edit = TextEdit {
                    range: Backend::span_to_range(insert_at),
                    new_text: format!("use {module_name}\n\n"),
                };
                out.push(diag_with_fix(
                    "AIVI-S201",
                    StrictCategory::Import,
                    DiagnosticSeverity::WARNING,
                    format!(
                        "AIVI-S201 [{}]\nName not in scope.\nFound: `{}`.\nFix: Insert `use {module_name}`.",
                        StrictCategory::Import.as_str(),
                        name.name
                    ),
                    Backend::span_to_range(name.span.clone()),
                    Some(StrictFix {
                        title: format!("Insert `use {module_name}`"),
                        edits: vec![edit],
                        is_preferred: false,
                    }),
                ));
            }
        }
    }
}

fn strict_domain_operator_heuristics(file_modules: &[Module], out: &mut Vec<Diagnostic>) {
    // This is intentionally heuristic until we have typed spans in the LSP.
    // Emit a warning for `Date + Int`-like shapes (common footgun: missing unit/delta domain).
    fn walk_expr(expr: &aivi::Expr, out: &mut Vec<Diagnostic>) {
        match expr {
            aivi::Expr::Binary {
                op,
                left,
                right,
                span,
            } if op == "+" || op == "-" => {
                let left_is_date_like = matches!(
                    &**left,
                    aivi::Expr::Ident(n) if n.name.to_lowercase().contains("date")
                );
                let right_is_number =
                    matches!(&**right, aivi::Expr::Literal(aivi::Literal::Number { .. }));
                if left_is_date_like && right_is_number {
                    push_simple(
                        out,
                        "AIVI-S400",
                        StrictCategory::Domain,
                        DiagnosticSeverity::WARNING,
                        format!(
                            "AIVI-S400 [{}]\nPotential domain ambiguity.\nFound: date-like value `{}` {} numeric literal.\nFix: Use an explicit delta/unit (e.g. `1day`, `24h`) or a domain-specific add function.",
                            StrictCategory::Domain.as_str(),
                            match &**left { aivi::Expr::Ident(n) => &n.name, _ => "?" },
                            op
                        ),
                        span.clone(),
                    );
                }
                walk_expr(left, out);
                walk_expr(right, out);
            }
            _ => {}
        }
        // Keep this pass shallow; other checks already traverse expressions.
    }

    for module in file_modules {
        for item in &module.items {
            if let aivi::ModuleItem::Def(def) = item {
                walk_expr(&def.expr, out);
            }
        }
    }
}

fn strict_expected_type_coercions(
    file_module_names: &HashSet<String>,
    all_modules: &[Module],
    config: &StrictConfig,
    out: &mut Vec<Diagnostic>,
) {
    // Best-effort: run expected-type elaboration on a clone, then detect inserted `toText`/`TextNode`.
    // This catches the most impactful implicit coercions without requiring exposing typed spans yet.
    let mut modules = all_modules.to_vec();
    let diags = elaborate_expected_coercions(&mut modules);
    // If elaboration itself emits type errors, the normal typechecker already covers that.
    let _ = diags;

    fn collect_calls(expr: &aivi::Expr, out: &mut Vec<(String, aivi::Span, aivi::Span)>) {
        match expr {
            aivi::Expr::Call { func, args, span } => {
                if let aivi::Expr::Ident(name) = &**func {
                    if name.name == "toText" || name.name == "TextNode" {
                        let arg_span = args.first().map(expr_span);
                        if let Some(arg_span) = arg_span {
                            out.push((name.name.clone(), span.clone(), arg_span));
                        }
                    }
                }
                collect_calls(func, out);
                for arg in args {
                    collect_calls(arg, out);
                }
            }
            aivi::Expr::Tuple { items, .. } => items.iter().for_each(|e| collect_calls(e, out)),
            aivi::Expr::List { items, .. } => {
                items.iter().for_each(|i| collect_calls(&i.expr, out))
            }
            aivi::Expr::Record { fields, .. } | aivi::Expr::PatchLit { fields, .. } => {
                fields.iter().for_each(|f| collect_calls(&f.value, out))
            }
            aivi::Expr::Lambda { body, .. } => collect_calls(body, out),
            aivi::Expr::Match {
                scrutinee, arms, ..
            } => {
                if let Some(s) = scrutinee {
                    collect_calls(s, out);
                }
                for arm in arms {
                    if let Some(g) = &arm.guard {
                        collect_calls(g, out);
                    }
                    collect_calls(&arm.body, out);
                }
            }
            aivi::Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                collect_calls(cond, out);
                collect_calls(then_branch, out);
                collect_calls(else_branch, out);
            }
            aivi::Expr::Binary { left, right, .. } => {
                collect_calls(left, out);
                collect_calls(right, out);
            }
            aivi::Expr::Block { items, .. } => {
                for item in items {
                    match item {
                        aivi::BlockItem::Bind { expr, .. }
                        | aivi::BlockItem::Let { expr, .. }
                        | aivi::BlockItem::Filter { expr, .. }
                        | aivi::BlockItem::Yield { expr, .. }
                        | aivi::BlockItem::Recurse { expr, .. }
                        | aivi::BlockItem::Expr { expr, .. } => collect_calls(expr, out),
                        aivi::BlockItem::When { cond, effect, .. }
                        | aivi::BlockItem::Unless { cond, effect, .. } => {
                            collect_calls(cond, out);
                            collect_calls(effect, out);
                        }
                        aivi::BlockItem::Given {
                            cond, fail_expr, ..
                        } => {
                            collect_calls(cond, out);
                            collect_calls(fail_expr, out);
                        }
                        aivi::BlockItem::On {
                            transition,
                            handler,
                            ..
                        } => {
                            collect_calls(transition, out);
                            collect_calls(handler, out);
                        }
                    }
                }
            }
            aivi::Expr::UnaryNeg { expr, .. } => collect_calls(expr, out),
            aivi::Expr::FieldAccess { base, .. }
            | aivi::Expr::Index { base, .. }
            | aivi::Expr::Suffixed { base, .. } => collect_calls(base, out),
            aivi::Expr::TextInterpolate { parts, .. } => {
                for p in parts {
                    if let aivi::TextPart::Expr { expr, .. } = p {
                        collect_calls(expr, out);
                    }
                }
            }
            aivi::Expr::Ident(_)
            | aivi::Expr::Literal(_)
            | aivi::Expr::FieldSection { .. }
            | aivi::Expr::Raw { .. } => {}
            aivi::Expr::Mock {
                substitutions,
                body,
                ..
            } => {
                for sub in substitutions {
                    if let Some(v) = &sub.value {
                        collect_calls(v, out);
                    }
                }
                collect_calls(body, out);
            }
        }
    }

    // Compare elaborated defs to a non-elaborated parse to find newly-introduced coercions.
    // This is span-based; if the user already wrote `toText`, we won't flag unless the coercion
    // appears at a span that used to be non-coercing.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct SpanKey {
        sl: usize,
        sc: usize,
        el: usize,
        ec: usize,
    }

    impl From<&aivi::Span> for SpanKey {
        fn from(span: &aivi::Span) -> Self {
            Self {
                sl: span.start.line,
                sc: span.start.column,
                el: span.end.line,
                ec: span.end.column,
            }
        }
    }

    let mut original: HashMap<(String, String), HashSet<(String, SpanKey)>> = HashMap::new(); // (module, def) -> {(callee, call_span)}
    for m in all_modules.iter().cloned() {
        for item in &m.items {
            if let aivi::ModuleItem::Def(def) = item {
                let mut calls = Vec::new();
                collect_calls(&def.expr, &mut calls);
                let mut set = HashSet::new();
                for (callee, call_span, _arg_span) in calls {
                    set.insert((callee, SpanKey::from(&call_span)));
                }
                original.insert((m.name.name.clone(), def.name.name.clone()), set);
            }
        }
    }

    for m in &modules {
        if !file_module_names.contains(&m.name.name) {
            continue;
        }
        for item in &m.items {
            let aivi::ModuleItem::Def(def) = item else {
                continue;
            };
            let mut calls = Vec::new();
            collect_calls(&def.expr, &mut calls);
            let key = (m.name.name.clone(), def.name.name.clone());
            let before = original.get(&key);
            for (callee, call_span, arg_span) in calls {
                if before.is_some_and(|b| b.contains(&(callee.clone(), SpanKey::from(&call_span))))
                {
                    continue;
                }
                if call_span.start.line == 0 {
                    continue;
                }
                let sev = if config.forbid_implicit_coercions {
                    DiagnosticSeverity::ERROR
                } else {
                    DiagnosticSeverity::WARNING
                };
                let code = "AIVI-S500";
                out.push(diag_with_fix(
                    code,
                    StrictCategory::Type,
                    sev,
                    format!(
                        "{code} [{}]\nImplicit coercion inserted.\nFound: compiler-inserted `{callee}`.\nFix: Write the coercion explicitly (e.g. `{callee} <expr>`) or adjust types to avoid it.",
                        StrictCategory::Type.as_str(),
                    ),
                    Backend::span_to_range(arg_span),
                    None,
                ));
            }
        }
    }
}

fn strict_kernel_consistency(
    all_modules: &[Module],
    span_hint: aivi::Span,
    out: &mut Vec<Diagnostic>,
) {
    // Best-effort: lower to kernel and validate shallow invariants.
    let kernel = match std::panic::catch_unwind(|| {
        let hir = aivi::desugar_modules(all_modules);
        lower_kernel(hir)
    }) {
        Ok(k) => k,
        Err(_) => return,
    };

    // Kernel does not carry spans. Attach "compiler bug" invariants to the current file's
    // first module span so they are visible but scoped to the active document.
    let mut seen_ids = HashSet::new();

    fn walk_expr(
        expr: &KernelExpr,
        seen_ids: &mut HashSet<u32>,
        span_hint: &aivi::Span,
        out: &mut Vec<Diagnostic>,
    ) {
        let id = match expr {
            KernelExpr::Var { id, .. }
            | KernelExpr::LitNumber { id, .. }
            | KernelExpr::LitString { id, .. }
            | KernelExpr::TextInterpolate { id, .. }
            | KernelExpr::LitSigil { id, .. }
            | KernelExpr::LitBool { id, .. }
            | KernelExpr::LitDateTime { id, .. }
            | KernelExpr::Lambda { id, .. }
            | KernelExpr::App { id, .. }
            | KernelExpr::Call { id, .. }
            | KernelExpr::DebugFn { id, .. }
            | KernelExpr::Pipe { id, .. }
            | KernelExpr::List { id, .. }
            | KernelExpr::Tuple { id, .. }
            | KernelExpr::Record { id, .. }
            | KernelExpr::Patch { id, .. }
            | KernelExpr::FieldAccess { id, .. }
            | KernelExpr::Index { id, .. }
            | KernelExpr::Match { id, .. }
            | KernelExpr::If { id, .. }
            | KernelExpr::Binary { id, .. }
            | KernelExpr::Raw { id, .. }
            | KernelExpr::Mock { id, .. } => *id,
        };

        if !seen_ids.insert(id) {
            push_simple(
                out,
                "AIVI-S900",
                StrictCategory::Kernel,
                DiagnosticSeverity::WARNING,
                format!(
                    "AIVI-S900 [{}]\nKernel invariant violated.\nFound: duplicate expression id `{id}`.\nFix: Report a compiler bug (kernel ids must be unique).",
                    StrictCategory::Kernel.as_str()
                ),
                span_hint.clone(),
            );
        }

        match expr {
            KernelExpr::Lambda { body, .. } => walk_expr(body, seen_ids, span_hint, out),
            KernelExpr::App { func, arg, .. } => {
                walk_expr(func, seen_ids, span_hint, out);
                walk_expr(arg, seen_ids, span_hint, out);
            }
            KernelExpr::Call { func, args, .. } => {
                walk_expr(func, seen_ids, span_hint, out);
                for a in args {
                    walk_expr(a, seen_ids, span_hint, out);
                }
            }
            KernelExpr::DebugFn { body, .. } => walk_expr(body, seen_ids, span_hint, out),
            KernelExpr::Pipe { func, arg, .. } => {
                walk_expr(func, seen_ids, span_hint, out);
                walk_expr(arg, seen_ids, span_hint, out);
            }
            KernelExpr::TextInterpolate { parts, .. } => {
                for p in parts {
                    if let KernelTextPart::Expr { expr } = p {
                        walk_expr(expr, seen_ids, span_hint, out);
                    }
                }
            }
            KernelExpr::List { items, .. } => {
                for it in items {
                    walk_expr(&it.expr, seen_ids, span_hint, out);
                }
            }
            KernelExpr::Tuple { items, .. } => {
                if items.is_empty() {
                    push_simple(
                        out,
                        "AIVI-S901",
                        StrictCategory::Kernel,
                        DiagnosticSeverity::WARNING,
                        format!(
                            "AIVI-S901 [{}]\nKernel invariant violated.\nFound: empty tuple.\nFix: Report a compiler bug (tuples are 2+ items in surface syntax).",
                            StrictCategory::Kernel.as_str()
                        ),
                        span_hint.clone(),
                    );
                }
                for it in items {
                    walk_expr(it, seen_ids, span_hint, out);
                }
            }
            KernelExpr::Record { fields, .. } => {
                for f in fields {
                    walk_expr(&f.value, seen_ids, span_hint, out);
                }
            }
            KernelExpr::Patch { target, fields, .. } => {
                walk_expr(target, seen_ids, span_hint, out);
                for f in fields {
                    walk_expr(&f.value, seen_ids, span_hint, out);
                }
            }
            KernelExpr::FieldAccess { base, .. } => walk_expr(base, seen_ids, span_hint, out),
            KernelExpr::Index { base, index, .. } => {
                walk_expr(base, seen_ids, span_hint, out);
                walk_expr(index, seen_ids, span_hint, out);
            }
            KernelExpr::Match {
                scrutinee, arms, ..
            } => {
                walk_expr(scrutinee, seen_ids, span_hint, out);
                for arm in arms {
                    if let Some(g) = &arm.guard {
                        walk_expr(g, seen_ids, span_hint, out);
                    }
                    walk_expr(&arm.body, seen_ids, span_hint, out);
                }
            }
            KernelExpr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                walk_expr(cond, seen_ids, span_hint, out);
                walk_expr(then_branch, seen_ids, span_hint, out);
                walk_expr(else_branch, seen_ids, span_hint, out);
            }
            KernelExpr::Binary { left, right, .. } => {
                walk_expr(left, seen_ids, span_hint, out);
                walk_expr(right, seen_ids, span_hint, out);
            }
            KernelExpr::Var { .. }
            | KernelExpr::LitNumber { .. }
            | KernelExpr::LitString { .. }
            | KernelExpr::LitSigil { .. }
            | KernelExpr::LitBool { .. }
            | KernelExpr::LitDateTime { .. }
            | KernelExpr::Raw { .. } => {}
            KernelExpr::Mock {
                substitutions,
                body,
                ..
            } => {
                for sub in substitutions {
                    if let Some(v) = &sub.value {
                        walk_expr(v, seen_ids, span_hint, out);
                    }
                }
                walk_expr(body, seen_ids, span_hint, out);
            }
        }
    }

    for module in &kernel.modules {
        for def in &module.defs {
            walk_expr(&def.expr, &mut seen_ids, &span_hint, out);
        }
    }
}
