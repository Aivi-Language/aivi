use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticLabel {
    pub message: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub span: Span,
    pub labels: Vec<DiagnosticLabel>,
}

#[derive(Debug, Clone)]
pub struct FileDiagnostic {
    pub path: String,
    pub diagnostic: Diagnostic,
}

// ANSI color codes
const RED: &str = "\x1b[1;31m";
const YELLOW: &str = "\x1b[1;33m";
const CYAN: &str = "\x1b[1;36m";
const DARK_GRAY: &str = "\x1b[90m";
const WHITE: &str = "\x1b[97m";
const ORANGE: &str = "\x1b[38;5;208m";
const RESET: &str = "\x1b[0m";

pub fn file_diagnostics_have_errors(diagnostics: &[FileDiagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diag| diag.diagnostic.severity == DiagnosticSeverity::Error)
}

pub fn render_diagnostics(path: &str, diagnostics: &[Diagnostic], use_color: bool) -> String {
    let mut output = String::new();
    let source = std::fs::read_to_string(path).ok();
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }
        output.push_str(&render_diagnostic_with_source(
            path,
            diagnostic,
            source.as_deref(),
            use_color,
        ));
    }
    output
}

fn caret_color(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Error => RED,
        DiagnosticSeverity::Warning => YELLOW,
    }
}

fn caret_message_color(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Error => ORANGE,
        DiagnosticSeverity::Warning => YELLOW,
    }
}

fn render_diagnostic_with_source(
    path: &str,
    diagnostic: &Diagnostic,
    source: Option<&str>,
    use_color: bool,
) -> String {
    let mut output = String::new();
    let start = &diagnostic.span.start;
    let severity_label = match diagnostic.severity {
        DiagnosticSeverity::Error => "error",
        DiagnosticSeverity::Warning => "warning",
    };
    if use_color {
        output.push_str(&format!(
            "{YELLOW}{severity_label}[{}]{RESET} {DARK_GRAY}{}:{}:{}{RESET}\n  {WHITE}{}{RESET}\n",
            diagnostic.code, path, start.line, start.column, diagnostic.message
        ));
    } else {
        output.push_str(&format!(
            "{severity_label}[{}] {}:{}:{}\n  {}\n",
            diagnostic.code, path, start.line, start.column, diagnostic.message
        ));
    }
    if let Some(source) = source {
        if let Some(frame) = render_source_frame(
            source,
            &diagnostic.span,
            Some(&diagnostic.message),
            use_color,
            diagnostic.severity,
        ) {
            output.push_str(&frame);
        }
    }
    for label in &diagnostic.labels {
        let pos = &label.span.start;
        if use_color {
            output.push_str(&format!(
                "{CYAN}note{RESET}: {WHITE}{}{RESET} at {DARK_GRAY}{}:{}:{}{RESET}\n",
                label.message, path, pos.line, pos.column
            ));
        } else {
            output.push_str(&format!(
                "note: {} at {}:{}:{}\n",
                label.message, path, pos.line, pos.column
            ));
        }
        if let Some(source) = source {
            if let Some(frame) = render_source_frame(
                source,
                &label.span,
                None,
                use_color,
                diagnostic.severity,
            ) {
                output.push_str(&frame);
            }
        }
    }
    output.trim_end().to_string()
}

fn render_source_frame(
    source: &str,
    span: &Span,
    message: Option<&str>,
    use_color: bool,
    severity: DiagnosticSeverity,
) -> Option<String> {
    let line_index = span.start.line.checked_sub(1)?;
    let line = source.lines().nth(line_index)?;
    let line_no = span.start.line;
    let width = line_no.to_string().len();

    let mut output = String::new();
    if use_color {
        output.push_str(&format!("{DARK_GRAY}{:>width$} |{RESET}\n", ""));
        output.push_str(&format!("{DARK_GRAY}{line_no:>width$} |{RESET} {line}\n"));
    } else {
        output.push_str("  |\n");
        output.push_str(&format!("{line_no:>width$} | {line}\n"));
    }

    let line_chars: Vec<char> = line.chars().collect();
    let line_len = line_chars.len();
    let mut start_col = span.start.column;
    if start_col == 0 {
        start_col = 1;
    }
    if start_col > line_len + 1 {
        start_col = line_len + 1;
    }
    let mut end_col = if span.start.line == span.end.line {
        span.end.column
    } else {
        start_col
    };
    if end_col < start_col {
        end_col = start_col;
    }
    if end_col > line_len {
        end_col = line_len.max(start_col);
    }
    let caret_len = end_col.saturating_sub(start_col).saturating_add(1);

    let padding = " ".repeat(start_col.saturating_sub(1));
    let carets = "^".repeat(caret_len);
    if use_color {
        let cc = caret_color(severity);
        let mc = caret_message_color(severity);
        let mut caret_line = format!("{DARK_GRAY}{:>width$} |{RESET} {padding}{cc}{carets}{RESET}", "");
        if let Some(message) = message {
            caret_line.push(' ');
            caret_line.push_str(mc);
            caret_line.push_str(message);
            caret_line.push_str(RESET);
        }
        caret_line.push('\n');
        output.push_str(&caret_line);
    } else {
        let mut caret_line = format!("{:>width$} | {padding}{carets}", "");
        if let Some(message) = message {
            caret_line.push(' ');
            caret_line.push_str(message);
        }
        caret_line.push('\n');
        output.push_str(&caret_line);
    }
    Some(output)
}
