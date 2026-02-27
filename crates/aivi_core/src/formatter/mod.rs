// This module mirrors `crates/aivi/src/formatter.rs`, but lives in `aivi_core` so that
// `mod doc; mod engine;` resolves against `crates/aivi_core/src/formatter/*`.

mod doc;
mod engine;

use rayon::prelude::*;

/// Minimum number of top-level segments before we bother spawning parallel work.
/// Below this threshold the overhead of Rayon task scheduling exceeds any gain.
const MIN_SEGMENTS_FOR_PARALLEL: usize = 4;

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
    // Fast path: files without matrix literals need only a single formatting
    // pass.  The collapse/re-format loop exists exclusively to stabilise
    // `~mat[...]` column alignment across passes; skip it entirely for the
    // common case to cut runtime roughly in half.
    if !content.contains("~mat[") {
        return format_parallel(content, options);
    }

    // Transformations like semicolon removal or comma stripping can change the
    // token stream on re-lexing (e.g. `& ; &` → `& &` → `&&`).  Iterate
    // until a fixed point is reached (typically 1-3 passes).
    let mut result = format_parallel(content, options);
    for _ in 0..4 {
        // Collapse any multi-line `~mat[...]` back to a single line with `;`
        // row separators so the next pass can re-detect and re-align the matrix.
        let collapsed = collapse_multiline_matrix(&result);
        let next = format_parallel(&collapsed, options);
        if next == result {
            break;
        }
        result = next;
    }
    result
}

/// Split the file into independent top-level segments at depth-0 blank lines
/// and format them in parallel using Rayon.  Falls back to sequential formatting
/// when the file is too small to benefit.
fn format_parallel(content: &str, options: FormatOptions) -> String {
    let segments = split_top_level_segments(content);
    if segments.len() < MIN_SEGMENTS_FOR_PARALLEL {
        return engine::format_text_with_options(content, options);
    }

    let formatted: Vec<String> = segments
        .par_iter()
        .map(|seg| {
            let mut out = engine::format_text_with_options(seg, options);
            // Each segment is formatted as a standalone file, so it has a trailing
            // newline.  Strip it — we re-join with blank-line separators below.
            while out.ends_with('\n') {
                out.pop();
            }
            out
        })
        .collect();

    let mut result = formatted.join("\n\n");
    // Ensure exactly one trailing newline.
    while result.ends_with('\n') {
        result.pop();
    }
    result.push('\n');
    result
}

/// Split source text into top-level segments at blank lines where the delimiter
/// depth (`{`, `[`, `(`) is zero.  Each segment contains one or more top-level
/// declarations.  Consecutive blank lines at depth 0 are collapsed into segment
/// boundaries; the blank lines themselves are not included in any segment.
fn split_top_level_segments(content: &str) -> Vec<&str> {
    /// Return the portion of `line` before any `//` line comment, respecting strings.
    fn strip_line_comment(line: &str, already_in_string: bool) -> &str {
        let mut in_str = already_in_string;
        let mut escape = false;
        let bytes = line.as_bytes();
        for i in 0..bytes.len() {
            let ch = bytes[i];
            if escape {
                escape = false;
                continue;
            }
            if ch == b'\\' && in_str {
                escape = true;
                continue;
            }
            if in_str {
                if ch == b'"' {
                    in_str = false;
                }
                continue;
            }
            if ch == b'"' {
                in_str = true;
                continue;
            }
            if ch == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                return &line[..i];
            }
        }
        line
    }

    let lines: Vec<&str> = content.split('\n').collect();
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut escape_next = false;

    // For each line, track the delimiter depth at the END of that line.
    // A blank line at depth 0 is a valid split point.
    let mut line_end_depths: Vec<i32> = Vec::with_capacity(lines.len());
    for line in &lines {
        // Strip line comments before scanning delimiters.
        let effective = strip_line_comment(line, in_string);
        for ch in effective.chars() {
            if escape_next {
                escape_next = false;
                continue;
            }
            if ch == '\\' && in_string {
                escape_next = true;
                continue;
            }
            if in_string {
                if ch == '"' {
                    in_string = false;
                }
                continue;
            }
            match ch {
                '"' => {
                    in_string = true;
                }
                '{' | '[' | '(' => depth += 1,
                '}' | ']' | ')' => depth = (depth - 1).max(0),
                _ => {}
            }
        }
        line_end_depths.push(depth);
    }

    // Find split points: blank lines where both the previous non-blank line and
    // this line are at depth 0.
    let mut segments: Vec<&str> = Vec::new();
    // Track byte offsets for zero-copy slicing of `content`.
    let mut seg_start_byte: usize = 0;
    let mut last_non_blank_line: Option<usize> = None;
    let mut i = 0;

    // Precompute byte offset of each line start for O(1) slicing.
    let mut line_byte_starts: Vec<usize> = Vec::with_capacity(lines.len() + 1);
    {
        let mut offset = 0usize;
        for line in &lines {
            line_byte_starts.push(offset);
            offset += line.len() + 1; // +1 for the '\n'
        }
        line_byte_starts.push(offset); // sentinel for end
    }

    while i < lines.len() {
        if lines[i].trim().is_empty() {
            // Blank line — check if this is a valid split point.
            let at_depth_0 = last_non_blank_line
                .map(|l| line_end_depths[l] == 0)
                .unwrap_or(true);

            if at_depth_0 {
                if let Some(prev_line) = last_non_blank_line {
                    // Don't split between consecutive `use` groups — the post-render
                    // pass needs to see them together to manage inter-group blank lines.
                    let prev_is_use = lines[prev_line].trim_start().starts_with("use ");
                    let next_non_blank = lines[i + 1..].iter().find(|l| !l.trim().is_empty());
                    let next_is_use = next_non_blank
                        .map(|l| l.trim_start().starts_with("use "))
                        .unwrap_or(false);
                    let between_uses = prev_is_use && next_is_use;

                    // Only split when the previous non-blank line starts at column 0
                    // (no leading whitespace).  Indented lines may be continuations of
                    // the preceding definition whose formatting depends on state from
                    // earlier lines (e.g. `rhs_block_base_indent` after `=`).
                    let prev_trimmed = lines[prev_line].trim_start();
                    let prev_at_col0 =
                        !prev_trimmed.is_empty() && prev_trimmed.len() == lines[prev_line].len();

                    if !between_uses && prev_at_col0 {
                        // End the current segment at the end of the last non-blank line.
                        let end_byte = line_byte_starts[prev_line] + lines[prev_line].len();
                        let seg = &content[seg_start_byte..end_byte.min(content.len())];
                        if !seg.trim().is_empty() {
                            segments.push(seg);
                        }
                        // Skip all consecutive blank lines.
                        while i < lines.len() && lines[i].trim().is_empty() {
                            i += 1;
                        }
                        seg_start_byte = if i < lines.len() {
                            line_byte_starts[i]
                        } else {
                            content.len()
                        };
                        continue;
                    }
                }
            }
        } else {
            last_non_blank_line = Some(i);
        }
        i += 1;
    }

    // Flush the last segment.
    if seg_start_byte < content.len() {
        let seg = &content[seg_start_byte..];
        // Trim trailing newlines from the last segment for consistency.
        let trimmed = seg.trim_end_matches('\n');
        if !trimmed.is_empty() {
            segments.push(trimmed);
        }
    }

    segments
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
    fn format_gtk_sigil_wraps_five_plus_attributes_and_indents_nested_tags() {
        let text = "module demo\n\nx=~<gtk><object a=\"1\" b=\"2\" c=\"3\" d=\"4\" e=\"5\"><child><object class=\"GtkLabel\"/></child></object></gtk>\n";
        let formatted = format_text(text);
        assert!(formatted.contains("\n      <object\n"));
        assert!(formatted.contains("\n        a=\"1\"\n"));
        assert!(formatted.contains("\n        e=\"5\">\n"));
        assert!(formatted.contains("\n        <child>\n"));
        assert!(formatted.contains("\n          <object class=\"GtkLabel\" />\n"));
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

    /// Regression test: `;` between non-operator tokens must not insert a phantom
    /// space that the second pass then removes (non-idempotency).
    #[test]
    fn semicolon_no_phantom_space_idempotent() {
        // Replacement-char followed by semicolon — was non-idempotent before the fix.
        let bytes: &[u8] = &[0xda, 0x0a, 0x3f, 0x3f, 0x3f, 0x3b];
        let input = String::from_utf8_lossy(bytes);
        let out1 = format_text(&input);
        let out2 = format_text(&out1);
        assert_eq!(out1, out2, "not idempotent: pass1={:?} pass2={:?}", out1, out2);

        // Unknown tokens separated by `;` must stay idempotent.
        let input2 = "\u{FFFD};\u{FFFD}";
        let out1 = format_text(input2);
        let out2 = format_text(&out1);
        assert_eq!(out1, out2, "not idempotent for unknown;unknown");

        // Operators separated by `;` must still get a space (no merging).
        let input3 = "<;-";
        let out1 = format_text(input3);
        assert!(out1.contains("< -") || out1.trim() == "< -",
            "expected space between < and - but got {:?}", out1);
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
}

#[cfg(test)]
mod bench {
    use super::*;
    #[test]
    fn bench_format_large_file() {
        let content = std::fs::read_to_string("/tmp/mailfox_main.aivi").unwrap_or_default();
        if content.is_empty() {
            return;
        }
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
