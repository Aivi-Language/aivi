//! REPL session engine: state management, input classification, evaluation, and snapshots.
//!
//! This module is the non-UI foundation of `aivi repl`. The TUI layer (tui.rs) consumes
//! `ReplEngine` via `snapshot()` and `submit()`. A plain-text fallback is available via
//! `run_plain()`.

use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::Path;

use aivi::{
    check_modules, check_types, desugar_modules, elaborate_expected_coercions,
    embedded_stdlib_modules, file_diagnostics_have_errors, infer_value_types_full, parse_modules,
    render_diagnostics, resolve_import_names, AiviError, ClassDecl, DomainDecl, DomainItem,
    FileDiagnostic, Module, ModuleItem, RecordTypeField, ReplJitSession, TypeAlias, TypeDecl,
    TypeExpr, TypeSig,
};

use super::doc_index::{DocIndex, QuickInfoEntry, DOC_INDEX_JSON};
use super::{ColorMode, ReplOptions, SymbolPane};

// ── Transcript types ──────────────────────────────────────────────────────────

/// The semantic role of a transcript line, used by the TUI for styled rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TranscriptKind {
    /// User input prompt line.
    Input,
    /// Successful expression result: `text` is `"value :: Type"`.
    ValueResult,
    /// Successful definition: `text` is `"name :: Type  (defined)"`.
    Defined,
    /// Compiler / type / runtime error text.
    Error,
    /// Compiler warning text.
    #[allow(dead_code)]
    Warning,
    /// System / status message (`✓ …`).
    System,
    /// Slash-command output (pre-formatted text).
    CommandOutput,
    /// Inline type annotation shown below an expression.
    #[allow(dead_code)]
    TypeAnnotation,
}

#[derive(Debug, Clone)]
pub(crate) struct TranscriptEntry {
    pub(crate) kind: TranscriptKind,
    /// Display text. In TUI mode the TUI applies its own styles; no ANSI codes embedded here.
    pub(crate) text: String,
}

// ── Symbol inventory types ────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SymbolKind {
    Type,
    Function,
    Value,
    Module,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CompletionKind {
    Command,
    Constructor,
    Function,
    Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompletionMode {
    Command,
    SlashFilter,
    Symbol,
}

#[derive(Debug, Clone)]
pub(crate) struct SymbolEntry {
    pub(crate) kind: SymbolKind,
    pub(crate) name: String,
}

#[derive(Debug, Clone)]
pub(crate) struct CompletionItem {
    pub(crate) kind: CompletionKind,
    pub(crate) label: String,
    pub(crate) insert_text: String,
    pub(crate) detail: String,
}

#[derive(Debug, Clone)]
pub(crate) struct CompletionState {
    pub(crate) mode: CompletionMode,
    pub(crate) replace_start: usize,
    pub(crate) replace_end: usize,
    pub(crate) items: Vec<CompletionItem>,
}

#[derive(Debug, Clone)]
struct SymbolCompletion {
    kind: CompletionKind,
    name: String,
    detail: String,
    module: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExplainKind {
    Module,
    Type,
    Class,
    Domain,
    Function,
    Value,
    Constructor,
}

#[derive(Debug, Clone)]
struct ExplainSubject {
    kind: ExplainKind,
    name: String,
    module: Option<String>,
    signature: Option<String>,
    quick_info: Option<QuickInfoEntry>,
}

// ── Session snapshot (TUI-facing read API) ─────────────────────────────────────

#[derive(Debug, Clone)]
pub(crate) struct ReplSnapshot {
    /// Full accumulated transcript (all turns).
    pub(crate) transcript: Vec<TranscriptEntry>,
    /// Which symbol pane is currently open (None = pane hidden).
    pub(crate) symbol_pane: Option<SymbolPane>,
    /// Entries to render inside the symbol pane.
    pub(crate) symbols: Vec<SymbolEntry>,
    /// Current turn number (1-based; 0 before any eval).
    pub(crate) turn: usize,
}

// ── Core session state ────────────────────────────────────────────────────────

pub(crate) struct ReplEngine {
    options: ReplOptions,
    doc_index: DocIndex,
    /// Accumulated `use` module paths (e.g. `"aivi.text"`).
    imports: Vec<String>,
    /// Accumulated user definition source snippets (verbatim lines).
    definitions: Vec<String>,
    /// Navigation history (submitted non-command inputs).
    history: Vec<String>,
    /// Full transcript for display.
    transcript: Vec<TranscriptEntry>,
    /// Current symbol pane (None = hidden).
    active_pane: Option<SymbolPane>,
    /// Cached symbol entries for the current pane.
    cached_symbols: Vec<SymbolEntry>,
    /// Evaluation turn counter.
    turn: usize,
    /// Stdlib modules that are visible by default through the prelude bootstrap.
    default_visible_modules: Vec<String>,
    /// Builtin values/constructors that are available from the core scope.
    builtin_completions: Vec<SymbolCompletion>,
    /// Inferred type strings for session-defined names. Updated after every successful compile.
    session_types: HashMap<String, String>,
    /// Persisted source-signal values that should survive across REPL turns.
    jit_session: ReplJitSession,
    /// Static inventory of stdlib symbols grouped by module for scope-aware completions.
    stdlib_module_completions: HashMap<String, Vec<SymbolCompletion>>,
    /// Session-local ADT constructors derived from successful definitions and loads.
    session_constructors: Vec<SymbolCompletion>,
    /// Whether top-level effect expressions should be executed automatically.
    autorun_effects: bool,
}

impl ReplEngine {
    /// Create a new engine, pre-loading the stdlib symbol inventory.
    pub(crate) fn new(options: &ReplOptions) -> Result<Self, AiviError> {
        let stdlib = embedded_stdlib_modules();
        let doc_index = DocIndex::from_json(DOC_INDEX_JSON).map_err(|err| {
            AiviError::InvalidCommand(format!("internal REPL doc index error: {err}"))
        })?;
        let default_visible_modules = collect_default_visible_modules(&stdlib);
        let visible_stdlib_count: usize = stdlib
            .iter()
            .filter(|module| default_visible_modules.contains(&module.name.name))
            .map(count_exportable_items)
            .sum();
        let mut engine = ReplEngine {
            options: options.clone(),
            doc_index,
            imports: Vec::new(),
            definitions: Vec::new(),
            history: Vec::new(),
            transcript: Vec::new(),
            active_pane: None,
            cached_symbols: Vec::new(),
            turn: 0,
            default_visible_modules,
            builtin_completions: builtin_symbol_completions(),
            session_types: HashMap::new(),
            jit_session: ReplJitSession::new(),
            stdlib_module_completions: collect_module_completion_symbols(&stdlib, false),
            session_constructors: Vec::new(),
            autorun_effects: true,
        };

        engine.transcript.push(TranscriptEntry {
            kind: TranscriptKind::System,
            text: format!(
                "aivi repl 0.1  ·  prelude loaded  ·  {} symbols in scope",
                visible_stdlib_count
            ),
        });

        Ok(engine)
    }

    // ── Public pane controls ────────────────────────────────────────────────

    pub(crate) fn toggle_symbol_pane(&mut self) {
        if self.active_pane.is_some() {
            self.active_pane = None;
            self.cached_symbols.clear();
        } else {
            self.active_pane = Some(SymbolPane::Types);
            self.cached_symbols = self.build_symbol_entries(SymbolPane::Types, "");
        }
    }

    pub(crate) fn close_symbol_pane(&mut self) {
        self.active_pane = None;
        self.cached_symbols.clear();
    }

    /// Clear the visible transcript while preserving all session state (definitions, imports).
    /// Called by Ctrl+L in the TUI.
    pub(crate) fn clear_transcript(&mut self) {
        self.transcript.clear();
        self.transcript.push(TranscriptEntry {
            kind: TranscriptKind::System,
            text: "transcript cleared".to_owned(),
        });
    }

    pub(crate) fn set_symbol_pane(&mut self, pane: SymbolPane) {
        self.active_pane = Some(pane);
        self.cached_symbols = self.build_symbol_entries(pane, "");
    }

    // ── Snapshot API ────────────────────────────────────────────────────────

    pub(crate) fn snapshot(&self) -> ReplSnapshot {
        ReplSnapshot {
            transcript: self.transcript.clone(),
            symbol_pane: self.active_pane,
            symbols: self.cached_symbols.clone(),
            turn: self.turn,
        }
    }

    pub(crate) fn completion_state(&self, input: &str, cursor: usize) -> Option<CompletionState> {
        let cursor = cursor.min(input.len());
        if let Some(state) = self.command_completion_state(input, cursor) {
            return Some(state);
        }
        if let Some(state) = self.functions_filter_completion_state(input, cursor) {
            return Some(state);
        }
        self.symbol_completion_state(input, cursor)
    }

    // ── Plain (non-TUI) run loop ─────────────────────────────────────────────

    /// Blocking read-eval-print loop for non-TUI contexts (pipe-friendly).
    /// Reads from stdin line by line; prints results to stdout.
    pub(crate) fn run_plain(&mut self) -> Result<(), AiviError> {
        let use_color = self.use_color();

        // Print the startup header.
        for entry in &self.transcript {
            println!("{}", plain_entry_text(entry));
        }

        let prompt = if use_color { "❯ " } else { "> " };

        let stdin = io::stdin();
        loop {
            print!("{prompt}");
            io::stdout().flush().map_err(AiviError::Io)?;

            let mut line = String::new();
            match stdin.lock().read_line(&mut line) {
                Ok(0) => break, // Ctrl+D / EOF
                Ok(_) => {}
                Err(e) => return Err(AiviError::Io(e)),
            }

            let input = line.trim_end_matches('\n').trim_end_matches('\r').trim();
            if input.is_empty() {
                continue;
            }
            if matches!(input, ":q" | "/quit" | "/exit") {
                break;
            }

            let snapshot_before = self.transcript.len();
            let snap = self.submit(input)?;

            // Print only the new entries added in this turn.
            for entry in &snap.transcript[snapshot_before..] {
                let text = plain_entry_text(entry);
                if !text.is_empty() {
                    println!("{text}");
                }
            }
        }

        Ok(())
    }

    // ── Submit (core evaluation entry point) ─────────────────────────────────

    /// Process one line (or multiline block) of user input.
    /// Returns the full accumulated snapshot after processing.
    pub(crate) fn submit(&mut self, input: &str) -> Result<ReplSnapshot, AiviError> {
        let input = input.trim();
        if input.is_empty() {
            return Ok(self.snapshot());
        }

        self.transcript.push(TranscriptEntry {
            kind: TranscriptKind::Input,
            text: input.to_owned(),
        });

        if input.starts_with('/') {
            self.dispatch_slash(input);
        } else {
            self.history.push(input.to_owned());
            self.turn += 1;
            self.eval_input(input);
        }

        Ok(self.snapshot())
    }

    // ── Slash command dispatcher ─────────────────────────────────────────────

    fn dispatch_slash(&mut self, input: &str) {
        // Split into: cmd, first_arg (optional), rest (optional)
        let mut parts = input.splitn(3, ' ');
        let cmd = parts.next().unwrap_or("");
        let arg1 = parts.next().unwrap_or("").trim();
        let rest = parts.next().unwrap_or("").trim();

        match cmd {
            "/help" => self.cmd_help(),
            "/explain" => self.cmd_explain(arg1),
            "/clear" => self.cmd_clear(),
            "/reset" => self.cmd_reset(),
            "/types" => self.cmd_types(arg1),
            "/values" => self.cmd_values(arg1),
            "/functions" => self.cmd_functions(arg1),
            "/modules" => self.cmd_modules(),
            "/history" => self.cmd_history(arg1),
            "/use" => self.cmd_use(arg1),
            "/load" => self.cmd_load(arg1),
            "/openapi" => self.cmd_openapi(arg1, rest),
            "/autorun" => self.cmd_autorun(arg1),
            _ => {
                let msg = match closest_command(cmd) {
                    Some(sug) => format!(
                        "Unknown command `{cmd}`. Did you mean `{sug}`? Type /help for all commands."
                    ),
                    None => format!("Unknown command `{cmd}`. Type /help for all commands."),
                };
                self.push_error(msg);
            }
        }
    }

    fn cmd_help(&mut self) {
        let text = "\
Command Reference

  /help                         print this reference
  /explain <name>               show quick info, signature, and module
  /use <module.path>            add import to session (errors on unknown modules)
  /types [filter]               types in scope (stdlib + session)
  /values [filter]              session-defined values + inferred types
  /functions [filter]           functions in scope with module names
  /modules                      show loaded modules in session
  /clear                        clear transcript (keep session state)
  /reset                        clear transcript + session state
  /history [n]                  show last n inputs (default: 20)
  /load <path>                  load .aivi file into session
  /openapi file <path> [as <n>] inject OpenAPI spec file as module
  /openapi url <url> [as <n>]   inject OpenAPI spec URL as module
  /autorun [on|off]             toggle top-level effect autorun (default: on)

  Ctrl+D on empty input: exit   Tab: accept suggestion";
        self.push_command_output(text.to_owned());
        self.set_symbol_pane(SymbolPane::Types);
    }

    fn cmd_explain(&mut self, query: &str) {
        let query = query.trim();
        if query.is_empty() {
            self.push_error(
                "/explain expects a type, function, value, or module name. Example: /explain isAlnum"
                    .to_owned(),
            );
            return;
        }

        let matches = self.resolve_explain_subjects(query);
        match matches.as_slice() {
            [] => self.push_error(format!(
                "Nothing matched `{query}`. Try `/types`, `/functions`, or a qualified name like `aivi.text.isAlnum`."
            )),
            [subject] => self.push_command_output(render_explain_subject(subject)),
            many => self.push_error(render_explain_ambiguity(query, many)),
        }
    }

    fn cmd_clear(&mut self) {
        self.transcript.clear();
        self.transcript.push(TranscriptEntry {
            kind: TranscriptKind::System,
            text: "transcript cleared".to_owned(),
        });
    }

    fn cmd_reset(&mut self) {
        self.imports.clear();
        self.definitions.clear();
        self.history.clear();
        self.session_types.clear();
        self.jit_session.reset();
        self.session_constructors.clear();
        self.autorun_effects = true;
        self.turn = 0;
        self.cached_symbols.clear();
        self.active_pane = None;
        self.transcript.clear();
        self.transcript.push(TranscriptEntry {
            kind: TranscriptKind::System,
            text: "session reset".to_owned(),
        });
    }

    fn cmd_autorun(&mut self, arg: &str) {
        match arg {
            "" => self.push_command_output(format!(
                "autorun is {}",
                if self.autorun_effects { "on" } else { "off" }
            )),
            "on" => {
                self.autorun_effects = true;
                self.push_command_output("autorun enabled for top-level effects".to_owned());
            }
            "off" => {
                self.autorun_effects = false;
                self.push_command_output("autorun disabled for top-level effects".to_owned());
            }
            other => self.push_error(format!(
                "Invalid /autorun mode `{other}`. Use `/autorun on` or `/autorun off`."
            )),
        }
    }

    fn cmd_types(&mut self, filter: &str) {
        self.set_symbol_pane(SymbolPane::Types);
        let entries = self.build_symbol_entries(SymbolPane::Types, filter);
        self.cached_symbols = self.build_symbol_entries(SymbolPane::Types, "");

        let header = if filter.is_empty() {
            format!("{} types in scope", entries.len())
        } else {
            format!("{} types in scope (filter: {filter})", entries.len())
        };
        let mut lines = vec![header];
        for e in &entries {
            lines.push(format!("  [type] {}", e.name));
        }
        self.push_command_output(lines.join("\n"));
    }

    fn cmd_values(&mut self, filter: &str) {
        self.set_symbol_pane(SymbolPane::Values);
        let entries = self.build_symbol_entries(SymbolPane::Values, filter);
        self.cached_symbols = self.build_symbol_entries(SymbolPane::Values, "");

        if self.definitions.is_empty() {
            self.push_command_output("No values defined yet. Try: x = 42".to_owned());
        } else {
            let header = if filter.is_empty() {
                format!(
                    "{} session value{}",
                    entries.len(),
                    if entries.len() == 1 { "" } else { "s" }
                )
            } else {
                format!(
                    "{} session value{} (filter: {filter})",
                    entries.len(),
                    if entries.len() == 1 { "" } else { "s" }
                )
            };
            let mut lines = vec![header];
            for e in &entries {
                lines.push(format!("  [val]  {}", e.name));
            }
            self.push_command_output(lines.join("\n"));
        }
    }

    fn cmd_functions(&mut self, filter: &str) {
        self.set_symbol_pane(SymbolPane::Functions);
        let entries = self.build_symbol_entries(SymbolPane::Functions, filter);
        self.cached_symbols = self.build_symbol_entries(SymbolPane::Functions, "");

        if entries.is_empty() {
            self.push_command_output("No functions in scope.".to_owned());
        } else {
            let header = if filter.is_empty() {
                format!(
                    "{} function{}",
                    entries.len(),
                    if entries.len() == 1 { "" } else { "s" }
                )
            } else {
                format!(
                    "{} function{} (filter: {filter})",
                    entries.len(),
                    if entries.len() == 1 { "" } else { "s" }
                )
            };
            let mut lines = vec![header];
            for e in &entries {
                lines.push(format!("  [fn]   {}", e.name));
            }
            self.push_command_output(lines.join("\n"));
        }
    }

    fn cmd_modules(&mut self) {
        let mut lines = vec!["Loaded modules".to_owned()];
        lines.push("  aivi.prelude  (always)".to_owned());
        for imp in &self.imports {
            lines.push(format!("  {imp}"));
        }
        self.push_command_output(lines.join("\n"));
        self.set_symbol_pane(SymbolPane::Types);
    }

    fn cmd_history(&mut self, arg: &str) {
        let n: usize = arg.parse().unwrap_or(20);
        let total = self.history.len();
        let start = total.saturating_sub(n);
        let header = format!(
            "Last {} input{}",
            total.min(n),
            if total.min(n) == 1 { "" } else { "s" }
        );
        let mut lines = vec![header];
        for (i, h) in self.history[start..].iter().enumerate() {
            lines.push(format!("  {:>3}  {h}", start + i + 1));
        }
        self.push_command_output(lines.join("\n"));
    }

    fn cmd_use(&mut self, module_path: &str) {
        if module_path.is_empty() {
            self.push_error("/use expects a module path. Example: /use aivi.text".to_owned());
            return;
        }
        if !self.module_exists(module_path) {
            self.push_error(format!(
                "Unknown module `{module_path}`. Example: /use aivi.text"
            ));
            return;
        }
        if !self.imports.iter().any(|import| import == module_path) {
            self.imports.push(module_path.to_owned());
        }
        self.transcript.push(TranscriptEntry {
            kind: TranscriptKind::System,
            text: format!("added import: {module_path}"),
        });
    }

    fn cmd_load(&mut self, path_str: &str) {
        if path_str.is_empty() {
            self.push_error(
                "/load expects a file path. Example: /load ./my_module.aivi".to_owned(),
            );
            return;
        }
        let path = Path::new(path_str);
        match std::fs::read_to_string(path) {
            Err(e) => {
                self.push_error(format!("Could not read `{path_str}`: {e}"));
            }
            Ok(content) => {
                let display_path = path.display().to_string();
                match compile_repl_modules(path, &content) {
                    Ok(_) => {}
                    Err(file_diags) => {
                        self.push_diagnostics_as_error(&display_path, &file_diags);
                        return;
                    }
                }
                self.definitions.push(content.clone());
                self.session_types = self.current_session_types().into_iter().collect();
                self.session_constructors = self.current_session_constructors();
                self.transcript.push(TranscriptEntry {
                    kind: TranscriptKind::System,
                    text: format!("loaded `{path_str}`"),
                });
            }
        }
    }

    fn cmd_openapi(&mut self, kind: &str, rest: &str) {
        match kind {
            "file" | "url" => {
                let source_label = if kind == "file" { "file" } else { "URL" };
                let (source, alias) = parse_openapi_args(rest);
                if source.is_empty() {
                    self.push_error(format!(
                        "/openapi {kind} expects a {source_label}. Example: /openapi {kind} spec.yaml [as petstore]"
                    ));
                    return;
                }

                if kind == "file" && !Path::new(source).exists() {
                    self.push_error(format!(
                        "OpenAPI file not found: `{source}`\n\
                         Check the path and try again."
                    ));
                    return;
                }

                if kind == "url"
                    && !source.starts_with("http://")
                    && !source.starts_with("https://")
                {
                    self.push_error(format!(
                        "Invalid URL `{source}`: must start with http:// or https://"
                    ));
                    return;
                }

                let binding_name = alias.unwrap_or_else(|| derive_module_name(source));
                let loader_call = if kind == "file" {
                    format!("openapi.fromFile \"{source}\"")
                } else {
                    format!("openapi.fromUrl \"{source}\"")
                };
                let snippet = format!("@static\n{binding_name} = {loader_call}");

                // Validate by running the snippet through the normal compile pipeline.
                let module_source = self.synthesize_module(&snippet, InputKind::Definition);
                let path = Path::new("<repl_session>");
                let all_modules = match compile_repl_modules(path, &module_source) {
                    Ok(modules) => modules,
                    Err(diags) => {
                        self.push_diagnostics_as_error("<repl_session>", &diags);
                        return;
                    }
                };

                let infer = infer_value_types_full(&all_modules);
                let infer_diags: Vec<FileDiagnostic> = infer
                    .diagnostics
                    .into_iter()
                    .filter(|d| d.path == "<repl_session>")
                    .collect();

                if file_diagnostics_have_errors(&infer_diags) {
                    self.push_diagnostics_as_error("<repl_session>", &infer_diags);
                    return;
                }

                // Inject the @static snippet and register the binding in the symbol inventory.
                // Use the inferred type from this compilation (mirrors what eval_input does for
                // definitions), with "?" as a fallback if type inference can't resolve it.
                let binding_type = infer
                    .type_strings
                    .get("repl_session")
                    .and_then(|types| types.get(binding_name.as_str()))
                    .cloned()
                    .unwrap_or_else(|| "?".to_owned());

                self.definitions.push(snippet);
                self.session_types
                    .insert(binding_name.clone(), binding_type);
                self.session_constructors = self.current_session_constructors();
                if let Some(pane) = self.active_pane {
                    self.cached_symbols = self.build_symbol_entries(pane, "");
                }

                self.transcript.push(TranscriptEntry {
                    kind: TranscriptKind::System,
                    text: format!(
                        "OpenAPI {source_label} `{source}` bound as `{binding_name}` \
                         — use `{binding_name} {{}}` to create a client"
                    ),
                });
            }
            _ => {
                self.push_error(
                    "/openapi expects `file` or `url`. Example: /openapi file spec.yaml".to_owned(),
                );
            }
        }
    }

    // ── Evaluation ──────────────────────────────────────────────────────────

    fn eval_input(&mut self, input: &str) {
        let classification = classify_input(input);

        // Build a synthetic module source containing all session context.
        let source = self.synthesize_module(input, classification);
        let path = Path::new("<repl_session>");
        let all_modules = match compile_repl_modules(path, &source) {
            Ok(modules) => modules,
            Err(diags) => {
                self.push_diagnostics_as_error("<repl_session>", &diags);
                return;
            }
        };

        let infer = infer_value_types_full(&all_modules);
        let infer_diags: Vec<FileDiagnostic> = infer
            .diagnostics
            .into_iter()
            .filter(|d| d.path == "<repl_session>")
            .collect();

        if file_diagnostics_have_errors(&infer_diags) {
            self.push_diagnostics_as_error("<repl_session>", &infer_diags);
            return;
        }

        let session_types = infer
            .type_strings
            .get("repl_session")
            .cloned()
            .unwrap_or_default();

        match classification {
            InputKind::Definition => {
                let defined_names = extract_defined_names(input);
                self.jit_session.forget_bindings(&defined_names);
                self.definitions.push(input.to_owned());
                if defined_names.is_empty() {
                    self.transcript.push(TranscriptEntry {
                        kind: TranscriptKind::Defined,
                        text: "(defined)".to_owned(),
                    });
                } else {
                    for name in &defined_names {
                        let type_str = session_types
                            .get(name.as_str())
                            .cloned()
                            .unwrap_or_else(|| "?".to_owned());
                        // Persist the inferred type for symbol pane lookups.
                        self.session_types.insert(name.clone(), type_str.clone());
                        self.transcript.push(TranscriptEntry {
                            kind: TranscriptKind::Defined,
                            text: format!("{name} :: {type_str}  (defined)"),
                        });
                    }
                }

                self.session_constructors = self.current_session_constructors();

                // Rebuild cached symbols.
                if let Some(pane) = self.active_pane {
                    self.cached_symbols = self.build_symbol_entries(pane, "");
                }
            }
            InputKind::Expression => {
                let result_type = session_types
                    .get("_replResult")
                    .cloned()
                    .unwrap_or_else(|| "?".to_owned());
                let program = desugar_modules(&all_modules);
                let capture_binding_names: Vec<String> =
                    self.session_types.keys().cloned().collect();
                match self.jit_session.evaluate_binding_detailed(
                    program,
                    infer.cg_types,
                    infer.monomorph_plan,
                    infer.source_schemas,
                    &all_modules,
                    "_replResult",
                    self.autorun_effects,
                    &capture_binding_names,
                ) {
                    Ok(result) => {
                        self.push_captured_stream(
                            TranscriptKind::CommandOutput,
                            &result.stdout_text,
                        );
                        self.push_captured_stream(TranscriptKind::Error, &result.stderr_text);
                        let suffix = if result.effect_ran { "  (autorun)" } else { "" };
                        self.transcript.push(TranscriptEntry {
                            kind: TranscriptKind::ValueResult,
                            text: format!("{} :: {}{}", result.value_text, result_type, suffix),
                        });
                    }
                    Err(err) => self.push_error(err.to_string()),
                }
            }
        }
    }

    fn synthesize_module(&self, input: &str, kind: InputKind) -> String {
        let mut lines = vec!["module repl_session".to_owned(), String::new()];
        lines.push("use aivi.prelude".to_owned());
        for imp in &self.imports {
            lines.push(format!("use {imp}"));
        }
        let existing_sig_names: HashSet<String> = self
            .definitions
            .iter()
            .flat_map(|def| extract_type_sig_names(def))
            .collect();
        let mut inferred_sigs: Vec<_> = self
            .session_types
            .iter()
            .filter(|(name, _)| !existing_sig_names.contains(*name))
            .collect();
        inferred_sigs.sort_by(|a, b| a.0.cmp(b.0));
        lines.push(String::new());
        for (name, type_str) in inferred_sigs {
            lines.push(format!("{name} : {type_str}"));
        }
        if !self.session_types.is_empty() {
            lines.push(String::new());
        }
        for def in &self.definitions {
            lines.push(def.clone());
            lines.push(String::new());
        }
        match kind {
            InputKind::Definition => lines.push(input.to_owned()),
            InputKind::Expression => lines.push(format!("_replResult = {input}")),
        }
        lines.join("\n")
    }

    fn push_diagnostics_as_error(&mut self, path: &str, diags: &[FileDiagnostic]) {
        // Use render_diagnostics (no color — TUI applies its own styles).
        let rendered = render_diagnostics(
            path,
            &diags
                .iter()
                .map(|d| d.diagnostic.clone())
                .collect::<Vec<_>>(),
            false,
        );
        if !rendered.is_empty() {
            self.push_error(rendered);
        }
    }

    // ── Symbol inventory ────────────────────────────────────────────────────

    fn build_symbol_entries(&self, pane: SymbolPane, filter: &str) -> Vec<SymbolEntry> {
        let session_types: Vec<(String, String)> = self
            .session_types
            .iter()
            .map(|(name, ty)| (name.clone(), ty.clone()))
            .collect();
        match pane {
            SymbolPane::Types => {
                let mut names: Vec<String> = Vec::new();
                for module in embedded_stdlib_modules()
                    .into_iter()
                    .filter(|module| self.module_is_visible(&module.name.name))
                {
                    for item in &module.items {
                        let name = match item {
                            ModuleItem::TypeDecl(td) => Some(td.name.name.clone()),
                            ModuleItem::TypeAlias(ta) => Some(ta.name.name.clone()),
                            ModuleItem::DomainDecl(dd) => Some(dd.name.name.clone()),
                            _ => None,
                        };
                        if let Some(n) = name {
                            if !names.contains(&n) {
                                names.push(n);
                            }
                        }
                    }
                }
                names.sort();
                let mut entries: Vec<SymbolEntry> = names
                    .into_iter()
                    .filter(|n| matches_filter(n, filter))
                    .map(|name| SymbolEntry {
                        kind: SymbolKind::Type,
                        name,
                    })
                    .collect();
                for (name, type_str) in session_types {
                    if matches_filter(&name, filter) {
                        entries.push(SymbolEntry {
                            kind: SymbolKind::Type,
                            name: format!("{name} :: {type_str}"),
                        });
                    }
                }
                entries
            }
            SymbolPane::Functions => self
                .function_value_inventory()
                .into_iter()
                .filter(|item| matches!(item.kind, CompletionKind::Function))
                .filter(|item| matches_filter(&item.name, filter))
                .map(|item| SymbolEntry {
                    kind: SymbolKind::Function,
                    name: format_function_entry(&item),
                })
                .collect(),
            SymbolPane::Values => {
                let mut entries: Vec<SymbolEntry> = session_types
                    .into_iter()
                    .filter(|(name, _)| matches_filter(name, filter))
                    .map(|(name, type_str)| SymbolEntry {
                        kind: SymbolKind::Value,
                        name: if type_str.contains("->") {
                            format!("{name}  {}", format_function_placeholder(&type_str))
                        } else {
                            format!("{name}  {}", format_opaque_placeholder(&type_str))
                        },
                    })
                    .collect();
                entries.sort_by(|a, b| a.name.cmp(&b.name));
                entries
            }
            SymbolPane::Modules => {
                let mut entries = vec![SymbolEntry {
                    kind: SymbolKind::Module,
                    name: "aivi.prelude".to_owned(),
                }];
                for imp in &self.imports {
                    if matches_filter(imp, filter) {
                        entries.push(SymbolEntry {
                            kind: SymbolKind::Module,
                            name: imp.clone(),
                        });
                    }
                }
                entries
            }
        }
    }

    fn current_session_types(&self) -> Vec<(String, String)> {
        if self.definitions.is_empty() {
            return Vec::new();
        }

        let source = self.synthesize_module("", InputKind::Definition);
        let path = Path::new("<repl_session>");
        let Ok(all_modules) = compile_repl_modules(path, &source) else {
            return Vec::new();
        };

        let infer = infer_value_types_full(&all_modules);
        if file_diagnostics_have_errors(&infer.diagnostics) {
            return Vec::new();
        }

        let mut entries: Vec<(String, String)> = infer
            .type_strings
            .get("repl_session")
            .into_iter()
            .flat_map(|defs| defs.iter().map(|(name, ty)| (name.clone(), ty.clone())))
            .filter(|(name, _)| !name.starts_with('_'))
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries
    }

    fn current_session_constructors(&self) -> Vec<SymbolCompletion> {
        let modules = self.current_session_modules();
        modules
            .iter()
            .flat_map(|module| collect_completion_symbols(module, true))
            .filter(|item| item.kind == CompletionKind::Constructor)
            .collect()
    }

    fn current_session_modules(&self) -> Vec<Module> {
        if self.definitions.is_empty() {
            return Vec::new();
        }

        let source = self.synthesize_module("", InputKind::Definition);
        let path = Path::new("<repl_session>");
        compile_repl_modules(path, &source).unwrap_or_default()
    }

    fn command_completion_state(&self, input: &str, cursor: usize) -> Option<CompletionState> {
        let trimmed = input.trim_start();
        if !trimmed.starts_with('/') {
            return None;
        }
        let leading_ws = input.len() - trimmed.len();
        if cursor < leading_ws {
            return None;
        }

        let command_end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
        let replace_start = leading_ws;
        let replace_end = leading_ws + command_end;
        if cursor > replace_end {
            return None;
        }

        let fragment = &input[replace_start..replace_end];
        let items: Vec<CompletionItem> = slash_command_suggestions(fragment)
            .into_iter()
            .map(|name| CompletionItem {
                kind: CompletionKind::Command,
                label: name.to_owned(),
                insert_text: name.to_owned(),
                detail: command_summary(name).to_owned(),
            })
            .collect();
        if items.is_empty() {
            None
        } else {
            Some(CompletionState {
                mode: CompletionMode::Command,
                replace_start,
                replace_end,
                items,
            })
        }
    }

    fn functions_filter_completion_state(
        &self,
        input: &str,
        cursor: usize,
    ) -> Option<CompletionState> {
        const COMMAND: &str = "/functions";

        let trimmed = input.trim_start();
        if !trimmed.starts_with(COMMAND) {
            return None;
        }

        let leading_ws = input.len() - trimmed.len();
        let rest = &trimmed[COMMAND.len()..];
        if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
            return None;
        }

        let command_end = leading_ws + COMMAND.len();
        if cursor < command_end {
            return None;
        }

        let filter_start = if rest.trim().is_empty() {
            input.len()
        } else {
            leading_ws + COMMAND.len() + rest.find(|c: char| !c.is_whitespace()).unwrap_or(0)
        };
        let filter = rest.trim();
        let items: Vec<CompletionItem> = self
            .function_value_inventory()
            .into_iter()
            .filter(|item| item.kind == CompletionKind::Function)
            .filter(|item| matches_filter(&item.name, filter))
            .map(|item| CompletionItem {
                kind: CompletionKind::Function,
                label: format_function_completion_label(&item),
                insert_text: item.name,
                detail: item.detail,
            })
            .collect();
        if items.is_empty() {
            None
        } else {
            Some(CompletionState {
                mode: CompletionMode::SlashFilter,
                replace_start: filter_start.min(input.len()),
                replace_end: input.len(),
                items,
            })
        }
    }

    fn symbol_completion_state(&self, input: &str, cursor: usize) -> Option<CompletionState> {
        let (replace_start, replace_end) = completion_token_range(input, cursor);
        if replace_start == replace_end || cursor < replace_start {
            return None;
        }

        let fragment = &input[replace_start..cursor];
        if fragment.is_empty() {
            return None;
        }

        let items: Vec<CompletionItem> = if starts_with_upper(fragment) {
            self.constructor_inventory()
                .into_iter()
                .filter(|item| item.name.starts_with(fragment))
                .map(symbol_completion_item)
                .collect()
        } else if starts_with_lower_or_underscore(fragment) {
            self.function_value_inventory()
                .into_iter()
                .filter(|item| item.name.starts_with(fragment))
                .map(symbol_completion_item)
                .collect()
        } else {
            Vec::new()
        };
        if items.is_empty() {
            None
        } else {
            Some(CompletionState {
                mode: CompletionMode::Symbol,
                replace_start,
                replace_end,
                items,
            })
        }
    }

    fn function_value_inventory(&self) -> Vec<SymbolCompletion> {
        let mut session_items: Vec<SymbolCompletion> = self
            .session_types
            .iter()
            .map(|(name, ty)| {
                let kind = if ty.contains("->") {
                    CompletionKind::Function
                } else {
                    CompletionKind::Value
                };
                SymbolCompletion {
                    kind,
                    name: name.clone(),
                    detail: ty.clone(),
                    module: "repl_session".to_owned(),
                }
            })
            .collect();
        session_items.sort_by(|a, b| a.name.cmp(&b.name));

        let stdlib_items = self
            .visible_stdlib_completions()
            .into_iter()
            .filter(|item| matches!(item.kind, CompletionKind::Function | CompletionKind::Value));
        let builtin_items = self
            .builtin_completions
            .iter()
            .filter(|item| matches!(item.kind, CompletionKind::Function | CompletionKind::Value))
            .cloned()
            .collect();

        merge_symbol_completions(
            merge_symbol_completions(session_items, stdlib_items.collect()),
            builtin_items,
        )
    }

    fn constructor_inventory(&self) -> Vec<SymbolCompletion> {
        merge_symbol_completions(
            merge_symbol_completions(
                self.session_constructors.clone(),
                self.builtin_constructor_completions(),
            ),
            self.visible_stdlib_completions()
                .into_iter()
                .filter(|item| item.kind == CompletionKind::Constructor)
                .collect(),
        )
    }

    fn builtin_constructor_completions(&self) -> Vec<SymbolCompletion> {
        self.builtin_completions
            .iter()
            .filter(|item| item.kind == CompletionKind::Constructor)
            .cloned()
            .collect()
    }

    fn visible_stdlib_completions(&self) -> Vec<SymbolCompletion> {
        self.visible_stdlib_module_names()
            .into_iter()
            .filter_map(|module_name| self.stdlib_module_completions.get(&module_name))
            .flat_map(|items| items.iter().cloned())
            .collect()
    }

    fn visible_stdlib_module_names(&self) -> Vec<String> {
        let mut names = self.default_visible_modules.clone();
        for import in &self.imports {
            if !names.contains(import) {
                names.push(import.clone());
            }
        }
        names
    }

    fn module_is_visible(&self, module_name: &str) -> bool {
        self.default_visible_modules
            .iter()
            .any(|visible| visible == module_name)
            || self.imports.iter().any(|import| import == module_name)
    }

    fn module_exists(&self, module_name: &str) -> bool {
        self.stdlib_module_completions.contains_key(module_name)
            || self
                .current_session_modules()
                .into_iter()
                .any(|module| module.name.name == module_name)
    }

    fn resolve_explain_subjects(&self, query: &str) -> Vec<ExplainSubject> {
        let inventory = self.explain_inventory();

        let module_matches: Vec<ExplainSubject> = inventory
            .iter()
            .filter(|subject| subject.kind == ExplainKind::Module && subject.name == query)
            .cloned()
            .collect();
        if !module_matches.is_empty() {
            return module_matches;
        }

        if let Some((module, name)) = query.rsplit_once('.') {
            let qualified: Vec<ExplainSubject> = inventory
                .iter()
                .filter(|subject| subject.name == name && subject.module.as_deref() == Some(module))
                .cloned()
                .collect();
            if !qualified.is_empty() {
                return qualified;
            }
        }

        inventory
            .into_iter()
            .filter(|subject| subject.name == query)
            .collect()
    }

    fn explain_inventory(&self) -> Vec<ExplainSubject> {
        let mut subjects = Vec::new();
        let mut seen = HashSet::new();
        let session_modules = self.current_session_modules();
        let stdlib_modules = embedded_stdlib_modules();

        for module in stdlib_modules.iter().chain(session_modules.iter()) {
            self.push_module_explain_subject(module, &mut subjects, &mut seen);
            for item in &module.items {
                self.push_item_explain_subjects(&module.name.name, item, &mut subjects, &mut seen);
            }
        }

        for (name, ty) in &self.session_types {
            self.push_explain_subject(
                &mut subjects,
                &mut seen,
                ExplainKind::from_type_string(ty),
                name.clone(),
                Some("repl_session".to_owned()),
                Some(ty.clone()),
            );
        }

        for item in &self.builtin_completions {
            let kind = match item.kind {
                CompletionKind::Constructor => ExplainKind::Constructor,
                CompletionKind::Function => ExplainKind::Function,
                CompletionKind::Value => ExplainKind::Value,
                CompletionKind::Command => continue,
            };
            self.push_explain_subject(
                &mut subjects,
                &mut seen,
                kind,
                item.name.clone(),
                Some(item.module.clone()),
                Some(item.detail.clone()),
            );
        }

        subjects.sort_by(|a, b| {
            a.name
                .cmp(&b.name)
                .then(subject_module_name(a).cmp(subject_module_name(b)))
                .then(a.kind.label().cmp(b.kind.label()))
        });
        subjects
    }

    fn push_module_explain_subject(
        &self,
        module: &Module,
        subjects: &mut Vec<ExplainSubject>,
        seen: &mut HashSet<String>,
    ) {
        self.push_explain_subject(
            subjects,
            seen,
            ExplainKind::Module,
            module.name.name.clone(),
            None,
            None,
        );
    }

    fn push_item_explain_subjects(
        &self,
        module_name: &str,
        item: &ModuleItem,
        subjects: &mut Vec<ExplainSubject>,
        seen: &mut HashSet<String>,
    ) {
        match item {
            ModuleItem::TypeSig(sig) => {
                self.push_explain_subject(
                    subjects,
                    seen,
                    ExplainKind::from_type_expr(&sig.ty),
                    sig.name.name.clone(),
                    Some(module_name.to_owned()),
                    Some(render_type_expr(&sig.ty)),
                );
            }
            ModuleItem::TypeDecl(type_decl) => {
                self.push_explain_subject(
                    subjects,
                    seen,
                    ExplainKind::Type,
                    type_decl.name.name.clone(),
                    Some(module_name.to_owned()),
                    Some(render_type_decl_signature(type_decl)),
                );
            }
            ModuleItem::TypeAlias(type_alias) => {
                self.push_explain_subject(
                    subjects,
                    seen,
                    ExplainKind::Type,
                    type_alias.name.name.clone(),
                    Some(module_name.to_owned()),
                    Some(render_type_alias_signature(type_alias)),
                );
            }
            ModuleItem::ClassDecl(class_decl) => {
                self.push_explain_subject(
                    subjects,
                    seen,
                    ExplainKind::Class,
                    class_decl.name.name.clone(),
                    Some(module_name.to_owned()),
                    Some(render_class_signature(class_decl)),
                );
            }
            ModuleItem::DomainDecl(domain_decl) => {
                self.push_explain_subject(
                    subjects,
                    seen,
                    ExplainKind::Domain,
                    domain_decl.name.name.clone(),
                    Some(module_name.to_owned()),
                    Some(render_domain_signature(domain_decl)),
                );
                for domain_item in &domain_decl.items {
                    match domain_item {
                        DomainItem::TypeSig(sig) => {
                            self.push_explain_subject(
                                subjects,
                                seen,
                                ExplainKind::from_type_expr(&sig.ty),
                                sig.name.name.clone(),
                                Some(module_name.to_owned()),
                                Some(render_type_expr(&sig.ty)),
                            );
                        }
                        DomainItem::TypeAlias(type_decl) => {
                            self.push_explain_subject(
                                subjects,
                                seen,
                                ExplainKind::Type,
                                type_decl.name.name.clone(),
                                Some(module_name.to_owned()),
                                Some(render_type_decl_signature(type_decl)),
                            );
                        }
                        DomainItem::Def(_) | DomainItem::LiteralDef(_) => {}
                    }
                }
            }
            ModuleItem::Def(_) | ModuleItem::InstanceDecl(_) => {}
        }
    }

    fn push_explain_subject(
        &self,
        subjects: &mut Vec<ExplainSubject>,
        seen: &mut HashSet<String>,
        kind: ExplainKind,
        name: String,
        module: Option<String>,
        signature: Option<String>,
    ) {
        let key = format!(
            "{}|{}|{}",
            kind.label(),
            module.as_deref().unwrap_or(""),
            name
        );
        if !seen.insert(key) {
            return;
        }

        let quick_info = match kind {
            ExplainKind::Module => self.doc_index.lookup_module(&name).cloned(),
            _ => self
                .doc_index
                .lookup_best(&name, module.as_deref())
                .cloned(),
        };

        subjects.push(ExplainSubject {
            kind,
            name,
            module,
            signature,
            quick_info,
        });
    }

    // ── Helpers ─────────────────────────────────────────────────────────────

    fn push_error(&mut self, text: String) {
        self.transcript.push(TranscriptEntry {
            kind: TranscriptKind::Error,
            text,
        });
    }

    fn push_command_output(&mut self, text: String) {
        self.transcript.push(TranscriptEntry {
            kind: TranscriptKind::CommandOutput,
            text,
        });
    }

    fn push_captured_stream(&mut self, kind: TranscriptKind, text: &str) {
        if text.is_empty() {
            return;
        }
        for chunk in text.split_inclusive('\n') {
            let line = chunk.strip_suffix('\n').unwrap_or(chunk).to_owned();
            self.transcript.push(TranscriptEntry {
                kind: kind.clone(),
                text: line,
            });
        }
    }

    fn use_color(&self) -> bool {
        match self.options.color_mode {
            ColorMode::Auto => io::stdout().is_terminal(),
            ColorMode::Always => true,
            ColorMode::Never => false,
        }
    }
}

fn compile_repl_modules(path: &Path, source: &str) -> Result<Vec<Module>, Vec<FileDiagnostic>> {
    let path_text = path.display().to_string();
    let (mut session_modules, mut parse_diags) = parse_modules(path, source);
    let mut all_modules = embedded_stdlib_modules();
    all_modules.append(&mut session_modules);
    resolve_import_names(&mut all_modules);

    parse_diags.extend(
        check_modules(&all_modules)
            .into_iter()
            .filter(|d| d.path == path_text),
    );
    if file_diagnostics_have_errors(&parse_diags) {
        return Err(parse_diags);
    }

    let elab_diags: Vec<FileDiagnostic> = elaborate_expected_coercions(&mut all_modules)
        .into_iter()
        .filter(|d| d.path == path_text)
        .collect();
    if file_diagnostics_have_errors(&elab_diags) {
        return Err(elab_diags);
    }

    let type_diags: Vec<FileDiagnostic> = check_types(&all_modules)
        .into_iter()
        .filter(|d| d.path == path_text)
        .collect();
    if file_diagnostics_have_errors(&type_diags) {
        return Err(type_diags);
    }

    Ok(all_modules)
}

// ── Input classification ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputKind {
    Definition,
    Expression,
}

/// Heuristically classify a line of AIVI input as a definition or an expression.
fn classify_input(input: &str) -> InputKind {
    let trimmed = input.trim_start();

    // Keyword-introduced definitions.
    if trimmed.starts_with("type ")
        || trimmed.starts_with("domain ")
        || trimmed.starts_with("class ")
        || trimmed.starts_with("instance ")
        || trimmed.starts_with("opaque ")
    {
        return InputKind::Definition;
    }

    // `name = ...` (binding) — ensure it's not `==`.
    if let Some(eq_pos) = trimmed.find('=') {
        let after = trimmed.get(eq_pos + 1..).unwrap_or("");
        if !after.starts_with('=') {
            let lhs = trimmed[..eq_pos].trim();
            if is_valid_binding_lhs(lhs) {
                return InputKind::Definition;
            }
        }
    }

    // `name : Type` (type signature) — ensure it's not `::`.
    if let Some(colon_pos) = trimmed.find(':') {
        let before = trimmed[..colon_pos].trim();
        let after = trimmed.get(colon_pos + 1..).unwrap_or("");
        if !after.starts_with(':') && is_valid_identifier(before) {
            return InputKind::Definition;
        }
    }

    InputKind::Expression
}

fn is_valid_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '\'')
}

fn is_valid_binding_lhs(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut tokens = s.split_whitespace();
    let name = tokens.next().unwrap_or("");
    is_valid_identifier(name) && tokens.all(|t| is_valid_identifier(t) || t == "_")
}

/// Extract top-level binding names from a definition snippet.
fn extract_defined_names(input: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        if let Some(eq_pos) = trimmed.find('=') {
            let after = trimmed.get(eq_pos + 1..).unwrap_or("");
            if after.starts_with('=') {
                continue; // `==`
            }
            let lhs = trimmed[..eq_pos].trim();
            let candidate = lhs.split_whitespace().next().unwrap_or("");
            if is_valid_identifier(candidate) && !names.contains(&candidate.to_owned()) {
                names.push(candidate.to_owned());
            }
        }
    }
    names
}

fn extract_type_sig_names(input: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        if let Some(colon_pos) = trimmed.find(':') {
            let before = trimmed[..colon_pos].trim();
            let after = trimmed.get(colon_pos + 1..).unwrap_or("");
            if !after.starts_with(':')
                && is_valid_identifier(before)
                && !names.contains(&before.to_owned())
            {
                names.push(before.to_owned());
            }
        }
    }
    names
}

// ── Module helpers ────────────────────────────────────────────────────────────

fn count_exportable_items(module: &Module) -> usize {
    module
        .items
        .iter()
        .filter(|item| {
            matches!(
                item,
                ModuleItem::TypeDecl(_)
                    | ModuleItem::TypeAlias(_)
                    | ModuleItem::DomainDecl(_)
                    | ModuleItem::TypeSig(_)
                    | ModuleItem::Def(_)
            )
        })
        .count()
}

fn collect_default_visible_modules(stdlib: &[Module]) -> Vec<String> {
    let mut modules = vec!["aivi.prelude".to_owned()];
    if let Some(prelude) = stdlib
        .iter()
        .find(|module| module.name.name == "aivi.prelude")
    {
        for use_decl in &prelude.uses {
            if !modules.contains(&use_decl.module.name) {
                modules.push(use_decl.module.name.clone());
            }
        }
    }
    modules
}

fn builtin_symbol_completions() -> Vec<SymbolCompletion> {
    vec![
        SymbolCompletion {
            kind: CompletionKind::Value,
            name: "Unit".to_owned(),
            detail: "Unit".to_owned(),
            module: "aivi".to_owned(),
        },
        SymbolCompletion {
            kind: CompletionKind::Constructor,
            name: "True".to_owned(),
            detail: "Bool".to_owned(),
            module: "aivi".to_owned(),
        },
        SymbolCompletion {
            kind: CompletionKind::Constructor,
            name: "False".to_owned(),
            detail: "Bool".to_owned(),
            module: "aivi".to_owned(),
        },
        SymbolCompletion {
            kind: CompletionKind::Constructor,
            name: "None".to_owned(),
            detail: "Option A".to_owned(),
            module: "aivi".to_owned(),
        },
        SymbolCompletion {
            kind: CompletionKind::Constructor,
            name: "Some".to_owned(),
            detail: "A -> Option A".to_owned(),
            module: "aivi".to_owned(),
        },
        SymbolCompletion {
            kind: CompletionKind::Constructor,
            name: "Ok".to_owned(),
            detail: "A -> Result E A".to_owned(),
            module: "aivi".to_owned(),
        },
        SymbolCompletion {
            kind: CompletionKind::Constructor,
            name: "Err".to_owned(),
            detail: "E -> Result E A".to_owned(),
            module: "aivi".to_owned(),
        },
        SymbolCompletion {
            kind: CompletionKind::Function,
            name: "pure".to_owned(),
            detail: "A -> Effect E A".to_owned(),
            module: "aivi".to_owned(),
        },
        SymbolCompletion {
            kind: CompletionKind::Function,
            name: "fail".to_owned(),
            detail: "E -> Effect E A".to_owned(),
            module: "aivi".to_owned(),
        },
        SymbolCompletion {
            kind: CompletionKind::Function,
            name: "attempt".to_owned(),
            detail: "Effect E A -> Effect E (Result E A)".to_owned(),
            module: "aivi".to_owned(),
        },
        SymbolCompletion {
            kind: CompletionKind::Function,
            name: "load".to_owned(),
            detail: "Effect E A -> Effect E A".to_owned(),
            module: "aivi".to_owned(),
        },
        SymbolCompletion {
            kind: CompletionKind::Function,
            name: "constructorName".to_owned(),
            detail: "A -> Text".to_owned(),
            module: "aivi".to_owned(),
        },
        SymbolCompletion {
            kind: CompletionKind::Function,
            name: "constructorOrdinal".to_owned(),
            detail: "A -> Int".to_owned(),
            module: "aivi".to_owned(),
        },
        SymbolCompletion {
            kind: CompletionKind::Function,
            name: "print".to_owned(),
            detail: "Text -> Effect Text Unit".to_owned(),
            module: "aivi".to_owned(),
        },
        SymbolCompletion {
            kind: CompletionKind::Function,
            name: "println".to_owned(),
            detail: "Text -> Effect Text Unit".to_owned(),
            module: "aivi".to_owned(),
        },
    ]
}

// ── Plain display ─────────────────────────────────────────────────────────────

fn plain_entry_text(entry: &TranscriptEntry) -> String {
    match entry.kind {
        TranscriptKind::Input => format!("> {}", entry.text),
        TranscriptKind::Error => format!("  ✖ {}", entry.text),
        TranscriptKind::Warning => format!("  ⚠ {}", entry.text),
        TranscriptKind::System => format!("  ✓ {}", entry.text),
        _ => format!("  {}", entry.text),
    }
}

// ── Slash command helpers ─────────────────────────────────────────────────────

pub(crate) const SLASH_COMMANDS: &[&str] = &[
    "/help",
    "/explain",
    "/use",
    "/types",
    "/values",
    "/functions",
    "/autorun",
    "/modules",
    "/clear",
    "/reset",
    "/history",
    "/load",
    "/openapi",
];

fn closest_command(input: &str) -> Option<&'static str> {
    let input_lower = input.to_lowercase();
    SLASH_COMMANDS
        .iter()
        .filter_map(|&cmd| {
            let dist = levenshtein(&input_lower, cmd);
            if dist <= 4 {
                Some((cmd, dist))
            } else {
                None
            }
        })
        .min_by_key(|&(_, d)| d)
        .map(|(cmd, _)| cmd)
}

pub(crate) fn slash_command_suggestions(input: &str) -> Vec<&'static str> {
    let trimmed = input.trim_start();
    if !trimmed.starts_with('/') {
        return Vec::new();
    }
    let command_fragment = trimmed.split_whitespace().next().unwrap_or(trimmed);
    if trimmed.contains(char::is_whitespace) && command_fragment != "/" {
        return Vec::new();
    }

    let mut suggestions: Vec<&'static str> = if command_fragment == "/" {
        SLASH_COMMANDS.to_vec()
    } else {
        SLASH_COMMANDS
            .iter()
            .copied()
            .filter(|cmd| cmd.starts_with(command_fragment))
            .collect()
    };
    suggestions.sort_unstable();
    suggestions
}

pub(crate) fn command_accepts_argument(command: &str) -> bool {
    matches!(
        command,
        "/explain"
            | "/use"
            | "/types"
            | "/values"
            | "/functions"
            | "/autorun"
            | "/history"
            | "/load"
            | "/openapi"
    )
}

pub(crate) fn command_summary(command: &str) -> &'static str {
    match command {
        "/help" => "print command reference",
        "/explain" => "show quick info for a symbol",
        "/use" => "add import to session",
        "/types" => "list types in scope",
        "/values" => "list session values with types",
        "/functions" => "list functions in scope",
        "/autorun" => "toggle top-level effect autorun",
        "/modules" => "show loaded modules",
        "/clear" => "clear transcript",
        "/reset" => "reset transcript and session state",
        "/history" => "show recent inputs",
        "/load" => "load a .aivi file",
        "/openapi" => "inject an OpenAPI module",
        _ => "",
    }
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (m, n) = (a.len(), b.len());
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for (i, row) in dp.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in dp[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if a[i - 1] == b[j - 1] {
                dp[i - 1][j - 1]
            } else {
                1 + dp[i - 1][j].min(dp[i][j - 1]).min(dp[i - 1][j - 1])
            };
        }
    }
    dp[m][n]
}

fn matches_filter(name: &str, filter: &str) -> bool {
    filter.is_empty() || name.contains(filter)
}

fn render_type_decl_signature(type_decl: &TypeDecl) -> String {
    let head = render_type_head(&type_decl.name.name, &type_decl.params);
    if type_decl.opaque {
        format!("opaque {head}")
    } else {
        format!("type {head}")
    }
}

fn render_type_alias_signature(type_alias: &TypeAlias) -> String {
    let head = render_type_head(&type_alias.name.name, &type_alias.params);
    let prefix = if type_alias.opaque { "opaque" } else { "type" };
    format!(
        "{prefix} {head} = {}",
        render_type_expr(&type_alias.aliased)
    )
}

fn render_class_signature(class_decl: &ClassDecl) -> String {
    if class_decl.params.is_empty() {
        format!("class {}", class_decl.name.name)
    } else {
        format!(
            "class {} {}",
            class_decl.name.name,
            class_decl
                .params
                .iter()
                .map(render_type_expr)
                .collect::<Vec<_>>()
                .join(" ")
        )
    }
}

fn render_domain_signature(domain_decl: &DomainDecl) -> String {
    format!(
        "domain {} over {}",
        domain_decl.name.name,
        render_type_expr(&domain_decl.over)
    )
}

fn render_type_head(name: &str, params: &[aivi::SpannedName]) -> String {
    if params.is_empty() {
        name.to_owned()
    } else {
        format!(
            "{} {}",
            name,
            params
                .iter()
                .map(|param| param.name.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        )
    }
}

fn starts_with_upper(text: &str) -> bool {
    text.chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
}

fn starts_with_lower_or_underscore(text: &str) -> bool {
    text.chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_lowercase() || ch == '_')
}

fn completion_token_range(input: &str, cursor: usize) -> (usize, usize) {
    let cursor = cursor.min(input.len());
    let mut start = cursor;
    while start > 0 {
        let prev = input[..start].char_indices().next_back();
        let Some((idx, ch)) = prev else { break };
        if !is_identifier_completion_char(ch) {
            break;
        }
        start = idx;
    }

    let mut end = cursor;
    while end < input.len() {
        let Some(ch) = input[end..].chars().next() else {
            break;
        };
        if !is_identifier_completion_char(ch) {
            break;
        }
        end += ch.len_utf8();
    }

    (start, end)
}

fn is_identifier_completion_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '\''
}

fn format_function_entry(item: &SymbolCompletion) -> String {
    format!("{} :: {}  ({})", item.name, item.detail, item.module)
}

fn format_function_completion_label(item: &SymbolCompletion) -> String {
    format!("{} ({})", item.name, item.module)
}

fn symbol_completion_item(item: SymbolCompletion) -> CompletionItem {
    CompletionItem {
        kind: item.kind,
        label: item.name.clone(),
        insert_text: item.name,
        detail: item.detail,
    }
}

fn merge_symbol_completions(
    primary: Vec<SymbolCompletion>,
    secondary: Vec<SymbolCompletion>,
) -> Vec<SymbolCompletion> {
    let mut seen = HashSet::new();
    let mut merged = Vec::new();

    for item in primary.into_iter().chain(secondary) {
        if seen.insert(item.name.clone()) {
            merged.push(item);
        }
    }

    merged.sort_by(|a, b| a.name.cmp(&b.name).then(a.kind.cmp(&b.kind)));
    merged
}

fn collect_module_completion_symbols(
    modules: &[Module],
    include_opaque_ctors: bool,
) -> HashMap<String, Vec<SymbolCompletion>> {
    let mut by_module = HashMap::new();
    for module in modules {
        by_module.insert(
            module.name.name.clone(),
            collect_completion_symbols(module, include_opaque_ctors),
        );
    }
    by_module
}

fn collect_completion_symbols(
    module: &Module,
    include_opaque_ctors: bool,
) -> Vec<SymbolCompletion> {
    let mut seen = HashSet::new();
    let mut symbols = Vec::new();

    for item in &module.items {
        match item {
            ModuleItem::TypeSig(sig) => {
                push_type_sig_completion(&mut symbols, &mut seen, &module.name.name, sig);
            }
            ModuleItem::TypeDecl(type_decl) => {
                push_type_decl_constructors(
                    &mut symbols,
                    &mut seen,
                    &module.name.name,
                    type_decl,
                    include_opaque_ctors,
                );
            }
            ModuleItem::DomainDecl(domain) => {
                for domain_item in &domain.items {
                    match domain_item {
                        DomainItem::TypeSig(sig) => {
                            push_type_sig_completion(
                                &mut symbols,
                                &mut seen,
                                &module.name.name,
                                sig,
                            );
                        }
                        DomainItem::TypeAlias(type_decl) => {
                            push_type_decl_constructors(
                                &mut symbols,
                                &mut seen,
                                &module.name.name,
                                type_decl,
                                true,
                            );
                        }
                        DomainItem::Def(_) | DomainItem::LiteralDef(_) => {}
                    }
                }
            }
            ModuleItem::Def(_)
            | ModuleItem::TypeAlias(_)
            | ModuleItem::ClassDecl(_)
            | ModuleItem::InstanceDecl(_) => {}
        }
    }

    symbols.sort_by(|a, b| a.name.cmp(&b.name).then(a.kind.cmp(&b.kind)));
    symbols
}

fn push_type_sig_completion(
    symbols: &mut Vec<SymbolCompletion>,
    seen: &mut HashSet<String>,
    module: &str,
    sig: &TypeSig,
) {
    let name = sig.name.name.clone();
    if !seen.insert(name.clone()) {
        return;
    }

    let detail = render_type_expr(&sig.ty);
    let kind = if detail.contains("->") {
        CompletionKind::Function
    } else {
        CompletionKind::Value
    };
    symbols.push(SymbolCompletion {
        kind,
        name,
        detail,
        module: module.to_owned(),
    });
}

fn push_type_decl_constructors(
    symbols: &mut Vec<SymbolCompletion>,
    seen: &mut HashSet<String>,
    module: &str,
    type_decl: &TypeDecl,
    include_opaque_ctors: bool,
) {
    if type_decl.opaque && !include_opaque_ctors {
        return;
    }

    for ctor in &type_decl.constructors {
        let name = ctor.name.name.clone();
        if !seen.insert(name.clone()) {
            continue;
        }
        symbols.push(SymbolCompletion {
            kind: CompletionKind::Constructor,
            name,
            detail: render_constructor_type(type_decl, ctor),
            module: module.to_owned(),
        });
    }
}

fn render_constructor_type(type_decl: &TypeDecl, ctor: &aivi::TypeCtor) -> String {
    let result_type = if type_decl.params.is_empty() {
        type_decl.name.name.clone()
    } else {
        let params = type_decl
            .params
            .iter()
            .map(|param| param.name.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        format!("{} {params}", type_decl.name.name)
    };

    if ctor.args.is_empty() {
        result_type
    } else {
        let args = ctor
            .args
            .iter()
            .map(render_type_expr_as_arg)
            .collect::<Vec<_>>()
            .join(" -> ");
        format!("{args} -> {result_type}")
    }
}

fn render_type_expr_as_arg(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Func { .. } => format!("({})", render_type_expr(ty)),
        _ => render_type_expr(ty),
    }
}

fn render_type_expr(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Name(name) => name.name.clone(),
        TypeExpr::And { items, .. } => items
            .iter()
            .map(render_type_expr)
            .collect::<Vec<_>>()
            .join(" & "),
        TypeExpr::Apply { base, args, .. } => {
            let base_str = match base.as_ref() {
                TypeExpr::Func { .. } => format!("({})", render_type_expr(base)),
                _ => render_type_expr(base),
            };
            let args_str = args.iter().map(render_type_expr_as_arg).collect::<Vec<_>>();
            format!("{} {}", base_str, args_str.join(" "))
        }
        TypeExpr::Func { params, result, .. } => {
            let params = params
                .iter()
                .map(render_type_expr_as_arg)
                .collect::<Vec<_>>()
                .join(" -> ");
            format!("{params} -> {}", render_type_expr(result))
        }
        TypeExpr::Record { fields, .. } => {
            let fields = fields
                .iter()
                .map(|field| match field {
                    RecordTypeField::Named { name, ty } => {
                        format!("{}: {}", name.name, render_type_expr(ty))
                    }
                    RecordTypeField::Spread { ty, .. } => {
                        format!("...{}", render_type_expr(ty))
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{ {fields} }}")
        }
        TypeExpr::Tuple { items, .. } => format!(
            "({})",
            items
                .iter()
                .map(render_type_expr)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeExpr::Star { .. } => "*".to_owned(),
        TypeExpr::Unknown { .. } => "_".to_owned(),
    }
}

fn render_explain_subject(subject: &ExplainSubject) -> String {
    let mut lines = vec![format!("{} `{}`", subject.kind.heading(), subject.name)];
    lines.push(format!("module: {}", subject_module_name(subject)));
    if let Some(signature) = subject_signature(subject) {
        lines.push(format!("signature: {signature}"));
    }
    lines.push(String::new());
    lines.push("Quick info:".to_owned());
    match &subject.quick_info {
        Some(entry) => {
            let content = entry.content.trim();
            if content.is_empty() {
                lines.push("  no indexed docs available for this symbol yet.".to_owned());
            } else {
                lines.extend(content.lines().map(|line| {
                    if line.is_empty() {
                        String::new()
                    } else {
                        format!("  {line}")
                    }
                }));
            }
        }
        None => lines.push("  no indexed docs available for this symbol yet.".to_owned()),
    }
    lines.join("\n")
}

fn render_explain_ambiguity(query: &str, subjects: &[ExplainSubject]) -> String {
    let mut lines = vec![format!(
        "`{query}` matches multiple symbols. Use `/explain module.name` to disambiguate:"
    )];
    for subject in subjects {
        let mut line = format!(
            "  [{}] {} ({})",
            subject.kind.label(),
            subject.name,
            subject_module_name(subject)
        );
        if let Some(signature) = subject_signature(subject) {
            line.push_str(&format!(" :: {signature}"));
        }
        lines.push(line);
    }
    lines.join("\n")
}

fn subject_signature(subject: &ExplainSubject) -> Option<&str> {
    subject.signature.as_deref().or_else(|| {
        subject
            .quick_info
            .as_ref()
            .and_then(|entry| entry.signature.as_deref())
    })
}

fn subject_module_name(subject: &ExplainSubject) -> &str {
    subject.module.as_deref().unwrap_or(subject.name.as_str())
}

fn parse_openapi_args(rest: &str) -> (&str, Option<String>) {
    if let Some(as_pos) = rest.find(" as ") {
        let source = rest[..as_pos].trim();
        let alias = rest[as_pos + 4..].trim().to_owned();
        (source, Some(alias))
    } else {
        (rest.trim(), None)
    }
}

fn derive_module_name(source: &str) -> String {
    let base = source
        .rsplit('/')
        .next()
        .unwrap_or(source)
        .rsplit('\\')
        .next()
        .unwrap_or(source);
    base.trim_end_matches(".yaml")
        .trim_end_matches(".yml")
        .trim_end_matches(".json")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .to_lowercase()
}

// ── Public display helpers ────────────────────────────────────────────────────

/// Format a function value for the `/values` pane (when value is a function).
#[allow(dead_code)]
pub(crate) fn format_function_placeholder(type_str: &str) -> String {
    format!("<function :: {type_str}>")
}

impl ExplainKind {
    fn from_type_expr(ty: &TypeExpr) -> Self {
        if render_type_expr(ty).contains("->") {
            Self::Function
        } else {
            Self::Value
        }
    }

    fn from_type_string(type_str: &str) -> Self {
        if type_str.contains("->") {
            Self::Function
        } else {
            Self::Value
        }
    }

    fn label(self) -> &'static str {
        match self {
            ExplainKind::Module => "module",
            ExplainKind::Type => "type",
            ExplainKind::Class => "class",
            ExplainKind::Domain => "domain",
            ExplainKind::Function => "function",
            ExplainKind::Value => "value",
            ExplainKind::Constructor => "constructor",
        }
    }

    fn heading(self) -> &'static str {
        match self {
            ExplainKind::Module => "Module",
            ExplainKind::Type => "Type",
            ExplainKind::Class => "Class",
            ExplainKind::Domain => "Domain",
            ExplainKind::Function => "Function",
            ExplainKind::Value => "Value",
            ExplainKind::Constructor => "Constructor",
        }
    }
}

/// Format an opaque/complex value for the `/values` pane.
#[allow(dead_code)]
pub(crate) fn format_opaque_placeholder(type_str: &str) -> String {
    format!("<value :: {type_str}>")
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::io::{Read, Seek, SeekFrom, Write};
    #[cfg(unix)]
    use std::os::fd::AsRawFd;
    #[cfg(unix)]
    use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
    #[cfg(unix)]
    use std::sync::{Mutex, OnceLock};
    #[cfg(unix)]
    use tempfile::tempfile;

    fn make_engine() -> ReplEngine {
        let opts = ReplOptions {
            color_mode: ColorMode::Never,
            plain_mode: false,
        };
        ReplEngine::new(&opts).expect("engine creation failed")
    }

    #[cfg(unix)]
    fn capture_stderr<T>(f: impl FnOnce() -> T) -> (T, String) {
        static STDERR_CAPTURE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _capture_guard = STDERR_CAPTURE_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("stderr capture mutex poisoned");

        struct StderrCapture {
            target_fd: libc::c_int,
            saved_fd: libc::c_int,
            capture: std::fs::File,
        }

        impl StderrCapture {
            fn start() -> Self {
                let target_fd = std::io::stderr().as_raw_fd();
                let saved_fd = unsafe { libc::dup(target_fd) };
                assert!(saved_fd >= 0, "failed to dup stderr fd");

                let capture = tempfile().expect("create capture file");
                assert!(
                    unsafe { libc::dup2(capture.as_raw_fd(), target_fd) } >= 0,
                    "failed to redirect stderr"
                );

                Self {
                    target_fd,
                    saved_fd,
                    capture,
                }
            }

            fn finish(mut self) -> String {
                self.restore();
                self.capture
                    .seek(SeekFrom::Start(0))
                    .expect("rewind capture file");
                let mut output = String::new();
                self.capture
                    .read_to_string(&mut output)
                    .expect("read capture file");
                output
            }

            fn restore(&mut self) {
                if self.saved_fd < 0 {
                    return;
                }

                std::io::stderr().flush().expect("flush stderr");
                assert!(
                    unsafe { libc::dup2(self.saved_fd, self.target_fd) } >= 0,
                    "failed to restore stderr"
                );
                unsafe {
                    libc::close(self.saved_fd);
                }
                self.saved_fd = -1;
            }
        }

        impl Drop for StderrCapture {
            fn drop(&mut self) {
                self.restore();
            }
        }

        let capture = StderrCapture::start();
        let result = catch_unwind(AssertUnwindSafe(f));
        let output = capture.finish();

        match result {
            Ok(result) => (result, output),
            Err(panic) => resume_unwind(panic),
        }
    }

    // ── Input classification ────────────────────────────────────────────────

    #[test]
    fn classify_simple_binding() {
        assert_eq!(classify_input("x = 42"), InputKind::Definition);
    }

    #[test]
    fn classify_function_def() {
        assert_eq!(classify_input("double n = n * 2"), InputKind::Definition);
    }

    #[test]
    fn classify_type_sig() {
        assert_eq!(classify_input("double : Int -> Int"), InputKind::Definition);
    }

    #[test]
    fn classify_type_decl() {
        assert_eq!(
            classify_input("type Color = Red | Green | Blue"),
            InputKind::Definition
        );
    }

    #[test]
    fn classify_domain_decl() {
        assert_eq!(
            classify_input("domain Error = NotFound | InvalidInput"),
            InputKind::Definition
        );
    }

    #[test]
    fn classify_expression_addition() {
        assert_eq!(classify_input("1 + 2"), InputKind::Expression);
    }

    #[test]
    fn classify_expression_fn_call() {
        assert_eq!(classify_input("map f xs"), InputKind::Expression);
    }

    #[test]
    fn classify_equality_check_is_expression() {
        assert_eq!(classify_input("x == y"), InputKind::Expression);
    }

    #[test]
    fn classify_opaque_decl() {
        assert_eq!(classify_input("opaque UserId = Int"), InputKind::Definition);
    }

    // ── extract_defined_names ───────────────────────────────────────────────

    #[test]
    fn extract_single_binding() {
        assert_eq!(extract_defined_names("x = 42"), vec!["x"]);
    }

    #[test]
    fn extract_multiple_bindings() {
        let src = "a = 1\nb = 2\nc = 3";
        let names = extract_defined_names(src);
        assert!(names.contains(&"a".to_owned()));
        assert!(names.contains(&"b".to_owned()));
        assert!(names.contains(&"c".to_owned()));
    }

    #[test]
    fn extract_skips_comments() {
        let src = "// this is x = 1\nx = 42";
        assert_eq!(extract_defined_names(src), vec!["x"]);
    }

    #[test]
    fn extract_no_names_from_expression() {
        assert!(extract_defined_names("1 + 2").is_empty());
    }

    // ── Session state / submit ──────────────────────────────────────────────

    #[test]
    fn new_engine_has_startup_message() {
        let engine = make_engine();
        assert!(!engine.transcript.is_empty());
        assert!(matches!(engine.transcript[0].kind, TranscriptKind::System));
        assert!(engine.transcript[0].text.contains("aivi repl"));
    }

    #[test]
    fn submit_empty_returns_snapshot_without_changes() {
        let mut engine = make_engine();
        let before_len = engine.transcript.len();
        let snap = engine.submit("   ").unwrap();
        assert_eq!(snap.transcript.len(), before_len);
    }

    #[test]
    fn submit_adds_input_entry() {
        let mut engine = make_engine();
        let snap = engine.submit("/help").unwrap();
        let has_input = snap
            .transcript
            .iter()
            .any(|e| matches!(e.kind, TranscriptKind::Input) && e.text.contains("/help"));
        assert!(has_input);
    }

    #[test]
    fn slash_help_adds_command_output() {
        let mut engine = make_engine();
        let snap = engine.submit("/help").unwrap();
        let has_cmd = snap
            .transcript
            .iter()
            .any(|e| matches!(e.kind, TranscriptKind::CommandOutput) && e.text.contains("/help"));
        assert!(has_cmd);
    }

    #[test]
    fn slash_explain_requires_argument() {
        let mut engine = make_engine();
        let snap = engine.submit("/explain").unwrap();
        assert!(snap.transcript.iter().any(|entry| {
            matches!(entry.kind, TranscriptKind::Error)
                && entry
                    .text
                    .contains("/explain expects a type, function, value, or module name")
        }));
    }

    #[test]
    fn slash_explain_shows_doc_signature_and_module() {
        let mut engine = make_engine();
        let snap = engine.submit("/explain isAlnum").unwrap();
        assert!(snap.transcript.iter().any(|entry| {
            matches!(entry.kind, TranscriptKind::CommandOutput)
                && entry.text.contains("Function `isAlnum`")
                && entry.text.contains("module: aivi.text")
                && entry.text.contains("signature: Char -> Bool")
                && entry.text.contains("\n\nQuick info:\n")
                && entry.text.contains("Unicode letter or digit")
        }));
    }

    #[test]
    fn slash_explain_uses_session_type_info_when_docs_are_missing() {
        let mut engine = make_engine();
        engine
            .submit("double : Int -> Int\ndouble = n => n * 2")
            .unwrap();
        let snap = engine.submit("/explain double").unwrap();
        assert!(snap.transcript.iter().any(|entry| {
            matches!(entry.kind, TranscriptKind::CommandOutput)
                && entry.text.contains("Function `double`")
                && entry.text.contains("module: repl_session")
                && entry.text.contains("signature: Int -> Int")
                && entry
                    .text
                    .contains("\n\nQuick info:\n  no indexed docs available for this symbol yet.")
        }));
    }

    #[test]
    fn slash_clear_resets_transcript() {
        let mut engine = make_engine();
        engine.submit("/use aivi.text").unwrap();
        engine.submit("/clear").unwrap();
        // After /clear, only the system "cleared" message remains.
        assert_eq!(engine.transcript.len(), 1);
        assert!(matches!(engine.transcript[0].kind, TranscriptKind::System));
        // The import survived.
        assert!(engine.imports.contains(&"aivi.text".to_owned()));
    }

    #[test]
    fn slash_reset_clears_everything() {
        let mut engine = make_engine();
        engine.submit("/use aivi.text").unwrap();
        engine.definitions.push("x = 1".to_owned());
        engine.submit("/reset").unwrap();
        assert!(engine.imports.is_empty());
        assert!(engine.definitions.is_empty());
        assert_eq!(engine.turn, 0);
    }

    #[test]
    fn slash_use_adds_import() {
        let mut engine = make_engine();
        engine.submit("/use aivi.text").unwrap();
        assert!(engine.imports.contains(&"aivi.text".to_owned()));
    }

    #[test]
    fn slash_use_deduplicates() {
        let mut engine = make_engine();
        engine.submit("/use aivi.text").unwrap();
        engine.submit("/use aivi.text").unwrap();
        assert_eq!(
            engine.imports.iter().filter(|i| *i == "aivi.text").count(),
            1
        );
    }

    #[test]
    fn expression_submit_keeps_map_working_after_using_default_visible_logic_module() {
        let mut engine = make_engine();
        engine.submit("/use aivi.logic").unwrap();
        let snap = engine.submit("Some 5 |> map (_ + 1)").unwrap();
        assert!(snap.transcript.iter().any(|entry| {
            matches!(entry.kind, TranscriptKind::ValueResult)
                && entry.text == "Some 6 :: Option Int"
        }));
    }

    #[test]
    fn slash_use_without_arg_shows_error() {
        let mut engine = make_engine();
        let snap = engine.submit("/use").unwrap();
        let has_error = snap
            .transcript
            .iter()
            .any(|e| matches!(e.kind, TranscriptKind::Error));
        assert!(has_error);
    }

    #[test]
    fn slash_use_invalid_module_shows_error() {
        let mut engine = make_engine();
        let snap = engine.submit("/use aivi.not_a_real_module").unwrap();
        assert!(snap.transcript.iter().any(|entry| {
            matches!(entry.kind, TranscriptKind::Error)
                && entry
                    .text
                    .contains("Unknown module `aivi.not_a_real_module`")
        }));
        assert!(!engine
            .imports
            .contains(&"aivi.not_a_real_module".to_owned()));
    }

    #[test]
    fn unknown_slash_command_shows_error() {
        let mut engine = make_engine();
        let snap = engine.submit("/bogus").unwrap();
        let has_error = snap
            .transcript
            .iter()
            .any(|e| matches!(e.kind, TranscriptKind::Error));
        assert!(has_error);
    }

    #[test]
    fn slash_autorun_reports_current_state() {
        let mut engine = make_engine();
        let snap = engine.submit("/autorun").unwrap();
        assert!(snap.transcript.iter().any(|entry| {
            matches!(entry.kind, TranscriptKind::CommandOutput) && entry.text == "autorun is on"
        }));
    }

    #[test]
    fn slash_autorun_off_disables_execution() {
        let mut engine = make_engine();
        engine.submit("/autorun off").unwrap();
        let snap = engine.submit("println \"hi\"").unwrap();
        assert!(snap.transcript.iter().any(|entry| {
            matches!(entry.kind, TranscriptKind::ValueResult)
                && entry.text.starts_with("<effect> :: Effect Text")
        }));
        assert!(!snap.transcript.iter().any(|entry| entry.text == "hi"));
    }

    #[test]
    fn slash_autorun_on_reenables_execution() {
        let mut engine = make_engine();
        engine.submit("/autorun off").unwrap();
        engine.submit("/autorun on").unwrap();
        let snap = engine.submit("println \"hi\"").unwrap();
        assert!(snap.transcript.iter().any(|entry| {
            matches!(entry.kind, TranscriptKind::CommandOutput) && entry.text == "hi"
        }));
        assert!(snap.transcript.iter().any(|entry| {
            matches!(entry.kind, TranscriptKind::ValueResult)
                && entry.text.starts_with("Unit :: Effect Text")
                && entry.text.ends_with("  (autorun)")
        }));
    }

    #[test]
    fn slash_autorun_invalid_mode_shows_error() {
        let mut engine = make_engine();
        let snap = engine.submit("/autorun maybe").unwrap();
        assert!(snap.transcript.iter().any(|entry| {
            matches!(entry.kind, TranscriptKind::Error)
                && entry.text.contains("Use `/autorun on` or `/autorun off`")
        }));
    }

    #[test]
    fn slash_values_empty_session_hint() {
        let mut engine = make_engine();
        let snap = engine.submit("/values").unwrap();
        let has_hint = snap.transcript.iter().any(|e| {
            matches!(e.kind, TranscriptKind::CommandOutput)
                && e.text.contains("No values defined yet")
        });
        assert!(has_hint);
    }

    #[test]
    fn values_pane_lists_defined_values_with_types() {
        let mut engine = make_engine();
        engine.submit("x = 42").unwrap();
        let snap = engine.submit("/values").unwrap();
        // The symbol entry name should contain the binding name and its inferred type.
        // With `x = 42`, the type may be `Int` or a qualified form like `Num a => a`.
        // We only require the name "x" to appear.
        assert!(
            snap.symbols.iter().any(|entry| entry.name.contains("x")),
            "expected 'x' in symbols, got: {:?}",
            snap.symbols.iter().map(|e| &e.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn expression_submit_shows_runtime_value_and_type() {
        let mut engine = make_engine();
        let snap = engine.submit("(Some 2 |> map (_ + 2)) ?? 0").unwrap();
        assert!(snap.transcript.iter().any(|entry| {
            matches!(entry.kind, TranscriptKind::ValueResult) && entry.text == "4 :: Int"
        }));
    }

    #[test]
    fn expression_submit_resolves_overloaded_get_or_else_for_option() {
        let mut engine = make_engine();
        engine.submit("/use aivi.option").unwrap();
        let snap = engine.submit("Some 5 |> getOrElse 0").unwrap();
        assert!(snap.transcript.iter().any(|entry| {
            matches!(entry.kind, TranscriptKind::ValueResult) && entry.text == "5 :: Int"
        }));
        assert!(
            !snap
                .transcript
                .iter()
                .any(|entry| matches!(entry.kind, TranscriptKind::Error)),
            "unexpected errors: {:?}",
            snap.transcript
        );
    }

    #[test]
    fn expression_submit_resolves_overloaded_get_or_else_lazy_for_option() {
        let mut engine = make_engine();
        engine.submit("/use aivi.option").unwrap();
        let snap = engine.submit("Some 5 |> getOrElseLazy (_ => 0)").unwrap();
        assert!(snap.transcript.iter().any(|entry| {
            matches!(entry.kind, TranscriptKind::ValueResult) && entry.text == "5 :: Int"
        }));
        assert!(
            !snap
                .transcript
                .iter()
                .any(|entry| matches!(entry.kind, TranscriptKind::Error)),
            "unexpected errors: {:?}",
            snap.transcript
        );
    }

    #[test]
    fn expression_submit_resolves_overloaded_get_or_else_for_result() {
        let mut engine = make_engine();
        engine.submit("/use aivi.result").unwrap();
        let snap = engine.submit("Err \"boom\" |> getOrElse 0").unwrap();
        assert!(snap.transcript.iter().any(|entry| {
            matches!(entry.kind, TranscriptKind::ValueResult) && entry.text == "0 :: Int"
        }));
        assert!(
            !snap
                .transcript
                .iter()
                .any(|entry| matches!(entry.kind, TranscriptKind::Error)),
            "unexpected errors: {:?}",
            snap.transcript
        );
    }

    #[test]
    fn expression_submit_resolves_overloaded_get_or_else_lazy_for_result() {
        let mut engine = make_engine();
        engine.submit("/use aivi.result").unwrap();
        let snap = engine
            .submit("Err \"boom\" |> getOrElseLazy (_ => 0)")
            .unwrap();
        assert!(snap.transcript.iter().any(|entry| {
            matches!(entry.kind, TranscriptKind::ValueResult) && entry.text == "0 :: Int"
        }));
        assert!(
            !snap
                .transcript
                .iter()
                .any(|entry| matches!(entry.kind, TranscriptKind::Error)),
            "unexpected errors: {:?}",
            snap.transcript
        );
    }

    #[test]
    fn expression_submit_resolves_option_to_result() {
        let mut engine = make_engine();
        engine.submit("/use aivi.option").unwrap();
        let snap = engine.submit("Some 5 |> toResult \"err\"").unwrap();
        assert!(snap.transcript.iter().any(|entry| {
            matches!(entry.kind, TranscriptKind::ValueResult)
                && entry.text == "Ok 5 :: Result Text Int"
        }));
        assert!(
            !snap
                .transcript
                .iter()
                .any(|entry| matches!(entry.kind, TranscriptKind::Error)),
            "unexpected errors: {:?}",
            snap.transcript
        );
    }

    #[test]
    fn expression_submit_resolves_option_to_list() {
        let mut engine = make_engine();
        engine.submit("/use aivi.option").unwrap();
        let snap = engine.submit("Some 5 |> toList").unwrap();
        assert!(snap.transcript.iter().any(|entry| {
            matches!(entry.kind, TranscriptKind::ValueResult) && entry.text == "[5] :: List Int"
        }));
        assert!(
            !snap
                .transcript
                .iter()
                .any(|entry| matches!(entry.kind, TranscriptKind::Error)),
            "unexpected errors: {:?}",
            snap.transcript
        );
    }

    #[test]
    fn expression_submit_resolves_overloaded_get_or_else_for_validation() {
        let mut engine = make_engine();
        engine.submit("/use aivi.validation").unwrap();
        let snap = engine.submit("Valid 5 |> getOrElse 0").unwrap();
        assert!(snap.transcript.iter().any(|entry| {
            matches!(entry.kind, TranscriptKind::ValueResult) && entry.text == "5 :: Int"
        }));
        assert!(
            !snap
                .transcript
                .iter()
                .any(|entry| matches!(entry.kind, TranscriptKind::Error)),
            "unexpected errors: {:?}",
            snap.transcript
        );
    }

    #[test]
    fn expression_submit_resolves_path_normalize() {
        let mut engine = make_engine();
        engine.submit("/use aivi.path").unwrap();
        let snap = engine
            .submit(
                "{ absolute: False, segments: [\"a\", \"..\", \"b\"] } |> normalize |> toString",
            )
            .unwrap();
        assert!(snap.transcript.iter().any(|entry| {
            matches!(entry.kind, TranscriptKind::ValueResult) && entry.text == "b :: Text"
        }));
        assert!(
            !snap
                .transcript
                .iter()
                .any(|entry| matches!(entry.kind, TranscriptKind::Error)),
            "unexpected errors: {:?}",
            snap.transcript
        );
    }

    #[test]
    fn expression_submit_can_use_prior_definitions() {
        let mut engine = make_engine();
        engine.submit("x = 41").unwrap();
        let snap = engine.submit("x + 1").unwrap();
        assert!(snap.transcript.iter().any(|entry| {
            matches!(entry.kind, TranscriptKind::ValueResult) && entry.text == "42 :: Int"
        }));
    }

    #[test]
    fn expression_submit_peeks_derived_signal_from_session_definition() {
        let mut engine = make_engine();
        engine.submit("/use aivi.reactive").unwrap();
        engine.submit("x = signal 1").unwrap();
        engine.submit("y = x ->> _ + 1").unwrap();
        let snap = engine.submit("peek y").unwrap();
        let last = snap.transcript.last().expect("value result");
        assert_eq!(last.kind, TranscriptKind::ValueResult);
        assert_eq!(last.text, "2 :: Int");
    }

    #[test]
    fn expression_submit_signal_write_sugar_returns_unit_type() {
        let mut engine = make_engine();
        engine.submit("/use aivi.reactive").unwrap();
        engine.submit("x = signal 1").unwrap();
        let snap = engine.submit("x <<- 2").unwrap();
        let last = snap.transcript.last().expect("value result");
        assert_eq!(last.kind, TranscriptKind::ValueResult);
        assert_eq!(last.text, "Unit :: Unit");

        let peek = engine.submit("peek x").unwrap();
        let last = peek.transcript.last().expect("peek result");
        assert_eq!(last.kind, TranscriptKind::ValueResult);
        assert_eq!(last.text, "2 :: Int");
    }

    #[test]
    fn expression_submit_persists_signal_updates_for_derived_signals() {
        let mut engine = make_engine();
        engine.submit("/use aivi.reactive").unwrap();
        engine.submit("x = signal 3").unwrap();
        engine.submit("y = x ->> _ * 2").unwrap();

        let update = engine.submit("x <<- 9").unwrap();
        let last = update.transcript.last().expect("update result");
        assert_eq!(last.kind, TranscriptKind::ValueResult);
        assert_eq!(last.text, "Unit :: Unit");

        let peek = engine.submit("peek y").unwrap();
        let last = peek.transcript.last().expect("peek result");
        assert_eq!(last.kind, TranscriptKind::ValueResult);
        assert_eq!(last.text, "18 :: Int");
    }

    #[cfg(unix)]
    #[test]
    fn expression_submit_does_not_leak_debug_output_to_stderr() {
        let mut engine = make_engine();
        engine.submit("/use aivi.reactive").unwrap();
        engine.submit("x = signal \"a\"").unwrap();
        engine.submit("y = x ->> \"... {_}\"").unwrap();

        let (_, stderr) = capture_stderr(|| engine.submit("peek y").unwrap());
        assert!(
            stderr.trim().is_empty(),
            "expected empty stderr during expression submit, got {stderr:?}"
        );
    }

    #[test]
    fn expression_submit_autoruns_top_level_console_effects() {
        let mut engine = make_engine();
        let snap = engine.submit("println \"hi\"").unwrap();
        assert!(snap.transcript.iter().any(|entry| {
            matches!(entry.kind, TranscriptKind::CommandOutput) && entry.text == "hi"
        }));
        assert!(snap.transcript.iter().any(|entry| {
            matches!(entry.kind, TranscriptKind::ValueResult)
                && entry.text.starts_with("Unit :: Effect Text")
                && entry.text.ends_with("  (autorun)")
        }));
    }

    #[test]
    fn functions_pane_lists_defined_functions_with_signatures() {
        let mut engine = make_engine();
        engine
            .submit("double : Int -> Int\ndouble = n => n * 2")
            .unwrap();
        let snap = engine.submit("/functions").unwrap();
        assert!(snap
            .symbols
            .iter()
            .any(|entry| entry.name.contains("double :: Int -> Int  (repl_session)")));
        assert!(snap.transcript.iter().any(|entry| {
            matches!(entry.kind, TranscriptKind::CommandOutput)
                && entry.text.contains("double :: Int -> Int  (repl_session)")
        }));
    }

    #[test]
    fn functions_pane_shows_imported_module_names() {
        let mut engine = make_engine();
        engine.submit("/use aivi.json").unwrap();
        let snap = engine.submit("/functions decodeText").unwrap();
        assert!(snap
            .symbols
            .iter()
            .any(|entry| entry.name.contains("decodeText") && entry.name.contains("(aivi.json)")));
    }

    #[test]
    fn modules_command_reports_prelude_module_name() {
        let mut engine = make_engine();
        let snap = engine.submit("/modules").unwrap();
        assert!(snap
            .transcript
            .iter()
            .any(|entry| entry.text.contains("aivi.prelude")));
    }

    #[test]
    fn slash_history_no_entries() {
        let mut engine = make_engine();
        let snap = engine.submit("/history").unwrap();
        let has_cmd = snap
            .transcript
            .iter()
            .any(|e| matches!(e.kind, TranscriptKind::CommandOutput) && e.text.contains("Last 0"));
        assert!(has_cmd);
    }

    // ── Snapshot ────────────────────────────────────────────────────────────

    #[test]
    fn snapshot_reflects_transcript() {
        let mut engine = make_engine();
        engine.submit("/help").unwrap();
        let snap = engine.snapshot();
        assert!(!snap.transcript.is_empty());
        assert_eq!(snap.turn, 0); // /help is a command, not an eval turn
    }

    #[test]
    fn snapshot_pane_none_by_default() {
        let engine = make_engine();
        let snap = engine.snapshot();
        assert!(snap.symbol_pane.is_none());
        assert!(snap.symbols.is_empty());
    }

    #[test]
    fn toggle_pane_opens_then_closes() {
        let mut engine = make_engine();
        engine.toggle_symbol_pane();
        assert!(engine.active_pane.is_some());
        engine.toggle_symbol_pane();
        assert!(engine.active_pane.is_none());
    }

    #[test]
    fn set_pane_values_renders_empty_hint_after_cmd() {
        let mut engine = make_engine();
        // Submit /values so the engine sets the pane + shows the hint.
        let snap = engine.submit("/values").unwrap();
        assert_eq!(snap.symbol_pane, Some(SymbolPane::Values));
        // symbols list should be empty (no user definitions yet).
        assert!(snap.symbols.is_empty());
    }

    #[test]
    fn set_pane_types_has_entries() {
        let mut engine = make_engine();
        engine.set_symbol_pane(SymbolPane::Types);
        let snap = engine.snapshot();
        assert_eq!(snap.symbol_pane, Some(SymbolPane::Types));
        // Stdlib should have type entries.
        assert!(!snap.symbols.is_empty());
        assert!(snap.symbols.iter().all(|e| e.kind == SymbolKind::Type));
    }

    // ── Closest command suggestion ──────────────────────────────────────────

    #[test]
    fn closest_command_typo_hlep() {
        assert_eq!(closest_command("/hlep"), Some("/help"));
    }

    #[test]
    fn closest_command_no_match_far_off() {
        assert!(closest_command("/zzzzzzzzz").is_none());
    }

    #[test]
    fn slash_command_suggestions_show_all_for_slash() {
        let suggestions = slash_command_suggestions("/");
        assert!(suggestions.contains(&"/help"));
        assert!(suggestions.contains(&"/values"));
    }

    #[test]
    fn slash_command_suggestions_filter_by_prefix() {
        assert_eq!(slash_command_suggestions("/va"), vec!["/values"]);
    }

    #[test]
    fn command_completion_keeps_full_result_set() {
        let engine = make_engine();
        let completion = engine
            .completion_state("/", 1)
            .expect("expected completion state for slash command root");
        assert_eq!(completion.mode, CompletionMode::Command);
        assert_eq!(completion.items.len(), SLASH_COMMANDS.len());
    }

    #[test]
    fn slash_functions_filter_shows_module_name_in_labels() {
        let mut engine = make_engine();
        engine.submit("/use aivi.text").unwrap();
        let completion = engine
            .completion_state("/functions jo", "/functions jo".len())
            .expect("expected completion state for /functions filter");
        assert_eq!(completion.mode, CompletionMode::SlashFilter);
        assert!(completion
            .items
            .iter()
            .any(|item| item.label == "join (aivi.text)" && item.insert_text == "join"));
    }

    #[test]
    fn lowercase_symbol_completion_uses_default_prelude_scope() {
        let engine = make_engine();
        let completion = engine
            .completion_state("jo", 2)
            .expect("expected join suggestion from prelude-visible modules");
        assert!(completion.items.iter().any(|item| item.label == "join"));
    }

    #[test]
    fn uppercase_symbol_completion_suggests_constructors() {
        let engine = make_engine();
        let completion = engine
            .completion_state("S", 1)
            .expect("expected constructor completion for uppercase prefix");
        assert!(completion
            .items
            .iter()
            .any(|item| item.kind == CompletionKind::Constructor && item.label == "Some"));
    }

    #[test]
    fn importing_module_expands_completion_scope() {
        let mut engine = make_engine();
        let before = engine
            .completion_state("decodeT", "decodeT".len())
            .map(|state| {
                state
                    .items
                    .into_iter()
                    .map(|item| item.label)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        assert!(!before.iter().any(|label| label == "decodeText"));

        engine.submit("/use aivi.json").unwrap();
        let after = engine
            .completion_state("decodeT", "decodeT".len())
            .expect("expected completions after /use aivi.json");
        assert!(after.items.iter().any(|item| item.label == "decodeText"));
    }

    // ── /openapi command ────────────────────────────────────────────────────

    #[test]
    fn openapi_no_kind_shows_error() {
        let mut engine = make_engine();
        let snap = engine.submit("/openapi").unwrap();
        let has_error = snap
            .transcript
            .iter()
            .any(|e| matches!(e.kind, TranscriptKind::Error));
        assert!(has_error);
    }

    #[test]
    fn openapi_unknown_kind_shows_error() {
        let mut engine = make_engine();
        let snap = engine.submit("/openapi grpc some.proto").unwrap();
        let has_error = snap
            .transcript
            .iter()
            .any(|e| matches!(e.kind, TranscriptKind::Error) && e.text.contains("`file` or `url`"));
        assert!(has_error, "expected error mentioning 'file' or 'url'");
    }

    #[test]
    fn openapi_file_missing_source_shows_error() {
        let mut engine = make_engine();
        let snap = engine.submit("/openapi file").unwrap();
        let has_error = snap
            .transcript
            .iter()
            .any(|e| matches!(e.kind, TranscriptKind::Error));
        assert!(has_error);
    }

    #[test]
    fn openapi_url_missing_source_shows_error() {
        let mut engine = make_engine();
        let snap = engine.submit("/openapi url").unwrap();
        let has_error = snap
            .transcript
            .iter()
            .any(|e| matches!(e.kind, TranscriptKind::Error));
        assert!(has_error);
    }

    #[test]
    fn openapi_file_nonexistent_shows_error() {
        let mut engine = make_engine();
        let snap = engine
            .submit("/openapi file /nonexistent/path/does_not_exist.json as myApi")
            .unwrap();
        let has_error = snap
            .transcript
            .iter()
            .any(|e| matches!(e.kind, TranscriptKind::Error) && e.text.contains("not found"));
        assert!(has_error, "expected 'not found' error for missing file");
    }

    #[test]
    fn openapi_url_invalid_format_shows_error() {
        let mut engine = make_engine();
        let snap = engine
            .submit("/openapi url not-a-url/spec.json as myApi")
            .unwrap();
        let has_error = snap
            .transcript
            .iter()
            .any(|e| matches!(e.kind, TranscriptKind::Error) && e.text.contains("Invalid URL"));
        assert!(has_error, "expected 'Invalid URL' error");
    }

    #[test]
    fn openapi_file_success_injects_static_definition() {
        // Use the petstore.json from the integration-tests directory.
        let petstore = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../integration-tests/syntax/decorators/petstore.json"
        );
        if !std::path::Path::new(petstore).exists() {
            // Skip if the file is not reachable from this test's working directory.
            return;
        }
        let mut engine = make_engine();
        let snap = engine
            .submit(&format!("/openapi file {petstore} as petStoreApi"))
            .unwrap();

        // Should not have any error.
        let has_error = snap
            .transcript
            .iter()
            .any(|e| matches!(e.kind, TranscriptKind::Error));
        assert!(!has_error, "unexpected error: {:?}", snap.transcript);

        // A success system message should appear.
        let has_success = snap
            .transcript
            .iter()
            .any(|e| matches!(e.kind, TranscriptKind::System) && e.text.contains("petStoreApi"));
        assert!(
            has_success,
            "expected success message mentioning petStoreApi"
        );

        // The @static snippet should be in definitions.
        assert!(
            engine
                .definitions
                .iter()
                .any(|d| d.contains("petStoreApi") && d.contains("openapi.fromFile")),
            "expected @static snippet in definitions"
        );
    }

    #[test]
    fn openapi_file_success_symbols_visible() {
        let petstore = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../integration-tests/syntax/decorators/petstore.json"
        );
        if !std::path::Path::new(petstore).exists() {
            return;
        }
        let mut engine = make_engine();
        engine
            .submit(&format!("/openapi file {petstore} as petStoreApi"))
            .unwrap();

        // After injecting, /values should list petStoreApi.
        let snap = engine.submit("/values").unwrap();
        let lists_binding = snap.symbols.iter().any(|e| e.name.contains("petStoreApi"));
        assert!(
            lists_binding,
            "expected petStoreApi in /values after /openapi file"
        );
    }

    #[test]
    fn openapi_file_derives_binding_name_when_omitted() {
        let petstore = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../integration-tests/syntax/decorators/petstore.json"
        );
        if !std::path::Path::new(petstore).exists() {
            return;
        }
        let mut engine = make_engine();
        // No `as <name>` — should auto-derive "petstore" from the filename.
        let snap = engine.submit(&format!("/openapi file {petstore}")).unwrap();
        let has_success = snap
            .transcript
            .iter()
            .any(|e| matches!(e.kind, TranscriptKind::System) && e.text.contains("petstore"));
        assert!(
            has_success,
            "expected auto-derived name 'petstore' in message"
        );
    }

    // ── parse_openapi_args ──────────────────────────────────────────────────

    #[test]
    fn parse_openapi_args_with_as_clause() {
        let (source, alias) = parse_openapi_args("./petstore.json as petApi");
        assert_eq!(source, "./petstore.json");
        assert_eq!(alias, Some("petApi".to_owned()));
    }

    #[test]
    fn parse_openapi_args_without_as_clause() {
        let (source, alias) = parse_openapi_args("./petstore.yaml");
        assert_eq!(source, "./petstore.yaml");
        assert!(alias.is_none());
    }

    #[test]
    fn parse_openapi_args_url_with_as_clause() {
        let (source, alias) =
            parse_openapi_args("https://api.example.com/v1/openapi.json as exampleApi");
        assert_eq!(source, "https://api.example.com/v1/openapi.json");
        assert_eq!(alias, Some("exampleApi".to_owned()));
    }

    // ── Derive module name ──────────────────────────────────────────────────

    #[test]
    fn derive_module_name_yaml() {
        assert_eq!(derive_module_name("specs/petstore.yaml"), "petstore");
    }

    #[test]
    fn derive_module_name_url() {
        assert_eq!(
            derive_module_name("https://api.example.com/openapi.json"),
            "openapi"
        );
    }

    // ── Format helpers ──────────────────────────────────────────────────────

    #[test]
    fn format_function_placeholder_round_trips() {
        let s = format_function_placeholder("Int -> Int");
        assert!(s.contains("function"));
        assert!(s.contains("Int -> Int"));
    }

    #[test]
    fn format_opaque_placeholder_round_trips() {
        let s = format_opaque_placeholder("MyType");
        assert!(s.contains("value"));
        assert!(s.contains("MyType"));
    }

    // ── History increments turn ─────────────────────────────────────────────

    #[test]
    fn turn_increments_on_non_command_submit() {
        let mut engine = make_engine();
        assert_eq!(engine.turn, 0);
        // We submit an expression that will likely fail to typecheck, but turn should still
        // increment.
        let _ = engine.submit("1 + 2");
        assert_eq!(engine.turn, 1);
    }

    #[test]
    fn turn_does_not_increment_on_slash_command() {
        let mut engine = make_engine();
        engine.submit("/help").unwrap();
        assert_eq!(engine.turn, 0);
    }
}
