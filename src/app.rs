use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::path::PathBuf;
use std::time::Duration;

use crate::editor::{Editor, EditorAction};
use crate::file_io;
use crate::status_bar::StatusBar;

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
        }
    }

    /// Main event loop. Draws the UI and processes events until quit.
    pub fn run(&mut self, terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
        loop {
            terminal.draw(|frame| self.render(frame))?;

            // Poll with 50ms timeout so timed status messages can expire
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
                    // No-op — status bar reads modified flag directly
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

    /// Render the editor and status bar.
    fn render(&self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Fill(1),   // Editor area
                Constraint::Length(1), // Status bar
            ])
            .split(frame.area());

        // Render editor widget
        frame.render_widget(self.editor.widget(), chunks[0]);

        // Render status bar based on current mode
        match self.mode {
            AppMode::Editing => {
                self.status_bar.render(
                    frame,
                    chunks[1],
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
                frame.render_widget(bar, chunks[1]);
            }
            AppMode::PromptFilename => {
                let prompt = format!(" Save as: {}_", self.filename_input);
                let bar = Paragraph::new(Span::raw(prompt))
                    .style(Style::default().bg(Color::Blue).fg(Color::White));
                frame.render_widget(bar, chunks[1]);
            }
        }
    }
}
