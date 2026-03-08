use aivi::AiviError;
use super::ReplOptions;

/// Core REPL evaluation engine.
///
/// Owns the session state (accumulated definitions, imports, history) and
/// drives the parse → typecheck → eval pipeline. Both the plain-text loop
/// (`run_plain`) and the TUI frontend (`tui::run`) operate on this engine.
pub(crate) struct ReplEngine {
    #[allow(dead_code)]
    options: ReplOptions,
}

impl ReplEngine {
    pub(crate) fn new(options: &ReplOptions) -> Result<Self, AiviError> {
        Ok(Self {
            options: options.clone(),
        })
    }

    /// Plain read-eval-print loop. Reads from stdin line by line; pipe-friendly.
    pub(crate) fn run_plain(&mut self) -> Result<(), AiviError> {
        // TODO(repl-engine): implement plain eval loop
        Err(AiviError::InvalidCommand(
            "repl engine not yet implemented".to_string(),
        ))
    }
}
