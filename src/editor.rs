use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use ratatui_textarea::{CursorMove, TextArea};
use std::path::PathBuf;

use crate::highlighter::{self, MarkdownHighlighter};

/// Actions that the editor can signal to the application layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorAction {
    Save,
    RequestQuit,
    ContentChanged,
}

/// Wraps `ratatui_textarea::TextArea` with custom nano-style keybindings,
/// modified tracking, file path management, and markdown syntax highlighting (D-09).
pub struct Editor<'a> {
    textarea: TextArea<'a>,
    modified: bool,
    filepath: Option<PathBuf>,
    highlighter: MarkdownHighlighter,
    /// Persistent scroll top for the custom highlighted rendering.
    scroll_top: usize,
}

impl<'a> Editor<'a> {
    /// Create a new editor, optionally pre-loaded with content and a file path.
    pub fn new(content: Option<String>, filepath: Option<PathBuf>) -> Self {
        let mut textarea = match content {
            Some(text) => {
                let lines: Vec<String> = text.lines().map(String::from).collect();
                if lines.is_empty() {
                    TextArea::default()
                } else {
                    TextArea::new(lines)
                }
            }
            None => TextArea::default(),
        };

        // Line numbers: dimmed, right-aligned (D-08)
        textarea.set_line_number_style(Style::default().fg(Color::DarkGray));

        Editor {
            textarea,
            modified: false,
            filepath,
            highlighter: MarkdownHighlighter::new(),
            scroll_top: 0,
        }
    }

    /// Process a key event using nano-style keybindings (D-05, D-06, D-07).
    /// Returns an optional action for the app layer to handle.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<EditorAction> {
        match (key.modifiers, key.code) {
            // Save (D-05)
            (KeyModifiers::CONTROL, KeyCode::Char('s')) => Some(EditorAction::Save),

            // Quit (D-05)
            (KeyModifiers::CONTROL, KeyCode::Char('q')) => Some(EditorAction::RequestQuit),

            // Undo (D-05)
            (KeyModifiers::CONTROL, KeyCode::Char('z')) => {
                self.textarea.undo();
                Some(EditorAction::ContentChanged)
            }

            // Redo (D-05)
            (KeyModifiers::CONTROL, KeyCode::Char('y')) => {
                self.textarea.redo();
                Some(EditorAction::ContentChanged)
            }

            // Word jump left (D-05)
            (KeyModifiers::CONTROL, KeyCode::Left) => {
                self.textarea.move_cursor(CursorMove::WordBack);
                None
            }

            // Word jump right (D-05)
            (KeyModifiers::CONTROL, KeyCode::Right) => {
                self.textarea.move_cursor(CursorMove::WordForward);
                None
            }

            // Home — line start (D-05)
            (KeyModifiers::NONE, KeyCode::Home) => {
                self.textarea.move_cursor(CursorMove::Head);
                None
            }

            // End — line end (D-05)
            (KeyModifiers::NONE, KeyCode::End) => {
                self.textarea.move_cursor(CursorMove::End);
                None
            }

            // Ctrl+Home — document start (D-05)
            (KeyModifiers::CONTROL, KeyCode::Home) => {
                self.textarea.move_cursor(CursorMove::Top);
                None
            }

            // Ctrl+End — document end (D-05)
            (KeyModifiers::CONTROL, KeyCode::End) => {
                self.textarea.move_cursor(CursorMove::Bottom);
                None
            }

            // Arrow keys — cursor movement
            (KeyModifiers::NONE, KeyCode::Up) => {
                self.textarea.move_cursor(CursorMove::Up);
                None
            }
            (KeyModifiers::NONE, KeyCode::Down) => {
                self.textarea.move_cursor(CursorMove::Down);
                None
            }
            (KeyModifiers::NONE, KeyCode::Left) => {
                self.textarea.move_cursor(CursorMove::Back);
                None
            }
            (KeyModifiers::NONE, KeyCode::Right) => {
                self.textarea.move_cursor(CursorMove::Forward);
                None
            }

            // Page Up / Page Down
            (KeyModifiers::NONE, KeyCode::PageUp) => {
                self.textarea.move_cursor(CursorMove::ParagraphBack);
                None
            }
            (KeyModifiers::NONE, KeyCode::PageDown) => {
                self.textarea.move_cursor(CursorMove::ParagraphForward);
                None
            }

            // Ctrl+C does nothing (D-07)
            (KeyModifiers::CONTROL, KeyCode::Char('c')) => None,

            // All other keys: pass to textarea for basic input handling.
            // CRITICAL: use input_without_shortcuts() to avoid Emacs keybinding conflicts.
            _ => {
                let changed = self.textarea.input_without_shortcuts(key);
                if changed {
                    self.modified = true;
                    Some(EditorAction::ContentChanged)
                } else {
                    None
                }
            }
        }
    }

    /// Return the full content of the editor as a single string.
    pub fn content(&self) -> String {
        self.textarea.lines().join("\n")
    }

    /// Return the cursor position as (row, col).
    pub fn cursor_position(&self) -> (usize, usize) {
        self.textarea.cursor()
    }

    /// Whether the buffer has been modified since last save.
    pub fn is_modified(&self) -> bool {
        self.modified
    }

    /// Mark the buffer as saved (resets modified flag).
    pub fn mark_saved(&mut self) {
        self.modified = false;
    }

    /// Display name: filename if available, otherwise "[untitled]" (D-02).
    pub fn display_name(&self) -> &str {
        self.filepath
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("[untitled]")
    }

    /// Return a reference to the underlying TextArea for rendering.
    pub fn widget(&self) -> &TextArea<'a> {
        &self.textarea
    }

    /// Return the file path, if any.
    pub fn filepath(&self) -> Option<&PathBuf> {
        self.filepath.as_ref()
    }

    /// Set the file path (used when saving an untitled buffer).
    pub fn set_filepath(&mut self, path: PathBuf) {
        self.filepath = Some(path);
    }

    /// Render the editor with syntax-highlighted markdown text (D-09, D-10).
    ///
    /// Since tui-textarea does not expose per-span styling hooks, we render the
    /// editor content ourselves: highlighted lines via syntect, line numbers,
    /// and cursor positioning — while still using tui-textarea for input handling.
    pub fn render_highlighted(&mut self, frame: &mut Frame, area: Rect) {
        let total_lines = self.textarea.lines().len();
        let height = area.height as usize;

        if height == 0 {
            return;
        }

        // Update scroll offset to keep cursor visible.
        let (cursor_row, cursor_col) = self.textarea.cursor();
        self.update_scroll(cursor_row, height);
        let scroll_top = self.scroll_top;

        // Get visible line range
        let visible_end = std::cmp::min(scroll_top + height, total_lines);

        // Highlight text through the visible range.
        let full_text = self.textarea.lines().join("\n");
        let highlighted = self.highlighter.highlight_range(&full_text, scroll_top, visible_end);

        // Build final lines with line numbers prepended
        let mut display_lines: Vec<Line<'static>> = Vec::with_capacity(height);
        for (i, hl_line) in highlighted.into_iter().enumerate() {
            let row = scroll_top + i;
            let lnum = highlighter::line_number_span(row, total_lines);

            let mut spans = vec![lnum];
            spans.extend(hl_line.spans);

            let mut line = Line::from(spans);
            // Subtle underline on cursor line for visibility
            if row == cursor_row {
                line = line.style(Style::default().add_modifier(Modifier::UNDERLINED));
            }
            display_lines.push(line);
        }

        // Pad remaining area with empty lines (for short files)
        while display_lines.len() < height {
            display_lines.push(Line::from(""));
        }

        let paragraph = Paragraph::new(display_lines);
        frame.render_widget(paragraph, area);

        // Place the cursor at the correct position
        let lnum_width = highlighter::line_number_width(total_lines);
        let cursor_x = area.x + lnum_width as u16 + cursor_col as u16;
        let cursor_y = area.y + (cursor_row - scroll_top) as u16;
        if cursor_x < area.x + area.width && cursor_y < area.y + area.height {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }

    /// Update scroll_top to keep the cursor within the visible viewport.
    fn update_scroll(&mut self, cursor_row: usize, height: usize) {
        if cursor_row < self.scroll_top {
            // Cursor above viewport — scroll up
            self.scroll_top = cursor_row;
        } else if cursor_row >= self.scroll_top + height {
            // Cursor below viewport — scroll down
            self.scroll_top = cursor_row + 1 - height;
        }
        // Otherwise cursor is within viewport — don't change scroll
    }
}
