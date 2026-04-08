use anyhow::Result;
use crossterm::cursor::SetCursorStyle;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
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
use crate::vim::{InsertPosition, Motion, VimCommand, VimHandler};

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
    /// Settings overlay panel.
    Settings,
}

/// Which row is selected in the settings panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsRow {
    Theme,
    Mode,
    Profile,
    SyncIndicator,
    WordWrap,
    Save,
    Reset,
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
    /// Source line -> preview line mapping for scroll sync.
    source_to_preview: Vec<usize>,
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
    #[allow(dead_code)]
    pub editing_mode: crate::config::EditingMode,
    /// Vim key handler state machine (Some when vim mode, None when nano).
    vim_handler: Option<VimHandler>,
    /// Split ratio as percentage (0-100) for editor pane width. Default 50.
    split_ratio: u16,
    /// Track layout areas for mouse hit testing.
    editor_area: Option<Rect>,
    /// Track preview area for mouse hit testing.
    preview_area: Option<Rect>,
    /// Track divider area for mouse hit testing.
    divider_area: Option<Rect>,
    /// Whether user is currently dragging the divider.
    dragging_divider: bool,
    /// Whether user is currently drag-selecting text.
    drag_selecting: bool,
    /// System clipboard provider (OSC 52 + platform-native).
    clipboard: Box<dyn crate::clipboard::ClipboardProvider>,
    /// Whether we've already shown the "clipboard unavailable" warning.
    clipboard_warned: bool,
    /// Live config state for :set commands and saving.
    config: crate::config::Config,
    /// Selected row in settings panel.
    settings_row: SettingsRow,
    /// Index into the theme list for the settings panel.
    settings_theme_index: usize,
    /// Index into the mode list for the settings panel.
    settings_mode_index: usize,
    /// Index into the render profile list for the settings panel.
    settings_profile_index: usize,
}

impl<'a> App<'a> {
    pub fn new(
        content: Option<String>,
        filepath: Option<PathBuf>,
        theme: crate::theme::Theme,
        editing_mode: crate::config::EditingMode,
        clipboard: Box<dyn crate::clipboard::ClipboardProvider>,
        config: crate::config::Config,
    ) -> Self {
        let is_vim = editing_mode == crate::config::EditingMode::Vim;
        let initial_mode = if is_vim { AppMode::Normal } else { AppMode::Editing };
        let vim_handler = if is_vim { Some(VimHandler::new()) } else { None };

        let mut editor = Editor::new(content, filepath, theme.clone());
        editor.word_wrap = config.word_wrap;

        App {
            editor,
            mode: initial_mode,
            should_quit: false,
            filename_input: String::new(),
            status_bar: StatusBar::new(),
            quit_after_save: false,
            layout_mode: LayoutMode::Split, // D-11: default to split
            preview: Preview::new(),
            renderer: TuiMarkdownRenderer,
            preview_text: ratatui::text::Text::default(),
            source_to_preview: Vec::new(),
            content_dirty: true,            // Render initial content on first frame
            last_edit_time: Some(Instant::now()),
            search_query: String::new(),
            search_cursor_before: (0, 0),
            search_match_index: 0,
            search_match_count: 0,
            theme,
            editing_mode,
            vim_handler,
            split_ratio: 50,
            editor_area: None,
            preview_area: None,
            divider_area: None,
            dragging_divider: false,
            drag_selecting: false,
            clipboard,
            clipboard_warned: false,
            settings_theme_index: {
                let themes = crate::theme::Theme::available_themes();
                themes.iter().position(|t| *t == config.theme).unwrap_or(0)
            },
            settings_mode_index: match config.mode {
                crate::config::EditingMode::Vim => 0,
                crate::config::EditingMode::Nano => 1,
            },
            settings_profile_index: crate::config::RenderProfile::all()
                .iter()
                .position(|p| *p == config.render_profile)
                .unwrap_or(0),
            config,
            settings_row: SettingsRow::Theme,
        }
    }

    /// Synchronize preview scroll to editor cursor position.
    /// Uses source-to-preview line mapping when available, falls back to proportional.
    /// Only active in Split mode. Called during render on every frame.
    fn sync_preview_scroll(&mut self, preview_area_height: u16) {
        if self.layout_mode != LayoutMode::Split {
            return;
        }
        let (cursor_row, _) = self.editor.cursor_position();
        let total_preview = self.preview_text.lines.len() as u16;

        if total_preview == 0 {
            self.preview.set_scroll(0);
            return;
        }

        // Use the source-to-preview line map for precise scroll sync
        let target_line = if !self.source_to_preview.is_empty() && cursor_row < self.source_to_preview.len() {
            self.source_to_preview[cursor_row] as u16
        } else {
            // Fallback to proportional mapping
            let total_source = self.editor.line_count();
            let ratio = cursor_row as f64 / (total_source - 1).max(1) as f64;
            (ratio * total_preview as f64) as u16
        };

        // Center target in viewport
        let centered = target_line.saturating_sub(preview_area_height / 3);
        let max_scroll = total_preview.saturating_sub(preview_area_height);
        self.preview.set_scroll(centered.min(max_scroll));
    }

    /// Debounced preview update (D-04). Only re-renders after 80ms idle.
    fn maybe_update_preview(&mut self) {
        if self.content_dirty {
            if let Some(last_edit) = self.last_edit_time {
                if last_edit.elapsed() >= Duration::from_millis(80) {
                    let content = self.editor.content();
                    let result = self.renderer.render(&content, self.config.render_profile);
                    self.preview_text = result.text;
                    self.source_to_preview = result.source_to_preview;
                    self.content_dirty = false;
                    self.last_edit_time = None;
                }
            }
        }
    }

    /// Main event loop. Draws the UI and processes events until quit.
    pub fn run(&mut self, terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>) -> Result<()> {
        loop {
            // Update preview before drawing (debounced)
            self.maybe_update_preview();

            terminal.draw(|frame| self.render(frame))?;

            // Poll with 50ms timeout so timed status messages can expire
            // and debounced preview can trigger
            if event::poll(Duration::from_millis(50))? {
                match event::read()? {
                    Event::Key(key) => {
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
                            AppMode::Settings => self.handle_settings_key(key),
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
                    Event::Mouse(mouse) => {
                        self.handle_mouse_event(mouse);
                    }
                    Event::Paste(text) => {
                        // Bracketed paste: insert text at cursor regardless of mode
                        self.editor.textarea_mut().insert_str(&text);
                        self.mark_content_changed();
                    }
                    _ => {} // Event::Resize is a no-op — ratatui re-renders on next draw() (FOUND-06)
                }
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

        // Ctrl+C copies selection to system clipboard (nano mode)
        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('c') {
            let text = self.editor.get_selection_text();
            if !text.is_empty() {
                if self.clipboard.write(&text).is_ok() {
                    self.status_bar.set_message("Copied to clipboard");
                } else {
                    self.status_bar.set_message("Clipboard unavailable");
                }
                self.editor.textarea_mut().cancel_selection();
            }
            return;
        }

        // Ctrl+V pastes from system clipboard (nano mode)
        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('v') {
            if let Ok(text) = self.clipboard.read() {
                if !text.is_empty() {
                    self.editor.textarea_mut().insert_str(&text);
                    self.mark_content_changed();
                }
            }
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

    /// Jump to the next search match (for n in normal mode).
    fn find_next_match(&mut self) {
        if self.search_match_count > 0 {
            self.editor.textarea_mut().search_forward(false);
            self.search_match_index = (self.search_match_index + 1) % self.search_match_count;
            self.status_bar.set_message(&format!(
                "/{} [{}/{}]", self.search_query, self.search_match_index + 1, self.search_match_count
            ));
        }
    }

    /// Jump to the previous search match (for N in normal mode).
    fn find_prev_match(&mut self) {
        if self.search_match_count > 0 {
            self.editor.textarea_mut().search_back(false);
            self.search_match_index = if self.search_match_index == 0 {
                self.search_match_count - 1
            } else {
                self.search_match_index - 1
            };
            self.status_bar.set_message(&format!(
                "?{} [{}/{}]", self.search_query, self.search_match_index + 1, self.search_match_count
            ));
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

    /// Write text to both the internal vim yank register and the system clipboard.
    /// Shows a one-time warning if clipboard is unavailable.
    fn yank_to_clipboard(&mut self, text: &str) {
        if let Some(ref mut handler) = self.vim_handler {
            handler.set_yank_register(text.to_string());
        }
        if let Err(_e) = self.clipboard.write(text) {
            if !self.clipboard_warned {
                self.status_bar.set_message("System clipboard unavailable - using internal register");
                self.clipboard_warned = true;
            }
        }
    }

    /// Mark content as changed (sets dirty flag and edit timestamp).
    fn mark_content_changed(&mut self) {
        self.content_dirty = true;
        self.last_edit_time = Some(Instant::now());
    }

    /// Convert a Motion to a CursorMove for textarea operations.
    fn motion_to_cursor_move(motion: &Motion) -> CursorMove {
        match motion {
            Motion::Left => CursorMove::Back,
            Motion::Right => CursorMove::Forward,
            Motion::Up => CursorMove::Up,
            Motion::Down => CursorMove::Down,
            Motion::WordForward | Motion::WordEnd => CursorMove::WordForward,
            Motion::WordBack => CursorMove::WordBack,
            Motion::LineStart => CursorMove::Head,
            Motion::LineEnd => CursorMove::End,
            Motion::FileStart => CursorMove::Top,
            Motion::FileEnd => CursorMove::Bottom,
            Motion::ParagraphUp => CursorMove::ParagraphBack,
            Motion::ParagraphDown => CursorMove::ParagraphForward,
            Motion::Line | Motion::ToEnd | Motion::ToStart => CursorMove::Head, // handled specially
        }
    }

    /// Execute a VimCommand by translating it to textarea/editor operations.
    fn execute_vim_command(&mut self, cmd: VimCommand) {
        match cmd {
            VimCommand::Move(cursor_cmd) => {
                self.editor.textarea_mut().move_cursor(cursor_cmd.to_cursor_move());
            }
            VimCommand::MoveN(cursor_cmd, n) => {
                for _ in 0..n {
                    self.editor.textarea_mut().move_cursor(cursor_cmd.to_cursor_move());
                }
            }
            VimCommand::Delete { motion } => {
                self.execute_vim_operator_delete(motion);
            }
            VimCommand::Change { motion } => {
                self.execute_vim_operator_change(motion);
            }
            VimCommand::Yank { motion } => {
                self.execute_vim_operator_yank(motion);
            }
            VimCommand::DeleteChar => {
                self.editor.textarea_mut().delete_next_char();
                self.mark_content_changed();
            }
            VimCommand::PasteAfter => {
                let text = match self.clipboard.read() {
                    Ok(clip_text) if !clip_text.is_empty() => clip_text,
                    _ => self.vim_handler.as_ref()
                        .map(|h| h.yank_register().to_string())
                        .unwrap_or_default(),
                };
                if !text.is_empty() {
                    if text.ends_with('\n') {
                        // Line-wise paste: paste on next line
                        self.editor.textarea_mut().move_cursor(CursorMove::End);
                        self.editor.textarea_mut().insert_newline();
                        let trimmed = text.trim_end_matches('\n');
                        self.editor.textarea_mut().insert_str(trimmed);
                    } else {
                        self.editor.textarea_mut().move_cursor(CursorMove::Forward);
                        self.editor.textarea_mut().insert_str(&text);
                    }
                    self.mark_content_changed();
                }
            }
            VimCommand::PasteBefore => {
                let text = match self.clipboard.read() {
                    Ok(clip_text) if !clip_text.is_empty() => clip_text,
                    _ => self.vim_handler.as_ref()
                        .map(|h| h.yank_register().to_string())
                        .unwrap_or_default(),
                };
                if !text.is_empty() {
                    if text.ends_with('\n') {
                        // Line-wise paste: paste on previous line
                        self.editor.textarea_mut().move_cursor(CursorMove::Head);
                        self.editor.textarea_mut().insert_newline();
                        self.editor.textarea_mut().move_cursor(CursorMove::Up);
                        let trimmed = text.trim_end_matches('\n');
                        self.editor.textarea_mut().insert_str(trimmed);
                    } else {
                        self.editor.textarea_mut().insert_str(&text);
                    }
                    self.mark_content_changed();
                }
            }
            VimCommand::EnterInsert(pos) => {
                match pos {
                    InsertPosition::BeforeCursor => {}
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
                        self.mark_content_changed();
                    }
                    InsertPosition::NewLineAbove => {
                        self.editor.textarea_mut().move_cursor(CursorMove::Head);
                        self.editor.textarea_mut().insert_newline();
                        self.editor.textarea_mut().move_cursor(CursorMove::Up);
                        self.mark_content_changed();
                    }
                }
                self.mode = AppMode::Insert;
            }
            VimCommand::ExitInsert => {
                self.mode = AppMode::Normal;
            }
            VimCommand::EnterVisual { line_wise } => {
                if line_wise {
                    // Line-wise: select from start of current line
                    self.editor.textarea_mut().move_cursor(CursorMove::Head);
                    self.editor.textarea_mut().start_selection();
                    self.editor.textarea_mut().move_cursor(CursorMove::End);
                } else {
                    self.editor.textarea_mut().start_selection();
                }
                self.mode = AppMode::Visual;
            }
            VimCommand::ExitVisual => {
                self.editor.textarea_mut().cancel_selection();
                self.mode = AppMode::Normal;
            }
            VimCommand::EnterCommand => {
                self.mode = AppMode::Command;
            }
            VimCommand::ExitCommand => {
                self.mode = AppMode::Normal;
            }
            VimCommand::CommandExecute(ref cmd_str) => {
                let cmd_str = cmd_str.clone();
                self.execute_ex_command(&cmd_str);
                // Only reset to Normal if the command didn't switch to another mode
                if self.mode == AppMode::Command {
                    self.mode = AppMode::Normal;
                }
            }
            VimCommand::Save => {
                if self.editor.filepath().is_some() {
                    self.do_save();
                    self.mode = AppMode::Normal;
                } else {
                    self.filename_input = String::new();
                    self.quit_after_save = false;
                    self.mode = AppMode::PromptFilename;
                }
            }
            VimCommand::Quit { force } => {
                if force || !self.editor.is_modified() {
                    self.should_quit = true;
                } else {
                    self.mode = AppMode::ConfirmQuit;
                }
            }
            VimCommand::SaveAndQuit => {
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
            VimCommand::EnterSearch => {
                self.search_query.clear();
                self.search_cursor_before = self.editor.cursor_position();
                self.search_match_index = 0;
                self.search_match_count = 0;
                self.mode = AppMode::Search;
            }
            VimCommand::Undo => {
                self.editor.textarea_mut().undo();
                self.mark_content_changed();
            }
            VimCommand::Redo => {
                self.editor.textarea_mut().redo();
                self.mark_content_changed();
            }
            VimCommand::ContentChanged => {
                self.mark_content_changed();
            }
            VimCommand::VisualDelete => {
                let line_wise = self.vim_handler.as_ref()
                    .map(|h| h.was_visual_line_wise())
                    .unwrap_or(false);
                if line_wise {
                    // Line-wise: expand selection to full lines, then cut
                    if let Some(((sr, _), (er, _))) = self.editor.textarea_mut().selection_range() {
                        self.editor.textarea_mut().cancel_selection();
                        // Select full lines sr..=er
                        let mut yanked = String::new();
                        let lines = self.editor.textarea_mut().lines().to_vec();
                        for r in sr..=er {
                            if r < lines.len() {
                                yanked.push_str(&lines[r]);
                                yanked.push('\n');
                            }
                        }
                        self.yank_to_clipboard(&yanked);
                        // Delete lines from bottom to top to avoid index shift
                        self.editor.textarea_mut().move_cursor(CursorMove::Jump(sr as u16, 0));
                        for _ in sr..=er {
                            self.editor.delete_current_line();
                        }
                    }
                } else {
                    // Char-wise: yank selected text then cut
                    let text = self.editor.get_selection_text();
                    self.yank_to_clipboard(&text);
                    self.editor.textarea_mut().cut();
                }
                self.editor.textarea_mut().cancel_selection();
                self.mark_content_changed();
                self.mode = AppMode::Normal;
            }
            VimCommand::VisualChange => {
                let line_wise = self.vim_handler.as_ref()
                    .map(|h| h.was_visual_line_wise())
                    .unwrap_or(false);
                if line_wise {
                    if let Some(((sr, _), (er, _))) = self.editor.textarea_mut().selection_range() {
                        self.editor.textarea_mut().cancel_selection();
                        let mut yanked = String::new();
                        let lines = self.editor.textarea_mut().lines().to_vec();
                        for r in sr..=er {
                            if r < lines.len() {
                                yanked.push_str(&lines[r]);
                                yanked.push('\n');
                            }
                        }
                        self.yank_to_clipboard(&yanked);
                        // Delete all selected lines, then leave an empty line for insert
                        self.editor.textarea_mut().move_cursor(CursorMove::Jump(sr as u16, 0));
                        // Delete content of first line, then remaining lines
                        self.editor.delete_current_line_content();
                        for _ in (sr + 1)..=er {
                            // Delete the line below (now at position sr+1, but after deletion it shifts)
                            self.editor.textarea_mut().move_cursor(CursorMove::End);
                            self.editor.textarea_mut().delete_next_char(); // delete newline joining next line
                            self.editor.textarea_mut().start_selection();
                            self.editor.textarea_mut().move_cursor(CursorMove::End);
                            self.editor.textarea_mut().cut();
                        }
                    }
                } else {
                    let text = self.editor.get_selection_text();
                    self.yank_to_clipboard(&text);
                    self.editor.textarea_mut().cut();
                }
                self.editor.textarea_mut().cancel_selection();
                self.mark_content_changed();
                self.mode = AppMode::Insert;
                if let Some(handler) = self.vim_handler.as_mut() {
                    handler.set_mode_insert();
                }
            }
            VimCommand::VisualYank => {
                let text = self.editor.get_selection_text();
                self.yank_to_clipboard(&text);
                self.editor.textarea_mut().cancel_selection();
                self.mode = AppMode::Normal;
            }
            VimCommand::VisualIndent => {
                if let Some(((sr, _), (er, _))) = self.editor.textarea_mut().selection_range() {
                    self.editor.textarea_mut().cancel_selection();
                    for row in sr..=er {
                        self.editor.textarea_mut().move_cursor(CursorMove::Jump(row as u16, 0));
                        self.editor.textarea_mut().insert_str("  ");
                    }
                    // Re-select the range for continued visual mode operations
                    self.editor.textarea_mut().move_cursor(CursorMove::Jump(sr as u16, 0));
                    self.editor.textarea_mut().start_selection();
                    self.editor.textarea_mut().move_cursor(CursorMove::Jump(er as u16, 0));
                    self.editor.textarea_mut().move_cursor(CursorMove::End);
                    self.mark_content_changed();
                }
            }
            VimCommand::VisualOutdent => {
                if let Some(((sr, _), (er, _))) = self.editor.textarea_mut().selection_range() {
                    self.editor.textarea_mut().cancel_selection();
                    for row in sr..=er {
                        let line = self.editor.textarea_mut().lines()[row].clone();
                        let spaces = line.chars().take(2).take_while(|c| *c == ' ').count();
                        if spaces > 0 {
                            self.editor.textarea_mut().move_cursor(CursorMove::Jump(row as u16, 0));
                            for _ in 0..spaces {
                                self.editor.textarea_mut().delete_next_char();
                            }
                        }
                    }
                    // Re-select the range for continued visual mode operations
                    self.editor.textarea_mut().move_cursor(CursorMove::Jump(sr as u16, 0));
                    self.editor.textarea_mut().start_selection();
                    self.editor.textarea_mut().move_cursor(CursorMove::Jump(er as u16, 0));
                    self.editor.textarea_mut().move_cursor(CursorMove::End);
                    self.mark_content_changed();
                }
            }
            VimCommand::PageDown => {
                let page_size = self.editor_area.map(|a| a.height as usize).unwrap_or(20);
                for _ in 0..page_size {
                    self.editor.textarea_mut().move_cursor(CursorMove::Down);
                }
            }
            VimCommand::PageUp => {
                let page_size = self.editor_area.map(|a| a.height as usize).unwrap_or(20);
                for _ in 0..page_size {
                    self.editor.textarea_mut().move_cursor(CursorMove::Up);
                }
            }
            VimCommand::HalfPageDown => {
                let half = self.editor_area.map(|a| a.height as usize / 2).unwrap_or(10);
                for _ in 0..half {
                    self.editor.textarea_mut().move_cursor(CursorMove::Down);
                }
            }
            VimCommand::HalfPageUp => {
                let half = self.editor_area.map(|a| a.height as usize / 2).unwrap_or(10);
                for _ in 0..half {
                    self.editor.textarea_mut().move_cursor(CursorMove::Up);
                }
            }
            VimCommand::ReplaceChar(ch) => {
                self.editor.textarea_mut().delete_next_char();
                self.editor.textarea_mut().insert_char(ch);
                self.editor.textarea_mut().move_cursor(CursorMove::Back);
                self.mark_content_changed();
            }
            VimCommand::JoinLines => {
                let (row, _) = self.editor.textarea_mut().cursor();
                let total = self.editor.textarea_mut().lines().len();
                if row + 1 < total {
                    // Move to end of current line, delete newline, insert space
                    self.editor.textarea_mut().move_cursor(CursorMove::End);
                    self.editor.textarea_mut().delete_next_char(); // delete the newline
                    // Remove leading whitespace from the joined line
                    loop {
                        let col = self.editor.textarea_mut().cursor().1;
                        let ch = self.editor.textarea_mut().lines().get(row)
                            .and_then(|l| l.chars().nth(col));
                        match ch {
                            Some(' ') | Some('\t') => {
                                self.editor.textarea_mut().delete_next_char();
                            }
                            _ => break,
                        }
                    }
                    self.editor.textarea_mut().insert_char(' ');
                    self.mark_content_changed();
                }
            }
            VimCommand::SearchNext => {
                if !self.search_query.is_empty() {
                    self.find_next_match();
                }
            }
            VimCommand::SearchPrev => {
                if !self.search_query.is_empty() {
                    self.find_prev_match();
                }
            }
            VimCommand::None | VimCommand::CommandAppend(_) | VimCommand::CommandBackspace => {}
        }
    }

    /// Execute a delete operator with a motion.
    fn execute_vim_operator_delete(&mut self, motion: Motion) {
        match motion {
            Motion::Line => {
                // dd: delete current line
                let text = self.editor.delete_current_line();
                self.yank_to_clipboard(&text);
                self.mark_content_changed();
            }
            Motion::ToEnd => {
                // D / d$: select to end of line, delete
                self.editor.textarea_mut().start_selection();
                self.editor.textarea_mut().move_cursor(CursorMove::End);
                let text = self.editor.cut_selection();
                self.yank_to_clipboard(&text);
                self.mark_content_changed();
            }
            Motion::ToStart => {
                // d0: select to start of line, delete
                self.editor.textarea_mut().start_selection();
                self.editor.textarea_mut().move_cursor(CursorMove::Head);
                let text = self.editor.cut_selection();
                self.yank_to_clipboard(&text);
                self.mark_content_changed();
            }
            other => {
                let cursor_move = Self::motion_to_cursor_move(&other);
                self.editor.textarea_mut().start_selection();
                self.editor.textarea_mut().move_cursor(cursor_move);
                let text = self.editor.cut_selection();
                self.yank_to_clipboard(&text);
                self.mark_content_changed();
            }
        }
    }

    /// Execute a change operator with a motion (delete + enter insert mode).
    fn execute_vim_operator_change(&mut self, motion: Motion) {
        match motion {
            Motion::Line => {
                // cc: delete line content but keep the line, enter insert
                let text = self.editor.delete_current_line_content();
                self.yank_to_clipboard(&text);
                self.mark_content_changed();
                self.mode = AppMode::Insert;
                if let Some(handler) = self.vim_handler.as_mut() {
                    handler.set_mode_insert();
                }
            }
            Motion::ToEnd => {
                self.editor.textarea_mut().start_selection();
                self.editor.textarea_mut().move_cursor(CursorMove::End);
                let text = self.editor.cut_selection();
                self.yank_to_clipboard(&text);
                self.mark_content_changed();
                self.mode = AppMode::Insert;
                if let Some(handler) = self.vim_handler.as_mut() {
                    handler.set_mode_insert();
                }
            }
            other => {
                let cursor_move = Self::motion_to_cursor_move(&other);
                self.editor.textarea_mut().start_selection();
                self.editor.textarea_mut().move_cursor(cursor_move);
                let text = self.editor.cut_selection();
                self.yank_to_clipboard(&text);
                self.mark_content_changed();
                self.mode = AppMode::Insert;
                if let Some(handler) = self.vim_handler.as_mut() {
                    handler.set_mode_insert();
                }
            }
        }
    }

    /// Execute a yank operator with a motion (copy without deleting).
    fn execute_vim_operator_yank(&mut self, motion: Motion) {
        match motion {
            Motion::Line => {
                // yy: yank current line
                let text = self.editor.yank_current_line();
                self.yank_to_clipboard(&text);
            }
            Motion::ToEnd => {
                self.editor.textarea_mut().start_selection();
                self.editor.textarea_mut().move_cursor(CursorMove::End);
                let text = self.editor.get_selection_text();
                self.editor.textarea_mut().cancel_selection();
                self.yank_to_clipboard(&text);
            }
            other => {
                let cursor_move = Self::motion_to_cursor_move(&other);
                self.editor.textarea_mut().start_selection();
                self.editor.textarea_mut().move_cursor(cursor_move);
                let text = self.editor.get_selection_text();
                self.editor.textarea_mut().cancel_selection();
                self.yank_to_clipboard(&text);
            }
        }
    }

    /// Execute an ex command (typed after ':').
    fn execute_ex_command(&mut self, cmd: &str) {
        match cmd.trim() {
            "w" => {
                if self.editor.filepath().is_some() {
                    self.do_save();
                } else {
                    self.filename_input = String::new();
                    self.quit_after_save = false;
                    self.mode = AppMode::PromptFilename;
                }
            }
            "q" => {
                if self.editor.is_modified() {
                    self.status_bar.set_message("No write since last change (add ! to override)");
                } else {
                    self.should_quit = true;
                }
            }
            "q!" => {
                self.should_quit = true;
            }
            "wq" | "x" => {
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
            other if other.starts_with("set ") || other == "set" => {
                self.execute_set_command(other);
            }
            other => {
                // Try parsing as a line number (e.g. :42)
                if let Ok(line_num) = other.trim().parse::<usize>() {
                    let target = if line_num == 0 { 0 } else { line_num - 1 };
                    let max_line = self.editor.textarea_mut().lines().len().saturating_sub(1);
                    let target = target.min(max_line);
                    self.editor.textarea_mut().move_cursor(ratatui_textarea::CursorMove::Jump(target as u16, 0));
                } else {
                    self.status_bar.set_message(&format!("Not a command: {}", other));
                }
            }
        }
    }

    /// Handle :set commands for live configuration.
    ///
    /// Supported:
    ///   :set                    -- show current settings
    ///   :set theme <name>       -- switch theme (ocean, dracula, solarized-light, gruvbox-dark)
    ///   :set mode <vim|nano>    -- switch editing mode (takes effect on next launch)
    ///   :set save               -- persist current settings to config file
    fn execute_set_command(&mut self, cmd: &str) {
        let args: Vec<&str> = cmd.split_whitespace().collect();
        match args.as_slice() {
            // :set -- open settings panel
            ["set"] => {
                self.settings_row = SettingsRow::Theme;
                self.mode = AppMode::Settings;
            }
            // :set theme <name>
            ["set", "theme", name] => {
                let name = name.to_lowercase();
                if let Some(new_theme) = crate::theme::Theme::by_name(&name) {
                    let cap = crate::theme::detect_color_capability();
                    let applied = if cap == crate::theme::ColorCapability::Color256 {
                        new_theme.with_256_color_fallback()
                    } else {
                        new_theme
                    };
                    self.editor.apply_theme(applied.clone());
                    self.theme = applied;
                    self.config.theme = name.clone();
                    self.content_dirty = true;
                    self.status_bar.set_message(&format!("Theme set to {}", name));
                } else {
                    let available = crate::theme::Theme::available_themes().join(", ");
                    self.status_bar.set_message(
                        &format!("Unknown theme '{}'. Available: {}", name, available),
                    );
                }
            }
            // :set mode <vim|nano>
            ["set", "mode", mode] => {
                match mode.to_lowercase().as_str() {
                    "vim" => {
                        self.config.mode = crate::config::EditingMode::Vim;
                        self.status_bar.set_message("Mode set to vim (restart to apply)");
                    }
                    "nano" => {
                        self.config.mode = crate::config::EditingMode::Nano;
                        self.status_bar.set_message("Mode set to nano (restart to apply)");
                    }
                    other => {
                        self.status_bar.set_message(
                            &format!("Unknown mode '{}'. Available: vim, nano", other),
                        );
                    }
                }
            }
            // :set profile <name>
            ["set", "profile", name] => {
                if let Some(profile) = crate::config::RenderProfile::from_name(name) {
                    self.config.render_profile = profile;
                    self.settings_profile_index = crate::config::RenderProfile::all()
                        .iter()
                        .position(|p| *p == profile)
                        .unwrap_or(0);
                    self.content_dirty = true;
                    self.status_bar.set_message(&format!("Preview profile set to {}", profile.display_name()));
                } else {
                    let available: Vec<&str> = crate::config::RenderProfile::all()
                        .iter()
                        .map(|p| p.name())
                        .collect();
                    self.status_bar.set_message(
                        &format!("Unknown profile '{}'. Available: {}", name, available.join(", ")),
                    );
                }
            }
            // :set wrap -- toggle word wrap
            ["set", "wrap"] => {
                self.config.word_wrap = !self.config.word_wrap;
                self.editor.word_wrap = self.config.word_wrap;
                let state = if self.config.word_wrap { "on" } else { "off" };
                self.status_bar.set_message(&format!("Word wrap {}", state));
            }
            // :set save -- persist to config file
            ["set", "save"] => {
                match crate::config::save_config(&self.config) {
                    Ok(path) => {
                        self.status_bar.set_message(&format!("Config saved to {}", path));
                    }
                    Err(e) => {
                        self.status_bar.set_message(&format!("Failed to save: {}", e));
                    }
                }
            }
            _ => {
                self.status_bar.set_message("Usage: :set [theme <name>|mode <vim|nano>|profile <..>|wrap|save]");
            }
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

        let cmd = self.vim_handler.as_mut()
            .map(|h| h.handle_key(key))
            .unwrap_or(VimCommand::None);
        self.execute_vim_command(cmd);
    }

    /// Handle key events in Vim Insert mode.
    fn handle_vim_insert_key(&mut self, key: crossterm::event::KeyEvent) {
        let cmd = self.vim_handler.as_mut()
            .map(|h| h.handle_key(key))
            .unwrap_or(VimCommand::None);

        match cmd {
            VimCommand::ExitInsert => {
                self.execute_vim_command(cmd);
            }
            VimCommand::None => {
                // Forward all non-Esc keys to textarea for text input
                let changed = self.editor.textarea_mut().input_without_shortcuts(key);
                if changed {
                    self.mark_content_changed();
                }
            }
            _ => {
                self.execute_vim_command(cmd);
            }
        }
    }

    /// Handle key events in Vim Visual mode.
    fn handle_vim_visual_key(&mut self, key: crossterm::event::KeyEvent) {
        // Ctrl+P toggles layout mode
        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('p') {
            self.layout_mode = self.layout_mode.next();
            self.status_bar.set_message(self.layout_mode.label());
            return;
        }

        let cmd = self.vim_handler.as_mut()
            .map(|h| h.handle_key(key))
            .unwrap_or(VimCommand::None);

        match cmd {
            // Motions extend selection (selection is active, just move cursor)
            VimCommand::Move(cursor_cmd) => {
                self.editor.textarea_mut().move_cursor(cursor_cmd.to_cursor_move());
            }
            VimCommand::MoveN(cursor_cmd, n) => {
                for _ in 0..n {
                    self.editor.textarea_mut().move_cursor(cursor_cmd.to_cursor_move());
                }
            }
            // All other commands (ExitVisual, VisualDelete, etc.) go through execute
            VimCommand::None => {}
            other => {
                self.execute_vim_command(other);
            }
        }
    }

    /// Handle key events in Vim Command mode.
    fn handle_vim_command_key(&mut self, key: crossterm::event::KeyEvent) {
        let cmd = self.vim_handler.as_mut()
            .map(|h| h.handle_key(key))
            .unwrap_or(VimCommand::None);
        self.execute_vim_command(cmd);
    }

    /// Handle mouse events: click, scroll, drag-select, divider drag (D-19 through D-23).
    fn handle_mouse_event(&mut self, mouse: crossterm::event::MouseEvent) {
        use crossterm::event::{MouseEventKind, MouseButton};

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let col = mouse.column;
                let row = mouse.row;

                // Check if click is on divider (D-22)
                if let Some(div) = self.divider_area {
                    if col >= div.x && col < div.x + div.width && row >= div.y && row < div.y + div.height {
                        self.dragging_divider = true;
                        return;
                    }
                }

                // Check if click is in editor area (D-19)
                if let Some(editor) = self.editor_area {
                    if col >= editor.x && col < editor.x + editor.width && row >= editor.y && row < editor.y + editor.height {
                        self.click_to_editor_cursor(col, row, &editor);
                        return;
                    }
                }

                // Click in preview area -- no action (preview is read-only)
            }

            MouseEventKind::Drag(MouseButton::Left) => {
                let col = mouse.column;
                let row = mouse.row;

                // Divider drag (D-22)
                if self.dragging_divider {
                    if let Some(editor) = self.editor_area {
                        // Calculate new split ratio from mouse x position
                        let total_width = editor.width + 1 + self.preview_area.map(|p| p.width).unwrap_or(0);
                        if total_width > 0 {
                            let editor_x_start = editor.x;
                            let relative_x = col.saturating_sub(editor_x_start);
                            let new_ratio = ((relative_x as f32 / total_width as f32) * 100.0) as u16;
                            // Clamp to reasonable bounds (min 20%, max 80%)
                            self.split_ratio = new_ratio.clamp(20, 80);
                        }
                    }
                    return;
                }

                // Text drag selection in editor (D-21)
                // Stay in current mode — don't enter Visual. Just track selection
                // internally and copy to clipboard on mouse-up.
                if let Some(editor) = self.editor_area {
                    // Clamp to editor area to prevent runaway scrolling
                    let clamped_col = col.clamp(editor.x, editor.x + editor.width - 1);
                    let clamped_row = row.clamp(editor.y, editor.y + editor.height - 1);
                    if !self.drag_selecting {
                        self.editor.textarea_mut().start_selection();
                        self.drag_selecting = true;
                    }
                    self.click_to_editor_cursor(clamped_col, clamped_row, &editor);
                }
            }

            MouseEventKind::Up(MouseButton::Left) => {
                if self.drag_selecting {
                    // Copy selection to clipboard
                    let text = self.editor.get_selection_text();
                    if !text.is_empty() {
                        if self.vim_handler.is_some() {
                            self.yank_to_clipboard(&text);
                        } else {
                            let _ = self.clipboard.write(&text);
                        }
                        self.status_bar.set_message("Copied to clipboard");
                    }
                    self.editor.textarea_mut().cancel_selection();
                }
                self.dragging_divider = false;
                self.drag_selecting = false;
            }

            MouseEventKind::ScrollUp => {
                let col = mouse.column;
                // Determine which pane to scroll (D-20)
                if let Some(editor) = self.editor_area {
                    if col >= editor.x && col < editor.x + editor.width {
                        // Scroll editor up by 3 lines
                        for _ in 0..3 {
                            self.editor.textarea_mut().move_cursor(CursorMove::Up);
                        }
                        return;
                    }
                }
                if let Some(preview) = self.preview_area {
                    if col >= preview.x && col < preview.x + preview.width {
                        self.preview.scroll_up(3);
                    }
                }
            }

            MouseEventKind::ScrollDown => {
                let col = mouse.column;
                if let Some(editor) = self.editor_area {
                    if col >= editor.x && col < editor.x + editor.width {
                        for _ in 0..3 {
                            self.editor.textarea_mut().move_cursor(CursorMove::Down);
                        }
                        return;
                    }
                }
                if let Some(preview) = self.preview_area {
                    if col >= preview.x && col < preview.x + preview.width {
                        self.preview.scroll_down(3);
                    }
                }
            }

            _ => {} // Other mouse events ignored
        }
    }

    /// Translate a screen click position to an editor cursor position (D-19).
    fn click_to_editor_cursor(&mut self, screen_col: u16, screen_row: u16, editor_area: &Rect) {
        // Calculate editor-relative position
        let relative_row = (screen_row - editor_area.y) as usize;
        let relative_col = (screen_col - editor_area.x) as usize;

        // Account for line number gutter width
        let total_lines = self.editor.line_count();
        let lnum_width = crate::highlighter::line_number_width(total_lines);

        // Calculate actual text column (subtract line number width)
        let text_col = if relative_col > lnum_width {
            relative_col - lnum_width
        } else {
            0
        };

        // Convert visual click to logical position (handles word wrap)
        let content_width = (editor_area.width as usize).saturating_sub(lnum_width);
        let (clamped_row, clamped_col) = self.editor.visual_click_to_logical(relative_row, text_col, content_width);

        // Cancel any existing selection if not drag-selecting
        if !self.drag_selecting {
            self.editor.textarea_mut().cancel_selection();
        }

        self.editor.textarea_mut().move_cursor(CursorMove::Jump(clamped_row as u16, clamped_col as u16));
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
                        Constraint::Percentage(self.split_ratio),
                        Constraint::Length(1),
                        Constraint::Percentage(100 - self.split_ratio),
                    ])
                    .split(body_area);

                // Store areas for mouse hit testing
                self.editor_area = Some(chunks[0]);
                self.divider_area = Some(chunks[1]);
                self.preview_area = Some(chunks[2]);

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

                // Compute highlight line for sync indicator
                let highlight = if self.config.sync_indicator {
                    let (cursor_row, _) = self.editor.cursor_position();
                    if !self.source_to_preview.is_empty() && cursor_row < self.source_to_preview.len() {
                        Some(self.source_to_preview[cursor_row] as u16)
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Preview right
                self.preview.render(frame, chunks[2], &self.preview_text, highlight);
            }
            LayoutMode::EditorOnly => {
                self.editor_area = Some(body_area);
                self.divider_area = None;
                self.preview_area = None;
                self.editor.render_highlighted(frame, body_area, &search_query);
            }
            LayoutMode::PreviewOnly => {
                self.editor_area = None;
                self.divider_area = None;
                self.preview_area = Some(body_area);
                self.preview.render(frame, body_area, &self.preview_text, None);
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
            AppMode::Settings => {
                // Status bar hint
                let bar = Paragraph::new(Span::raw(
                    " Settings: \u{2191}\u{2193} Navigate | \u{2190}\u{2192} Change | Enter Save & Close | Esc Cancel",
                ))
                .style(Style::default().bg(self.theme.prompt_bg).fg(self.theme.prompt_fg));
                frame.render_widget(bar, status_area);
            }
        }

        // Settings overlay (rendered on top of everything)
        if self.mode == AppMode::Settings {
            self.render_settings_overlay(frame, body_area);
        }
    }

    /// Render the settings overlay panel centered on screen.
    fn render_settings_overlay(&self, frame: &mut Frame, area: Rect) {
        use ratatui::widgets::{Block, Borders, Clear};

        let themes = crate::theme::Theme::available_themes();
        let modes = ["vim", "nano"];
        let profiles = crate::config::RenderProfile::all();

        // Center a box
        let popup_w = 50u16;
        let popup_h = 17u16;
        let x = area.x + area.width.saturating_sub(popup_w) / 2;
        let y = area.y + area.height.saturating_sub(popup_h) / 2;
        let popup_area = Rect::new(x, y, popup_w.min(area.width), popup_h.min(area.height));

        // Clear background
        frame.render_widget(Clear, popup_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.divider_fg))
            .title(" Settings ")
            .title_style(Style::default().fg(self.theme.status_bar_fg).add_modifier(ratatui::style::Modifier::BOLD))
            .style(Style::default().bg(self.theme.status_bar_bg));
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let normal_style = Style::default().fg(self.theme.status_bar_fg).bg(self.theme.status_bar_bg);
        let selected_style = Style::default().fg(self.theme.search_active_fg).bg(self.theme.search_active_bg).add_modifier(ratatui::style::Modifier::BOLD);
        let value_style = Style::default().fg(self.theme.search_active_bg).bg(self.theme.status_bar_bg);

        let theme_default = if self.settings_theme_index == 0 { " (default)" } else { "" };
        let mode_default = if self.settings_mode_index == 0 { " (default)" } else { "" };
        let profile_default = if self.settings_profile_index == 0 { " (default)" } else { "" };

        let sync_label = if self.config.sync_indicator { "On (default)" } else { "Off" };
        let wrap_label = if self.config.word_wrap { "On" } else { "Off (default)" };

        let rows = [
            (SettingsRow::Theme, "Theme", format!("\u{25C0} {}{} \u{25B6}", themes[self.settings_theme_index], theme_default)),
            (SettingsRow::Mode, "Mode", format!("\u{25C0} {}{} \u{25B6}", modes[self.settings_mode_index], mode_default)),
            (SettingsRow::Profile, "Preview", format!("\u{25C0} {}{} \u{25B6}", profiles[self.settings_profile_index].display_name(), profile_default)),
            (SettingsRow::SyncIndicator, "Sync Line", format!("\u{25C0} {} \u{25B6}", sync_label)),
            (SettingsRow::WordWrap, "Word Wrap", format!("\u{25C0} {} \u{25B6}", wrap_label)),
            (SettingsRow::Save, "", "[ Save to config ]".to_string()),
            (SettingsRow::Reset, "", "[ Reset defaults ]".to_string()),
        ];

        for (i, (row_id, label, value)) in rows.iter().enumerate() {
            let y_offset = i as u16 * 2;
            if y_offset >= inner.height {
                break;
            }
            let row_area = Rect::new(inner.x, inner.y + y_offset, inner.width, 1);
            let is_selected = *row_id == self.settings_row;

            if *row_id == SettingsRow::Save || *row_id == SettingsRow::Reset {
                // Center the save button
                let padding = inner.width.saturating_sub(value.len() as u16) / 2;
                let padded = format!("{:>w$}{}", "", value, w = padding as usize);
                let style = if is_selected { selected_style } else { normal_style };
                frame.render_widget(Paragraph::new(Span::styled(padded, style)), row_area);
            } else {
                let label_w = 10;
                let line = ratatui::text::Line::from(vec![
                    Span::styled(
                        format!("  {:<w$}", label, w = label_w),
                        if is_selected { selected_style } else { normal_style },
                    ),
                    Span::styled(
                        value.clone(),
                        if is_selected { selected_style } else { value_style },
                    ),
                ]);
                frame.render_widget(Paragraph::new(line), row_area);
            }
        }
    }

    /// Handle key events in the settings overlay.
    fn handle_settings_key(&mut self, key: crossterm::event::KeyEvent) {
        let themes = crate::theme::Theme::available_themes();
        let n_themes = themes.len();
        let profiles = crate::config::RenderProfile::all();
        let n_profiles = profiles.len();

        match key.code {
            KeyCode::Esc => {
                // Cancel — restore to what config had before opening
                self.mode = if self.editing_mode == crate::config::EditingMode::Vim {
                    AppMode::Normal
                } else {
                    AppMode::Editing
                };
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.settings_row = match self.settings_row {
                    SettingsRow::Theme => SettingsRow::Reset,
                    SettingsRow::Mode => SettingsRow::Theme,
                    SettingsRow::Profile => SettingsRow::Mode,
                    SettingsRow::SyncIndicator => SettingsRow::Profile,
                    SettingsRow::WordWrap => SettingsRow::SyncIndicator,
                    SettingsRow::Save => SettingsRow::WordWrap,
                    SettingsRow::Reset => SettingsRow::Save,
                };
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.settings_row = match self.settings_row {
                    SettingsRow::Theme => SettingsRow::Mode,
                    SettingsRow::Mode => SettingsRow::Profile,
                    SettingsRow::Profile => SettingsRow::SyncIndicator,
                    SettingsRow::SyncIndicator => SettingsRow::WordWrap,
                    SettingsRow::WordWrap => SettingsRow::Save,
                    SettingsRow::Save => SettingsRow::Reset,
                    SettingsRow::Reset => SettingsRow::Theme,
                };
            }
            KeyCode::Left | KeyCode::Char('h') => {
                match self.settings_row {
                    SettingsRow::Theme => {
                        self.settings_theme_index = if self.settings_theme_index == 0 {
                            n_themes - 1
                        } else {
                            self.settings_theme_index - 1
                        };
                        self.apply_settings_theme(themes[self.settings_theme_index]);
                    }
                    SettingsRow::Mode => {
                        self.settings_mode_index = 1 - self.settings_mode_index;
                    }
                    SettingsRow::Profile => {
                        self.settings_profile_index = if self.settings_profile_index == 0 {
                            n_profiles - 1
                        } else {
                            self.settings_profile_index - 1
                        };
                        self.config.render_profile = profiles[self.settings_profile_index];
                        self.content_dirty = true;
                    }
                    SettingsRow::SyncIndicator => {
                        self.config.sync_indicator = !self.config.sync_indicator;
                    }
                    SettingsRow::WordWrap => {
                        self.config.word_wrap = !self.config.word_wrap;
                        self.editor.word_wrap = self.config.word_wrap;
                    }
                    SettingsRow::Save | SettingsRow::Reset => {}
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                match self.settings_row {
                    SettingsRow::Theme => {
                        self.settings_theme_index = (self.settings_theme_index + 1) % n_themes;
                        self.apply_settings_theme(themes[self.settings_theme_index]);
                    }
                    SettingsRow::Mode => {
                        self.settings_mode_index = 1 - self.settings_mode_index;
                    }
                    SettingsRow::Profile => {
                        self.settings_profile_index = (self.settings_profile_index + 1) % n_profiles;
                        self.config.render_profile = profiles[self.settings_profile_index];
                        self.content_dirty = true;
                    }
                    SettingsRow::SyncIndicator => {
                        self.config.sync_indicator = !self.config.sync_indicator;
                    }
                    SettingsRow::WordWrap => {
                        self.config.word_wrap = !self.config.word_wrap;
                        self.editor.word_wrap = self.config.word_wrap;
                    }
                    SettingsRow::Save | SettingsRow::Reset => {}
                }
            }
            KeyCode::Enter => {
                match self.settings_row {
                    SettingsRow::Save => {
                        // Persist to config file
                        self.config.mode = if self.settings_mode_index == 0 {
                            crate::config::EditingMode::Vim
                        } else {
                            crate::config::EditingMode::Nano
                        };
                        self.config.render_profile = profiles[self.settings_profile_index];
                        match crate::config::save_config(&self.config) {
                            Ok(path) => self.status_bar.set_message(&format!("Saved to {}", path)),
                            Err(e) => self.status_bar.set_message(&format!("Error: {}", e)),
                        }
                        self.mode = if self.editing_mode == crate::config::EditingMode::Vim {
                            AppMode::Normal
                        } else {
                            AppMode::Editing
                        };
                    }
                    SettingsRow::Reset => {
                        // Reset to defaults
                        self.settings_theme_index = 0; // ocean
                        self.settings_mode_index = 0;  // vim
                        self.settings_profile_index = 0; // github
                        self.apply_settings_theme("ocean");
                        self.config.mode = crate::config::EditingMode::Vim;
                        self.config.theme = "ocean".to_string();
                        self.config.render_profile = crate::config::RenderProfile::Github;
                        self.config.sync_indicator = true;
                        self.config.word_wrap = false;
                        self.editor.word_wrap = false;
                        self.content_dirty = true;
                        self.status_bar.set_message("Reset to defaults");
                    }
                    _ => {
                        // Enter on Theme/Mode/Profile row — close settings
                        self.config.mode = if self.settings_mode_index == 0 {
                            crate::config::EditingMode::Vim
                        } else {
                            crate::config::EditingMode::Nano
                        };
                        self.config.render_profile = profiles[self.settings_profile_index];
                        self.mode = if self.editing_mode == crate::config::EditingMode::Vim {
                            AppMode::Normal
                        } else {
                            AppMode::Editing
                        };
                    }
                }
            }
            _ => {}
        }
    }

    /// Apply a theme by name from the settings panel (live preview).
    fn apply_settings_theme(&mut self, name: &str) {
        if let Some(new_theme) = crate::theme::Theme::by_name(name) {
            let cap = crate::theme::detect_color_capability();
            let applied = if cap == crate::theme::ColorCapability::Color256 {
                new_theme.with_256_color_fallback()
            } else {
                new_theme
            };
            self.editor.apply_theme(applied.clone());
            self.theme = applied;
            self.config.theme = name.to_string();
            self.content_dirty = true;
        }
    }
}
