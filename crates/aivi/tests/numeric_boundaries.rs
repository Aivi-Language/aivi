use std::path::Path;

use aivi::{file_diagnostics_have_errors, lex_cst, parse_modules};

// ---- Lexer tests ----

/// Returns all non-whitespace / non-newline tokens from the lex output.
fn non_trivia(source: &str) -> Vec<(String, String)> {
    let (tokens, _) = lex_cst(source);
    tokens
        .into_iter()
        .filter(|t| t.kind != "whitespace" && t.kind != "newline")
        .map(|t| (t.kind, t.text))
        .collect()
}

#[test]
fn lex_integer_zero() {
    let toks = non_trivia("0");
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0].0, "number");
    assert_eq!(toks[0].1, "0");
}

#[test]
fn lex_float_zero() {
    let toks = non_trivia("0.0");
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0].0, "number");
    assert_eq!(toks[0].1, "0.0");
}

#[test]
fn lex_max_i64_literal() {
    let toks = non_trivia("9223372036854775807");
    assert_eq!(toks.len(), 1, "expected a single number token");
    assert_eq!(toks[0].0, "number");
    assert_eq!(toks[0].1, "9223372036854775807");
}

/// A literal one beyond i64::MAX does not fit in i64, but the lexer should
/// still produce a single Number token — bounds checking is a compile-time or
/// evaluation concern, not a lexer concern.
#[test]
fn lex_overflow_i64_literal_preserved_as_token() {
    let toks = non_trivia("9223372036854775808");
    assert_eq!(
        toks.len(),
        1,
        "expected a single number token for out-of-range literal"
    );
    assert_eq!(toks[0].0, "number");
    assert_eq!(toks[0].1, "9223372036854775808");
}

/// Negative literals are represented as unary minus applied to a positive
/// literal; the lexer emits a symbol token for `-` followed by a number token.
#[test]
fn lex_negative_zero_float_is_unary_minus_plus_number() {
    let toks = non_trivia("-0.0");
    assert_eq!(toks.len(), 2, "expected symbol '-' then number '0.0'");
    assert_eq!(toks[0].0, "symbol");
    assert_eq!(toks[0].1, "-");
    assert_eq!(toks[1].0, "number");
    assert_eq!(toks[1].1, "0.0");
}

#[test]
fn lex_negative_integer_is_unary_minus_plus_number() {
    let toks = non_trivia("-42");
    assert_eq!(toks.len(), 2);
    assert_eq!(toks[0].0, "symbol");
    assert_eq!(toks[0].1, "-");
    assert_eq!(toks[1].0, "number");
    assert_eq!(toks[1].1, "42");
}

#[test]
fn lex_float_with_decimals() {
    let toks = non_trivia("3.14");
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0].0, "number");
    assert_eq!(toks[0].1, "3.14");
}

#[test]
fn lex_number_emits_no_diagnostics() {
    let (_, diags) = lex_cst("9223372036854775807");
    assert!(diags.is_empty(), "unexpected lexer diagnostics: {diags:?}");
}

#[test]
fn lex_out_of_range_integer_emits_no_diagnostics() {
    // The lexer does not validate numeric range; that is deferred to evaluation.
    let (_, diags) = lex_cst("9223372036854775808");
    assert!(
        diags.is_empty(),
        "unexpected lexer diagnostics for out-of-range literal: {diags:?}"
    );
}

// ---- Parser tests ----

fn parse_ok(source: &str) {
    let (_, diagnostics) = parse_modules(Path::new("test.aivi"), source);
    assert!(
        !file_diagnostics_have_errors(&diagnostics),
        "unexpected parse errors: {diagnostics:?}"
    );
}

#[test]
fn parse_integer_zero_binding() {
    parse_ok("module test\nx = 0");
}

#[test]
fn parse_float_zero_binding() {
    parse_ok("module test\nx = 0.0");
}

#[test]
fn parse_max_i64_binding() {
    parse_ok("module test\nx = 9223372036854775807");
}

#[test]
fn parse_negative_float_zero_binding() {
    // Parsed as unary negation of 0.0
    parse_ok("module test\nx = -0.0");
}

#[test]
fn parse_negative_integer_binding() {
    parse_ok("module test\nx = -42");
}

#[test]
fn parse_float_literal_binding() {
    parse_ok("module test\nx = 3.14");
}

/// An out-of-range integer literal is lexed as a Number token and the parser
/// should accept it without error (range semantics are handled downstream).
#[test]
fn parse_out_of_range_integer_literal() {
    parse_ok("module test\nx = 9223372036854775808");
}
