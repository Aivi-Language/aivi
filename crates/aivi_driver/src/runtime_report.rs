use std::fmt;

use aivi_core::{
    embedded_stdlib_source, render_diagnostic_with_source, Diagnostic, DiagnosticLabel,
    DiagnosticSeverity, SourceKind, SourceOrigin,
};

const RED: &str = "\x1b[1;31m";
const CYAN: &str = "\x1b[1;36m";
const GREEN: &str = "\x1b[1;32m";
const DARK_GRAY: &str = "\x1b[90m";
const WHITE: &str = "\x1b[97m";
const RESET: &str = "\x1b[0m";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeNoteKind {
    Note,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeNote {
    pub kind: RuntimeNoteKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeFrameKind {
    Function,
    Builtin,
    Effect,
    Reactive,
    Native,
    Context,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFrame {
    pub kind: RuntimeFrameKind,
    pub name: String,
    pub origin: Option<SourceOrigin>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLabel {
    pub message: String,
    pub origin: SourceOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeReport {
    pub code: String,
    pub message: String,
    pub primary: Option<SourceOrigin>,
    pub labels: Vec<RuntimeLabel>,
    pub notes: Vec<RuntimeNote>,
    pub frames: Vec<RuntimeFrame>,
}

impl RuntimeReport {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            primary: None,
            labels: Vec::new(),
            notes: Vec::new(),
            frames: Vec::new(),
        }
    }

    pub fn message(message: impl Into<String>) -> Self {
        Self::new("RT0000", message)
    }

    pub fn with_primary(mut self, origin: SourceOrigin) -> Self {
        self.primary = Some(origin);
        self
    }

    pub fn with_label(mut self, label: RuntimeLabel) -> Self {
        self.labels.push(label);
        self
    }

    pub fn with_note(mut self, message: impl Into<String>) -> Self {
        self.notes.push(RuntimeNote {
            kind: RuntimeNoteKind::Note,
            message: message.into(),
        });
        self
    }

    pub fn with_hint(mut self, message: impl Into<String>) -> Self {
        self.notes.push(RuntimeNote {
            kind: RuntimeNoteKind::Help,
            message: message.into(),
        });
        self
    }

    pub fn with_frame(mut self, frame: RuntimeFrame) -> Self {
        self.frames.push(frame);
        self
    }
}

impl fmt::Display for RuntimeReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", render_runtime_report(self, false))
    }
}

fn load_source(origin: &SourceOrigin) -> Option<String> {
    match origin.source_kind {
        SourceKind::EmbeddedStdlib => origin
            .embedded_module_name()
            .and_then(embedded_stdlib_source)
            .map(ToOwned::to_owned),
        SourceKind::User => std::fs::read_to_string(&origin.path).ok(),
        SourceKind::Generated | SourceKind::Synthetic => None,
    }
}

fn render_runtime_note(note: &RuntimeNote, use_color: bool) -> String {
    match note.kind {
        RuntimeNoteKind::Note => {
            if use_color {
                format!("{CYAN}note{RESET}: {WHITE}{}{RESET}", note.message)
            } else {
                format!("note: {}", note.message)
            }
        }
        RuntimeNoteKind::Help => {
            if use_color {
                format!("{GREEN}help{RESET}: {WHITE}{}{RESET}", note.message)
            } else {
                format!("help: {}", note.message)
            }
        }
    }
}

fn render_cross_file_label(label: &RuntimeLabel, use_color: bool) -> String {
    if use_color {
        format!(
            "{CYAN}note{RESET}: {WHITE}{}{RESET}\n  {DARK_GRAY}-->  {}{RESET}",
            label.message,
            label.origin.start_position_text()
        )
    } else {
        format!(
            "note: {}\n  -->  {}",
            label.message,
            label.origin.start_position_text()
        )
    }
}

fn frame_name(frame: &RuntimeFrame) -> String {
    match frame.kind {
        RuntimeFrameKind::Function => frame.name.clone(),
        RuntimeFrameKind::Builtin => format!("builtin {}", frame.name),
        RuntimeFrameKind::Effect => format!("effect {}", frame.name),
        RuntimeFrameKind::Reactive => format!("reactive {}", frame.name),
        RuntimeFrameKind::Native => format!("native {}", frame.name),
        RuntimeFrameKind::Context => frame.name.clone(),
    }
}

pub fn render_runtime_report(report: &RuntimeReport, use_color: bool) -> String {
    let mut output = String::new();

    if let Some(primary) = &report.primary {
        let mut diag = Diagnostic::new(
            report.code.clone(),
            DiagnosticSeverity::Error,
            report.message.clone(),
            primary.span.clone(),
            report
                .labels
                .iter()
                .filter(|label| label.origin.path == primary.path)
                .map(|label| DiagnosticLabel {
                    message: label.message.clone(),
                    span: label.origin.span.clone(),
                })
                .collect(),
        );
        for note in report
            .notes
            .iter()
            .filter(|note| note.kind == RuntimeNoteKind::Help)
        {
            diag = diag.with_hint(note.message.clone());
        }
        let source = load_source(primary);
        output.push_str(&render_diagnostic_with_source(
            &primary.path,
            &diag,
            source.as_deref(),
            use_color,
        ));
    } else if use_color {
        output.push_str(&format!(
            "{RED}error[{}]{RESET}: {WHITE}{}{RESET}",
            report.code, report.message
        ));
    } else {
        output.push_str(&format!("error[{}]: {}", report.code, report.message));
    }

    let cross_file_labels: Vec<&RuntimeLabel> = match &report.primary {
        Some(primary) => report
            .labels
            .iter()
            .filter(|label| label.origin.path != primary.path)
            .collect(),
        None => report.labels.iter().collect(),
    };
    for label in cross_file_labels {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&render_cross_file_label(label, use_color));
    }

    for note in report.notes.iter().filter(|note| {
        note.kind == RuntimeNoteKind::Note
            || (report.primary.is_none() && note.kind == RuntimeNoteKind::Help)
    }) {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&render_runtime_note(note, use_color));
    }

    if !report.frames.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push('\n');
        if use_color {
            output.push_str(&format!("{CYAN}stack{RESET}:\n"));
        } else {
            output.push_str("stack:\n");
        }
        for (index, frame) in report.frames.iter().enumerate() {
            let location = frame
                .origin
                .as_ref()
                .map(|origin| origin.start_position_text())
                .unwrap_or_else(|| "<unknown location>".to_string());
            if use_color {
                output.push_str(&format!(
                    "  {DARK_GRAY}{index}:{RESET} {WHITE}{}{RESET} {DARK_GRAY}at {}{RESET}\n",
                    frame_name(frame),
                    location,
                ));
            } else {
                output.push_str(&format!(
                    "  {index}: {} at {}\n",
                    frame_name(frame),
                    location
                ));
            }
        }
        output.truncate(output.trim_end().len());
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_notes_render_without_primary_origin() {
        let rendered = render_runtime_report(
            &RuntimeReport::new("RT9999", "boom").with_hint("try again"),
            false,
        );

        assert!(rendered.contains("error[RT9999]: boom"));
        assert!(rendered.contains("help: try again"));
    }
}
