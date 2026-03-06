// This module mirrors `crates/aivi/src/formatter.rs`, but lives in `aivi_core` so that
// `mod doc; mod engine;` resolves against `crates/aivi_core/src/formatter/*`.

mod doc;
mod engine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BraceStyle {
    /// K&R / TS/Java style: `if cond {` / `x => {` (default).
    Kr,
    /// Allman: opening brace on its own line.
    Allman,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatOptions {
    pub indent_size: usize,
    pub max_blank_lines: usize,
    pub brace_style: BraceStyle,
    /// Target width for `Doc` groups. Current rules rarely reflow expressions, so this is
    /// primarily a future-proof knob.
    pub max_width: usize,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            indent_size: 2,
            max_blank_lines: 1,
            brace_style: BraceStyle::Kr,
            max_width: 100,
        }
    }
}

pub fn format_text(content: &str) -> String {
    format_text_with_options(content, FormatOptions::default())
}

pub fn format_text_with_options(content: &str, options: FormatOptions) -> String {
    // Fast path: files without matrix literals need only a single formatting pass.
    if !content.contains("~mat[") {
        return engine::format_text_with_options(content, options);
    }

    // Transformations like semicolon removal or comma stripping can change the
    // token stream on re-lexing (e.g. `& ; &` → `& &` → `&&`).  Iterate
    // until a fixed point is reached (typically 1-3 passes).
    let mut result = engine::format_text_with_options(content, options);
    for _ in 0..4 {
        // Collapse any multi-line `~mat[...]` back to a single line with `;`
        // row separators so the next pass can re-detect and re-align the matrix.
        let collapsed = collapse_multiline_matrix(&result);
        let next = engine::format_text_with_options(&collapsed, options);
        if next == result {
            break;
        }
        result = next;
    }
    result
}

/// Collapse multi-line `~mat[row1\n      row2]` back to `~mat[row1;row2]`.
///
/// After the first format pass the matrix is spread across lines (with
/// column-aligned padding) and the closing `]` appears at the END of the last
/// row.  Subsequent passes don't recognise the pattern because the `;` row
/// separator is gone.  This helper re-inserts `;` so the engine can re-detect
/// and re-format the matrix correctly.
///
/// Matrices where the `]` is on its own line (written directly in source) are
/// left untouched — those are handled stably without collapsing.
fn collapse_multiline_matrix(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        // Find `~mat[` in the line and check whether the `[` is unclosed.
        if let Some(mat_pos) = line.find("~mat[") {
            let bracket_pos = mat_pos + "~mat[".len() - 1; // position of `[`
            let mut depth: i32 = 0;
            let mut closed = false;
            for ch in line[bracket_pos..].chars() {
                if ch == '[' {
                    depth += 1;
                } else if ch == ']' {
                    depth -= 1;
                    if depth == 0 {
                        closed = true;
                        break;
                    }
                }
            }
            if !closed {
                // Look ahead to see if the closing `]` ends a content line
                // (i.e. the formatted multi-row style where `]` trails content).
                // If `]` is alone on its own line, leave the matrix untouched.
                let mut lookahead = i + 1;
                let mut depth2 = depth;
                let mut closer_on_own_line = false;
                let mut found_closer = false;
                while lookahead < lines.len() && depth2 != 0 {
                    let cont = lines[lookahead].trim();
                    if cont.is_empty() {
                        lookahead += 1;
                        continue;
                    }
                    for ch in cont.chars() {
                        if ch == '[' {
                            depth2 += 1;
                        } else if ch == ']' {
                            depth2 -= 1;
                            if depth2 == 0 {
                                found_closer = true;
                                // `]` is alone on this line if the trimmed content
                                // is exactly `]`.
                                closer_on_own_line = cont == "]";
                                break;
                            }
                        }
                    }
                    if found_closer {
                        break;
                    }
                    lookahead += 1;
                }
                // Only collapse the pattern produced by the formatter itself
                // (where `]` trails content, not on its own line).
                if found_closer && !closer_on_own_line {
                    let mut merged = line.trim_end().to_string();
                    i += 1;
                    let mut depth3 = depth;
                    while i < lines.len() && depth3 != 0 {
                        let cont = lines[i].trim();
                        if cont.is_empty() {
                            i += 1;
                            continue;
                        }
                        let mut ends_with_close = false;
                        for ch in cont.chars() {
                            if ch == '[' {
                                depth3 += 1;
                            } else if ch == ']' {
                                depth3 -= 1;
                                if depth3 == 0 {
                                    ends_with_close = true;
                                    break;
                                }
                            }
                        }
                        let row_content = if ends_with_close {
                            if let Some(pos) = cont.rfind(']') {
                                cont[..pos].trim_end()
                            } else {
                                cont
                            }
                        } else {
                            cont
                        };
                        if !row_content.is_empty() {
                            merged.push(';');
                            merged.push_str(row_content);
                        }
                        if ends_with_close {
                            merged.push(']');
                        }
                        i += 1;
                    }
                    out.push(merged);
                    continue;
                }
            }
        }
        out.push(line.to_string());
        i += 1;
    }
    out.join("\n")
}

fn is_op(text: &str) -> bool {
    matches!(
        text,
        "=" | "+"
            | "-"
            | "*"
            | "×"
            | "/"
            | "%"
            | "->"
            | "=>"
            | "<-"
            | "<|"
            | "|>"
            | "?"
            | "|"
            | "++"
            | "::"
            | ".."
            | ":="
            | "??"
            | "^"
            | "=="
            | "!="
            | "<"
            | ">"
            | "<="
            | ">="
            | "&&"
            | "||"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_respects_indent_size() {
        let text = "module demo\n\nmain = do Effect {\n_<-print \"hi\"\n}\n";
        let formatted = format_text_with_options(
            text,
            FormatOptions {
                indent_size: 4,
                max_blank_lines: 1,
                brace_style: BraceStyle::Kr,
                max_width: 100,
            },
        );
        let inner_line = formatted
            .lines()
            .nth(3)
            .expect("expected formatted inner effect line");
        assert!(inner_line.starts_with("    "));
    }

    #[test]
    fn format_respects_max_blank_lines() {
        let text = "module demo\n\n\n\nmain = 1\n";
        let formatted = format_text_with_options(
            text,
            FormatOptions {
                indent_size: 2,
                max_blank_lines: 1,
                brace_style: BraceStyle::Kr,
                max_width: 100,
            },
        );
        assert!(!formatted.contains("\n\n\n"));
    }

    #[test]
    fn format_html_sigil_uses_one_tag_per_line_and_nested_indent() {
        let text = "module demo\n\nx=~<html><div class=\"card\"><span>ok</span></div></html>\n";
        let formatted = format_text(text);
        assert_eq!(
            formatted,
            "module demo\n\nx = ~<html>\n      <div class=\"card\">\n        <span>\n          ok\n        </span>\n      </div>\n    </html>\n"
        );
    }

    #[test]
    fn format_gtk_sigil_keeps_short_attr_lists_on_single_line() {
        let text = "module demo\n\nx=~<gtk><object a=\"1\" b=\"2\" c=\"3\" d=\"4\"><child><object class=\"GtkLabel\"/></child></object></gtk>\n";
        let formatted = format_text(text);
        assert!(formatted.contains("<object a=\"1\" b=\"2\" c=\"3\" d=\"4\">"));
        assert!(formatted.contains("\n          <object class=\"GtkLabel\" />\n"));
    }

    #[test]
    fn format_gtk_sigil_keeps_five_plus_attributes_inline_when_source_is_inline() {
        let text = "module demo\n\nx=~<gtk><object a=\"1\" b=\"2\" c=\"3\" d=\"4\" e=\"5\"><child><object class=\"GtkLabel\"/></child></object></gtk>\n";
        let formatted = format_text(text);
        assert!(formatted.contains("<object a=\"1\" b=\"2\" c=\"3\" d=\"4\" e=\"5\">"));
        assert!(formatted.contains("\n        <child>\n"));
        assert!(formatted.contains("\n          <object class=\"GtkLabel\" />\n"));
    }

    #[test]
    fn format_gtk_sigil_wraps_attributes_when_source_has_newlines() {
        let text =
            "module demo\n\nx=~<gtk><object\n  a=\"1\"\n  b=\"2\"\n  c=\"3\"></object></gtk>\n";
        let formatted = format_text(text);
        assert!(formatted.contains("\n      <object\n"));
        assert!(formatted.contains("\n        a=\"1\"\n"));
        assert!(formatted.contains("\n        c=\"3\">\n"));
    }

    #[test]
    fn format_gtk_sigil_keeps_short_wrapped_record_attribute_values_inline() {
        let text = "module demo\n\nx=~<gtk><object class=\"GtkBox\" props={ { orientation: \"vertical\", spacing: 0, marginStart: 8 } }></object></gtk>\n";
        let formatted = format_text(text);
        assert!(
            formatted
                .contains("<object class=\"GtkBox\" props={{ orientation: \"vertical\", spacing: 0, marginStart: 8 }}>")
        );
        assert!(!formatted.contains("\n        orientation: \"vertical\",\n"));
    }

    #[test]
    fn format_gtk_sigil_formats_wrapped_record_attribute_values() {
        let text = "module demo\n\nx=~<gtk><object class=\"GtkBox\" props={ { orientation: \"vertical\", spacing: 0, marginStart: 8, marginEnd: 8, marginBottom: 8 } }></object></gtk>\n";
        let formatted = format_text(text);
        assert!(formatted.contains("<object class=\"GtkBox\" props={{\n"));
        assert!(formatted.contains("\n        orientation: \"vertical\",\n"));
        assert!(formatted.contains("\n        spacing: 0,\n"));
        assert!(formatted.contains("\n        marginStart: 8,\n"));
        assert!(formatted.contains("\n        marginEnd: 8,\n"));
        assert!(formatted.contains("\n        marginBottom: 8\n"));
        assert!(formatted.contains("\n      }}>\n"));
    }

    #[test]
    fn format_html_sigil_formats_long_wrapped_record_attribute_values() {
        let text =
            "module demo\n\nx=~<html><div props={ { a: 1, b: 2, c: 3, d: 4 } }></div></html>\n";
        let formatted = format_text(text);
        assert!(formatted.contains("<div props={{\n"));
        assert!(formatted.contains("\n        a: 1,\n"));
        assert!(formatted.contains("\n        b: 2,\n"));
        assert!(formatted.contains("\n        c: 3,\n"));
        assert!(formatted.contains("\n        d: 4\n"));
        assert!(formatted.contains("\n      }}>\n"));
    }

    #[test]
    fn fuzz_crash_idempotent() {
        let input = "\u{00da}\n???;";
        let out1 = format_text(input);
        let out2 = format_text(&out1);
        assert_eq!(
            out1, out2,
            "not idempotent on crash input: pass1={:?} pass2={:?}",
            out1, out2
        );
    }

    /// Regression: unclosed opener inflated the delimiter stack, causing downstream
    /// lines to receive wrong indentation and context ("scrambled variables").
    #[test]
    fn mismatched_brackets_returns_input_unchanged() {
        // Unclosed paren
        let input = "module demo\n\nmain = do Effect {\n  x <- foo\n  y <- bar (\n  z <- baz\n}\n";
        assert_eq!(format_text(input), input);

        // Unclosed bracket
        let input2 =
            "module demo\n\nmain = do Effect {\n  x <- foo\n  y <- [1, 2, 3\n  z <- baz\n}\n";
        assert_eq!(format_text(input2), input2);

        // Unclosed brace leaking into unrelated definition
        let input3 = "module demo\n\nf = { x: 1\ng = y => y + 1\n";
        assert_eq!(format_text(input3), input3);

        // Extra closer
        let input4 = "module demo\n\nf = x => x + 1 )\ng = 2\n";
        assert_eq!(format_text(input4), input4);

        // Mismatched pair
        let input5 = "module demo\n\nf = (x => x]\n";
        assert_eq!(format_text(input5), input5);
    }

    /// Balanced brackets must still be formatted normally.
    #[test]
    fn balanced_brackets_still_formatted() {
        let input = "module demo\n\nmain = do Effect {\nx<-foo\n}\n";
        let formatted = format_text(input);
        assert_ne!(formatted, input, "balanced input should be formatted");
        assert!(formatted.contains("  x <- foo"));
    }

    /// Regression test: `;` between non-operator tokens must not insert a phantom
    /// space that the second pass then removes (non-idempotency).
    #[test]
    fn semicolon_no_phantom_space_idempotent() {
        // Replacement-char followed by semicolon — was non-idempotent before the fix.
        let bytes: &[u8] = &[0xda, 0x0a, 0x3f, 0x3f, 0x3f, 0x3b];
        let input = String::from_utf8_lossy(bytes);
        let out1 = format_text(&input);
        let out2 = format_text(&out1);
        assert_eq!(
            out1, out2,
            "not idempotent: pass1={:?} pass2={:?}",
            out1, out2
        );

        // Unknown tokens separated by `;` must stay idempotent.
        let input2 = "\u{FFFD};\u{FFFD}";
        let out1 = format_text(input2);
        let out2 = format_text(&out1);
        assert_eq!(out1, out2, "not idempotent for unknown;unknown");

        // Operators separated by `;` must still get a space (no merging).
        let input3 = "<;-";
        let out1 = format_text(input3);
        assert!(
            out1.contains("< -") || out1.trim() == "< -",
            "expected space between < and - but got {:?}",
            out1
        );
    }
}

#[cfg(test)]
mod align_tests {
    use super::*;
    #[test]
    fn align_uniform_records_in_list() {
        let input = "module demo\n\nappShortcuts = [\n  { key: \"n\", modifiers: \"ctrl\", action: \"compose\", label: \"New email\" }\n  { key: \"k\", modifiers: \"ctrl\", action: \"search\", label: \"Search\" }\n]\n";
        let out = format_text(input);
        // label should align (same column) across rows
        let lines: Vec<&str> = out.lines().collect();
        let label_cols: Vec<usize> = lines
            .iter()
            .filter(|l| l.contains("label:"))
            .map(|l| l.find("label:").expect("label: present"))
            .collect();
        assert!(label_cols.len() >= 2, "at least 2 label fields");
        assert_eq!(
            label_cols[0], label_cols[1],
            "label fields should be aligned"
        );
        // Verify idempotency
        let out2 = format_text(&out);
        assert_eq!(out, out2, "alignment should be idempotent");
    }

    #[test]
    fn align_consecutive_eq_bindings() {
        let input = "module demo\n\nmain = do Effect {\n  vendor = bill.billerName\n  amount = bill.amountDue ?? 0.0\n  dueAt = bill.dueDate ?? \"\"\n  logicalKey = sha256 \"{emailId}:{vendor}:{toText amount}\"\n  billId = sha256 \"{emailId}:{pv}:{logicalKey}\"\n}\n";
        let out = format_text(input);
        let lines: Vec<&str> = out.lines().collect();
        let eq_cols: Vec<usize> = lines
            .iter()
            .filter(|l| l.contains(" = "))
            .filter(|l| !l.contains("main"))
            .map(|l| l.find(" = ").expect("= present"))
            .collect();
        assert!(eq_cols.len() >= 2, "at least 2 binding lines");
        assert!(
            eq_cols.iter().all(|&c| c == eq_cols[0]),
            "all = should be aligned at same column, got {:?}",
            eq_cols,
        );
        let out2 = format_text(&out);
        assert_eq!(out, out2, "alignment should be idempotent");
    }

    #[test]
    fn align_consecutive_eq_bindings_top_level() {
        let input = "module demo\n\nadd = a => b => a + b\nsubtract = a => b => a - b\nmultiply = a => b => a * b\n";
        let out = format_text(input);
        let lines: Vec<&str> = out.lines().collect();
        let eq_cols: Vec<usize> = lines
            .iter()
            .filter(|l| l.contains(" = "))
            .map(|l| l.find(" = ").expect("= present"))
            .collect();
        assert_eq!(eq_cols.len(), 3, "3 binding lines");
        assert!(
            eq_cols.iter().all(|&c| c == eq_cols[0]),
            "all = should be aligned, got {:?}",
            eq_cols,
        );
        let out2 = format_text(&out);
        assert_eq!(out, out2, "alignment should be idempotent");
    }
}

#[cfg(test)]
mod bench {
    use super::*;
    #[test]
    #[ignore = "benchmark: requires /tmp/mailfox_main.aivi"]
    fn bench_format_large_file() {
        let content = std::fs::read_to_string("/tmp/mailfox_main.aivi")
            .expect("/tmp/mailfox_main.aivi must exist for this benchmark");
        let start = std::time::Instant::now();
        let n = 200u32;
        for _ in 0..n {
            let _ = format_text(&content);
        }
        let elapsed = start.elapsed();
        eprintln!(
            "bench: {} iters in {:?} = {:?}/iter",
            n,
            elapsed,
            elapsed / n
        );
    }
}
