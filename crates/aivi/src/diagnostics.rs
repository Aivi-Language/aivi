use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Serialize)]
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
    pub message: String,
    pub span: Span,
    pub labels: Vec<DiagnosticLabel>,
}

#[derive(Debug, Clone)]
pub struct FileDiagnostic {
    pub path: String,
    pub diagnostic: Diagnostic,
}

pub fn render_diagnostics(path: &str, diagnostics: &[Diagnostic]) -> String {
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
        ));
    }
    output
}

pub fn render_diagnostic(path: &str, diagnostic: &Diagnostic) -> String {
    render_diagnostic_with_source(path, diagnostic, None)
}

fn render_diagnostic_with_source(
    path: &str,
    diagnostic: &Diagnostic,
    source: Option<&str>,
) -> String {
    let mut output = String::new();
    let start = &diagnostic.span.start;
    output.push_str(&format!(
        "error[{}] {}:{}:{} {}\n",
        diagnostic.code, path, start.line, start.column, diagnostic.message
    ));
    if let Some(source) = source {
        if let Some(frame) = render_source_frame(source, &diagnostic.span, Some(&diagnostic.message))
        {
            output.push_str(&frame);
        }
    }
    for label in &diagnostic.labels {
        let pos = &label.span.start;
        output.push_str(&format!(
            "note: {} at {}:{}:{}\n",
            label.message, path, pos.line, pos.column
        ));
        if let Some(source) = source {
            if let Some(frame) = render_source_frame(source, &label.span, None) {
                output.push_str(&frame);
            }
        }
    }
    output.trim_end().to_string()
}

fn render_source_frame(source: &str, span: &Span, message: Option<&str>) -> Option<String> {
    let line_index = span.start.line.checked_sub(1)?;
    let line = source.lines().nth(line_index)?;
    let line_no = span.start.line;
    let width = line_no.to_string().len();

    let mut output = String::new();
    output.push_str("  |\n");
    output.push_str(&format!("{line_no:>width$} | {line}\n"));

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
    let mut caret_line = format!("{:>width$} | {padding}{carets}", "");
    if let Some(message) = message {
        caret_line.push(' ');
        caret_line.push_str(message);
    }
    caret_line.push('\n');
    output.push_str(&caret_line);
    Some(output)
}
