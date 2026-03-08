//! Full-screen TUI REPL shell built with ratatui + crossterm.
//!
//! Entry point: `run(engine, options)`.  The caller constructs a `ReplEngine`
//! and passes it here.
//!
//! Interface contract (implemented by sibling modules):
//!   - `super::{ReplOptions, ColorMode, SymbolPane}`
//!   - `super::engine::{ReplEngine, ReplSnapshot, TranscriptEntry, TranscriptKind, SymbolEntry, SymbolKind}`

use std::io::{self, IsTerminal, Stdout};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};

use aivi::AiviError;

use super::engine::{
    command_accepts_argument, slash_command_suggestions, ReplEngine, ReplSnapshot, SymbolEntry,
    SymbolKind, TranscriptEntry, TranscriptKind,
};
use super::{ColorMode, ReplOptions, SymbolPane};

// ─── Colour palette ──────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub(crate) struct Palette {
    pub use_color: bool,
}

impl Palette {
    pub(crate) fn new(mode: ColorMode, is_tty: bool) -> Self {
        let use_color = match mode {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => {
                is_tty
                    && std::env::var("NO_COLOR").is_err()
                    && std::env::var("TERM").map_or(true, |t| t != "dumb")
            }
        };
        Self { use_color }
    }

    fn prompt(self) -> Style {
        if self.use_color {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        }
    }

    fn input_text(self) -> Style {
        if self.use_color {
            Style::default().fg(Color::White)
        } else {
            Style::default()
        }
    }

    fn result_value(self) -> Style {
        if self.use_color {
            Style::default().fg(Color::LightGreen)
        } else {
            Style::default()
        }
    }

    fn type_annotation(self) -> Style {
        if self.use_color {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::DIM)
        } else {
            Style::default()
        }
    }

    fn defined_badge(self) -> Style {
        if self.use_color {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::DIM)
        } else {
            Style::default()
        }
    }

    fn error_prefix(self) -> Style {
        if self.use_color {
            Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        }
    }

    fn error_body(self) -> Style {
        if self.use_color {
            Style::default().fg(Color::Red)
        } else {
            Style::default()
        }
    }

    fn warning_prefix(self) -> Style {
        if self.use_color {
            Style::default()
                .fg(Color::LightYellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        }
    }

    fn system_message(self) -> Style {
        if self.use_color {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::DIM)
        } else {
            Style::default()
        }
    }

    fn command_output(self) -> Style {
        if self.use_color {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        }
    }

    fn symbol_badge_type(self) -> Style {
        if self.use_color {
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::DIM)
        } else {
            Style::default()
        }
    }

    fn symbol_badge_fn(self) -> Style {
        if self.use_color {
            Style::default().fg(Color::Blue).add_modifier(Modifier::DIM)
        } else {
            Style::default()
        }
    }

    fn symbol_badge_val(self) -> Style {
        if self.use_color {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::DIM)
        } else {
            Style::default()
        }
    }

    fn border(self) -> Style {
        if self.use_color {
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM)
        } else {
            Style::default()
        }
    }

    fn status_bar(self) -> Style {
        if self.use_color {
            Style::default()
                .fg(Color::DarkGray)
                .bg(Color::Black)
                .add_modifier(Modifier::DIM)
        } else {
            Style::default().add_modifier(Modifier::REVERSED)
        }
    }

    fn hint(self) -> Style {
        if self.use_color {
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM)
        } else {
            Style::default()
        }
    }

    fn suggestion(self) -> Style {
        if self.use_color {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        }
    }

    fn selected_suggestion(self) -> Style {
        self.suggestion()
            .add_modifier(Modifier::REVERSED | Modifier::BOLD)
    }

    /// Box-drawing characters: Unicode when colour is enabled, ASCII when degraded.
    #[allow(dead_code)]
    pub(crate) fn box_chars(self) -> BoxChars {
        if self.use_color {
            BoxChars {
                horizontal: "─",
                vertical: "│",
                top_left: "┌",
                top_right: "┐",
                bottom_left: "└",
                bottom_right: "┘",
            }
        } else {
            BoxChars {
                horizontal: "-",
                vertical: "|",
                top_left: "+",
                top_right: "+",
                bottom_left: "+",
                bottom_right: "+",
            }
        }
    }

    fn prompt_char(self) -> &'static str {
        if self.use_color {
            "❯"
        } else {
            ">"
        }
    }
}

#[allow(dead_code)]
pub(crate) struct BoxChars {
    pub horizontal: &'static str,
    pub vertical: &'static str,
    pub top_left: &'static str,
    pub top_right: &'static str,
    pub bottom_left: &'static str,
    pub bottom_right: &'static str,
}

// ─── TUI application state ────────────────────────────────────────────────────

struct TuiState {
    /// Current text in the input buffer (may span multiple lines via '\n').
    input: String,
    /// Cursor byte offset within `input`.
    cursor: usize,
    /// How many transcript rows we've scrolled up from the bottom.
    scroll_offset: usize,
    /// Submitted input history, oldest first.
    history: Vec<String>,
    history_idx: Option<usize>,
    /// Saved input while browsing history.
    history_saved: String,
    quit: bool,
    snapshot: ReplSnapshot,
    palette: Palette,
    suggestion_index: usize,
}

impl TuiState {
    fn new(snapshot: ReplSnapshot, palette: Palette) -> Self {
        Self {
            input: String::new(),
            cursor: 0,
            scroll_offset: 0,
            history: Vec::new(),
            history_idx: None,
            history_saved: String::new(),
            quit: false,
            snapshot,
            palette,
            suggestion_index: 0,
        }
    }

    fn input_is_empty(&self) -> bool {
        self.input.trim().is_empty()
    }

    fn push_char(&mut self, c: char) {
        self.input.insert(self.cursor, c);
        self.cursor += c.len_utf8();
        self.scroll_offset = 0;
        self.suggestion_index = 0;
    }

    fn delete_back(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let prev = self.input[..self.cursor]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.input.drain(prev..self.cursor);
        self.cursor = prev;
        self.suggestion_index = 0;
    }

    fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        if self.history_idx.is_none() {
            self.history_saved = self.input.clone();
        }
        let next = match self.history_idx {
            None => self.history.len().saturating_sub(1),
            Some(0) => 0,
            Some(i) => i - 1,
        };
        self.history_idx = Some(next);
        self.input = self.history[next].clone();
        self.cursor = self.input.len();
        self.suggestion_index = 0;
    }

    fn history_down(&mut self) {
        let Some(idx) = self.history_idx else { return };
        if idx + 1 >= self.history.len() {
            self.history_idx = None;
            self.input = self.history_saved.clone();
        } else {
            self.history_idx = Some(idx + 1);
            self.input = self.history[idx + 1].clone();
        }
        self.cursor = self.input.len();
        self.suggestion_index = 0;
    }

    fn record_history(&mut self, input: String) {
        if !input.trim().is_empty()
            && self.history.last().map(String::as_str) != Some(input.as_str())
        {
            self.history.push(input);
        }
        self.history_idx = None;
        self.history_saved.clear();
    }

    fn command_suggestions(&self) -> Vec<&'static str> {
        visible_command_suggestions(&self.input)
    }

    fn has_command_suggestions(&self) -> bool {
        !self.command_suggestions().is_empty()
    }

    fn selected_suggestion(&self) -> Option<&'static str> {
        let suggestions = self.command_suggestions();
        if suggestions.is_empty() {
            None
        } else {
            Some(suggestions[self.suggestion_index.min(suggestions.len() - 1)])
        }
    }

    fn suggestion_up(&mut self) {
        let suggestions = self.command_suggestions();
        if suggestions.is_empty() {
            return;
        }
        self.suggestion_index = self
            .suggestion_index
            .checked_sub(1)
            .unwrap_or(suggestions.len() - 1);
    }

    fn suggestion_down(&mut self) {
        let suggestions = self.command_suggestions();
        if suggestions.is_empty() {
            return;
        }
        self.suggestion_index = (self.suggestion_index + 1) % suggestions.len();
    }

    fn accept_command_suggestion(&mut self) -> bool {
        let Some(suggestion) = self.selected_suggestion() else {
            return false;
        };
        self.input = suggestion.to_owned();
        if command_accepts_argument(suggestion) {
            self.input.push(' ');
        }
        self.cursor = self.input.len();
        self.suggestion_index = 0;
        true
    }
}

// ─── Plain (non-TTY) fallback ─────────────────────────────────────────────────

/// Minimal read-eval-print loop for non-TTY contexts (piped input, CI, etc.).
pub(crate) fn run_plain(mut engine: ReplEngine, options: &ReplOptions) -> Result<(), AiviError> {
    use std::io::BufRead;
    let palette = Palette::new(options.color_mode, false);
    let prompt = format!("{} ", palette.prompt_char());
    print!("{prompt}");
    let _ = io::Write::flush(&mut io::stdout());
    for line in io::stdin().lock().lines() {
        let input = line.map_err(AiviError::Io)?;
        if matches!(input.trim(), ":q" | "/quit") {
            break;
        }
        let snap = engine.submit(&input)?;
        for entry in &snap.transcript {
            println!("{}", entry_to_plain(entry));
        }
        print!("{prompt}");
        let _ = io::Write::flush(&mut io::stdout());
    }
    Ok(())
}

fn entry_to_plain(entry: &TranscriptEntry) -> String {
    match entry.kind {
        TranscriptKind::Input => format!("> {}", entry.text),
        TranscriptKind::Error => format!("  ✖ {}", entry.text),
        TranscriptKind::Warning => format!("  ⚠ {}", entry.text),
        TranscriptKind::System => format!("  ✓ {}", entry.text),
        _ => format!("  {}", entry.text),
    }
}

// ─── TUI entry point ─────────────────────────────────────────────────────────

/// Runs the full-screen TUI REPL.  Restores the terminal unconditionally on
/// return (both success and error paths).
pub(crate) fn run(mut engine: ReplEngine, options: &ReplOptions) -> Result<(), AiviError> {
    let is_tty = io::stdout().is_terminal();
    let palette = Palette::new(options.color_mode, is_tty);

    if !is_tty {
        return run_plain(engine, options);
    }

    let snapshot = engine.snapshot();

    enable_raw_mode().map_err(AiviError::Io)?;
    let mut stdout = io::stdout();
    // Do not enable mouse capture: terminals need normal mouse behavior so users can
    // select and copy transcript text from the REPL.
    execute!(stdout, EnterAlternateScreen).map_err(AiviError::Io)?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend).map_err(AiviError::Io)?;
    let mut state = TuiState::new(snapshot, palette);

    let result = event_loop(&mut terminal, &mut engine, &mut state);

    // Always restore — even if event_loop returns an error.
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    result
}

// ─── Main event loop ──────────────────────────────────────────────────────────

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    engine: &mut ReplEngine,
    state: &mut TuiState,
) -> Result<(), AiviError> {
    loop {
        terminal
            .draw(|frame| render(frame, state))
            .map_err(AiviError::Io)?;

        if state.quit {
            break;
        }

        match event::read().map_err(AiviError::Io)? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                handle_key(key.code, key.modifiers, engine, state)?;
            }
            Event::Resize(_, _) => {
                // Re-draw happens automatically at the top of the loop.
            }
            _ => {}
        }
    }
    Ok(())
}

// ─── Key handling ─────────────────────────────────────────────────────────────

fn handle_key(
    code: KeyCode,
    modifiers: KeyModifiers,
    engine: &mut ReplEngine,
    state: &mut TuiState,
) -> Result<(), AiviError> {
    let ctrl = modifiers.contains(KeyModifiers::CONTROL);
    let shift = modifiers.contains(KeyModifiers::SHIFT);

    match code {
        // Exit on Ctrl+D when input is empty.
        KeyCode::Char('d') if ctrl => {
            if state.input_is_empty() {
                state.quit = true;
            }
        }

        // Cancel current input line (keep session state).
        KeyCode::Char('c') if ctrl => {
            state.input.clear();
            state.cursor = 0;
            state.history_idx = None;
            state.suggestion_index = 0;
        }

        // Clear transcript display (session state survives).
        KeyCode::Char('l') if ctrl => {
            engine.clear_transcript();
            state.snapshot = engine.snapshot();
            state.scroll_offset = 0;
        }

        // Submit input.
        KeyCode::Enter if !shift => {
            let input = std::mem::take(&mut state.input);
            state.cursor = 0;
            state.scroll_offset = 0;
            state.suggestion_index = 0;
            state.record_history(input.clone());
            state.snapshot = engine.submit(&input)?;
        }

        // Shift+Enter → insert literal newline (multi-line mode).
        KeyCode::Enter if shift => {
            state.push_char('\n');
        }

        // History navigation or command suggestion selection.
        KeyCode::Up => {
            if state.has_command_suggestions() {
                state.suggestion_up();
            } else {
                state.history_up();
            }
        }
        KeyCode::Down => {
            if state.has_command_suggestions() {
                state.suggestion_down();
            } else {
                state.history_down();
            }
        }

        // Transcript scrolling.
        KeyCode::PageUp => {
            state.scroll_offset = state.scroll_offset.saturating_add(5);
        }
        KeyCode::PageDown => {
            state.scroll_offset = state.scroll_offset.saturating_sub(5);
        }

        // Accept a command suggestion, otherwise toggle the symbol pane.
        KeyCode::Tab => {
            if !state.accept_command_suggestion() {
                engine.toggle_symbol_pane();
                state.snapshot = engine.snapshot();
            }
        }
        KeyCode::Esc => {
            engine.close_symbol_pane();
            state.snapshot = engine.snapshot();
        }

        // Text editing.
        KeyCode::Char(c) => state.push_char(c),
        KeyCode::Backspace => state.delete_back(),

        KeyCode::Left => {
            if state.cursor > 0 {
                let prev = state.input[..state.cursor]
                    .char_indices()
                    .next_back()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                state.cursor = prev;
            }
        }
        KeyCode::Right => {
            if state.cursor < state.input.len() {
                let step = state.input[state.cursor..]
                    .chars()
                    .next()
                    .map_or(0, char::len_utf8);
                state.cursor += step;
            }
        }
        KeyCode::Home => {
            state.cursor = state.input[..state.cursor]
                .rfind('\n')
                .map(|i| i + 1)
                .unwrap_or(0);
        }
        KeyCode::End => {
            state.cursor = state.input[state.cursor..]
                .find('\n')
                .map(|i| state.cursor + i)
                .unwrap_or(state.input.len());
        }

        _ => {}
    }
    Ok(())
}

// ─── Rendering ────────────────────────────────────────────────────────────────

fn render(frame: &mut Frame, state: &TuiState) {
    let snap = &state.snapshot;
    let palette = state.palette;
    let show_pane = snap.symbol_pane.is_some();

    // Vertical split: main area / status bar / input area.
    let suggestion_count = visible_command_suggestions(&state.input).len() as u16;
    let input_height = (count_input_lines(&state.input) as u16).clamp(1, 5) + suggestion_count + 2;
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(input_height),
        ])
        .split(frame.area());

    let (transcript_rect, pane_rect) = if show_pane {
        let h = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(outer[0]);
        (h[0], Some(h[1]))
    } else {
        (outer[0], None)
    };

    render_transcript(frame, transcript_rect, snap, state, palette);
    if let Some(area) = pane_rect {
        render_symbol_pane(frame, area, snap, palette);
    }
    render_status_bar(frame, outer[1], snap, palette);
    render_input(frame, outer[2], state, palette);
}

fn render_transcript(
    frame: &mut Frame,
    area: Rect,
    snap: &ReplSnapshot,
    state: &TuiState,
    palette: Palette,
) {
    let all_lines: Vec<ListItem> = snap
        .transcript
        .iter()
        .flat_map(|entry| transcript_entry_to_lines(entry, palette))
        .map(ListItem::new)
        .collect();

    let total = all_lines.len() as u16;
    let visible = area.height.saturating_sub(2); // account for borders
    let max_scroll = total.saturating_sub(visible) as usize;
    let effective = state.scroll_offset.min(max_scroll);
    let start = (total as usize).saturating_sub(visible as usize + effective);

    let visible_items: Vec<ListItem> = all_lines.into_iter().skip(start).collect();

    let block = Block::default()
        .title(" AIVI REPL ")
        .borders(Borders::ALL)
        .border_style(palette.border());

    frame.render_widget(List::new(visible_items).block(block), area);
}

fn render_symbol_pane(frame: &mut Frame, area: Rect, snap: &ReplSnapshot, palette: Palette) {
    let title = match snap.symbol_pane {
        Some(SymbolPane::Types) => " /types ",
        Some(SymbolPane::Values) => " /values ",
        Some(SymbolPane::Functions) => " /functions ",
        Some(SymbolPane::Modules) => " /modules ",
        None => "",
    };

    let items: Vec<ListItem> = if snap.symbols.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  (empty)",
            palette.hint(),
        )))]
    } else {
        snap.symbols
            .iter()
            .map(|s| symbol_entry_to_item(s, palette))
            .collect()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(palette.border());

    frame.render_widget(List::new(items).block(block), area);
}

fn render_status_bar(frame: &mut Frame, area: Rect, snap: &ReplSnapshot, palette: Palette) {
    let pane_hint = if snap.symbol_pane.is_some() {
        "[Esc: close pane]"
    } else {
        "[Tab: symbols]"
    };
    let text = format!(
        "  session:{}  module:repl_session  {}  [Ctrl+D: exit]  [Ctrl+L: clear]",
        snap.turn, pane_hint,
    );
    frame.render_widget(Paragraph::new(text).style(palette.status_bar()), area);
}

fn render_input(frame: &mut Frame, area: Rect, state: &TuiState, palette: Palette) {
    let prompt_char = palette.prompt_char();
    let before = &state.input[..state.cursor];
    let cursor_ch = state.input[state.cursor..].chars().next();
    let cursor_str = cursor_ch.map_or(" ".to_string(), |c| c.to_string());
    let after_start = state.cursor + cursor_ch.map_or(0, char::len_utf8);
    let after = &state.input[after_start.min(state.input.len())..];

    let mut lines = vec![Line::from(vec![
        Span::styled(format!("{prompt_char} "), palette.prompt()),
        Span::styled(before.to_string(), palette.input_text()),
        Span::styled(
            cursor_str,
            Style::default().add_modifier(Modifier::REVERSED),
        ),
        Span::styled(after.to_string(), palette.input_text()),
    ])];

    let suggestions = visible_command_suggestions(&state.input);
    let selected_index = state
        .suggestion_index
        .min(suggestions.len().saturating_sub(1));
    for (idx, suggestion) in suggestions.iter().enumerate() {
        let style = if idx == selected_index {
            palette.selected_suggestion()
        } else {
            palette.suggestion()
        };
        lines.push(Line::from(vec![
            Span::styled("  ", palette.hint()),
            Span::styled(*suggestion, style),
            Span::styled(
                if idx == 0 { "  [Tab: accept]" } else { "" },
                palette.hint(),
            ),
        ]));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(palette.border());
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
}

// ─── Transcript entry → styled lines ─────────────────────────────────────────

fn transcript_entry_to_lines(entry: &TranscriptEntry, palette: Palette) -> Vec<Line<'static>> {
    let text = entry.text.clone();
    match entry.kind {
        TranscriptKind::Input => {
            vec![Line::from(vec![
                Span::styled(format!("{} ", palette.prompt_char()), palette.prompt()),
                Span::styled(text, palette.input_text()),
            ])]
        }
        TranscriptKind::ValueResult => {
            if let Some((val, ty)) = text.split_once(" :: ") {
                vec![Line::from(vec![
                    Span::raw("  "),
                    Span::styled(val.to_string(), palette.result_value()),
                    Span::raw(" :: "),
                    Span::styled(ty.to_string(), palette.type_annotation()),
                ])]
            } else {
                vec![Line::from(vec![
                    Span::raw("  "),
                    Span::styled(text, palette.result_value()),
                ])]
            }
        }
        TranscriptKind::Defined => {
            let (body, has_badge) = if let Some(i) = text.rfind("  (defined)") {
                (text[..i].to_string(), true)
            } else {
                (text.clone(), false)
            };
            let mut spans = vec![Span::raw("  "), Span::styled(body, palette.result_value())];
            if has_badge {
                spans.push(Span::styled("  (defined)", palette.defined_badge()));
            }
            vec![Line::from(spans)]
        }
        TranscriptKind::Error => text
            .lines()
            .enumerate()
            .map(|(i, line)| {
                if i == 0 {
                    Line::from(vec![
                        Span::styled("  ✖ ", palette.error_prefix()),
                        Span::styled(line.to_string(), palette.error_body()),
                    ])
                } else {
                    Line::from(vec![
                        Span::raw("    "),
                        Span::styled(line.to_string(), palette.error_body()),
                    ])
                }
            })
            .collect(),
        TranscriptKind::Warning => text
            .lines()
            .enumerate()
            .map(|(i, line)| {
                if i == 0 {
                    Line::from(vec![
                        Span::styled("  ⚠ ", palette.warning_prefix()),
                        Span::styled(line.to_string(), palette.warning_prefix()),
                    ])
                } else {
                    Line::from(vec![Span::raw("    "), Span::raw(line.to_string())])
                }
            })
            .collect(),
        TranscriptKind::System => vec![Line::from(vec![
            Span::styled("  ✓ ", palette.system_message()),
            Span::styled(text, palette.system_message()),
        ])],
        TranscriptKind::CommandOutput => vec![Line::from(vec![
            Span::raw("  "),
            Span::styled(text, palette.command_output()),
        ])],
        TranscriptKind::TypeAnnotation => vec![Line::from(vec![
            Span::raw("    "),
            Span::styled(text, palette.type_annotation()),
        ])],
    }
}

fn symbol_entry_to_item(entry: &SymbolEntry, palette: Palette) -> ListItem<'static> {
    let (badge, style) = match entry.kind {
        SymbolKind::Type => ("[type]", palette.symbol_badge_type()),
        SymbolKind::Function => ("[fn]  ", palette.symbol_badge_fn()),
        SymbolKind::Value => ("[val] ", palette.symbol_badge_val()),
        SymbolKind::Module => ("[mod] ", palette.symbol_badge_type()),
    };
    ListItem::new(Line::from(vec![
        Span::styled(badge, style),
        Span::raw("  "),
        Span::raw(entry.name.clone()),
    ]))
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

pub(crate) fn count_input_lines(input: &str) -> usize {
    if input.is_empty() {
        1
    } else {
        input.lines().count().max(1)
    }
}

fn visible_command_suggestions(input: &str) -> Vec<&'static str> {
    slash_command_suggestions(input)
        .into_iter()
        .take(5)
        .collect()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_never_gives_plain_borders() {
        let p = Palette::new(ColorMode::Never, true);
        assert!(!p.use_color);
        let b = p.box_chars();
        assert_eq!(b.horizontal, "-");
        assert_eq!(b.vertical, "|");
    }

    #[test]
    fn palette_force_gives_unicode_borders() {
        let p = Palette::new(ColorMode::Always, false);
        assert!(p.use_color);
        let b = p.box_chars();
        assert_eq!(b.horizontal, "─");
    }

    #[test]
    fn palette_auto_respects_no_color_env() {
        std::env::set_var("NO_COLOR", "1");
        let p = Palette::new(ColorMode::Auto, true);
        assert!(!p.use_color);
        std::env::remove_var("NO_COLOR");
    }

    #[test]
    fn palette_auto_respects_dumb_term() {
        std::env::set_var("TERM", "dumb");
        let p = Palette::new(ColorMode::Auto, true);
        assert!(!p.use_color);
        std::env::remove_var("TERM");
    }

    #[test]
    fn count_lines_empty_is_one() {
        assert_eq!(count_input_lines(""), 1);
    }

    #[test]
    fn visible_command_suggestions_show_matches() {
        let suggestions = visible_command_suggestions("/va");
        assert_eq!(suggestions, vec!["/values"]);
    }

    #[test]
    fn tab_completion_accepts_command_and_adds_space_for_args() {
        let snapshot = ReplSnapshot {
            transcript: Vec::new(),
            symbol_pane: None,
            symbols: Vec::new(),
            turn: 0,
        };
        let palette = Palette::new(ColorMode::Never, true);
        let mut state = TuiState::new(snapshot, palette);
        state.input = "/op".to_owned();
        state.cursor = state.input.len();

        assert!(state.accept_command_suggestion());
        assert_eq!(state.input, "/openapi ");
    }

    #[test]
    fn count_lines_multiline() {
        assert_eq!(count_input_lines("a\nb\nc"), 3);
    }

    #[test]
    fn plain_entry_error_prefix() {
        let e = TranscriptEntry {
            kind: TranscriptKind::Error,
            text: "oops".into(),
        };
        assert!(entry_to_plain(&e).contains('✖'));
    }

    #[test]
    fn plain_entry_system_prefix() {
        let s = TranscriptEntry {
            kind: TranscriptKind::System,
            text: "ready".into(),
        };
        assert!(entry_to_plain(&s).contains('✓'));
    }
}
