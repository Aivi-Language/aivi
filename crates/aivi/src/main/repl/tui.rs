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
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
        Wrap,
    },
    Frame, Terminal,
};

use aivi::AiviError;

use super::engine::{
    command_accepts_argument, CompletionItem, CompletionKind, CompletionMode, CompletionState,
    ReplEngine, ReplSnapshot, SymbolEntry, SymbolKind, TranscriptEntry, TranscriptKind,
};
use super::{ColorMode, ReplOptions, SymbolPane};

const MAX_VISIBLE_SUGGESTIONS: usize = 5;

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

    fn symbol_badge_ctor(self) -> Style {
        if self.use_color {
            Style::default()
                .fg(Color::Magenta)
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
    /// Scroll offset for the symbol side-pane (0 = top).
    symbol_scroll_offset: usize,
    /// Submitted input history, oldest first.
    history: Vec<String>,
    history_idx: Option<usize>,
    /// Saved input while browsing history.
    history_saved: String,
    quit: bool,
    snapshot: ReplSnapshot,
    palette: Palette,
    suggestion_index: usize,
    completion_state: Option<CompletionState>,
    /// Last rendered symbol pane area (for mouse scroll hit-testing).
    symbol_pane_rect: Option<Rect>,
}

impl TuiState {
    fn new(snapshot: ReplSnapshot, palette: Palette) -> Self {
        Self {
            input: String::new(),
            cursor: 0,
            scroll_offset: 0,
            symbol_scroll_offset: 0,
            history: Vec::new(),
            history_idx: None,
            history_saved: String::new(),
            quit: false,
            snapshot,
            palette,
            suggestion_index: 0,
            completion_state: None,
            symbol_pane_rect: None,
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

    fn refresh_completions(&mut self, engine: &ReplEngine) {
        self.completion_state = engine.completion_state(&self.input, self.cursor);
        let max_index = self
            .completion_state
            .as_ref()
            .map_or(0, |state| state.items.len().saturating_sub(1));
        self.suggestion_index = self.suggestion_index.min(max_index);
    }

    fn suggestions(&self) -> &[CompletionItem] {
        self.completion_state
            .as_ref()
            .map_or(&[], |state| state.items.as_slice())
    }

    fn has_suggestions(&self) -> bool {
        !self.suggestions().is_empty()
    }

    fn suggestion_up(&mut self) {
        let suggestions = self.suggestions();
        if suggestions.is_empty() {
            return;
        }
        self.suggestion_index = self
            .suggestion_index
            .checked_sub(1)
            .unwrap_or(suggestions.len() - 1);
    }

    fn suggestion_down(&mut self) {
        let suggestions = self.suggestions();
        if suggestions.is_empty() {
            return;
        }
        self.suggestion_index = (self.suggestion_index + 1) % suggestions.len();
    }

    fn accept_suggestion(&mut self) -> bool {
        let Some(completion_state) = self.completion_state.clone() else {
            return false;
        };
        if completion_state.items.is_empty() {
            return false;
        }

        let selected_index = self
            .suggestion_index
            .min(completion_state.items.len().saturating_sub(1));
        let selected = &completion_state.items[selected_index];
        let mut replacement = selected.insert_text.clone();
        if matches!(completion_state.mode, CompletionMode::Command)
            && command_accepts_argument(&replacement)
        {
            replacement.push(' ');
        }

        self.input.replace_range(
            completion_state.replace_start..completion_state.replace_end,
            &replacement,
        );
        self.cursor = completion_state.replace_start + replacement.len();
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
    // Enable mouse capture for scroll-wheel support.  Text selection still
    // works in most terminals by holding Shift while selecting.
    execute!(
        stdout,
        EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )
    .map_err(AiviError::Io)?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend).map_err(AiviError::Io)?;
    let mut state = TuiState::new(snapshot, palette);
    state.refresh_completions(&engine);

    let result = event_loop(&mut terminal, &mut engine, &mut state);

    // Always restore — even if event_loop returns an error.
    let _ = disable_raw_mode();
    let _ = execute!(
        terminal.backend_mut(),
        crossterm::event::DisableMouseCapture,
        LeaveAlternateScreen
    );
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
            Event::Mouse(mouse) => {
                handle_mouse(mouse.kind, mouse.column, mouse.row, state);
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
            state.refresh_completions(engine);
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
            state.symbol_scroll_offset = 0;
            state.suggestion_index = 0;
            state.record_history(input.clone());
            state.snapshot = engine.submit(&input)?;
            state.refresh_completions(engine);
        }

        // Shift+Enter → insert literal newline (multi-line mode).
        KeyCode::Enter if shift => {
            state.push_char('\n');
            state.refresh_completions(engine);
        }

        // History navigation or command suggestion selection.
        KeyCode::Up => {
            if state.has_suggestions() {
                state.suggestion_up();
            } else {
                state.history_up();
                state.refresh_completions(engine);
            }
        }
        KeyCode::Down => {
            if state.has_suggestions() {
                state.suggestion_down();
            } else {
                state.history_down();
                state.refresh_completions(engine);
            }
        }

        // Scrolling: symbol pane when open, otherwise transcript.
        KeyCode::PageUp => {
            if state.snapshot.symbol_pane.is_some() {
                state.symbol_scroll_offset = state.symbol_scroll_offset.saturating_add(5);
            } else {
                state.scroll_offset = state.scroll_offset.saturating_add(5);
            }
        }
        KeyCode::PageDown => {
            if state.snapshot.symbol_pane.is_some() {
                state.symbol_scroll_offset = state.symbol_scroll_offset.saturating_sub(5);
            } else {
                state.scroll_offset = state.scroll_offset.saturating_sub(5);
            }
        }

        // Accept a command suggestion, otherwise toggle the symbol pane.
        KeyCode::Tab => {
            if state.accept_suggestion() {
                state.refresh_completions(engine);
            } else {
                engine.toggle_symbol_pane();
                state.snapshot = engine.snapshot();
                state.symbol_scroll_offset = 0;
            }
        }
        KeyCode::Esc => {
            engine.close_symbol_pane();
            state.snapshot = engine.snapshot();
            state.symbol_scroll_offset = 0;
        }

        // Text editing.
        KeyCode::Char(c) => {
            state.push_char(c);
            state.refresh_completions(engine);
        }
        KeyCode::Backspace => {
            state.delete_back();
            state.refresh_completions(engine);
        }

        KeyCode::Left => {
            if state.cursor > 0 {
                let prev = state.input[..state.cursor]
                    .char_indices()
                    .next_back()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                state.cursor = prev;
            }
            state.refresh_completions(engine);
        }
        KeyCode::Right => {
            if state.cursor < state.input.len() {
                let step = state.input[state.cursor..]
                    .chars()
                    .next()
                    .map_or(0, char::len_utf8);
                state.cursor += step;
            }
            state.refresh_completions(engine);
        }
        KeyCode::Home => {
            state.cursor = state.input[..state.cursor]
                .rfind('\n')
                .map(|i| i + 1)
                .unwrap_or(0);
            state.refresh_completions(engine);
        }
        KeyCode::End => {
            state.cursor = state.input[state.cursor..]
                .find('\n')
                .map(|i| state.cursor + i)
                .unwrap_or(state.input.len());
            state.refresh_completions(engine);
        }

        _ => {}
    }
    Ok(())
}

// ─── Mouse handling ──────────────────────────────────────────────────────────

fn handle_mouse(kind: MouseEventKind, col: u16, row: u16, state: &mut TuiState) {
    let scroll_amount = 3;
    let in_pane = state
        .symbol_pane_rect
        .is_some_and(|r| col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height);

    match kind {
        MouseEventKind::ScrollUp => {
            if in_pane {
                state.symbol_scroll_offset =
                    state.symbol_scroll_offset.saturating_sub(scroll_amount);
            } else {
                state.scroll_offset = state.scroll_offset.saturating_add(scroll_amount);
            }
        }
        MouseEventKind::ScrollDown => {
            if in_pane {
                state.symbol_scroll_offset =
                    state.symbol_scroll_offset.saturating_add(scroll_amount);
            } else {
                state.scroll_offset = state.scroll_offset.saturating_sub(scroll_amount);
            }
        }
        _ => {}
    }
}

// ─── Rendering ────────────────────────────────────────────────────────────────

fn render(frame: &mut Frame, state: &mut TuiState) {
    let snap = &state.snapshot;
    let palette = state.palette;
    let show_pane = snap.symbol_pane.is_some();

    // Vertical split: main area / status bar / input area.
    let suggestion_count = state.suggestions().len().min(MAX_VISIBLE_SUGGESTIONS) as u16;
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

    state.symbol_pane_rect = pane_rect;
    render_transcript(frame, transcript_rect, snap, state, palette);
    if let Some(area) = pane_rect {
        render_symbol_pane(frame, area, snap, state, palette);
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

    let visible_items: Vec<ListItem> = all_lines
        .into_iter()
        .skip(start)
        .take(visible as usize)
        .collect();

    let block = Block::default()
        .title(" AIVI REPL ")
        .borders(Borders::ALL)
        .border_style(palette.border());

    frame.render_widget(List::new(visible_items).block(block), area);
}

fn render_symbol_pane(
    frame: &mut Frame,
    area: Rect,
    snap: &ReplSnapshot,
    state: &TuiState,
    palette: Palette,
) {
    let title = match snap.symbol_pane {
        Some(SymbolPane::Types) => " /types ",
        Some(SymbolPane::Values) => " /values ",
        Some(SymbolPane::Functions) => " /functions ",
        Some(SymbolPane::Modules) => " /modules ",
        None => "",
    };

    let all_items: Vec<ListItem> = if snap.symbols.is_empty() {
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

    let total = all_items.len();
    let visible = area.height.saturating_sub(2) as usize; // borders
    let max_scroll = total.saturating_sub(visible);
    let effective_scroll = state.symbol_scroll_offset.min(max_scroll);

    let visible_items: Vec<ListItem> = all_items
        .into_iter()
        .skip(effective_scroll)
        .take(visible)
        .collect();

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(palette.border());

    frame.render_widget(List::new(visible_items).block(block), area);

    // Render scrollbar when content overflows.
    if total > visible {
        let mut scrollbar_state = ScrollbarState::new(max_scroll).position(effective_scroll);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .track_symbol(Some("░"))
                .thumb_symbol("█")
                .track_style(palette.border())
                .thumb_style(if palette.use_color {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default().add_modifier(Modifier::BOLD)
                }),
            area.inner(ratatui::layout::Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }
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

    let suggestions = state.suggestions();
    let selected_index = state
        .suggestion_index
        .min(suggestions.len().saturating_sub(1));
    let (visible_start, visible_end) =
        visible_suggestion_range(suggestions.len(), selected_index, MAX_VISIBLE_SUGGESTIONS);
    for (idx, suggestion) in suggestions[visible_start..visible_end].iter().enumerate() {
        let suggestion_idx = visible_start + idx;
        let style = if suggestion_idx == selected_index {
            palette.selected_suggestion()
        } else {
            palette.suggestion()
        };
        let badge_style = completion_badge_style(suggestion.kind, palette);
        let badge = completion_badge(suggestion.kind);
        let detail = match suggestion.kind {
            CompletionKind::Command => suggestion.detail.clone(),
            _ => format!(":: {}", suggestion.detail),
        };
        lines.push(Line::from(vec![
            Span::styled("  ", palette.hint()),
            Span::styled(badge, badge_style),
            Span::raw(" "),
            Span::styled(suggestion.label.clone(), style),
            Span::raw(" "),
            Span::styled(detail, palette.type_annotation()),
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

fn visible_suggestion_range(
    total: usize,
    selected_index: usize,
    max_visible: usize,
) -> (usize, usize) {
    if total == 0 || max_visible == 0 {
        return (0, 0);
    }

    let visible = total.min(max_visible);
    let selected_index = selected_index.min(total.saturating_sub(1));
    let start = selected_index
        .saturating_sub(visible.saturating_sub(1))
        .min(total - visible);
    (start, start + visible)
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
        TranscriptKind::CommandOutput => text
            .lines()
            .map(|line| {
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(line.to_string(), palette.command_output()),
                ])
            })
            .collect(),
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

fn completion_badge(kind: CompletionKind) -> &'static str {
    match kind {
        CompletionKind::Command => "[cmd]",
        CompletionKind::Constructor => "[ctor]",
        CompletionKind::Function => "[fn] ",
        CompletionKind::Value => "[val]",
    }
}

fn completion_badge_style(kind: CompletionKind, palette: Palette) -> Style {
    match kind {
        CompletionKind::Command => palette.suggestion(),
        CompletionKind::Constructor => palette.symbol_badge_ctor(),
        CompletionKind::Function => palette.symbol_badge_fn(),
        CompletionKind::Value => palette.symbol_badge_val(),
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, buffer::Buffer};

    fn buffer_row(buffer: &Buffer, y: u16) -> String {
        (0..buffer.area.width)
            .map(|x| buffer.cell((x, y)).expect("buffer row cell").symbol())
            .collect()
    }

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
        let engine = ReplEngine::new(&ReplOptions {
            color_mode: ColorMode::Never,
            plain_mode: false,
        })
        .unwrap();
        let snapshot = engine.snapshot();
        let palette = Palette::new(ColorMode::Never, true);
        let mut state = TuiState::new(snapshot, palette);
        state.input = "/va".to_owned();
        state.cursor = state.input.len();
        state.refresh_completions(&engine);
        assert_eq!(state.suggestions()[0].label, "/values");
    }

    #[test]
    fn visible_suggestion_range_scrolls_across_long_lists() {
        assert_eq!(
            visible_suggestion_range(11, 0, MAX_VISIBLE_SUGGESTIONS),
            (0, 5)
        );
        assert_eq!(
            visible_suggestion_range(11, 4, MAX_VISIBLE_SUGGESTIONS),
            (0, 5)
        );
        assert_eq!(
            visible_suggestion_range(11, 5, MAX_VISIBLE_SUGGESTIONS),
            (1, 6)
        );
        assert_eq!(
            visible_suggestion_range(11, 10, MAX_VISIBLE_SUGGESTIONS),
            (6, 11)
        );
    }

    #[test]
    fn arrow_navigation_reaches_suggestions_beyond_first_page() {
        let engine = ReplEngine::new(&ReplOptions {
            color_mode: ColorMode::Never,
            plain_mode: false,
        })
        .unwrap();
        let snapshot = engine.snapshot();
        let palette = Palette::new(ColorMode::Never, true);
        let mut state = TuiState::new(snapshot, palette);
        state.input = "/".to_owned();
        state.cursor = state.input.len();
        state.refresh_completions(&engine);

        assert!(state.suggestions().len() > MAX_VISIBLE_SUGGESTIONS);
        for _ in 0..6 {
            state.suggestion_down();
        }

        let visible = visible_suggestion_range(
            state.suggestions().len(),
            state.suggestion_index,
            MAX_VISIBLE_SUGGESTIONS,
        );
        assert_eq!(state.suggestion_index, 6);
        assert_eq!(visible, (2, 7));
    }

    #[test]
    fn tab_completion_accepts_command_and_adds_space_for_args() {
        let engine = ReplEngine::new(&ReplOptions {
            color_mode: ColorMode::Never,
            plain_mode: false,
        })
        .unwrap();
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
        state.refresh_completions(&engine);

        assert!(state.accept_suggestion());
        assert_eq!(state.input, "/openapi ");
    }

    #[test]
    fn enter_submits_current_input_even_when_suggestions_are_visible() {
        let mut engine = ReplEngine::new(&ReplOptions {
            color_mode: ColorMode::Never,
            plain_mode: false,
        })
        .unwrap();
        engine.submit("/use aivi.text").unwrap();
        let snapshot = engine.snapshot();
        let palette = Palette::new(ColorMode::Never, true);
        let mut state = TuiState::new(snapshot, palette);
        state.input = "/functions jo".to_owned();
        state.cursor = state.input.len();
        state.refresh_completions(&engine);

        handle_key(KeyCode::Enter, KeyModifiers::NONE, &mut engine, &mut state).unwrap();

        assert!(state.input.is_empty());
        assert!(state.snapshot.transcript.iter().any(|entry| matches!(
            entry.kind,
            TranscriptKind::CommandOutput
        ) && entry.text.contains("function")
            && entry.text.contains("filter: jo")
            && entry.text.contains("side panel")));
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

    #[test]
    fn command_output_renders_multiline_entries_as_multiple_lines() {
        let entry = TranscriptEntry {
            kind: TranscriptKind::CommandOutput,
            text: "Function `join`\nmodule: aivi.text\nsignature: Text -> List Text -> Text\n\nQuick info:\n  no indexed docs available for this symbol yet.".into(),
        };
        let lines = transcript_entry_to_lines(&entry, Palette::new(ColorMode::Never, true));
        let rendered: Vec<String> = lines
            .into_iter()
            .map(|line| {
                line.spans
                    .into_iter()
                    .map(|span| span.content.into_owned())
                    .collect::<String>()
            })
            .collect();
        assert_eq!(
            rendered,
            vec![
                "  Function `join`".to_owned(),
                "  module: aivi.text".to_owned(),
                "  signature: Text -> List Text -> Text".to_owned(),
                "  ".to_owned(),
                "  Quick info:".to_owned(),
                "    no indexed docs available for this symbol yet.".to_owned(),
            ]
        );
    }

    #[test]
    fn transcript_render_does_not_pollute_status_row() {
        let transcript = (0..12)
            .map(|idx| TranscriptEntry {
                kind: TranscriptKind::CommandOutput,
                text: format!("overflow line {idx}"),
            })
            .collect();
        let snapshot = ReplSnapshot {
            transcript,
            symbol_pane: None,
            symbols: Vec::new(),
            turn: 5,
        };
        let mut state = TuiState::new(snapshot, Palette::new(ColorMode::Never, true));
        let mut terminal = Terminal::new(TestBackend::new(90, 10)).unwrap();

        terminal.draw(|frame| render(frame, &mut state)).unwrap();

        let status_row = buffer_row(terminal.backend().buffer(), 6);
        let status_text =
            "  session:5  module:repl_session  [Tab: symbols]  [Ctrl+D: exit]  [Ctrl+L: clear]";
        assert_eq!(status_row, format!("{status_text:<90}"));
    }
}
