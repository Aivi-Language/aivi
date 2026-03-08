//! REPL session engine: state management, input classification, evaluation, and snapshots.
//!
//! This module is the non-UI foundation of `aivi repl`. The TUI layer (tui.rs) consumes
//! `ReplEngine` via `snapshot()` and `submit()`. A plain-text fallback is available via
//! `run_plain()`.

use std::io::{self, BufRead, IsTerminal, Write};
use std::path::Path;

use aivi::{
    check_modules, check_types, desugar_modules, embedded_stdlib_modules, evaluate_binding_jit,
    file_diagnostics_have_errors, infer_value_types_full, parse_modules, render_diagnostics,
    AiviError, FileDiagnostic, Module, ModuleItem,
};

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

#[derive(Debug, Clone)]
pub(crate) struct SymbolEntry {
    pub(crate) kind: SymbolKind,
    pub(crate) name: String,
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
    /// Inferred type strings for session-defined names. Updated after every successful compile.
    session_types: std::collections::HashMap<String, String>,
}

impl ReplEngine {
    /// Create a new engine, pre-loading the stdlib symbol inventory.
    pub(crate) fn new(options: &ReplOptions) -> Result<Self, AiviError> {
        let mut engine = ReplEngine {
            options: options.clone(),
            imports: Vec::new(),
            definitions: Vec::new(),
            history: Vec::new(),
            transcript: Vec::new(),
            active_pane: None,
            cached_symbols: Vec::new(),
            turn: 0,
            session_types: std::collections::HashMap::new(),
        };

        let stdlib = embedded_stdlib_modules();
        let stdlib_count: usize = stdlib.iter().map(count_exportable_items).sum();

        engine.transcript.push(TranscriptEntry {
            kind: TranscriptKind::System,
            text: format!(
                "aivi repl 0.1  ·  prelude loaded  ·  {} symbols in scope",
                stdlib_count
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
  /use <module.path>            add import to session
  /types [filter]               types in scope (stdlib + session)
  /values [filter]              session-defined values + inferred types
  /functions [filter]           functions in scope (stdlib + session)
  /modules                      show loaded modules in session
  /clear                        clear transcript (keep session state)
  /reset                        clear transcript + session state
  /history [n]                  show last n inputs (default: 20)
  /load <path>                  load .aivi file into session
  /openapi file <path> [as <n>] inject OpenAPI spec file as module
  /openapi url <url> [as <n>]   inject OpenAPI spec URL as module

  Ctrl+D on empty input: exit   Tab: toggle symbol pane";
        self.push_command_output(text.to_owned());
        self.set_symbol_pane(SymbolPane::Types);
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
        self.turn = 0;
        self.cached_symbols.clear();
        self.active_pane = None;
        self.transcript.clear();
        self.transcript.push(TranscriptEntry {
            kind: TranscriptKind::System,
            text: "session reset".to_owned(),
        });
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
        if !self.imports.contains(&module_path.to_owned()) {
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
                let (mut parsed_modules, mut all_diags) = parse_modules(path, &content);
                let mut modules = embedded_stdlib_modules();
                modules.append(&mut parsed_modules);
                all_diags.extend(check_modules(&modules));
                all_diags.extend(check_types(&modules));
                let file_diags: Vec<_> = all_diags
                    .into_iter()
                    .filter(|d| d.path == path_str)
                    .collect();
                if file_diagnostics_have_errors(&file_diags) {
                    self.push_diagnostics_as_error(path_str, &file_diags);
                    return;
                }
                self.definitions.push(content.clone());
                self.session_types = self.current_session_types().into_iter().collect();
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
                let (mut session_modules, mut parse_diags) = parse_modules(path, &module_source);
                let mut all_modules = embedded_stdlib_modules();
                all_modules.append(&mut session_modules);

                let resolver_diags = check_modules(&all_modules);
                parse_diags.extend(
                    resolver_diags
                        .into_iter()
                        .filter(|d| d.path == "<repl_session>"),
                );

                if file_diagnostics_have_errors(&parse_diags) {
                    self.push_diagnostics_as_error("<repl_session>", &parse_diags);
                    return;
                }

                let type_diags: Vec<FileDiagnostic> = check_types(&all_modules)
                    .into_iter()
                    .filter(|d| d.path == "<repl_session>")
                    .collect();

                if file_diagnostics_have_errors(&type_diags) {
                    self.push_diagnostics_as_error("<repl_session>", &type_diags);
                    return;
                }

                // Inject the @static snippet and register the binding in the symbol inventory.
                // Use the inferred type from this compilation (mirrors what eval_input does for
                // definitions), with "?" as a fallback if type inference can't resolve it.
                let infer = infer_value_types_full(&all_modules);
                let binding_type = infer
                    .type_strings
                    .get("repl_session")
                    .and_then(|types| types.get(binding_name.as_str()))
                    .cloned()
                    .unwrap_or_else(|| "?".to_owned());

                self.definitions.push(snippet);
                self.session_types
                    .insert(binding_name.clone(), binding_type);
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
        let (mut session_modules, mut parse_diags) = parse_modules(path, &source);

        // Prepend stdlib for type-checking.
        let stdlib = embedded_stdlib_modules();
        let mut all_modules: Vec<Module> = stdlib.clone();
        all_modules.append(&mut session_modules);

        // Filter diagnostics to only those from the session module.
        let resolver_diags = check_modules(&all_modules);
        parse_diags.extend(
            resolver_diags
                .into_iter()
                .filter(|d| d.path == "<repl_session>"),
        );

        if file_diagnostics_have_errors(&parse_diags) {
            self.push_diagnostics_as_error("<repl_session>", &parse_diags);
            return;
        }

        let type_diags: Vec<FileDiagnostic> = check_types(&all_modules)
            .into_iter()
            .filter(|d| d.path == "<repl_session>")
            .collect();

        if file_diagnostics_have_errors(&type_diags) {
            self.push_diagnostics_as_error("<repl_session>", &type_diags);
            return;
        }

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
                self.definitions.push(input.to_owned());

                let defined_names = extract_defined_names(input);
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
                match evaluate_binding_jit(
                    program,
                    infer.cg_types,
                    infer.monomorph_plan,
                    infer.source_schemas,
                    &all_modules,
                    "_replResult",
                ) {
                    Ok(value_text) => self.transcript.push(TranscriptEntry {
                        kind: TranscriptKind::ValueResult,
                        text: format!("{value_text} :: {result_type}"),
                    }),
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
        lines.push(String::new());
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
        let stdlib = embedded_stdlib_modules();
        let session_types: Vec<(String, String)> = self
            .session_types
            .iter()
            .map(|(name, ty)| (name.clone(), ty.clone()))
            .collect();
        match pane {
            SymbolPane::Types => {
                let mut names: Vec<String> = Vec::new();
                for module in &stdlib {
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
                    .filter(|n| filter.is_empty() || n.starts_with(filter))
                    .map(|name| SymbolEntry {
                        kind: SymbolKind::Type,
                        name,
                    })
                    .collect();
                for (name, type_str) in session_types {
                    if filter.is_empty() || name.starts_with(filter) {
                        entries.push(SymbolEntry {
                            kind: SymbolKind::Type,
                            name: format!("{name} :: {type_str}"),
                        });
                    }
                }
                entries
            }
            SymbolPane::Functions => {
                let mut names: Vec<String> = Vec::new();
                for module in &stdlib {
                    for item in &module.items {
                        if let ModuleItem::TypeSig(sig) = item {
                            let n = sig.name.name.clone();
                            if !names.contains(&n) {
                                names.push(n);
                            }
                        }
                    }
                }
                names.sort();
                let mut entries: Vec<SymbolEntry> = names
                    .into_iter()
                    .filter(|n| filter.is_empty() || n.starts_with(filter))
                    .map(|name| SymbolEntry {
                        kind: SymbolKind::Function,
                        name,
                    })
                    .collect();
                for (name, type_str) in session_types {
                    if type_str.contains("->") && (filter.is_empty() || name.starts_with(filter)) {
                        entries.push(SymbolEntry {
                            kind: SymbolKind::Function,
                            name: format!("{name} :: {type_str}"),
                        });
                    }
                }
                entries
            }
            SymbolPane::Values => {
                let mut entries: Vec<SymbolEntry> = session_types
                    .into_iter()
                    .filter(|(name, _)| filter.is_empty() || name.starts_with(filter))
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
                    if filter.is_empty() || imp.starts_with(filter) {
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

        let mut lines = vec!["module repl_session".to_owned(), String::new()];
        lines.push("use aivi.prelude".to_owned());
        for imp in &self.imports {
            lines.push(format!("use {imp}"));
        }
        lines.push(String::new());
        for def in &self.definitions {
            lines.push(def.clone());
            lines.push(String::new());
        }
        let source = lines.join("\n");
        let path = Path::new("<repl_session>");
        let (mut session_modules, parse_diags) = parse_modules(path, &source);
        if file_diagnostics_have_errors(&parse_diags) {
            return Vec::new();
        }

        let mut all_modules = embedded_stdlib_modules();
        all_modules.append(&mut session_modules);
        let resolver_diags = check_modules(&all_modules);
        if file_diagnostics_have_errors(&resolver_diags) {
            return Vec::new();
        }
        let type_diags = check_types(&all_modules);
        if file_diagnostics_have_errors(&type_diags) {
            return Vec::new();
        }

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

    fn use_color(&self) -> bool {
        match self.options.color_mode {
            ColorMode::Auto => io::stdout().is_terminal(),
            ColorMode::Always => true,
            ColorMode::Never => false,
        }
    }
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
    "/use",
    "/types",
    "/values",
    "/functions",
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
        "/use" | "/types" | "/values" | "/functions" | "/history" | "/load" | "/openapi"
    )
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

/// Format an opaque/complex value for the `/values` pane.
#[allow(dead_code)]
pub(crate) fn format_opaque_placeholder(type_str: &str) -> String {
    format!("<value :: {type_str}>")
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engine() -> ReplEngine {
        let opts = ReplOptions {
            color_mode: ColorMode::Never,
            plain_mode: false,
        };
        ReplEngine::new(&opts).expect("engine creation failed")
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
    fn expression_submit_can_use_prior_definitions() {
        let mut engine = make_engine();
        engine.submit("x = 41").unwrap();
        let snap = engine.submit("x + 1").unwrap();
        assert!(snap.transcript.iter().any(|entry| {
            matches!(entry.kind, TranscriptKind::ValueResult) && entry.text == "42 :: Int"
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
            .any(|entry| entry.name.contains("double :: Int -> Int")));
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
