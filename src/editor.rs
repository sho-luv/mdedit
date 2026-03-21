use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Color, Style};
use ratatui_textarea::{CursorMove, TextArea};
use std::path::PathBuf;

/// Actions that the editor can signal to the application layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorAction {
    Save,
    RequestQuit,
    ContentChanged,
}

/// Wraps `ratatui_textarea::TextArea` with custom nano-style keybindings,
/// modified tracking, and file path management.
pub struct Editor<'a> {
    textarea: TextArea<'a>,
    modified: bool,
    filepath: Option<PathBuf>,
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
}
