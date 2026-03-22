use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::editor::{Editor, EditorAction};
use crate::file_io;
use crate::markdown::{MarkdownRenderer, TuiMarkdownRenderer};
use crate::preview::Preview;
use crate::status_bar::StatusBar;

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
    /// Normal editing mode.
    Editing,
    /// Prompt: "Unsaved changes. Save? (y/n/Esc)" (D-13)
    ConfirmQuit,
    /// Prompt: "Save as: ___" for untitled buffers (D-02)
    PromptFilename,
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
}

impl<'a> App<'a> {
    pub fn new(content: Option<String>, filepath: Option<PathBuf>) -> Self {
        App {
            editor: Editor::new(content, filepath),
            mode: AppMode::Editing,
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
                            AppMode::ConfirmQuit => self.handle_confirm_quit_key(key),
                            AppMode::PromptFilename => self.handle_prompt_filename_key(key),
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
                        self.mode = AppMode::Editing;
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
                self.mode = AppMode::Editing;
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
                            self.mode = AppMode::Editing;
                        }
                    } else {
                        // Save failed — return to editing
                        self.mode = AppMode::Editing;
                    }
                    self.quit_after_save = false;
                }
            }
            KeyCode::Esc => {
                self.filename_input.clear();
                self.quit_after_save = false;
                self.mode = AppMode::Editing;
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

    /// Render the editor, preview, and status bar based on current layout mode.
    fn render(&mut self, frame: &mut Frame) {
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Fill(1), Constraint::Length(1)])
            .split(frame.area());

        let body_area = outer[0];
        let status_area = outer[1];

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
                self.editor.render_highlighted(frame, chunks[0]);

                // Divider (D-11: subtle dimmed vertical line)
                let divider_lines: Vec<ratatui::text::Line> = (0..chunks[1].height)
                    .map(|_| {
                        ratatui::text::Line::from(Span::styled(
                            "\u{2502}",
                            Style::default().fg(Color::DarkGray),
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
                self.editor.render_highlighted(frame, body_area);
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
                );
            }
            AppMode::ConfirmQuit => {
                let bar = Paragraph::new(Span::raw(
                    " Unsaved changes. Save? (y/n/Esc)",
                ))
                .style(Style::default().bg(Color::Red).fg(Color::White));
                frame.render_widget(bar, status_area);
            }
            AppMode::PromptFilename => {
                let prompt = format!(" Save as: {}_", self.filename_input);
                let bar = Paragraph::new(Span::raw(prompt))
                    .style(Style::default().bg(Color::Blue).fg(Color::White));
                frame.render_widget(bar, status_area);
            }
        }
    }
}
