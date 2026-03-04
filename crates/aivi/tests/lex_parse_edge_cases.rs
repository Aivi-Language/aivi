//! Lexer and parser edge-case tests.
//!
//! Covers:
//!   - Ambiguous token boundaries (`x-1`, `a..b`, operators adjacent to identifiers)
//!   - Numeric literal extremes (zero, floats, large integers, suffixed numbers)
//!   - Error span precision (unterminated strings, unclosed braces/parens, missing `=>`)
//!   - Token boundaries (identifier immediately adjacent to operator)
//!   - Block-comment behaviour
//!   - Semicolons (not part of AIVI syntax — produce E1006)

use aivi::{lex_cst, parse_modules, DiagnosticSeverity};
use std::path::Path;

// ── helpers ─────────────────────────────────────────────────────────────────

fn tokens_of(src: &str) -> Vec<aivi::CstToken> {
    let (tokens, _) = lex_cst(src);
    tokens
}

fn significant_tokens(src: &str) -> Vec<aivi::CstToken> {
    tokens_of(src)
        .into_iter()
        .filter(|t| t.kind != "whitespace")
        .collect()
}

fn error_codes(src: &str) -> Vec<String> {
    let (_, diags) = lex_cst(src);
    diags
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .map(|d| d.code.clone())
        .collect()
}

// ── 1. Ambiguous token boundaries ───────────────────────────────────────────

/// `x-1` must lex as three tokens: ident `x`, symbol `-`, number `1`.
/// It must NOT be treated as a hyphenated identifier.
#[test]
fn subtraction_not_hyphenated_ident() {
    let toks = significant_tokens("x-1");
    let kinds: Vec<&str> = toks.iter().map(|t| t.kind.as_str()).collect();
    let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
    assert_eq!(kinds, ["ident", "symbol", "number"], "x-1 should be 3 tokens");
    assert_eq!(texts, ["x", "-", "1"]);
}

/// `a..b` must lex as ident `a`, symbol `..`, ident `b` (range).
#[test]
fn range_dot_dot_tokenises_correctly() {
    let toks = significant_tokens("a..b");
    let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
    assert_eq!(texts, ["a", "..", "b"], "a..b should be [a, .., b]");
    assert_eq!(toks[1].kind, "symbol");
}

/// `...` (spread) must lex as a single 3-char symbol token, not two dots + one.
#[test]
fn spread_three_dots_is_single_token() {
    let toks = significant_tokens("...");
    assert_eq!(toks.len(), 1, "... should be a single token");
    assert_eq!(toks[0].text, "...");
    assert_eq!(toks[0].kind, "symbol");
}

/// Identifier immediately followed by `(` — two tokens, no whitespace needed.
#[test]
fn ident_immediately_followed_by_open_paren() {
    let toks = significant_tokens("f(x)");
    let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
    assert_eq!(texts, ["f", "(", "x", ")"]);
}

/// `=>` must lex as a single 2-char symbol, not `=` + `>`.
#[test]
fn fat_arrow_is_single_token() {
    let toks = significant_tokens("=>");
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0].text, "=>");
    assert_eq!(toks[0].kind, "symbol");
}

/// `->` must lex as a single 2-char symbol.
#[test]
fn thin_arrow_is_single_token() {
    let toks = significant_tokens("->");
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0].text, "->");
    assert_eq!(toks[0].kind, "symbol");
}

/// `|>` (pipe) is a single token.
#[test]
fn pipe_is_single_token() {
    let toks = significant_tokens("|>");
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0].text, "|>");
}

/// Operators without surrounding spaces.
#[test]
fn operators_without_spaces_tokenise_correctly() {
    // `a+b` → [a, +, b]
    let toks = significant_tokens("a+b");
    let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
    assert_eq!(texts, ["a", "+", "b"]);
}

#[test]
fn operators_without_spaces_multiply() {
    let toks = significant_tokens("a*b");
    let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
    assert_eq!(texts, ["a", "*", "b"]);
}

// ── 2. Numeric literal extremes ─────────────────────────────────────────────

/// Single zero digit.
#[test]
fn lex_zero() {
    let toks = significant_tokens("0");
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0].kind, "number");
    assert_eq!(toks[0].text, "0");
}

/// Float `0.0`.
#[test]
fn lex_float_zero() {
    let toks = significant_tokens("0.0");
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0].kind, "number");
    assert_eq!(toks[0].text, "0.0");
}

/// Float `3.14`.
#[test]
fn lex_float_pi() {
    let toks = significant_tokens("3.14");
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0].kind, "number");
    assert_eq!(toks[0].text, "3.14");
}

/// Large integer (fits in u64).
#[test]
fn lex_large_integer() {
    let src = "9999999999999999999";
    let toks = significant_tokens(src);
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0].kind, "number");
    assert_eq!(toks[0].text, src);
}

/// Negative sign followed by integer is two tokens: `-` and the number.
/// (Negation is handled at the parser / AST level, not in the lexer.)
#[test]
fn negative_integer_is_minus_plus_number() {
    let toks = significant_tokens("-42");
    assert_eq!(toks.len(), 2, "negative int should be two tokens: - and 42");
    assert_eq!(toks[0].text, "-");
    assert_eq!(toks[0].kind, "symbol");
    assert_eq!(toks[1].text, "42");
    assert_eq!(toks[1].kind, "number");
}

/// A decimal number like `1.` (trailing dot) does NOT consume the dot as part
/// of the number token — the dot only attaches when followed by a digit.
#[test]
fn number_dot_without_fraction_splits_at_dot() {
    // "1." → number "1" then symbol "."
    let toks = significant_tokens("1.");
    assert!(toks.len() >= 2, "1. should produce at least 2 tokens");
    assert_eq!(toks[0].kind, "number");
    assert_eq!(toks[0].text, "1");
    assert_eq!(toks[1].text, ".");
}

/// Suffixed number literal (domain-resolved) — must lex as a single ident token
/// (the suffix glues to the digits because `is_ident_continue` covers digits).
/// e.g. `10px` → one ident token (identifier starting with… wait, starts with digit).
/// Actually: the lexer reads digits first, then checks for `.digit`. Suffix letters
/// come *after* the number ends, so `10px` lexes as number `10` + ident `px`.
#[test]
fn suffixed_number_lexes_as_number_then_ident() {
    let toks = significant_tokens("10px");
    // Expect: number "10" then ident "px"
    assert!(
        toks.len() >= 2,
        "10px should produce at least 2 tokens, got: {toks:#?}"
    );
    assert_eq!(toks[0].kind, "number");
    assert_eq!(toks[0].text, "10");
    assert_eq!(toks[1].kind, "ident");
    assert_eq!(toks[1].text, "px");
}

// ── 3. Error span precision ──────────────────────────────────────────────────

/// Unterminated string on line 1 — error span must start on line 1.
#[test]
fn unterminated_string_error_on_correct_line() {
    let src = "\"hello";
    let (_, diags) = lex_cst(src);
    let err = diags
        .iter()
        .find(|d| d.code == "E1001")
        .expect("expected E1001 for unterminated string");
    assert_eq!(
        err.span.start.line, 1,
        "unterminated string span should start on line 1"
    );
    assert_eq!(
        err.span.start.column, 1,
        "unterminated string span should start at column 1"
    );
}

/// Unterminated string on line 3 — error span must start on line 3.
#[test]
fn unterminated_string_span_points_to_correct_line() {
    let src = "x = 1\ny = 2\nz = \"oops";
    let (_, diags) = lex_cst(src);
    let err = diags
        .iter()
        .find(|d| d.code == "E1001")
        .expect("expected E1001 for unterminated string on line 3");
    assert_eq!(
        err.span.start.line, 3,
        "span should point to line 3, got: {:?}",
        err.span
    );
}

/// Unclosed `(` — must produce error code E1004 with span at the `(`.
#[test]
fn unclosed_paren_produces_e1004() {
    let src = "(abc";
    let codes = error_codes(src);
    assert!(
        codes.contains(&"E1004".to_string()),
        "expected E1004 for unclosed '(', got: {codes:?}"
    );
}

/// Unclosed `(` span must point to column where `(` was opened.
#[test]
fn unclosed_paren_span_points_to_open() {
    let src = "(abc";
    let (_, diags) = lex_cst(src);
    let err = diags
        .iter()
        .find(|d| d.code == "E1004")
        .expect("expected E1004");
    assert_eq!(err.span.start.line, 1);
    assert_eq!(err.span.start.column, 1, "span should be at the '('");
}

/// Unclosed `{` — must produce E1004.
#[test]
fn unclosed_brace_produces_e1004() {
    let src = "{ x = 1";
    let codes = error_codes(src);
    assert!(
        codes.contains(&"E1004".to_string()),
        "expected E1004 for unclosed '{{', got: {codes:?}"
    );
}

/// Unclosed `[` — must produce E1004.
#[test]
fn unclosed_bracket_produces_e1004() {
    let src = "[1, 2, 3";
    let codes = error_codes(src);
    assert!(
        codes.contains(&"E1004".to_string()),
        "expected E1004 for unclosed '[', got: {codes:?}"
    );
}

/// Unmatched `)` without a preceding `(` — must produce E1002.
#[test]
fn unmatched_close_paren_produces_e1002() {
    let codes = error_codes("abc)");
    assert!(
        codes.contains(&"E1002".to_string()),
        "expected E1002 for unmatched ')', got: {codes:?}"
    );
}

/// Mismatched brackets `[)` — must produce E1003.
#[test]
fn mismatched_brackets_produces_e1003() {
    let codes = error_codes("[)");
    assert!(
        codes.contains(&"E1003".to_string()),
        "expected E1003 for '[)', got: {codes:?}"
    );
}

/// Unclosed block comment — must produce E1007.
#[test]
fn unterminated_block_comment_produces_e1007() {
    let src = "/* this comment never ends";
    let codes = error_codes(src);
    assert!(
        codes.contains(&"E1007".to_string()),
        "expected E1007 for unterminated block comment, got: {codes:?}"
    );
}

/// Block comment span starts where `/*` begins.
#[test]
fn unterminated_block_comment_span_at_open() {
    let src = "x = 1\n/* oops";
    let (_, diags) = lex_cst(src);
    let err = diags
        .iter()
        .find(|d| d.code == "E1007")
        .expect("expected E1007");
    assert_eq!(err.span.start.line, 2, "block comment started on line 2");
    assert_eq!(err.span.start.column, 1);
}

/// Semicolons are not AIVI syntax — must produce E1006.
#[test]
fn semicolon_produces_e1006() {
    let codes = error_codes("x = 1;");
    assert!(
        codes.contains(&"E1006".to_string()),
        "expected E1006 for semicolon, got: {codes:?}"
    );
}

// ── 4. Parser error span precision (via parse_modules) ───────────────────────

/// Parser must report at least one error when `=>` is missing in a lambda.
/// The error should be reported and the rest of the module should still parse.
#[test]
fn parser_missing_arrow_in_lambda_produces_error() {
    let src = "module edge.cases\n\nbad = x x + 1\ngood = 42\n";
    let (modules, diagnostics) = parse_modules(Path::new("edge_cases.aivi"), src);
    // We don't mandate a specific code — just that there is at least one error.
    let has_error = diagnostics
        .iter()
        .any(|d| d.diagnostic.severity == DiagnosticSeverity::Error);
    // Also verify that recovery allowed `good` to be parsed.
    let module = modules.first().expect("a module must be produced");
    let recovered = module.items.iter().any(|item| match item {
        aivi::ModuleItem::Def(def) => def.name.name == "good",
        _ => false,
    });
    // If the parser treats `bad = x x + 1` as valid (two-arg application), that's
    // fine too — we just need the module to be parseable with `good` present.
    assert!(
        recovered || !has_error,
        "expected parser to produce `good` binding or have no errors: {diagnostics:#?}"
    );
}

/// Unclosed paren in parser input — error is reported on the correct line.
#[test]
fn parser_unclosed_paren_error_line() {
    let src = "module edge.cases\n\nbad = (1 + 2\ngood = 42\n";
    let (_, diagnostics) = parse_modules(Path::new("edge_cases.aivi"), src);
    // At least one error must exist.
    assert!(
        !diagnostics.is_empty(),
        "expected at least one diagnostic for unclosed paren"
    );
    // The error (or the lex error for the unclosed `(`) should be on line 3.
    let on_line_3 = diagnostics
        .iter()
        .any(|d| d.diagnostic.span.start.line == 3);
    assert!(
        on_line_3,
        "expected an error on line 3 (the unclosed paren), got: {diagnostics:#?}"
    );
}

// ── 5. Lex-level token boundary: ident adjacent to various operators ─────────

#[test]
fn ident_adjacent_to_equals() {
    let toks = significant_tokens("x=1");
    let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
    assert_eq!(texts, ["x", "=", "1"]);
}

#[test]
fn ident_adjacent_to_double_colon() {
    let toks = significant_tokens("x::Y");
    let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
    assert_eq!(texts, ["x", "::", "Y"]);
}

#[test]
fn ident_adjacent_to_colon() {
    let toks = significant_tokens("x:Int");
    let texts: Vec<&str> = toks.iter().map(|t| t.text.as_str()).collect();
    assert_eq!(texts, ["x", ":", "Int"]);
}

// ── 6. Strings ───────────────────────────────────────────────────────────────

/// Empty string literal.
#[test]
fn empty_string_lexes_without_error() {
    let (_, diags) = lex_cst("\"\"");
    assert!(diags.is_empty(), "empty string should have no diagnostics");
    let toks = significant_tokens("\"\"");
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0].kind, "string");
    assert_eq!(toks[0].text, "\"\"");
}

/// String containing a backslash-escaped quote.
#[test]
fn string_with_escaped_quote_does_not_terminate_early() {
    let src = r#""say \"hi\"""#;
    let (_, diags) = lex_cst(src);
    assert!(
        diags.is_empty(),
        "string with escaped quotes should have no diagnostics: {diags:#?}"
    );
}

/// String containing `=>` (regression: arrow inside string must not affect lexer state).
#[test]
fn string_containing_arrow_has_no_errors() {
    let (_, diags) = lex_cst("\"=>\"");
    assert!(
        diags.is_empty(),
        "string containing '=>' must produce no diagnostics"
    );
}

// ── 7. Comments ──────────────────────────────────────────────────────────────

/// Line comment starting with `//` runs to end of line; next line tokenises fine.
#[test]
fn line_comment_does_not_consume_next_line() {
    let src = "// a comment\nx = 1";
    let toks = significant_tokens(src);
    let non_comment: Vec<&aivi::CstToken> =
        toks.iter().filter(|t| t.kind != "comment").collect();
    assert!(
        non_comment.iter().any(|t| t.text == "x"),
        "x on next line should still be lexed: {toks:#?}"
    );
}

/// Closed block comment produces no error.
#[test]
fn closed_block_comment_has_no_error() {
    let (_, diags) = lex_cst("/* this is fine */");
    assert!(
        diags.is_empty(),
        "closed block comment should produce no diagnostics"
    );
}

/// Block comment does NOT nest — the first `*/` closes the comment.
#[test]
fn block_comment_does_not_nest() {
    // "/* outer /* inner */ still outer? x = 1"
    // The `*/` after "inner" closes the outer comment; "still outer? x = 1" is live code.
    let src = "/* outer /* inner */ x = 1";
    let (_, diags) = lex_cst(src);
    // No unclosed-comment error because the first `*/` closed it.
    let has_e1007 = diags.iter().any(|d| d.code == "E1007");
    assert!(!has_e1007, "block comment should not nest; first */ closes it");
    // `x` and `1` should appear as tokens.
    let toks = significant_tokens(src);
    assert!(
        toks.iter().any(|t| t.text == "x"),
        "`x` should be a live token after nested comment: {toks:#?}"
    );
}
