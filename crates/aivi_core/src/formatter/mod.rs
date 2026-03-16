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
    let result = if !content.contains("~mat[") {
        // Fast path: files without matrix literals need only a single formatting pass.
        engine::format_text_with_options(content, options)
    } else {
        // Transformations like semicolon removal or comma stripping can change the
        // token stream on re-lexing (e.g. `& ; &` → `& &` → `&&`).  Iterate
        // until a fixed point is reached (typically 1-3 passes).
        let mut result = engine::format_text_with_options(content, options);
        for _ in 0..4 {
            let collapsed = collapse_multiline_matrix(&result);
            let next = engine::format_text_with_options(&collapsed, options);
            if next == result {
                break;
            }
            result = next;
        }
        result
    };

    // Post-pass: group consecutive `use` lines that share a common module prefix.
    group_use_imports(&result, options.indent_size)
}

/// Parsed representation of a single flat `use` line for grouping purposes.
struct UseLine {
    indent: String,
    module: String,
    suffix: String,
    has_selective: bool,
}

/// Parse a flat `use ...` line into its components.
/// Returns `None` for non-use lines, aliased/hiding imports, or multi-line grouped imports.
fn parse_use_line(line: &str) -> Option<UseLine> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with("use ") {
        return None;
    }
    let indent = &line[..line.len() - trimmed.len()];
    let rest = &trimmed[4..];

    let mod_end = rest
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.')
        .unwrap_or(rest.len());
    let module = rest[..mod_end].trim_end_matches('.');
    if module.is_empty() {
        return None;
    }
    let suffix = rest[mod_end..].to_string();
    let suffix_trimmed = suffix.trim_start();
    let has_selective = suffix_trimmed.starts_with('(');

    // Skip aliased or hiding imports.
    if suffix_trimmed.starts_with("as ") || suffix_trimmed.starts_with("hiding") {
        return None;
    }

    // Skip already-grouped imports: if parens are not balanced on this line,
    // it is a multi-line grouped import — leave it alone for idempotency.
    if has_selective {
        let mut depth: i32 = 0;
        for ch in suffix_trimmed.chars() {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
        }
        if depth != 0 {
            return None;
        }
    }

    Some(UseLine {
        indent: indent.to_string(),
        module: module.to_string(),
        suffix,
        has_selective,
    })
}

/// Group consecutive flat `use` lines sharing a common module prefix into
/// grouped import form. Only groups selective-import lines (with parens).
fn group_use_imports(text: &str, indent_size: usize) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;

    while i < lines.len() {
        let run_start = i;
        let mut use_lines: Vec<(usize, UseLine)> = Vec::new();

        while i < lines.len() {
            if let Some(ul) = parse_use_line(lines[i]) {
                use_lines.push((i, ul));
                i += 1;
            } else {
                break;
            }
        }

        if use_lines.len() < 2 {
            for (idx, _) in &use_lines {
                out.push(lines[*idx].to_string());
            }
            if use_lines.is_empty() {
                out.push(lines[run_start].to_string());
                i = run_start + 1;
            }
            continue;
        }

        // Group by common prefix (everything up to the last `.segment`).
        let mut groups: Vec<Vec<usize>> = Vec::new();
        let mut group_indices: std::collections::HashMap<String, usize> = Default::default();

        for (idx, (_line_idx, ul)) in use_lines.iter().enumerate() {
            if !ul.has_selective {
                let key = format!("__standalone_{idx}");
                group_indices.insert(key, groups.len());
                groups.push(vec![idx]);
                continue;
            }
            if let Some(dot_pos) = ul.module.rfind('.') {
                let prefix = &ul.module[..dot_pos];
                if let Some(&gi) = group_indices.get(prefix) {
                    groups[gi].push(idx);
                } else {
                    group_indices.insert(prefix.to_string(), groups.len());
                    groups.push(vec![idx]);
                }
            } else {
                let key = format!("__standalone_{idx}");
                group_indices.insert(key, groups.len());
                groups.push(vec![idx]);
            }
        }

        let inner_indent = " ".repeat(indent_size);

        for group in &groups {
            if group.len() < 2 {
                let (line_idx, _) = &use_lines[group[0]];
                out.push(lines[*line_idx].to_string());
                continue;
            }

            let first = &use_lines[group[0]].1;
            let prefix = {
                let dot_pos = first.module.rfind('.').expect("checked above");
                first.module[..dot_pos].to_string()
            };
            let indent = &first.indent;

            let mut grouped = format!("{indent}use {prefix} (");
            for &member_idx in group {
                let ul = &use_lines[member_idx].1;
                let dot_pos = ul.module.rfind('.').expect("checked above");
                let sub_module = &ul.module[dot_pos + 1..];
                let selective = ul.suffix.trim_start();
                grouped.push('\n');
                grouped.push_str(&format!("{indent}{inner_indent}{sub_module} {selective}"));
            }
            grouped.push('\n');
            grouped.push_str(&format!("{indent})"));
            out.push(grouped);
        }
    }

    out.join("\n")
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
            | "<<-"
            | "->>"
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
    fn format_preserves_space_before_accessor_argument_dot() {
        let input = "names = people |> map .name\n";
        let formatted = format_text(input);
        assert_eq!(formatted, input);

        let input2 = "name = user.name\n";
        let formatted2 = format_text(input2);
        assert_eq!(formatted2, input2);
    }

    #[test]
    fn format_html_sigil_uses_one_tag_per_line_and_nested_indent() {
        let text = "module demo\n\nx=~<html><div class=\"card\"><span>ok</span></div></html>\n";
        let formatted = format_text(text);
        assert_eq!(
            formatted,
            "module demo\n\nx = ~<html>\n  <div class=\"card\">\n    <span>\n      ok\n    </span>\n  </div>\n</html>\n"
        );
    }

    #[test]
    fn format_gtk_sigil_keeps_short_attr_lists_on_single_line() {
        let text = "module demo\n\nx=~<gtk><object a=\"1\" b=\"2\" c=\"3\" d=\"4\"><child><object class=\"GtkLabel\"/></child></object></gtk>\n";
        let formatted = format_text(text);
        assert!(formatted.contains("<object a=\"1\" b=\"2\" c=\"3\" d=\"4\">"));
        assert!(formatted.contains("\n      <object class=\"GtkLabel\" />\n"));
    }

    #[test]
    fn format_gtk_sigil_keeps_five_plus_attributes_inline_when_source_is_inline() {
        let text = "module demo\n\nx=~<gtk><object a=\"1\" b=\"2\" c=\"3\" d=\"4\" e=\"5\"><child><object class=\"GtkLabel\"/></child></object></gtk>\n";
        let formatted = format_text(text);
        assert!(formatted.contains("<object a=\"1\" b=\"2\" c=\"3\" d=\"4\" e=\"5\">"));
        assert!(formatted.contains("\n    <child>\n"));
        assert!(formatted.contains("\n      <object class=\"GtkLabel\" />\n"));
    }

    #[test]
    fn format_gtk_sigil_wraps_attributes_when_source_has_newlines() {
        let text =
            "module demo\n\nx=~<gtk><object\n  a=\"1\"\n  b=\"2\"\n  c=\"3\"></object></gtk>\n";
        let formatted = format_text(text);
        assert!(formatted.contains("\n  <object\n"));
        assert!(formatted.contains("\n    a=\"1\"\n"));
        assert!(formatted.contains("\n    c=\"3\">\n"));
    }

    #[test]
    fn format_gtk_sigil_keeps_short_wrapped_record_attribute_values_inline() {
        let text = "module demo\n\nx=~<gtk><object class=\"GtkBox\" props={ { orientation: \"vertical\", spacing: 0, marginStart: 8 } }></object></gtk>\n";
        let formatted = format_text(text);
        assert!(
            formatted
                .contains("<object class=\"GtkBox\" props={{ orientation: \"vertical\", spacing: 0, marginStart: 8 }}>")
        );
        assert!(!formatted.contains("\n    orientation: \"vertical\",\n"));
    }

    #[test]
    fn format_gtk_sigil_formats_wrapped_record_attribute_values() {
        let text = "module demo\n\nx=~<gtk><object class=\"GtkBox\" props={ { orientation: \"vertical\", spacing: 0, marginStart: 8, marginEnd: 8, marginBottom: 8 } }></object></gtk>\n";
        let formatted = format_text(text);
        assert!(formatted.contains("<object class=\"GtkBox\" props={{\n"));
        assert!(formatted.contains("\n    orientation: \"vertical\",\n"));
        assert!(formatted.contains("\n    spacing: 0,\n"));
        assert!(formatted.contains("\n    marginStart: 8,\n"));
        assert!(formatted.contains("\n    marginEnd: 8,\n"));
        assert!(formatted.contains("\n    marginBottom: 8\n"));
        assert!(formatted.contains("\n  }}>\n"));
    }

    #[test]
    fn format_html_sigil_formats_long_wrapped_record_attribute_values() {
        let text =
            "module demo\n\nx=~<html><div props={ { a: 1, b: 2, c: 3, d: 4 } }></div></html>\n";
        let formatted = format_text(text);
        assert!(formatted.contains("<div props={{\n"));
        assert!(formatted.contains("\n    a: 1,\n"));
        assert!(formatted.contains("\n    b: 2,\n"));
        assert!(formatted.contains("\n    c: 3,\n"));
        assert!(formatted.contains("\n    d: 4\n"));
        assert!(formatted.contains("\n  }}>\n"));
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

    #[test]
    fn format_indents_multiclause_arms_after_lambda_arrow() {
        let input = "\
module demo

move : Pixel -> Direction -> Pixel
move = (x, y) =>
| Up => (x, y - 1)
| Down => (x, y + 1)
| Left => (x - 1, y)
| Right => (x + 1, y)
";
        let expected = "\
module demo

move : Pixel -> Direction -> Pixel
move = (x, y) =>
  | Up    => (x, y - 1)
  | Down  => (x, y + 1)
  | Left  => (x - 1, y)
  | Right => (x + 1, y)
";
        let formatted = format_text(input);
        assert_eq!(formatted, expected);
        assert_eq!(
            format_text(&formatted),
            expected,
            "formatting should be idempotent"
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

    #[test]
    fn format_groups_use_imports_with_shared_prefix() {
        let input = "module demo\n\nuse aivi.text.utils (toUpper, toLower)\nuse aivi.text.format (padLeft)\n\nmain = 1\n";
        let formatted = format_text(input);
        assert!(
            formatted.contains("use aivi.text ("),
            "expected grouped use, got:\n{}",
            formatted,
        );
        assert!(
            formatted.contains("  utils (toUpper, toLower)"),
            "expected utils sub-import, got:\n{}",
            formatted,
        );
        assert!(
            formatted.contains("  format (padLeft)"),
            "expected format sub-import, got:\n{}",
            formatted,
        );
        let out2 = format_text(&formatted);
        assert_eq!(
            formatted, out2,
            "grouped use formatting should be idempotent"
        );
    }

    #[test]
    fn format_does_not_group_wildcard_uses() {
        let input = "module demo\n\nuse aivi.text\nuse aivi.math\n\nmain = 1\n";
        let formatted = format_text(input);
        assert!(formatted.contains("use aivi.text\n"));
        assert!(formatted.contains("use aivi.math\n"));
    }

    #[test]
    fn format_does_not_group_single_use() {
        let input = "module demo\n\nuse aivi.text.utils (toUpper)\n\nmain = 1\n";
        let formatted = format_text(input);
        assert!(formatted.contains("use aivi.text.utils (toUpper)"));
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
mod garbage_bracket_tests {
    use super::*;

    /// Pure garbage: only brackets and random tokens. Must not panic.
    #[test]
    fn pure_garbage_brackets_does_not_panic() {
        let inputs = [
            "}}}0 )))",
            "((({{{[[[",
            "}])>",
            "{{{}}})))(((]]]",
            "module demo\n\n}}}0 )))\n",
            "))))}}}]]] let x = 1",
            "{{{((([[[)))]]]}}}",
            "({[)}]",
            "))))))))))",
            "((((((((((",
            "{ } ( ) [ ] { } ( ) [ ] } } }",
        ];
        for input in &inputs {
            let _ = format_text(input);
        }
    }

    /// Unbalanced input is returned verbatim (no formatting applied).
    #[test]
    fn garbage_brackets_returned_unchanged() {
        let inputs = [
            "}}}0 )))",
            "module demo\n\nf = x => x + 1\n}}}0 )))\ng = 2\n",
            "((({{{[[[\nmodule demo\nf = 1\n",
            "module demo\n\nf = (x => x ]\n",
            "module demo\n\nf = { x: 1\ng = y => y + 1\n",
        ];
        for input in &inputs {
            assert_eq!(
                format_text(input),
                *input,
                "unbalanced input should be returned unchanged: {:?}",
                input,
            );
        }
    }

    /// Valid code with garbage appended at the end should be returned unchanged
    /// (the garbage unbalances the file).
    #[test]
    fn valid_code_with_trailing_garbage_unchanged() {
        let input = "module demo\n\nf = x => x + 1\ng = y => y * 2\n}}}0 )))\n";
        assert_eq!(format_text(input), input);
    }

    /// Valid code with garbage prepended should be returned unchanged.
    #[test]
    fn valid_code_with_leading_garbage_unchanged() {
        let input = "}}}0 )))\nmodule demo\n\nf = x => x + 1\n";
        assert_eq!(format_text(input), input);
    }

    /// Garbage interspersed in otherwise valid code — unbalanced, returned unchanged.
    #[test]
    fn garbage_interspersed_in_valid_code() {
        let input = "module demo\n\nf = x => (x + 1\n))) extra\ng = 2\n";
        assert_eq!(format_text(input), input);
    }

    /// Balanced brackets with random tokens inside should not panic.
    #[test]
    fn balanced_brackets_with_random_content_no_panic() {
        let inputs = [
            "(0)",
            "{}}}{", // still unbalanced overall
            "({[]})",
            "((({{{[[[]]]}}})))",
            "module demo\n\nf = (((x)))\n",
            "module demo\n\nf = {a: {b: {c: 1}}}\n",
        ];
        for input in &inputs {
            let _ = format_text(input);
        }
    }

    /// Strings containing brackets should not affect the balance check.
    #[test]
    fn brackets_inside_strings_ignored_by_balance_check() {
        // The string contains unbalanced brackets, but the bracket check should skip strings.
        // We intentionally omit spaces around `=` to verify the formatter runs (it adds spaces).
        let input = "module demo\n\nf=\"{{{)))}}}\" |> g\n";
        let formatted = format_text(input);
        // This is balanced code (string content not counted), so it should format
        assert!(
            formatted.contains("f = "),
            "balanced code (brackets in strings) should be formatted, got: {:?}",
            formatted,
        );
    }

    /// Real-world scenario: user accidentally deletes a closing brace in a do-block.
    #[test]
    fn missing_closing_brace_in_do_block() {
        let input = "\
module demo

main = do Effect {
  x <- foo
  y <- bar
  pure x

helper = a => a + 1
";
        assert_eq!(
            format_text(input),
            input,
            "missing closing brace should return input unchanged",
        );
    }

    /// Real-world scenario: extra closing paren in a function call chain.
    #[test]
    fn extra_closing_paren_in_pipeline() {
        let input = "module demo\n\nresult = xs |> map f) |> filter g\n";
        assert_eq!(format_text(input), input);
    }

    /// Idempotency: formatting balanced code twice gives the same result.
    #[test]
    fn idempotency_on_complex_balanced_code() {
        let input = "\
module demo

use aivi

State = { count: Int, name: Text }

f : State -> State
f = s => s <| { count: s.count + 1 }

g = do Effect {
  x <- pure 1
  y <- pure (x + 2)
  pure (x + y)
}

view = state =>
  ~<gtk>
    <GtkBox orientation=\"vertical\" spacing=\"8\">
      <GtkLabel label=\"hello\" />
    </GtkBox>
  </gtk>
";
        let out1 = format_text(input);
        let out2 = format_text(&out1);
        assert_eq!(out1, out2, "formatting should be idempotent");
    }
}
