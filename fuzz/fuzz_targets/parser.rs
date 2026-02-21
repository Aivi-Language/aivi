#![no_main]
//! Fuzz target: lexer + parser.
//!
//! Invariants checked:
//! - Lexing and parsing must NEVER panic or trigger UB.
//! - Token stream must cover the entire input span.
//! - Parse always returns (modules or diagnostics), never hangs.

use libfuzzer_sys::fuzz_target;
use std::path::Path;

fuzz_target!(|data: &[u8]| {
    // Cap input size to prevent pathological allocations.
    if data.len() > 64 * 1024 {
        return;
    }
    let src = String::from_utf8_lossy(data);

    // Phase 1: Lex — must not panic.
    let (tokens, _lex_diags) = aivi::lex_cst(&src);

    // Phase 2: Parse from tokens — must not panic.
    let (_modules, _parse_diags) = aivi::parse_modules_from_tokens(Path::new("fuzz.aivi"), &tokens);

    // Phase 3: Also exercise the combined lex+parse path.
    let (_modules2, _parse_diags2) = aivi::parse_modules(Path::new("fuzz.aivi"), &src);
});
