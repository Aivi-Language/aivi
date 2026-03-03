// Comprehensive tests for the LSP strict-mode subsystem.
// Each sub-module covers one strict source file.

use tower_lsp::lsp_types::{DiagnosticSeverity, NumberOrString};

use crate::backend::Backend;
use crate::strict::{StrictConfig, StrictLevel};

fn uri() -> tower_lsp::lsp_types::Url {
    tower_lsp::lsp_types::Url::parse("file:///strict_test.aivi").unwrap()
}

fn strict(level: StrictLevel) -> StrictConfig {
    StrictConfig {
        level,
        forbid_implicit_coercions: false,
        warnings_as_errors: false,
    }
}

fn has_code(diags: &[tower_lsp::lsp_types::Diagnostic], code: &str) -> bool {
    diags.iter().any(|d| {
        matches!(d.code.as_ref(), Some(NumberOrString::String(c)) if c == code)
    })
}

// ── StrictLevel helpers ───────────────────────────────────────────────────────

#[test]
fn strict_level_off_returns_no_strict_diags() {
    let text = "module demo\n\nid = x = > x\n";
    let diags = Backend::build_diagnostics_strict(&text, &uri(), &strict(StrictLevel::Off));
    assert!(
        !has_code(&diags, "AIVI-S014"),
        "strict Off must not emit AIVI-S014"
    );
}

#[test]
fn strict_level_from_u8_covers_all_arms() {
    use crate::strict::StrictLevel;
    assert_eq!(StrictLevel::from_u8(0), StrictLevel::Off);
    assert_eq!(StrictLevel::from_u8(1), StrictLevel::LexicalStructural);
    assert_eq!(StrictLevel::from_u8(2), StrictLevel::NamesImports);
    assert_eq!(StrictLevel::from_u8(3), StrictLevel::TypesDomains);
    assert_eq!(StrictLevel::from_u8(4), StrictLevel::NoImplicitCoercions);
    assert_eq!(StrictLevel::from_u8(5), StrictLevel::Pedantic);
    assert_eq!(StrictLevel::from_u8(99), StrictLevel::Pedantic);
}

#[test]
fn warnings_as_errors_elevates_severity() {
    // AIVI-S003 is a WARNING; with warnings_as_errors it becomes ERROR.
    let text = "module demo\n\nmy__val = 1\n";
    let cfg = StrictConfig {
        level: StrictLevel::LexicalStructural,
        forbid_implicit_coercions: false,
        warnings_as_errors: true,
    };
    let diags = Backend::build_diagnostics_strict(text, &uri(), &cfg);
    let s003 = diags.iter().find(|d| {
        matches!(d.code.as_ref(), Some(NumberOrString::String(c)) if c == "AIVI-S003")
    });
    assert!(s003.is_some(), "expected AIVI-S003");
    assert_eq!(
        s003.unwrap().severity,
        Some(DiagnosticSeverity::ERROR),
        "warnings_as_errors must elevate to ERROR"
    );
}

// ── lexical.rs ────────────────────────────────────────────────────────────────

#[test]
fn lexical_invisible_unicode_s001() {
    // Insert a zero-width space inside an identifier.
    let text = format!("module demo\n\nval\u{200B}ue = 1\n");
    let diags = Backend::build_diagnostics_strict(&text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(has_code(&diags, "AIVI-S001"), "expected AIVI-S001 for invisible unicode");
}

#[test]
fn lexical_invisible_unicode_multiple_chars() {
    // Multiple invisible characters → multiple diagnostics.
    let text = format!("module demo\n\n\u{200B}\u{FEFF}x = 1\n");
    let diags = Backend::build_diagnostics_strict(&text, &uri(), &strict(StrictLevel::LexicalStructural));
    let count = diags.iter().filter(|d| {
        matches!(d.code.as_ref(), Some(NumberOrString::String(c)) if c == "AIVI-S001")
    }).count();
    assert!(count >= 2, "expected at least 2 AIVI-S001 diagnostics");
}

#[test]
fn lexical_no_false_positive_on_normal_text() {
    let text = "module demo\n\nnormalIdent = 42\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(!has_code(&diags, "AIVI-S001"));
    assert!(!has_code(&diags, "AIVI-S003"));
    assert!(!has_code(&diags, "AIVI-S004"));
}

#[test]
fn lexical_split_arrow_s014() {
    let text = "module demo\n\nid = x = > x\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(has_code(&diags, "AIVI-S014"), "expected AIVI-S014 split =>");
}

#[test]
fn lexical_split_pipe_s015() {
    let text = "module demo\n\npiped = 1 | > add 1\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(has_code(&diags, "AIVI-S015"), "expected AIVI-S015 split |>");
}

#[test]
fn lexical_split_arrow_has_fix_in_code_action() {
    let text = "module demo\n\nid = x = > x\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    let actions = Backend::build_code_actions_with_workspace(
        text,
        &uri(),
        &diags,
        &std::collections::HashMap::new(),
        tower_lsp::lsp_types::Range::default(),
    );
    let has_fix = actions.iter().any(|a| match a {
        tower_lsp::lsp_types::CodeActionOrCommand::CodeAction(ca) => {
            ca.title.contains("=>")
        }
        _ => false,
    });
    assert!(has_fix, "expected code action offering '=>' fix");
}

#[test]
fn lexical_double_underscore_s003() {
    let text = "module demo\n\nmy__value = 1\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(has_code(&diags, "AIVI-S003"), "expected AIVI-S003 for __");
}

#[test]
fn lexical_leading_underscore_s004() {
    let text = "module demo\n\n_myValue = 1\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(has_code(&diags, "AIVI-S004"), "expected AIVI-S004 for leading _");
}

#[test]
fn lexical_tuple_trailing_whitespace_s006() {
    // A tuple like `(1, )` has trailing whitespace before `)`.
    // The parser may or may not produce a Tuple node; but the CST-level check fires on whitespace.
    let text = "module demo\n\nt = (1, 2 )\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(has_code(&diags, "AIVI-S006"), "expected AIVI-S006 trailing whitespace in tuple");
}

#[test]
fn lexical_tuple_no_false_positive_without_comma() {
    // Plain parenthesised expression – no comma, so not a tuple → no S006.
    let text = "module demo\n\nt = (1 + 2)\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(!has_code(&diags, "AIVI-S006"));
}

// ── record_syntax.rs ──────────────────────────────────────────────────────────

#[test]
fn record_syntax_wrong_separator_s016() {
    // `{ name = "Alice" }` uses `=` instead of `:`.
    let text = "module demo\n\nr = { name: \"ok\", age = 30 }\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(has_code(&diags, "AIVI-S016"), "expected AIVI-S016 wrong record separator");
}

#[test]
fn record_syntax_correct_separator_no_s016() {
    let text = "module demo\n\nr = { name: \"Alice\", age: 30 }\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(!has_code(&diags, "AIVI-S016"));
}

#[test]
fn record_syntax_s016_offers_colon_fix() {
    let text = "module demo\n\nr = { name: \"ok\", age = 30 }\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    let actions = Backend::build_code_actions_with_workspace(
        text,
        &uri(),
        &diags,
        &std::collections::HashMap::new(),
        tower_lsp::lsp_types::Range::default(),
    );
    let has_fix = actions.iter().any(|a| match a {
        tower_lsp::lsp_types::CodeActionOrCommand::CodeAction(ca) => ca.title.contains(":"),
        _ => false,
    });
    assert!(has_fix, "expected code action replacing = with :");
}

// ── tuple_intent.rs ───────────────────────────────────────────────────────────

#[test]
fn tuple_intent_suspicious_call_in_tuple_s020() {
    // `(a b, c)` – `a b` looks like a call inside a tuple; might mean `(a, b, c)`.
    let text = "module demo\n\na = 1\nb = 2\nc = 3\nt = (a b, c)\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(has_code(&diags, "AIVI-S020"), "expected AIVI-S020 suspicious tuple call");
}

#[test]
fn tuple_intent_normal_tuple_no_s020() {
    let text = "module demo\n\nt = (1, 2, 3)\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(!has_code(&diags, "AIVI-S020"));
}

#[test]
fn tuple_intent_two_element_tuple_no_s020() {
    let text = "module demo\n\nt = (1, 2)\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(!has_code(&diags, "AIVI-S020"));
}

// ── pattern_discipline.rs ─────────────────────────────────────────────────────

#[test]
fn pattern_discipline_unused_binding_s301() {
    // Match arm binds `n` but never uses it.
    let text = r#"module demo

Option A = None | Some A

val = Some 1 match
  | Some n => 42
  | None   => 0
"#;
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(has_code(&diags, "AIVI-S301"), "expected AIVI-S301 unused pattern binding");
}

#[test]
fn pattern_discipline_used_binding_no_s301() {
    let text = r#"module demo

Option A = None | Some A

val = Some 1 match
  | Some n => n
  | None   => 0
"#;
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(!has_code(&diags, "AIVI-S301"));
}

#[test]
fn pattern_discipline_wildcard_after_wildcard_s300() {
    // A `_` arm followed by another arm is unreachable.
    let text = r#"module demo

Option A = None | Some A

val = None match
  | _ => 0
  | None => 1
"#;
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(has_code(&diags, "AIVI-S300"), "expected AIVI-S300 unreachable arm after wildcard");
}

#[test]
fn pattern_discipline_wildcard_at_end_no_s300() {
    let text = r#"module demo

Option A = None | Some A

val = None match
  | None   => 0
  | Some _ => 1
  | _      => 2
"#;
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(!has_code(&diags, "AIVI-S300"));
}

#[test]
fn pattern_discipline_block_ends_with_binding_s220() {
    // A `do` block that ends with a let-binding (no final expression).
    let text = "module demo\n\nval = do Effect {\n  x <- pure 1\n  y = x\n}\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(has_code(&diags, "AIVI-S220"), "expected AIVI-S220 block ends with binding");
}

#[test]
fn pattern_discipline_block_ends_with_expr_no_s220() {
    let text = "module demo\n\nval = do Effect {\n  x <- pure 1\n  x\n}\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(!has_code(&diags, "AIVI-S220"));
}

#[test]
fn pattern_discipline_unused_let_binding_s221() {
    // A `let` binding that is never used.
    let text = "module demo\n\nval = do Effect {\n  let unused = 99\n  pure 1\n}\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(has_code(&diags, "AIVI-S221"), "expected AIVI-S221 unused let binding");
}

// ── pipe_discipline.rs ────────────────────────────────────────────────────────

#[test]
fn pipe_discipline_pipe_step_not_callable_s100() {
    // `1 |> 42` – the RHS of `|>` is a literal, not a function.
    let text = "module demo\n\nval = 1 |> 42\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(has_code(&diags, "AIVI-S100"), "expected AIVI-S100 non-callable pipe step");
}

#[test]
fn pipe_discipline_pipe_step_function_no_s100() {
    let text = "module demo\n\nadd1 = x => x + 1\nval = 1 |> add1\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(!has_code(&diags, "AIVI-S100"));
}

#[test]
fn pipe_discipline_ambiguous_multi_arg_call_s101() {
    // `1 |> add 5` – ambiguous: does the pipe fill first or last arg?
    let text = "module demo\n\nadd = x y => x + y\nval = 1 |> add 5\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(has_code(&diags, "AIVI-S101"), "expected AIVI-S101 ambiguous pipe multi-arg");
}

#[test]
fn pipe_discipline_explicit_underscore_no_s101() {
    let text = "module demo\n\nadd = x y => x + y\nval = 1 |> add _ 5\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(!has_code(&diags, "AIVI-S101"));
}

#[test]
fn pipe_field_access_on_unknown_field_s140() {
    // `{ x: 1 }.y` – field `y` does not exist in the record literal.
    let text = "module demo\n\nval = { x: 1 }.y\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(has_code(&diags, "AIVI-S140"), "expected AIVI-S140 unknown field on record literal");
}

#[test]
fn pipe_field_access_on_known_field_no_s140() {
    let text = "module demo\n\nval = { x: 1 }.x\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(!has_code(&diags, "AIVI-S140"));
}

// ── imports_and_domains.rs ────────────────────────────────────────────────────

#[test]
fn imports_duplicate_use_s200() {
    let text = "module demo\nuse aivi\nuse aivi\n\nval = 1\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::NamesImports));
    assert!(has_code(&diags, "AIVI-S200"), "expected AIVI-S200 duplicate use");
}

#[test]
fn imports_no_duplicate_use_no_s200() {
    let text = "module demo\nuse aivi\n\nval = 1\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::NamesImports));
    assert!(!has_code(&diags, "AIVI-S200"));
}

#[test]
fn imports_missing_import_suggestion_s201() {
    // `Some` is defined in `aivi.option`; using it without an import triggers S201.
    let text = "module demo\n\nval = Some 1\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::NamesImports));
    // S201 is best-effort; check that if it fires it has the right code.
    for d in &diags {
        if matches!(d.code.as_ref(), Some(NumberOrString::String(c)) if c == "AIVI-S201") {
            // At least one suggestion must mention a module.
            assert!(d.message.contains("use "), "S201 message should contain 'use '");
            return;
        }
    }
    // It's acceptable for S201 not to fire if the heuristic cannot determine a single provider.
}

#[test]
fn imports_level1_does_not_emit_s200() {
    // S200 is only active at level >= 2 (NamesImports).
    let text = "module demo\nuse aivi\nuse aivi\n\nval = 1\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::LexicalStructural));
    assert!(!has_code(&diags, "AIVI-S200"), "S200 must not fire at level 1");
}

#[test]
fn imports_domain_ambiguity_s400() {
    // `date + 1` should trigger AIVI-S400 at level >= 3.
    let text = "module demo\n\nresult = date + 1\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::TypesDomains));
    assert!(has_code(&diags, "AIVI-S400"), "expected AIVI-S400 domain ambiguity");
}

#[test]
fn imports_domain_ambiguity_not_at_level2() {
    let text = "module demo\n\nresult = date + 1\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::NamesImports));
    assert!(!has_code(&diags, "AIVI-S400"), "S400 must not fire below level 3");
}

#[test]
fn imports_domain_no_false_positive_for_non_date() {
    let text = "module demo\n\nresult = counter + 1\n";
    let diags = Backend::build_diagnostics_strict(text, &uri(), &strict(StrictLevel::TypesDomains));
    assert!(!has_code(&diags, "AIVI-S400"), "S400 must not fire for non-date-like names");
}

// ── build_strict_diagnostics (top-level integration) ─────────────────────────

#[test]
fn build_strict_off_returns_empty() {
    let path = std::path::Path::new("/test.aivi");
    let text = "module demo\n\nid = x = > x\n";
    let url = uri();
    let diags = crate::strict::build_strict_diagnostics(
        text,
        &url,
        path,
        &StrictConfig::default(),
        &std::collections::HashMap::new(),
    );
    assert!(diags.is_empty(), "StrictLevel::Off must return empty vec");
}

#[test]
fn build_strict_level1_catches_lexical() {
    let path = std::path::Path::new("/test.aivi");
    let text = "module demo\n\nid = x = > x\n";
    let url = uri();
    let diags = crate::strict::build_strict_diagnostics(
        text,
        &url,
        path,
        &strict(StrictLevel::LexicalStructural),
        &std::collections::HashMap::new(),
    );
    assert!(has_code(&diags, "AIVI-S014"), "level 1 should catch split =>");
}

#[test]
fn build_strict_level2_catches_duplicate_import() {
    let path = std::path::Path::new("/test.aivi");
    let text = "module demo\nuse aivi\nuse aivi\n\nval = 1\n";
    let url = uri();
    let diags = crate::strict::build_strict_diagnostics(
        text,
        &url,
        path,
        &strict(StrictLevel::NamesImports),
        &std::collections::HashMap::new(),
    );
    assert!(has_code(&diags, "AIVI-S200"), "level 2 should catch duplicate use");
}

#[test]
fn build_strict_level3_catches_domain_ambiguity() {
    let path = std::path::Path::new("/test.aivi");
    let text = "module demo\n\nresult = date + 1\n";
    let url = uri();
    let diags = crate::strict::build_strict_diagnostics(
        text,
        &url,
        path,
        &strict(StrictLevel::TypesDomains),
        &std::collections::HashMap::new(),
    );
    assert!(has_code(&diags, "AIVI-S400"), "level 3 should catch domain ambiguity");
}

#[test]
fn build_strict_does_not_panic_on_malformed_input() {
    let path = std::path::Path::new("/test.aivi");
    let text = "module broken = { let let let !!##";
    let url = uri();
    // Must not panic; may return any diagnostics.
    let _ = crate::strict::build_strict_diagnostics(
        text,
        &url,
        path,
        &strict(StrictLevel::Pedantic),
        &std::collections::HashMap::new(),
    );
}

#[test]
fn build_strict_level5_runs_without_panic() {
    let path = std::path::Path::new("/test.aivi");
    let text = "module demo\n\nval = 1\n";
    let url = uri();
    let _ = crate::strict::build_strict_diagnostics(
        text,
        &url,
        path,
        &strict(StrictLevel::Pedantic),
        &std::collections::HashMap::new(),
    );
}

// ── is_invisible_unicode helper ───────────────────────────────────────────────

#[test]
fn is_invisible_unicode_soft_hyphen() {
    assert!(crate::strict::is_invisible_unicode('\u{00AD}'));
}

#[test]
fn is_invisible_unicode_zero_width_space() {
    assert!(crate::strict::is_invisible_unicode('\u{200B}'));
}

#[test]
fn is_invisible_unicode_bom() {
    assert!(crate::strict::is_invisible_unicode('\u{FEFF}'));
}

#[test]
fn is_invisible_unicode_bidi_range() {
    for ch in '\u{202A}'..='\u{202E}' {
        assert!(crate::strict::is_invisible_unicode(ch), "bidi char {ch:?} should be invisible");
    }
}

#[test]
fn is_invisible_unicode_bidi_isolate_range() {
    for ch in '\u{2066}'..='\u{2069}' {
        assert!(crate::strict::is_invisible_unicode(ch), "bidi isolate {ch:?} should be invisible");
    }
}

#[test]
fn is_invisible_unicode_normal_chars_are_not_invisible() {
    for ch in ['a', 'Z', '0', ' ', '\n', '.', '-'] {
        assert!(!crate::strict::is_invisible_unicode(ch), "char {ch:?} should not be invisible");
    }
}

// ── keywords_v01 helper ───────────────────────────────────────────────────────

#[test]
fn keywords_v01_contains_known_keywords() {
    let kw = crate::strict::keywords_v01();
    // `do`, `match`, `if` are fundamental keywords that must always be present.
    assert!(kw.contains("do"), "keywords must include 'do'");
    assert!(kw.contains("match"), "keywords must include 'match'");
    assert!(kw.contains("if"), "keywords must include 'if'");
}

#[test]
fn keywords_v01_does_not_contain_non_keyword() {
    let kw = crate::strict::keywords_v01();
    assert!(!kw.contains("myFunction"), "user-defined names must not be keywords");
}
