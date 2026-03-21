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
}

impl<'a> App<'a> {
    pub fn new(content: Option<String>, filepath: Option<PathBuf>) -> Self {
        App {
            editor: Editor::new(content, filepath),
            mode: AppMode::Editing,
            should_quit: false,
            filename_input: String::new(),
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

    /// Handle key events in normal editing mode.
    fn handle_editing_key(&mut self, key: crossterm::event::KeyEvent) {
        if let Some(action) = self.editor.handle_key(key) {
            match action {
                EditorAction::Save => {
                    // TODO: Wire save logic in Plan 02 (file_io module)
                    // For now, just a no-op placeholder
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
                // TODO: Save before quitting (wire in Plan 02)
                self.should_quit = true;
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
                    // TODO: Actually save the file (wire in Plan 02)
                    self.filename_input.clear();
                    self.mode = AppMode::Editing;
                }
            }
            KeyCode::Esc => {
                self.filename_input.clear();
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
        let status_bar = match self.mode {
            AppMode::Editing => self.build_editing_status(),
            AppMode::ConfirmQuit => Paragraph::new(Span::raw(
                " Unsaved changes. Save? (y/n/Esc)",
            ))
            .style(Style::default().bg(Color::Red).fg(Color::White)),
            AppMode::PromptFilename => {
                let prompt = format!(" Save as: {}_", self.filename_input);
                Paragraph::new(Span::raw(prompt))
                    .style(Style::default().bg(Color::Blue).fg(Color::White))
            }
        };

        frame.render_widget(status_bar, chunks[1]);
    }

    /// Build the normal editing-mode status bar showing filename, modified
    /// indicator, and cursor position (D-15).
    fn build_editing_status(&self) -> Paragraph<'_> {
        let (row, col) = self.editor.cursor_position();
        let modified = if self.editor.is_modified() { " [+]" } else { "" };
        let status = format!(
            " {}{} | Ln {}, Col {}",
            self.editor.display_name(),
            modified,
            row + 1,
            col + 1,
        );
        Paragraph::new(Span::raw(status))
            .style(Style::default().bg(Color::DarkGray).fg(Color::White))
    }
}
