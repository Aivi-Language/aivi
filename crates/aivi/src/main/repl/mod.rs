mod engine;
mod tui;

use aivi::AiviError;
use std::io::IsTerminal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColorMode {
    Auto,
    Always,
    Never,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SymbolPane {
    Types,
    Values,
    Functions,
    Modules,
}

#[derive(Debug, Clone)]
pub(crate) struct ReplOptions {
    pub color_mode: ColorMode,
    pub plain_mode: bool,
}

impl ReplOptions {
    #[allow(dead_code)]
    fn color_enabled(&self) -> bool {
        match self.color_mode {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => {
                std::env::var_os("NO_COLOR").is_none()
                    && std::env::var("TERM").as_deref() != Ok("dumb")
                    && std::io::stdout().is_terminal()
            }
        }
    }
}

pub(crate) fn print_repl_help() {
    println!(
        "aivi repl — interactive REPL for the AIVI language\n\
        \n\
        USAGE:\n\
        \x20 aivi repl [OPTIONS]\n\
        \n\
        OPTIONS:\n\
        \x20 --color        Force ANSI color output\n\
        \x20 --no-color     Disable ANSI color output\n\
        \x20 --plain        Plain read-eval-print mode (no TUI, pipe-friendly)\n\
        \x20 -h, --help     Print this help\n\
        \n\
        SLASH COMMANDS (inside the REPL):\n\
        \x20 /help                              Print command reference\n\
        \x20 /use <module.path>                 Add import to session (errors on unknown modules)\n\
        \x20 /types [filter]                    Types in scope\n\
        \x20 /values [filter]                   Session-defined values with types\n\
        \x20 /functions [filter]                Functions in scope with module names\n\
        \x20 /autorun [on|off]                  Toggle top-level effect autorun (default: on)\n\
        \x20 /modules                           Show loaded modules\n\
        \x20 /clear                             Clear transcript (keep session state)\n\
        \x20 /reset                             Clear transcript + session state\n\
        \x20 /history [n]                       Show last n inputs (default: 20)\n\
        \x20 /load <path>                       Load .aivi file into session\n\
        \x20 /openapi file <path> [as <name>]   Inject OpenAPI spec file as module\n\
        \x20 /openapi url <url> [as <name>]     Inject OpenAPI spec URL as module\n\
        \n\
        Top-level effect expressions autorun by default so `print` / `println` show output.\n\
        Use `/autorun off` if you want effect values to stay inert.\n\
        \n\
        KEYBOARD SHORTCUTS:\n\
        \x20 Enter          Submit input / accept suggestion\n\
        \x20 Shift+Enter    Insert newline (multi-line input)\n\
        \x20 ↑ / ↓          Navigate history or suggestions\n\
        \x20 Ctrl+L         Clear transcript\n\
        \x20 Ctrl+C         Cancel current input\n\
        \x20 Ctrl+D         Exit (on empty input)\n\
        \x20 Tab            Accept suggestion or toggle symbol pane\n\
        \x20 Esc            Close symbol pane"
    );
}

pub(crate) fn cmd_repl(args: &[String]) -> Result<(), AiviError> {
    let mut color_mode = ColorMode::Auto;
    let mut plain_mode = false;

    for arg in args {
        match arg.as_str() {
            "--color" => color_mode = ColorMode::Always,
            "--no-color" => color_mode = ColorMode::Never,
            "--plain" => plain_mode = true,
            "-h" | "--help" => {
                print_repl_help();
                return Ok(());
            }
            other if other.starts_with('-') => {
                return Err(AiviError::InvalidCommand(format!(
                    "unknown repl flag: {other}\nRun `aivi repl --help` for usage."
                )));
            }
            other => {
                return Err(AiviError::InvalidCommand(format!(
                    "unexpected repl argument: {other}\nRun `aivi repl --help` for usage."
                )));
            }
        }
    }

    // Non-TTY stdin always forces plain mode.
    if !std::io::stdin().is_terminal() {
        plain_mode = true;
    }

    let options = ReplOptions {
        color_mode,
        plain_mode,
    };

    let mut engine = engine::ReplEngine::new(&options)?;

    if options.plain_mode {
        engine.run_plain()
    } else {
        tui::run(engine, &options)
    }
}
