//! Fuzz target: formatter.
//!
//! Invariants checked:
//! - `format_text` must NEVER panic, even on garbage input.
//! - Output must not explode: len(output) <= 8 × len(input) + 4096.
//! - Formatting valid code twice (idempotency) must be stable.
//! - `format_text_with_options` with various option combos must not panic.

use std::path::Path;

#[test]
fn formatter() {
    bolero::check!().for_each(|data: &[u8]| {
        // Cap input to avoid huge allocations.
        if data.len() > 64 * 1024 {
            return;
        }
        let src = String::from_utf8_lossy(data);

        // Phase 1: Default options — must not panic.
        let formatted = aivi::format_text(&src);

        // Phase 2: Output must not explode in size.
        let max_len = src.len().saturating_mul(8).saturating_add(4096);
        assert!(
            formatted.len() <= max_len,
            "Formatter output explosion: input {} bytes -> output {} bytes (limit {})",
            src.len(),
            formatted.len(),
            max_len,
        );

        // Phase 3: Idempotency — formatting the output again should match.
        let formatted2 = aivi::format_text(&formatted);
        assert_eq!(
            formatted, formatted2,
            "Formatter is not idempotent on this input"
        );

        // Phase 4: format_text_with_options must not panic with non-default options.
        let options_allman = aivi::FormatOptions {
            indent_size: 4,
            max_blank_lines: 2,
            brace_style: aivi::BraceStyle::Allman,
            max_width: 80,
        };
        let _ = aivi::format_text_with_options(&src, options_allman);

        let options_kr = aivi::FormatOptions {
            indent_size: 2,
            max_blank_lines: 0,
            brace_style: aivi::BraceStyle::Kr,
            max_width: 40,
        };
        let _ = aivi::format_text_with_options(&src, options_kr);

        // Phase 5: If the input parses without errors, verify the formatted output
        // still parses without errors (formatting must not break valid programs).
        let (_modules, parse_diags) = aivi::parse_modules(Path::new("fuzz.aivi"), &src);
        if !aivi::file_diagnostics_have_errors(&parse_diags) {
            let (_modules2, parse_diags2) =
                aivi::parse_modules(Path::new("fuzz.aivi"), &formatted);
            assert!(
                !aivi::file_diagnostics_have_errors(&parse_diags2),
                "Formatting broke a valid program"
            );
        }
    });
}
