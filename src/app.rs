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
            split_ratio: 50,
            editor_area: None,
            preview_area: None,
            divider_area: None,
            dragging_divider: false,
            drag_selecting: false,
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
                let text = self.vim_handler.as_ref()
                    .map(|h| h.yank_register().to_string())
                    .unwrap_or_default();
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
                let text = self.vim_handler.as_ref()
                    .map(|h| h.yank_register().to_string())
                    .unwrap_or_default();
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
                self.mode = AppMode::Normal;
            }
            VimCommand::Save => {
                if self.editor.filepath().is_some() {
                    self.do_save();
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
                        if let Some(handler) = self.vim_handler.as_mut() {
                            handler.set_yank_register(yanked);
                        }
                        // Delete lines from bottom to top to avoid index shift
                        self.editor.textarea_mut().move_cursor(CursorMove::Jump(sr as u16, 0));
                        for _ in sr..=er {
                            self.editor.delete_current_line();
                        }
                    }
                } else {
                    // Char-wise: yank selected text then cut
                    let text = self.editor.get_selection_text();
                    if let Some(handler) = self.vim_handler.as_mut() {
                        handler.set_yank_register(text);
                    }
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
                        if let Some(handler) = self.vim_handler.as_mut() {
                            handler.set_yank_register(yanked);
                        }
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
                    if let Some(handler) = self.vim_handler.as_mut() {
                        handler.set_yank_register(text);
                    }
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
                if let Some(handler) = self.vim_handler.as_mut() {
                    handler.set_yank_register(text);
                }
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
            VimCommand::None | VimCommand::CommandAppend(_) | VimCommand::CommandBackspace => {}
        }
    }

    /// Execute a delete operator with a motion.
    fn execute_vim_operator_delete(&mut self, motion: Motion) {
        match motion {
            Motion::Line => {
                // dd: delete current line
                let text = self.editor.delete_current_line();
                if let Some(handler) = self.vim_handler.as_mut() {
                    handler.set_yank_register(text);
                }
                self.mark_content_changed();
            }
            Motion::ToEnd => {
                // D / d$: select to end of line, delete
                self.editor.textarea_mut().start_selection();
                self.editor.textarea_mut().move_cursor(CursorMove::End);
                let text = self.editor.cut_selection();
                if let Some(handler) = self.vim_handler.as_mut() {
                    handler.set_yank_register(text);
                }
                self.mark_content_changed();
            }
            Motion::ToStart => {
                // d0: select to start of line, delete
                self.editor.textarea_mut().start_selection();
                self.editor.textarea_mut().move_cursor(CursorMove::Head);
                let text = self.editor.cut_selection();
                if let Some(handler) = self.vim_handler.as_mut() {
                    handler.set_yank_register(text);
                }
                self.mark_content_changed();
            }
            other => {
                let cursor_move = Self::motion_to_cursor_move(&other);
                self.editor.textarea_mut().start_selection();
                self.editor.textarea_mut().move_cursor(cursor_move);
                let text = self.editor.cut_selection();
                if let Some(handler) = self.vim_handler.as_mut() {
                    handler.set_yank_register(text);
                }
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
                if let Some(handler) = self.vim_handler.as_mut() {
                    handler.set_yank_register(text);
                }
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
                if let Some(handler) = self.vim_handler.as_mut() {
                    handler.set_yank_register(text);
                }
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
                if let Some(handler) = self.vim_handler.as_mut() {
                    handler.set_yank_register(text);
                }
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
                if let Some(handler) = self.vim_handler.as_mut() {
                    handler.set_yank_register(text);
                }
            }
            Motion::ToEnd => {
                self.editor.textarea_mut().start_selection();
                self.editor.textarea_mut().move_cursor(CursorMove::End);
                let text = self.editor.get_selection_text();
                self.editor.textarea_mut().cancel_selection();
                if let Some(handler) = self.vim_handler.as_mut() {
                    handler.set_yank_register(text);
                }
            }
            other => {
                let cursor_move = Self::motion_to_cursor_move(&other);
                self.editor.textarea_mut().start_selection();
                self.editor.textarea_mut().move_cursor(cursor_move);
                let text = self.editor.get_selection_text();
                self.editor.textarea_mut().cancel_selection();
                if let Some(handler) = self.vim_handler.as_mut() {
                    handler.set_yank_register(text);
                }
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
            other => {
                self.status_bar.set_message(&format!("Not a command: {}", other));
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
                if let Some(editor) = self.editor_area {
                    if col >= editor.x && col < editor.x + editor.width && row >= editor.y && row < editor.y + editor.height {
                        if !self.drag_selecting {
                            // Start selection on first drag event
                            self.editor.textarea_mut().start_selection();
                            self.drag_selecting = true;
                            // In vim mode, enter Visual mode (D-21)
                            if let Some(ref mut handler) = self.vim_handler {
                                handler.set_mode_visual(false); // char-wise
                                self.mode = AppMode::Visual;
                            }
                        }
                        // Move cursor to drag position (extends selection)
                        self.click_to_editor_cursor(col, row, &editor);
                    }
                }
            }

            MouseEventKind::Up(MouseButton::Left) => {
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

        // Calculate actual line (add scroll offset)
        let scroll_top = self.editor.scroll_top();
        let target_row = scroll_top + relative_row;

        // Clamp to valid range
        let max_row = total_lines.saturating_sub(1);
        let clamped_row = target_row.min(max_row);

        // Clamp column to line length
        let line_len = self.editor.textarea_mut().lines().get(clamped_row).map(|l| l.len()).unwrap_or(0);
        let clamped_col = text_col.min(line_len);

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

                // Preview right
                self.preview.render(frame, chunks[2], &self.preview_text);
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
