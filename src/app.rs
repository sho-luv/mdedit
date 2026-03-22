use anyhow::Result;
use crossterm::cursor::SetCursorStyle;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use regex::Regex;
use ratatui_textarea::CursorMove;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::editor::{Editor, EditorAction};
use crate::file_io;
use crate::markdown::{MarkdownRenderer, TuiMarkdownRenderer};
use crate::preview::Preview;
use crate::status_bar::StatusBar;
use crate::vim::{InsertPosition, VimCommand, VimHandler, VimMode};

/// Layout mode — controls how the main area is split (D-11, D-12, D-13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    Split,
    EditorOnly,
    PreviewOnly,
}

impl LayoutMode {
    pub fn next(self) -> Self {
        match self {
            Self::Split => Self::EditorOnly,
            Self::EditorOnly => Self::PreviewOnly,
            Self::PreviewOnly => Self::Split,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Split => "Split View",
            Self::EditorOnly => "Editor Only",
            Self::PreviewOnly => "Preview Only",
        }
    }
}

/// Application mode — determines how key events are routed and what the
/// status bar displays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppMode {
    /// Nano-style editing mode.
    Editing,
    /// Vim Normal mode.
    Normal,
    /// Vim Insert mode.
    Insert,
    /// Vim Visual mode.
    Visual,
    /// Vim Command mode (: prompt).
    Command,
    /// Prompt: "Unsaved changes. Save? (y/n/Esc)" (D-13)
    ConfirmQuit,
    /// Prompt: "Save as: ___" for untitled buffers (D-02)
    PromptFilename,
    /// Incremental search mode (D-05). Captures keystrokes for search query.
    Search,
}

/// Top-level application struct owning the editor and managing the event loop.
pub struct App<'a> {
    editor: Editor<'a>,
    mode: AppMode,
    should_quit: bool,
    /// Text buffer for the filename prompt.
    filename_input: String,
    /// Status bar widget with timed messages.
    status_bar: StatusBar,
    /// Flag indicating we should quit after saving (from ConfirmQuit -> 'y' flow).
    quit_after_save: bool,
    /// Current layout mode (split, editor-only, preview-only).
    layout_mode: LayoutMode,
    /// Preview component managing scroll state.
    preview: Preview,
    /// Markdown renderer (abstracted behind trait for replaceability).
    renderer: TuiMarkdownRenderer,
    /// Cached rendered preview text.
    preview_text: ratatui::text::Text<'static>,
    /// Flag: content changed since last preview render.
    content_dirty: bool,
    /// Timestamp of last edit (for debounce).
    last_edit_time: Option<Instant>,
    /// Current search query text (D-05).
    search_query: String,
    /// Cursor position when search started, for Esc restore (D-07).
    search_cursor_before: (usize, usize),
    /// Current match index (0-based) for [3/17] display (D-10).
    search_match_index: usize,
    /// Total match count for [3/17] display (D-10).
    search_match_count: usize,
    /// Resolved color theme (wired to rendering in Plan 02).
    pub theme: crate::theme::Theme,
    /// Editing mode -- vim or nano (used in Phase 5).
    pub editing_mode: crate::config::EditingMode,
    /// Vim key handler state machine (Some when vim mode, None when nano).
    vim_handler: Option<VimHandler>,
}

impl<'a> App<'a> {
    pub fn new(
        content: Option<String>,
        filepath: Option<PathBuf>,
        theme: crate::theme::Theme,
        editing_mode: crate::config::EditingMode,
    ) -> Self {
        let is_vim = editing_mode == crate::config::EditingMode::Vim;
        let initial_mode = if is_vim { AppMode::Normal } else { AppMode::Editing };
        let vim_handler = if is_vim { Some(VimHandler::new()) } else { None };

        App {
            editor: Editor::new(content, filepath, theme.clone()),
            mode: initial_mode,
            should_quit: false,
            filename_input: String::new(),
            status_bar: StatusBar::new(),
            quit_after_save: false,
            layout_mode: LayoutMode::Split, // D-11: default to split
            preview: Preview::new(),
            renderer: TuiMarkdownRenderer,
            preview_text: ratatui::text::Text::default(),
            content_dirty: true,            // Render initial content on first frame
            last_edit_time: Some(Instant::now()),
            search_query: String::new(),
            search_cursor_before: (0, 0),
            search_match_index: 0,
            search_match_count: 0,
            theme,
            editing_mode,
            vim_handler,
        }
    }

    /// Synchronize preview scroll to editor cursor position using proportional mapping (D-01).
    /// Only active in Split mode (D-02). Called during render on every frame (D-03).
    fn sync_preview_scroll(&mut self, preview_area_height: u16) {
        if self.layout_mode != LayoutMode::Split {
            return;
        }
        let (cursor_row, _) = self.editor.cursor_position();
        let total_source = self.editor.line_count();
        let total_preview = self.preview_text.lines.len() as u16;

        if total_source <= 1 {
            self.preview.set_scroll(0);
            return;
        }

        // Proportional ratio mapping (D-01)
        let ratio = cursor_row as f64 / (total_source - 1).max(1) as f64;
        let target_line = (ratio * total_preview as f64) as u16;
        // Center target in viewport for comfortable reading (D-04)
        let centered = target_line.saturating_sub(preview_area_height / 2);
        let max_scroll = total_preview.saturating_sub(preview_area_height);
        self.preview.set_scroll(centered.min(max_scroll));
    }

    /// Debounced preview update (D-04). Only re-renders after 80ms idle.
    fn maybe_update_preview(&mut self) {
        if self.content_dirty {
            if let Some(last_edit) = self.last_edit_time {
                if last_edit.elapsed() >= Duration::from_millis(80) {
                    let content = self.editor.content();
                    self.preview_text = self.renderer.render(&content);
                    self.content_dirty = false;
                    self.last_edit_time = None;
                }
            }
        }
    }

    /// Main event loop. Draws the UI and processes events until quit.
    pub fn run(&mut self, terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
        loop {
            // Update preview before drawing (debounced)
            self.maybe_update_preview();

            terminal.draw(|frame| self.render(frame))?;

            // Poll with 50ms timeout so timed status messages can expire
            // and debounced preview can trigger
            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    // IMPORTANT: filter for Press only to avoid double-handling
                    // on Windows and some terminals
                    if key.kind == KeyEventKind::Press {
                        match self.mode {
                            AppMode::Editing => self.handle_editing_key(key),
                            AppMode::Normal => self.handle_vim_key(key),
                            AppMode::Insert => self.handle_vim_insert_key(key),
                            AppMode::Visual => self.handle_vim_visual_key(key),
                            AppMode::Command => self.handle_vim_command_key(key),
                            AppMode::ConfirmQuit => self.handle_confirm_quit_key(key),
                            AppMode::PromptFilename => self.handle_prompt_filename_key(key),
                            AppMode::Search => self.handle_search_key(key),
                        }

                        // Cursor shape changes per mode
                        match self.mode {
                            AppMode::Normal | AppMode::Visual | AppMode::Command => {
                                let _ = crossterm::execute!(std::io::stdout(), SetCursorStyle::SteadyBlock);
                            }
                            AppMode::Insert | AppMode::Editing => {
                                let _ = crossterm::execute!(std::io::stdout(), SetCursorStyle::SteadyBar);
                            }
                            _ => {} // ConfirmQuit, PromptFilename, Search keep current
                        }
                    }
                }
                // Event::Resize is a no-op — ratatui re-renders on next draw() (FOUND-06)
            }

            if self.should_quit {
                break;
            }
        }
        Ok(())
    }

    /// Perform the save operation. Returns true if save succeeded.
    fn do_save(&mut self) -> bool {
        if let Some(path) = self.editor.filepath().cloned() {
            let content = self.editor.content();
            match file_io::save_file(&path, &content) {
                Ok(()) => {
                    self.editor.mark_saved();
                    self.status_bar.set_message("Saved");
                    true
                }
                Err(e) => {
                    self.status_bar
                        .set_message(&format!("Save failed: {}", e));
                    false
                }
            }
        } else {
            // No filepath — need to prompt for filename
            false
        }
    }

    /// Handle key events in normal editing mode.
    fn handle_editing_key(&mut self, key: crossterm::event::KeyEvent) {
        // Ctrl+F enters search mode (D-05) — intercept before editor
        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('f') {
            self.search_query.clear();
            self.search_cursor_before = self.editor.cursor_position();
            self.search_match_index = 0;
            self.search_match_count = 0;
            self.mode = AppMode::Search;
            return;
        }

        // Ctrl+P toggles layout mode (D-12) — intercept before editor to avoid conflict (Pitfall 2)
        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('p') {
            self.layout_mode = self.layout_mode.next();
            self.status_bar.set_message(self.layout_mode.label()); // D-16
            return;
        }

        // In preview-only mode, route keys differently (D-13, D-14)
        if self.layout_mode == LayoutMode::PreviewOnly {
            match key.code {
                KeyCode::Up => { self.preview.scroll_up(1); return; }
                KeyCode::Down => { self.preview.scroll_down(1); return; }
                KeyCode::PageUp => { self.preview.scroll_up(20); return; }
                KeyCode::PageDown => { self.preview.scroll_down(20); return; }
                _ => {
                    // Any editing key switches back to split mode (D-14)
                    self.layout_mode = LayoutMode::Split;
                    // Fall through to normal editor key handling
                }
            }
        }

        if let Some(action) = self.editor.handle_key(key) {
            match action {
                EditorAction::Save => {
                    if self.editor.filepath().is_some() {
                        self.do_save();
                    } else {
                        // No filepath — prompt for filename
                        self.filename_input = String::new();
                        self.quit_after_save = false;
                        self.mode = AppMode::PromptFilename;
                    }
                }
                EditorAction::RequestQuit => {
                    if self.editor.is_modified() {
                        self.mode = AppMode::ConfirmQuit;
                    } else {
                        self.should_quit = true;
                    }
                }
                EditorAction::ContentChanged => {
                    self.content_dirty = true;
                    self.last_edit_time = Some(Instant::now());
                }
            }
        }
    }

    /// Return to the appropriate editing mode (Normal for vim, Editing for nano).
    fn return_to_editing_mode(&mut self) {
        if self.vim_handler.is_some() {
            self.mode = AppMode::Normal;
        } else {
            self.mode = AppMode::Editing;
        }
    }

    /// Handle key events in the "Unsaved changes" confirmation prompt (D-13).
    fn handle_confirm_quit_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if self.editor.filepath().is_some() {
                    // Save and quit
                    if self.do_save() {
                        self.should_quit = true;
                    } else {
                        // Save failed — return to editing
                        self.return_to_editing_mode();
                    }
                } else {
                    // No filepath — prompt for filename, then quit after save
                    self.filename_input = String::new();
                    self.quit_after_save = true;
                    self.mode = AppMode::PromptFilename;
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                // Quit without saving
                self.should_quit = true;
            }
            KeyCode::Esc => {
                // Cancel — return to editing
                self.return_to_editing_mode();
            }
            _ => {
                // Ignore other keys in this mode
            }
        }
    }

    /// Handle key events in the filename prompt (for untitled buffers).
    fn handle_prompt_filename_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                if !self.filename_input.is_empty() {
                    let path = PathBuf::from(&self.filename_input);
                    self.editor.set_filepath(path);
                    self.filename_input.clear();

                    // Now save with the new filepath
                    if self.do_save() {
                        if self.quit_after_save {
                            self.should_quit = true;
                        } else {
                            self.return_to_editing_mode();
                        }
                    } else {
                        // Save failed — return to editing
                        self.return_to_editing_mode();
                    }
                    self.quit_after_save = false;
                }
            }
            KeyCode::Esc => {
                self.filename_input.clear();
                self.quit_after_save = false;
                self.return_to_editing_mode();
            }
            KeyCode::Backspace => {
                self.filename_input.pop();
            }
            KeyCode::Char(c) => {
                self.filename_input.push(c);
            }
            _ => {}
        }
    }

    /// Handle key events in search mode (D-05 through D-11).
    fn handle_search_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                // Exit search, restore cursor to pre-search position (D-07)
                let (row, col) = self.search_cursor_before;
                self.editor.textarea_mut().move_cursor(CursorMove::Jump(row as u16, col as u16));
                // Clear search pattern to prevent ghost highlights (Pitfall 4)
                let _ = self.editor.textarea_mut().set_search_pattern("");
                self.search_query.clear();
                self.return_to_editing_mode();
            }
            KeyCode::Enter => {
                if self.search_match_count > 0 {
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        // Shift+Enter: previous match (D-07)
                        self.editor.textarea_mut().search_back(false);
                        self.search_match_index = if self.search_match_index == 0 {
                            self.search_match_count - 1
                        } else {
                            self.search_match_index - 1
                        };
                    } else {
                        // Enter: next match (D-07)
                        self.editor.textarea_mut().search_forward(false);
                        self.search_match_index = (self.search_match_index + 1) % self.search_match_count;
                    }
                    // Update cursor_before so Esc now stays at this match (D-07)
                    self.search_cursor_before = self.editor.cursor_position();
                }
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.update_search_pattern();
            }
            KeyCode::Char(c) => {
                self.search_query.push(c);
                self.update_search_pattern();
            }
            _ => {}
        }
    }

    /// Update the search pattern on the textarea and recount matches (D-06, D-09).
    fn update_search_pattern(&mut self) {
        if self.search_query.is_empty() {
            let _ = self.editor.textarea_mut().set_search_pattern("");
            self.search_match_count = 0;
            self.search_match_index = 0;
            return;
        }
        // Case-insensitive plain text search (D-09)
        let pattern = format!("(?i){}", regex::escape(&self.search_query));
        let _ = self.editor.textarea_mut().set_search_pattern(&pattern);

        // Count matches across all lines (D-10)
        if let Ok(re) = Regex::new(&pattern) {
            let content = self.editor.content();
            self.search_match_count = re.find_iter(&content).count();
        } else {
            self.search_match_count = 0;
        }
        self.search_match_index = 0;

        // Jump to first match from current position (D-06)
        if self.search_match_count > 0 {
            self.editor.textarea_mut().search_forward(true);
        }
    }

    /// Return the current search query if in search mode, empty string otherwise.
    pub fn current_search_query(&self) -> &str {
        if self.mode == AppMode::Search && !self.search_query.is_empty() {
            &self.search_query
        } else {
            ""
        }
    }

    /// Handle key events in Vim Normal mode.
    fn handle_vim_key(&mut self, key: crossterm::event::KeyEvent) {
        // Ctrl+P toggles layout mode -- intercept before vim handler
        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('p') {
            self.layout_mode = self.layout_mode.next();
            self.status_bar.set_message(self.layout_mode.label());
            return;
        }

        let handler = self.vim_handler.as_mut().unwrap();
        let cmd = handler.handle_key(key);

        match cmd {
            VimCommand::EnterInsert(pos) => {
                self.mode = AppMode::Insert;
                // Position cursor based on insert variant
                match pos {
                    InsertPosition::AfterCursor => {
                        self.editor.textarea_mut().move_cursor(CursorMove::Forward);
                    }
                    InsertPosition::LineStart => {
                        self.editor.textarea_mut().move_cursor(CursorMove::Head);
                    }
                    InsertPosition::LineEnd => {
                        self.editor.textarea_mut().move_cursor(CursorMove::End);
                    }
                    InsertPosition::NewLineBelow => {
                        self.editor.textarea_mut().move_cursor(CursorMove::End);
                        self.editor.textarea_mut().insert_newline();
                        self.content_dirty = true;
                        self.last_edit_time = Some(Instant::now());
                    }
                    InsertPosition::NewLineAbove => {
                        self.editor.textarea_mut().move_cursor(CursorMove::Head);
                        self.editor.textarea_mut().insert_newline();
                        self.editor.textarea_mut().move_cursor(CursorMove::Up);
                        self.content_dirty = true;
                        self.last_edit_time = Some(Instant::now());
                    }
                    InsertPosition::BeforeCursor => {} // no movement needed
                }
            }
            VimCommand::EnterCommand => {
                self.mode = AppMode::Command;
            }
            VimCommand::EnterVisual { line_wise: _ } => {
                self.mode = AppMode::Visual;
                self.editor.textarea_mut().start_selection();
            }
            VimCommand::EnterSearch => {
                self.search_query.clear();
                self.search_cursor_before = self.editor.cursor_position();
                self.search_match_index = 0;
                self.search_match_count = 0;
                self.mode = AppMode::Search;
            }
            VimCommand::None => {}
            _ => {} // Other commands handled in Plans 02/03
        }
    }

    /// Handle key events in Vim Insert mode.
    fn handle_vim_insert_key(&mut self, key: crossterm::event::KeyEvent) {
        let handler = self.vim_handler.as_mut().unwrap();
        let cmd = handler.handle_key(key);

        match cmd {
            VimCommand::ExitInsert => {
                self.mode = AppMode::Normal;
            }
            VimCommand::None => {
                // Forward all non-Esc keys to textarea for text input
                let changed = self.editor.textarea_mut().input_without_shortcuts(key);
                if changed {
                    self.content_dirty = true;
                    self.last_edit_time = Some(Instant::now());
                }
            }
            _ => {}
        }
    }

    /// Handle key events in Vim Visual mode.
    fn handle_vim_visual_key(&mut self, key: crossterm::event::KeyEvent) {
        let handler = self.vim_handler.as_mut().unwrap();
        let cmd = handler.handle_key(key);

        match cmd {
            VimCommand::ExitVisual => {
                self.editor.textarea_mut().cancel_selection();
                self.mode = AppMode::Normal;
            }
            VimCommand::None => {} // Visual motions handled in Plan 03
            _ => {}
        }
    }

    /// Handle key events in Vim Command mode.
    fn handle_vim_command_key(&mut self, key: crossterm::event::KeyEvent) {
        let handler = self.vim_handler.as_mut().unwrap();
        let cmd = handler.handle_key(key);

        match cmd {
            VimCommand::ExitCommand => {
                self.mode = AppMode::Normal;
            }
            VimCommand::Save => {
                self.mode = AppMode::Normal;
                if self.editor.filepath().is_some() {
                    self.do_save();
                } else {
                    self.filename_input = String::new();
                    self.quit_after_save = false;
                    self.mode = AppMode::PromptFilename;
                }
            }
            VimCommand::Quit { force } => {
                self.mode = AppMode::Normal;
                if force || !self.editor.is_modified() {
                    self.should_quit = true;
                } else {
                    self.mode = AppMode::ConfirmQuit;
                }
            }
            VimCommand::SaveAndQuit => {
                self.mode = AppMode::Normal;
                if self.editor.filepath().is_some() {
                    if self.do_save() {
                        self.should_quit = true;
                    }
                } else {
                    self.filename_input = String::new();
                    self.quit_after_save = true;
                    self.mode = AppMode::PromptFilename;
                }
            }
            VimCommand::CommandAppend(_) | VimCommand::CommandBackspace => {
                // Handler maintains buffer; nothing else to do
            }
            VimCommand::None => {}
            _ => {}
        }
    }

    /// Render the editor, preview, and status bar based on current layout mode.
    fn render(&mut self, frame: &mut Frame) {
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Fill(1), Constraint::Length(1)])
            .split(frame.area());

        let body_area = outer[0];
        let status_area = outer[1];

        let search_query = self.current_search_query().to_string();

        match self.layout_mode {
            LayoutMode::Split => {
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(50),
                        Constraint::Length(1),
                        Constraint::Percentage(50),
                    ])
                    .split(body_area);

                // Editor left — with syntax highlighting (D-09)
                self.editor.render_highlighted(frame, chunks[0], &search_query);

                // Divider (D-11: subtle dimmed vertical line)
                let divider_lines: Vec<ratatui::text::Line> = (0..chunks[1].height)
                    .map(|_| {
                        ratatui::text::Line::from(Span::styled(
                            "\u{2502}",
                            Style::default().fg(self.theme.divider_fg),
                        ))
                    })
                    .collect();
                frame.render_widget(Paragraph::new(divider_lines), chunks[1]);

                // Scroll sync: preview follows editor cursor (D-03)
                self.sync_preview_scroll(chunks[2].height);

                // Preview right
                self.preview.render(frame, chunks[2], &self.preview_text);
            }
            LayoutMode::EditorOnly => {
                self.editor.render_highlighted(frame, body_area, &search_query);
            }
            LayoutMode::PreviewOnly => {
                self.preview.render(frame, body_area, &self.preview_text);
            }
        }

        // Render status bar based on current mode
        match self.mode {
            AppMode::Editing => {
                self.status_bar.render(
                    frame,
                    status_area,
                    self.editor.display_name(),
                    self.editor.cursor_position(),
                    self.editor.is_modified(),
                    &self.theme,
                    None,
                );
            }
            AppMode::Normal => {
                let mode_info = Some(("-- NORMAL --", self.theme.mode_normal_bg));
                self.status_bar.render(
                    frame,
                    status_area,
                    self.editor.display_name(),
                    self.editor.cursor_position(),
                    self.editor.is_modified(),
                    &self.theme,
                    mode_info,
                );
            }
            AppMode::Insert => {
                let mode_info = Some(("-- INSERT --", self.theme.mode_insert_bg));
                self.status_bar.render(
                    frame,
                    status_area,
                    self.editor.display_name(),
                    self.editor.cursor_position(),
                    self.editor.is_modified(),
                    &self.theme,
                    mode_info,
                );
            }
            AppMode::Visual => {
                let mode_info = Some(("-- VISUAL --", self.theme.mode_visual_bg));
                self.status_bar.render(
                    frame,
                    status_area,
                    self.editor.display_name(),
                    self.editor.cursor_position(),
                    self.editor.is_modified(),
                    &self.theme,
                    mode_info,
                );
            }
            AppMode::Command => {
                let cmd = self.vim_handler.as_ref().map(|h| h.command_buffer()).unwrap_or("");
                let prompt = format!(" :{}_", cmd);
                let bar = Paragraph::new(Span::raw(prompt))
                    .style(Style::default().bg(self.theme.mode_command_bg).fg(self.theme.status_bar_fg));
                frame.render_widget(bar, status_area);
            }
            AppMode::ConfirmQuit => {
                let bar = Paragraph::new(Span::raw(
                    " Unsaved changes. Save? (y/n/Esc)",
                ))
                .style(Style::default().bg(self.theme.confirm_bg).fg(self.theme.confirm_fg));
                frame.render_widget(bar, status_area);
            }
            AppMode::PromptFilename => {
                let prompt = format!(" Save as: {}_", self.filename_input);
                let bar = Paragraph::new(Span::raw(prompt))
                    .style(Style::default().bg(self.theme.prompt_bg).fg(self.theme.prompt_fg));
                frame.render_widget(bar, status_area);
            }
            AppMode::Search => {
                let prompt = if self.search_match_count > 0 {
                    format!(" Search: {} [{}/{}]", self.search_query, self.search_match_index + 1, self.search_match_count)
                } else if self.search_query.is_empty() {
                    " Search: _".to_string()
                } else {
                    format!(" Search: {} [no matches]", self.search_query)
                };
                let bar = Paragraph::new(Span::raw(prompt))
                    .style(Style::default().bg(self.theme.prompt_bg).fg(self.theme.prompt_fg));
                frame.render_widget(bar, status_area);
            }
        }
    }
}
