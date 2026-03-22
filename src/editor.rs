use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
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

        // Set tab length as safety net (D-17); we intercept Tab ourselves
        textarea.set_tab_length(2);

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

            // Ctrl+Shift+Left -- select word backward (D-12)
            (modifiers, KeyCode::Left) if modifiers == KeyModifiers::SHIFT | KeyModifiers::CONTROL => {
                if !self.textarea.is_selecting() {
                    self.textarea.start_selection();
                }
                self.textarea.move_cursor(CursorMove::WordBack);
                None
            }

            // Ctrl+Shift+Right -- select word forward (D-12)
            (modifiers, KeyCode::Right) if modifiers == KeyModifiers::SHIFT | KeyModifiers::CONTROL => {
                if !self.textarea.is_selecting() {
                    self.textarea.start_selection();
                }
                self.textarea.move_cursor(CursorMove::WordForward);
                None
            }

            // Shift+Right -- extend selection forward (D-12)
            (KeyModifiers::SHIFT, KeyCode::Right) => {
                if !self.textarea.is_selecting() {
                    self.textarea.start_selection();
                }
                self.textarea.move_cursor(CursorMove::Forward);
                None
            }

            // Shift+Left -- extend selection backward (D-12)
            (KeyModifiers::SHIFT, KeyCode::Left) => {
                if !self.textarea.is_selecting() {
                    self.textarea.start_selection();
                }
                self.textarea.move_cursor(CursorMove::Back);
                None
            }

            // Shift+Up -- extend selection up (D-12)
            (KeyModifiers::SHIFT, KeyCode::Up) => {
                if !self.textarea.is_selecting() {
                    self.textarea.start_selection();
                }
                self.textarea.move_cursor(CursorMove::Up);
                None
            }

            // Shift+Down -- extend selection down (D-12)
            (KeyModifiers::SHIFT, KeyCode::Down) => {
                if !self.textarea.is_selecting() {
                    self.textarea.start_selection();
                }
                self.textarea.move_cursor(CursorMove::Down);
                None
            }

            // Shift+Home -- select to line start (D-12)
            (KeyModifiers::SHIFT, KeyCode::Home) => {
                if !self.textarea.is_selecting() {
                    self.textarea.start_selection();
                }
                self.textarea.move_cursor(CursorMove::Head);
                None
            }

            // Shift+End -- select to line end (D-12)
            (KeyModifiers::SHIFT, KeyCode::End) => {
                if !self.textarea.is_selecting() {
                    self.textarea.start_selection();
                }
                self.textarea.move_cursor(CursorMove::End);
                None
            }

            // Tab -- indent (D-17, D-19, D-20)
            (KeyModifiers::NONE, KeyCode::Tab) => {
                if let Some(((start_row, _), (end_row, _))) = self.textarea.selection_range() {
                    if start_row != end_row {
                        // Multi-line indent: insert 2 spaces at start of each selected line (D-19)
                        self.indent_lines(start_row, end_row);
                        self.modified = true;
                        return Some(EditorAction::ContentChanged);
                    }
                }
                // Single-line or no selection: insert 2 spaces at cursor (D-17)
                // insert_str deletes selection first if active (Pitfall 6)
                self.textarea.insert_str("  ");
                self.modified = true;
                Some(EditorAction::ContentChanged)
            }

            // Shift+Tab (BackTab) -- outdent (D-18, D-19)
            (_, KeyCode::BackTab) => {
                if let Some(((start_row, _), (end_row, _))) = self.textarea.selection_range() {
                    if start_row != end_row {
                        self.outdent_lines(start_row, end_row);
                        self.modified = true;
                        return Some(EditorAction::ContentChanged);
                    }
                }
                // Single-line outdent
                let (row, _) = self.textarea.cursor();
                self.outdent_line(row);
                self.modified = true;
                Some(EditorAction::ContentChanged)
            }

            // Word jump left (D-05) -- cancel selection on plain movement
            (KeyModifiers::CONTROL, KeyCode::Left) => {
                self.textarea.cancel_selection();
                self.textarea.move_cursor(CursorMove::WordBack);
                None
            }

            // Word jump right (D-05) -- cancel selection on plain movement
            (KeyModifiers::CONTROL, KeyCode::Right) => {
                self.textarea.cancel_selection();
                self.textarea.move_cursor(CursorMove::WordForward);
                None
            }

            // Home -- line start (D-05)
            (KeyModifiers::NONE, KeyCode::Home) => {
                self.textarea.cancel_selection();
                self.textarea.move_cursor(CursorMove::Head);
                None
            }

            // End -- line end (D-05)
            (KeyModifiers::NONE, KeyCode::End) => {
                self.textarea.cancel_selection();
                self.textarea.move_cursor(CursorMove::End);
                None
            }

            // Ctrl+Home -- document start (D-05)
            (KeyModifiers::CONTROL, KeyCode::Home) => {
                self.textarea.cancel_selection();
                self.textarea.move_cursor(CursorMove::Top);
                None
            }

            // Ctrl+End -- document end (D-05)
            (KeyModifiers::CONTROL, KeyCode::End) => {
                self.textarea.cancel_selection();
                self.textarea.move_cursor(CursorMove::Bottom);
                None
            }

            // Arrow keys -- cursor movement (cancel selection)
            (KeyModifiers::NONE, KeyCode::Up) => {
                self.textarea.cancel_selection();
                self.textarea.move_cursor(CursorMove::Up);
                None
            }
            (KeyModifiers::NONE, KeyCode::Down) => {
                self.textarea.cancel_selection();
                self.textarea.move_cursor(CursorMove::Down);
                None
            }
            (KeyModifiers::NONE, KeyCode::Left) => {
                self.textarea.cancel_selection();
                self.textarea.move_cursor(CursorMove::Back);
                None
            }
            (KeyModifiers::NONE, KeyCode::Right) => {
                self.textarea.cancel_selection();
                self.textarea.move_cursor(CursorMove::Forward);
                None
            }

            // Page Up / Page Down
            (KeyModifiers::NONE, KeyCode::PageUp) => {
                self.textarea.cancel_selection();
                self.textarea.move_cursor(CursorMove::ParagraphBack);
                None
            }
            (KeyModifiers::NONE, KeyCode::PageDown) => {
                self.textarea.cancel_selection();
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

    /// Return the total number of lines in the editor.
    pub fn line_count(&self) -> usize {
        self.textarea.lines().len()
    }

    /// Return a mutable reference to the underlying TextArea (for search in Plan 02).
    pub fn textarea_mut(&mut self) -> &mut TextArea<'a> {
        &mut self.textarea
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

    /// Indent multiple lines by inserting 2 spaces at the start of each (D-19).
    fn indent_lines(&mut self, start_row: usize, end_row: usize) {
        let (orig_row, orig_col) = self.textarea.cursor();
        self.textarea.cancel_selection();
        for row in start_row..=end_row {
            self.textarea.move_cursor(CursorMove::Jump(row as u16, 0));
            self.textarea.insert_str("  ");
        }
        // Restore cursor (adjusted for indent)
        let new_col = if orig_row >= start_row && orig_row <= end_row {
            orig_col + 2
        } else {
            orig_col
        };
        self.textarea.move_cursor(CursorMove::Jump(orig_row as u16, new_col as u16));
    }

    /// Outdent multiple lines by removing up to 2 leading spaces from each (D-19).
    fn outdent_lines(&mut self, start_row: usize, end_row: usize) {
        let (orig_row, orig_col) = self.textarea.cursor();
        self.textarea.cancel_selection();
        for row in start_row..=end_row {
            self.outdent_line(row);
        }
        // Restore cursor (adjusted for outdent)
        let line = &self.textarea.lines()[orig_row];
        let new_col = orig_col.min(line.len());
        self.textarea.move_cursor(CursorMove::Jump(orig_row as u16, new_col as u16));
    }

    /// Remove up to 2 leading spaces from a single line (D-18).
    fn outdent_line(&mut self, row: usize) {
        let line = &self.textarea.lines()[row];
        let spaces = line.chars().take(2).take_while(|c| *c == ' ').count();
        if spaces > 0 {
            self.textarea.move_cursor(CursorMove::Jump(row as u16, 0));
            for _ in 0..spaces {
                self.textarea.delete_next_char();
            }
        }
    }

    /// Get the selection range for a given line in byte offsets.
    /// Returns None if the line is not part of the selection.
    fn selection_byte_range(&self, line_idx: usize) -> Option<(usize, usize)> {
        let ((sr, sc), (er, ec)) = self.textarea.selection_range()?;
        let line = &self.textarea.lines()[line_idx];

        if line_idx < sr || line_idx > er {
            return None;
        }

        let start_byte = if line_idx == sr {
            line.char_indices().nth(sc).map(|(i, _)| i).unwrap_or(line.len())
        } else {
            0
        };

        let end_byte = if line_idx == er {
            line.char_indices().nth(ec).map(|(i, _)| i).unwrap_or(line.len())
        } else {
            line.len()
        };

        Some((start_byte, end_byte))
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

        // Selection highlight style (D-13)
        let selection_style = Style::default().bg(Color::Rgb(68, 68, 102));

        // Build final lines with line numbers prepended
        let mut display_lines: Vec<Line<'static>> = Vec::with_capacity(height);
        for (i, hl_line) in highlighted.into_iter().enumerate() {
            let row = scroll_top + i;
            let lnum = highlighter::line_number_span(row, total_lines);

            let mut spans: Vec<Span<'static>> = vec![lnum];

            // Apply selection overlay if this line has a selection
            let line_spans = if let Some((sel_start, sel_end)) = self.selection_byte_range(row) {
                apply_highlight_overlay(hl_line.spans, sel_start, sel_end, selection_style)
            } else {
                hl_line.spans
            };
            spans.extend(line_spans);

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
            // Cursor above viewport -- scroll up
            self.scroll_top = cursor_row;
        } else if cursor_row >= self.scroll_top + height {
            // Cursor below viewport -- scroll down
            self.scroll_top = cursor_row + 1 - height;
        }
        // Otherwise cursor is within viewport -- don't change scroll
    }
}

/// Apply a highlight overlay (e.g., selection or search highlight) to a set of spans.
/// Splits spans at byte boundaries `start` and `end`, applying `overlay_style` background
/// to the bytes within that range. Reusable for search highlights in Plan 02.
pub fn apply_highlight_overlay(
    spans: Vec<Span<'static>>,
    start: usize,
    end: usize,
    overlay_style: Style,
) -> Vec<Span<'static>> {
    if start >= end {
        return spans;
    }

    let mut result: Vec<Span<'static>> = Vec::new();
    let mut byte_pos: usize = 0;

    for span in spans {
        let span_len = span.content.len();
        let span_start = byte_pos;
        let span_end = byte_pos + span_len;

        if span_end <= start || span_start >= end {
            // Span is entirely outside the highlight range
            result.push(span);
        } else {
            // Span overlaps with highlight range -- split it
            let content = span.content.to_string();
            let base_style = span.style;

            // Part before highlight
            if span_start < start {
                let before_end = start - span_start;
                result.push(Span::styled(
                    content[..before_end].to_string(),
                    base_style,
                ));
            }

            // Highlighted part
            let hl_start = if start > span_start { start - span_start } else { 0 };
            let hl_end = if end < span_end { end - span_start } else { span_len };
            if hl_start < hl_end {
                let mut merged_style = base_style;
                // Apply overlay background while keeping foreground
                if let Some(bg) = overlay_style.bg {
                    merged_style = merged_style.bg(bg);
                }
                result.push(Span::styled(
                    content[hl_start..hl_end].to_string(),
                    merged_style,
                ));
            }

            // Part after highlight
            if span_end > end {
                let after_start = end - span_start;
                result.push(Span::styled(
                    content[after_start..].to_string(),
                    base_style,
                ));
            }
        }

        byte_pos = span_end;
    }

    result
}
