#![no_main]
//! Fuzz target: LSP-like pipeline.
//!
//! Simulates the operations the LSP server performs when a document is opened
//! or changed. Exercises the full stack that a real editor would trigger:
//!   lex → parse → resolve → typecheck → format
//!
//! Invariants checked:
//! - None of these stages may panic on arbitrary input.
//! - check_modules / check_types return diagnostics, never crash.
//! - format_text returns a string, never crashes.
//! - Re-parsing after formatting must not panic.

use libfuzzer_sys::fuzz_target;
use std::path::Path;

fuzz_target!(|data: &[u8]| {
    // LSP documents are typically small; cap at 32 KiB.
    if data.len() > 32 * 1024 {
        return;
    }
    let src = String::from_utf8_lossy(data);
    let path = Path::new("fuzz.aivi");

    // ── Phase 1: Parse (what LSP does on didOpen / didChange) ──
    let (modules, parse_diags) = aivi::parse_modules(path, &src);

    // ── Phase 2: Resolve (name resolution) ── must not panic even on invalid input.
    let resolve_diags = aivi::check_modules(&modules);

    // ── Phase 3: Typecheck ── must not panic even on invalid input.
    let type_diags = aivi::check_types(&modules);

    // ── Phase 4: Format (what LSP does on textDocument/formatting) ──
    let formatted = aivi::format_text(&src);

    // ── Phase 5: Re-parse the formatted output ── must not panic.
    let _ = aivi::parse_modules(path, &formatted);

    // ── Phase 6: If input was error-free, exercise desugaring + kernel lowering ──
    let all_diags: Vec<_> = parse_diags
        .into_iter()
        .chain(resolve_diags)
        .chain(type_diags)
        .collect();
    if !aivi::file_diagnostics_have_errors(&all_diags) {
        let hir = aivi::desugar_modules(&modules);
        let _kernel = aivi::lower_kernel(hir);
    }
});
