use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Hint,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    User,
    EmbeddedStdlib,
    Generated,
    Synthetic,
}

impl SourceKind {
    pub fn as_i64(self) -> i64 {
        match self {
            SourceKind::User => 0,
            SourceKind::EmbeddedStdlib => 1,
            SourceKind::Generated => 2,
            SourceKind::Synthetic => 3,
        }
    }

    pub fn from_i64(raw: i64) -> Self {
        match raw {
            1 => SourceKind::EmbeddedStdlib,
            2 => SourceKind::Generated,
            3 => SourceKind::Synthetic,
            _ => SourceKind::User,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceOrigin {
    pub path: String,
    pub span: Span,
    pub source_kind: SourceKind,
}

pub fn deserialize_optional_source_origin_lossy<'de, D>(
    deserializer: D,
) -> Result<Option<SourceOrigin>, D::Error>
where
    D: Deserializer<'de>,
{
    #[allow(dead_code)]
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum WireValue {
        Origin(SourceOrigin),
        LegacyString(String),
    }

    Ok(match Option::<WireValue>::deserialize(deserializer)? {
        Some(WireValue::Origin(origin)) => Some(origin),
        Some(WireValue::LegacyString(_)) | None => None,
    })
}

impl SourceOrigin {
    pub fn new(path: impl Into<String>, span: Span) -> Self {
        let path = path.into();
        let source_kind = classify_source_kind(&path);
        Self {
            path,
            span,
            source_kind,
        }
    }

    pub fn with_kind(path: impl Into<String>, span: Span, source_kind: SourceKind) -> Self {
        Self {
            path: path.into(),
            span,
            source_kind,
        }
    }

    pub fn start_position_text(&self) -> String {
        format!(
            "{}:{}:{}",
            self.path, self.span.start.line, self.span.start.column
        )
    }

    pub fn embedded_module_name(&self) -> Option<&str> {
        self.path
            .strip_prefix("<embedded:")
            .and_then(|rest| rest.strip_suffix('>'))
    }
}

pub fn classify_source_kind(path: &str) -> SourceKind {
    if path.starts_with("<embedded:") && path.ends_with('>') {
        SourceKind::EmbeddedStdlib
    } else if path.starts_with("<generated:") && path.ends_with('>') {
        SourceKind::Generated
    } else if path.starts_with('<') && path.ends_with('>') {
        SourceKind::Synthetic
    } else {
        SourceKind::User
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticLabel {
    pub message: String,
    pub span: Span,
}

/// A machine-applicable fix suggestion.
#[derive(Debug, Clone, Serialize)]
pub struct Suggestion {
    pub message: String,
    pub replacement: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub span: Span,
    pub labels: Vec<DiagnosticLabel>,
    /// Free-form help messages shown below the source frame.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hints: Vec<String>,
    /// A concrete code fix the user can apply.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<Suggestion>,
}

impl Diagnostic {
    /// Create a diagnostic with no hints or suggestion.
    pub fn new(
        code: impl Into<String>,
        severity: DiagnosticSeverity,
        message: impl Into<String>,
        span: Span,
        labels: Vec<DiagnosticLabel>,
    ) -> Self {
        Self {
            code: code.into(),
            severity,
            message: message.into(),
            span,
            labels,
            hints: Vec::new(),
            suggestion: None,
        }
    }

    /// Add a help hint to this diagnostic.
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hints.push(hint.into());
        self
    }

    /// Add a machine-applicable fix suggestion.
    pub fn with_suggestion(mut self, suggestion: Suggestion) -> Self {
        self.suggestion = Some(suggestion);
        self
    }
}

#[derive(Debug, Clone)]
pub struct FileDiagnostic {
    pub path: String,
    pub diagnostic: Diagnostic,
}

/// Find the best fuzzy match for `name` among `candidates`.
/// Returns `Some("did you mean `X`?")` if a close match is found.
pub fn fuzzy_suggest<'a>(name: &str, candidates: impl Iterator<Item = &'a str>) -> Option<String> {
    let threshold = name.len().div_ceil(3).clamp(1, 3);
    let mut best: Option<(&str, usize)> = None;
    for candidate in candidates {
        let dist = levenshtein(name, candidate);
        if dist <= threshold && best.as_ref().is_none_or(|(_, d)| dist < *d) {
            best = Some((candidate, dist));
        }
    }
    best.map(|(s, _)| format!("did you mean `{s}`?"))
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (m, n) = (a.len(), b.len());
    let mut prev = (0..=n).collect::<Vec<_>>();
    let mut curr = vec![0; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

// ANSI color codes
const RED: &str = "\x1b[1;31m";
const YELLOW: &str = "\x1b[1;33m";
const CYAN: &str = "\x1b[1;36m";
const GREEN: &str = "\x1b[1;32m";
const DARK_GRAY: &str = "\x1b[90m";
const WHITE: &str = "\x1b[97m";
const RESET: &str = "\x1b[0m";

pub fn file_diagnostics_have_errors(diagnostics: &[FileDiagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diag| diag.diagnostic.severity == DiagnosticSeverity::Error)
}

/// Render all diagnostics for a file, with a trailing summary line.
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
    // Summary line
    let errors = diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .count();
    let warnings = diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Warning)
        .count();
    if errors > 0 || warnings > 0 {
        output.push('\n');
        let mut parts = Vec::new();
        if errors > 0 {
            let label = if errors == 1 { "error" } else { "errors" };
            parts.push(format!("{errors} {label}"));
        }
        if warnings > 0 {
            let label = if warnings == 1 { "warning" } else { "warnings" };
            parts.push(format!("{warnings} {label}"));
        }
        let summary = parts.join("; ");
        if use_color {
            output.push_str(&format!("{RED}aborting due to {summary}{RESET}\n"));
        } else {
            output.push_str(&format!("aborting due to {summary}\n"));
        }
    }
    output
}

fn severity_color(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Error => RED,
        DiagnosticSeverity::Warning => YELLOW,
        DiagnosticSeverity::Hint => CYAN,
    }
}

fn caret_color(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Error => RED,
        DiagnosticSeverity::Warning => YELLOW,
        DiagnosticSeverity::Hint => CYAN,
    }
}

fn severity_label(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Error => "error",
        DiagnosticSeverity::Warning => "warning",
        DiagnosticSeverity::Hint => "hint",
    }
}

pub fn render_diagnostic(path: &str, diagnostic: &Diagnostic, use_color: bool) -> String {
    let source = std::fs::read_to_string(path).ok();
    render_diagnostic_with_source(path, diagnostic, source.as_deref(), use_color)
}

pub fn render_diagnostic_with_source(
    path: &str,
    diagnostic: &Diagnostic,
    source: Option<&str>,
    use_color: bool,
) -> String {
    let mut output = String::new();
    let start = &diagnostic.span.start;
    let sev = severity_label(diagnostic.severity);

    // Header: severity[CODE] path:line:col
    if use_color {
        let sc = severity_color(diagnostic.severity);
        output.push_str(&format!(
            "{sc}{sev}[{}]{RESET}: {WHITE}{}{RESET}\n",
            diagnostic.code, diagnostic.message
        ));
        output.push_str(&format!(
            "  {DARK_GRAY}-->  {}:{}:{}{RESET}\n",
            path, start.line, start.column
        ));
    } else {
        output.push_str(&format!(
            "{sev}[{}]: {}\n",
            diagnostic.code, diagnostic.message
        ));
        output.push_str(&format!(
            "  -->  {}:{}:{}\n",
            path, start.line, start.column
        ));
    }

    // Source frame with context
    if let Some(source) = source {
        output.push_str(&render_source_frame(
            source,
            &diagnostic.span,
            None,
            use_color,
            diagnostic.severity,
        ));
    }

    // Labels (secondary spans)
    for label in &diagnostic.labels {
        let pos = &label.span.start;
        if use_color {
            output.push_str(&format!(
                "{CYAN}note{RESET}: {WHITE}{}{RESET}\n",
                label.message,
            ));
            output.push_str(&format!(
                "  {DARK_GRAY}-->  {}:{}:{}{RESET}\n",
                path, pos.line, pos.column
            ));
        } else {
            output.push_str(&format!("note: {}\n", label.message));
            output.push_str(&format!("  -->  {}:{}:{}\n", path, pos.line, pos.column));
        }
        if let Some(source) = source {
            output.push_str(&render_source_frame(
                source,
                &label.span,
                None,
                use_color,
                diagnostic.severity,
            ));
        }
    }

    // Suggestion (machine-applicable fix)
    if let Some(ref suggestion) = diagnostic.suggestion {
        if use_color {
            output.push_str(&format!(
                "{GREEN}help{RESET}: {WHITE}{}{RESET}\n",
                suggestion.message,
            ));
        } else {
            output.push_str(&format!("help: {}\n", suggestion.message));
        }
        if let Some(source) = source {
            output.push_str(&render_suggestion_frame(source, suggestion, use_color));
        }
    }

    // Hints (free-form help text)
    for hint in &diagnostic.hints {
        if use_color {
            output.push_str(&format!("{GREEN}help{RESET}: {WHITE}{hint}{RESET}\n"));
        } else {
            output.push_str(&format!("help: {hint}\n"));
        }
    }

    output.trim_end().to_string()
}

/// Render a source frame with 1 line of context before, the span lines, and carets.
/// Supports both single-line and multiline spans.
fn render_source_frame(
    source: &str,
    span: &Span,
    _message: Option<&str>,
    use_color: bool,
    severity: DiagnosticSeverity,
) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let start_line = span.start.line;
    let end_line = span.end.line;

    // Determine the range of lines to show (1 context line before, span lines)
    let first_display = start_line.saturating_sub(1).max(1);
    let last_display = end_line.min(lines.len());

    // Width for line-number gutter
    let width = last_display.to_string().len().max(2);
    let mut output = String::new();

    let gutter_blank = |out: &mut String| {
        if use_color {
            out.push_str(&format!("{DARK_GRAY}{:>width$} │{RESET}\n", ""));
        } else {
            out.push_str(&format!("{:>width$} │\n", ""));
        }
    };

    let gutter_line = |out: &mut String, line_no: usize, content: &str, _is_span: bool| {
        if use_color {
            out.push_str(&format!(
                "{DARK_GRAY}{line_no:>width$} │{RESET} {content}\n"
            ));
        } else {
            out.push_str(&format!("{line_no:>width$} │ {content}\n"));
        }
    };

    gutter_blank(&mut output);

    // Display lines
    for line_no in first_display..=last_display {
        let idx = line_no.saturating_sub(1);
        if idx >= lines.len() {
            break;
        }
        let content = lines[idx];
        let is_span_line = line_no >= start_line && line_no <= end_line;
        gutter_line(&mut output, line_no, content, is_span_line);

        // Draw carets for span lines
        if is_span_line {
            let line_chars: Vec<char> = content.chars().collect();
            let line_len = line_chars.len();

            let (caret_start, caret_end) = if start_line == end_line {
                // Single-line span
                let mut s = span.start.column.max(1);
                if s > line_len + 1 {
                    s = line_len + 1;
                }
                let mut e = span.end.column;
                if e < s {
                    e = s;
                }
                if e > line_len {
                    e = line_len.max(s);
                }
                (s, e)
            } else if line_no == start_line {
                let s = span.start.column.max(1).min(line_len + 1);
                (s, line_len.max(s))
            } else if line_no == end_line {
                let e = span.end.column.min(line_len).max(1);
                (1, e)
            } else {
                // Middle line of multiline span: underline entire line
                if line_len > 0 {
                    (1, line_len)
                } else {
                    continue;
                }
            };

            let caret_len = caret_end.saturating_sub(caret_start).saturating_add(1);
            let padding = " ".repeat(caret_start.saturating_sub(1));
            let carets = "^".repeat(caret_len);

            if use_color {
                let cc = caret_color(severity);
                output.push_str(&format!(
                    "{DARK_GRAY}{:>width$} │{RESET} {padding}{cc}{carets}{RESET}\n",
                    ""
                ));
            } else {
                output.push_str(&format!("{:>width$} │ {padding}{carets}\n", ""));
            }
        }
    }

    output
}

/// Render a suggestion as a diff-like "try this" source frame.
fn render_suggestion_frame(source: &str, suggestion: &Suggestion, use_color: bool) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let line_no = suggestion.span.start.line;
    let idx = line_no.saturating_sub(1);
    if idx >= lines.len() {
        return String::new();
    }

    let original = lines[idx];
    let col_start = suggestion.span.start.column.saturating_sub(1);
    let col_end = if suggestion.span.start.line == suggestion.span.end.line {
        suggestion.span.end.column.min(original.len())
    } else {
        original.len()
    };

    // Build the replacement line
    let before = &original[..col_start.min(original.len())];
    let after = if col_end <= original.len() {
        &original[col_end..]
    } else {
        ""
    };
    let replaced = format!("{before}{}{after}", suggestion.replacement);

    let width = line_no.to_string().len().max(2);
    let mut output = String::new();

    if use_color {
        output.push_str(&format!("{DARK_GRAY}{:>width$} │{RESET}\n", ""));
        output.push_str(&format!(
            "{DARK_GRAY}{line_no:>width$} │{RESET} {GREEN}{replaced}{RESET}\n"
        ));
        // Underline the replacement part
        let tilde_start = col_start;
        let tilde_len = suggestion.replacement.len().max(1);
        let padding = " ".repeat(tilde_start);
        let tildes = "~".repeat(tilde_len);
        output.push_str(&format!(
            "{DARK_GRAY}{:>width$} │{RESET} {padding}{GREEN}{tildes}{RESET}\n",
            ""
        ));
    } else {
        output.push_str(&format!("{:>width$} │\n", ""));
        output.push_str(&format!("{line_no:>width$} │ {replaced}\n"));
        let tilde_start = col_start;
        let tilde_len = suggestion.replacement.len().max(1);
        let padding = " ".repeat(tilde_start);
        let tildes = "~".repeat(tilde_len);
        output.push_str(&format!("{:>width$} │ {padding}{tildes}\n", ""));
    }

    output
}
