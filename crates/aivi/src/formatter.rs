use crate::lexer::lex;
use crate::syntax;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatOptions {
    pub indent_size: usize,
    pub max_blank_lines: usize,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            indent_size: 2,
            max_blank_lines: 1,
        }
    }
}

pub fn format_text(content: &str) -> String {
    format_text_with_options(content, FormatOptions::default())
}

pub fn format_text_with_options(content: &str, options: FormatOptions) -> String {
    include!("formatter/format_text_with_options_body.rs")
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
        let text = "module demo\n\nmain = effect {\n_<-print \"hi\"\n}\n";
        let formatted = format_text_with_options(
            text,
            FormatOptions {
                indent_size: 4,
                max_blank_lines: 1,
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
            },
        );
        assert_eq!(formatted, "module demo\n\nmain = 1\n");
    }

    #[test]
    fn format_indents_multiline_match_arms_and_continuations() {
        let text = "module demo\n\nf = ?\n  | { a@{\n    x\n  } } => x\n| _ => 0\n";
        let formatted = format_text(text);
        assert_eq!(
            formatted,
            "module demo\n\nf = ?\n  | { a@{\n      x\n    } } => x\n  | _ => 0\n"
        );
    }
}
