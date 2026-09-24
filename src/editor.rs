use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use ratatui_textarea::{CursorMove, TextArea};
use std::path::PathBuf;

use unicode_width::UnicodeWidthChar;

use crate::highlighter::{self, MarkdownHighlighter};
use crate::theme::Theme;

/// Actions that the editor can signal to the application layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorAction {
    Save,
    RequestQuit,
    ContentChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineEnding {
    Lf,
    Crlf,
}

impl LineEnding {
    fn detect(text: &str) -> Self {
        if text.contains("\r\n") {
            Self::Crlf
        } else {
            Self::Lf
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::Crlf => "\r\n",
        }
    }
}

struct RenderContext<'a> {
    area: Rect,
    search_query: &'a str,
    cursor_row: usize,
    cursor_col: usize,
    total_lines: usize,
    height: usize,
    lnum_width: usize,
    content_width: usize,
}

/// Wraps `ratatui_textarea::TextArea` with custom nano-style keybindings,
/// modified tracking, file path management, and markdown syntax highlighting (D-09).
pub struct Editor<'a> {
    textarea: TextArea<'a>,
    modified: bool,
    filepath: Option<PathBuf>,
    highlighter: MarkdownHighlighter,
    line_ending: LineEnding,
    /// Persistent scroll top for the custom highlighted rendering.
    scroll_top: usize,
    /// Color theme for editor rendering.
    theme: Theme,
    /// Whether to visually wrap long lines.
    pub word_wrap: bool,
    /// Pane-relative screen row where the cursor's logical line begins (may
    /// be negative when the line's first rows are scrolled off). Scroll sync
    /// aligns this row with the line's first preview row.
    cursor_line_screen_row: i32,
    /// Horizontal scroll in cells (no-wrap mode only).
    hscroll: usize,
}

impl<'a> Editor<'a> {
    /// Create a new editor, optionally pre-loaded with content and a file path.
    pub fn new(content: Option<String>, filepath: Option<PathBuf>, theme: Theme) -> Self {
        let line_ending = content
            .as_deref()
            .map(LineEnding::detect)
            .unwrap_or(LineEnding::Lf);
        let mut textarea = match content {
            Some(text) => {
                let lines: Vec<String> = text
                    .split('\n')
                    .map(|line| {
                        if line_ending == LineEnding::Crlf {
                            line.strip_suffix('\r').unwrap_or(line).to_string()
                        } else {
                            line.to_string()
                        }
                    })
                    .collect();
                if lines.is_empty() {
                    TextArea::default()
                } else {
                    TextArea::new(lines)
                }
            }
            None => TextArea::default(),
        };

        // Line numbers: styled from theme
        textarea.set_line_number_style(Style::default().fg(theme.line_number_fg));

        // Set tab length as safety net (D-17); we intercept Tab ourselves
        textarea.set_tab_length(2);

        let highlighter = MarkdownHighlighter::new(&theme.syntect_theme);
        Editor {
            textarea,
            modified: false,
            filepath,
            highlighter,
            line_ending,
            scroll_top: 0,
            theme,
            word_wrap: false,
            cursor_line_screen_row: 0,
            hscroll: 0,
        }
    }

    /// Pane-relative screen row where the cursor's logical line starts, as of
    /// the last render. Differs from the cursor row only for wrapped lines.
    pub fn cursor_line_screen_row(&self) -> i32 {
        self.cursor_line_screen_row
    }

    /// Apply a new theme at runtime.
    pub fn apply_theme(&mut self, theme: Theme) {
        self.textarea
            .set_line_number_style(Style::default().fg(theme.line_number_fg));
        self.highlighter = MarkdownHighlighter::new(&theme.syntect_theme);
        self.theme = theme;
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
            (modifiers, KeyCode::Left)
                if modifiers == KeyModifiers::SHIFT | KeyModifiers::CONTROL =>
            {
                if !self.textarea.is_selecting() {
                    self.textarea.start_selection();
                }
                self.textarea.move_cursor(CursorMove::WordBack);
                None
            }

            // Ctrl+Shift+Right -- select word forward (D-12)
            (modifiers, KeyCode::Right)
                if modifiers == KeyModifiers::SHIFT | KeyModifiers::CONTROL =>
            {
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
        self.textarea.lines().join(self.line_ending.as_str())
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

    /// Translate a visual click position (relative row in viewport, text column)
    /// into a logical (line, col) accounting for word wrap and scroll.
    /// When word_wrap is off, this is a simple scroll_top + row calculation.
    /// When word_wrap is on, it accounts for wrapped visual lines.
    pub fn visual_click_to_logical(
        &self,
        rel_row: usize,
        text_col: usize,
        content_width: usize,
    ) -> (usize, usize) {
        let lines = self.textarea.lines();
        let total_lines = lines.len();
        if !self.word_wrap || content_width == 0 {
            // No wrap: scroll_top is a logical line number
            let logical_row = (self.scroll_top + rel_row).min(total_lines.saturating_sub(1));
            let line = lines.get(logical_row).map(String::as_str).unwrap_or("");
            let n = line.chars().count();
            (
                logical_row,
                char_at_display_col(line, 0, n, text_col + self.hscroll),
            )
        } else {
            // Word wrap: scroll_top is a visual line number
            let visual_line = self.scroll_top + rel_row;
            let (logical_row, sub_line) =
                self.logical_line_at_visual(visual_line, content_width, total_lines);
            let line = lines.get(logical_row).map(String::as_str).unwrap_or("");
            let n = line.chars().count();
            let starts = wrap_starts(line, content_width);
            let sub_line = sub_line.min(starts.len() - 1);
            let from = starts[sub_line];
            // Clicking past the end of a non-final row lands on its last char,
            // not the first char of the next row.
            let to = starts
                .get(sub_line + 1)
                .map(|next| next.saturating_sub(1).max(from))
                .unwrap_or(n);
            (logical_row, char_at_display_col(line, from, to, text_col))
        }
    }

    /// Whether syntax highlighting still has visible lines to catch up on
    /// (the app should redraw soon rather than idle).
    pub fn highlight_pending(&self) -> bool {
        self.highlighter.is_pending()
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
    #[allow(dead_code)]
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

    /// Delete the current line and return its content (with trailing newline for line-wise paste).
    /// Used by dd in vim mode.
    pub fn delete_current_line(&mut self) -> String {
        let (row, _) = self.textarea.cursor();
        let lines = self.textarea.lines();
        let line_count = lines.len();

        if line_count == 0 {
            return String::new();
        }

        let line_text = lines[row].clone();
        let yanked = format!("{}\n", line_text);

        // Select the entire line and delete it
        self.textarea.move_cursor(CursorMove::Head);
        self.textarea.start_selection();

        if row < line_count - 1 {
            // Not the last line: select to the start of the next line
            self.textarea.move_cursor(CursorMove::Down);
            self.textarea.move_cursor(CursorMove::Head);
        } else {
            // Last line: select to the end
            self.textarea.move_cursor(CursorMove::End);
            // If not the first line, also delete the preceding newline
            if row > 0 {
                self.textarea.cancel_selection();
                // Go to end of previous line
                self.textarea.move_cursor(CursorMove::Up);
                self.textarea.move_cursor(CursorMove::End);
                self.textarea.start_selection();
                self.textarea.move_cursor(CursorMove::Down);
                self.textarea.move_cursor(CursorMove::End);
            }
        }

        self.textarea.cut();
        self.modified = true;
        yanked
    }

    /// Delete the content of the current line (but keep the line itself).
    /// Used by cc in vim mode.
    pub fn delete_current_line_content(&mut self) -> String {
        let (row, _) = self.textarea.cursor();
        let line_text = self.textarea.lines()[row].clone();

        if line_text.is_empty() {
            return String::new();
        }

        self.textarea.move_cursor(CursorMove::Head);
        self.textarea.start_selection();
        self.textarea.move_cursor(CursorMove::End);
        self.textarea.cut();
        self.modified = true;
        line_text
    }

    /// Cut the current selection and return the cut text.
    pub fn cut_selection(&mut self) -> String {
        if let Some(((sr, sc), (er, ec))) = self.textarea.selection_range() {
            let text = self.extract_selection_text(sr, sc, er, ec);
            self.textarea.cut();
            self.modified = true;
            text
        } else {
            String::new()
        }
    }

    /// Get the current selection text without cutting.
    pub fn get_selection_text(&self) -> String {
        if let Some(((sr, sc), (er, ec))) = self.textarea.selection_range() {
            self.extract_selection_text(sr, sc, er, ec)
        } else {
            String::new()
        }
    }

    /// Extract text from selection coordinates.
    fn extract_selection_text(&self, sr: usize, sc: usize, er: usize, ec: usize) -> String {
        let lines = self.textarea.lines();
        if sr == er {
            // Single line selection
            let line = &lines[sr];
            let start = sc.min(line.len());
            let end = ec.min(line.len());
            line[start..end].to_string()
        } else {
            // Multi-line selection
            let mut result = String::new();
            for row in sr..=er {
                if row >= lines.len() {
                    break;
                }
                let line = &lines[row];
                if row == sr {
                    let start = sc.min(line.len());
                    result.push_str(&line[start..]);
                    result.push('\n');
                } else if row == er {
                    let end = ec.min(line.len());
                    result.push_str(&line[..end]);
                } else {
                    result.push_str(line);
                    result.push('\n');
                }
            }
            result
        }
    }

    /// Yank (copy) the current line and return its content (with trailing newline).
    pub fn yank_current_line(&self) -> String {
        let (row, _) = self.textarea.cursor();
        let lines = self.textarea.lines();
        if row < lines.len() {
            format!("{}\n", lines[row])
        } else {
            String::new()
        }
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
        self.textarea
            .move_cursor(CursorMove::Jump(orig_row as u16, new_col as u16));
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
        self.textarea
            .move_cursor(CursorMove::Jump(orig_row as u16, new_col as u16));
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
            line.char_indices()
                .nth(sc)
                .map(|(i, _)| i)
                .unwrap_or(line.len())
        } else {
            0
        };

        let end_byte = if line_idx == er {
            line.char_indices()
                .nth(ec)
                .map(|(i, _)| i)
                .unwrap_or(line.len())
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
    /// The `search_query` parameter enables search match highlighting (D-08).
    ///
    /// All horizontal math is in terminal cells (unicode display width), not
    /// bytes or chars, so wide characters (CJK, emoji) and multi-byte text
    /// (em dashes, curly quotes) wrap and place the cursor correctly.
    pub fn render_highlighted(&mut self, frame: &mut Frame, area: Rect, search_query: &str) {
        let total_lines = self.textarea.lines().len();
        if area.width == 0 || area.height == 0 {
            return;
        }
        let height = area.height as usize;

        let lnum_width = highlighter::line_number_width(total_lines);
        let content_width = (area.width as usize).saturating_sub(lnum_width);

        let (cursor_row, cursor_col) = self.textarea.cursor();
        let ctx = RenderContext {
            area,
            search_query,
            cursor_row,
            cursor_col,
            total_lines,
            height,
            lnum_width,
            content_width,
        };

        if self.word_wrap && content_width > 0 {
            self.hscroll = 0;
            self.render_highlighted_wrapped(frame, &ctx);
        } else {
            self.render_highlighted_nowrap(frame, &ctx);
        }
    }

    /// Case-insensitive literal regex for the active search query, if any.
    fn search_regex(search_query: &str) -> Option<regex::Regex> {
        if search_query.is_empty() {
            return None;
        }
        regex::Regex::new(&format!("(?i){}", regex::escape(search_query))).ok()
    }

    /// Byte offset of the cursor within its line (identifies the active match).
    fn cursor_byte(&self, ctx: &RenderContext<'_>) -> usize {
        let line = &self.textarea.lines()[ctx.cursor_row];
        line.char_indices()
            .nth(ctx.cursor_col)
            .map(|(i, _)| i)
            .unwrap_or(line.len())
    }

    /// Layer search-match and selection highlights over a highlighted line.
    fn overlay_spans(
        &self,
        row: usize,
        mut spans: Vec<Span<'static>>,
        search_re: Option<&regex::Regex>,
        ctx: &RenderContext<'_>,
    ) -> Vec<Span<'static>> {
        // Layer 1: Search match highlights (D-08)
        if let Some(re) = search_re {
            let line_text = &self.textarea.lines()[row];
            let active_match_byte = if row == ctx.cursor_row {
                Some(self.cursor_byte(ctx))
            } else {
                None
            };
            for m in re.find_iter(line_text) {
                let highlight_style = if active_match_byte == Some(m.start()) {
                    Style::default()
                        .bg(self.theme.search_active_bg)
                        .fg(self.theme.search_active_fg)
                } else {
                    Style::default()
                        .bg(self.theme.search_match_bg)
                        .fg(self.theme.search_match_fg)
                };
                spans = apply_highlight_overlay(spans, m.start(), m.end(), highlight_style);
            }
        }

        // Layer 2: Selection highlight (applied after search so it takes priority)
        if let Some((sel_start, sel_end)) = self.selection_byte_range(row) {
            let selection_style = Style::default().bg(self.theme.selection_bg);
            spans = apply_highlight_overlay(spans, sel_start, sel_end, selection_style);
        }
        spans
    }

    /// Render without word wrap. Long lines scroll horizontally to keep the
    /// cursor visible.
    fn render_highlighted_nowrap(&mut self, frame: &mut Frame, ctx: &RenderContext<'_>) {
        self.update_scroll(ctx.cursor_row, ctx.height);
        let scroll_top = self.scroll_top;
        let visible_end = std::cmp::min(scroll_top + ctx.height, ctx.total_lines);

        let highlighted =
            self.highlighter
                .highlight_range(self.textarea.lines(), scroll_top, visible_end);
        let search_re = Self::search_regex(ctx.search_query);

        // Horizontal scroll (in cells) so the cursor column stays on screen.
        let cursor_x = display_width_to(&self.textarea.lines()[ctx.cursor_row], ctx.cursor_col);
        if ctx.content_width == 0 {
            self.hscroll = 0;
        } else if cursor_x < self.hscroll {
            self.hscroll = cursor_x;
        } else if cursor_x >= self.hscroll + ctx.content_width {
            self.hscroll = cursor_x + 1 - ctx.content_width;
        }

        let mut display_lines: Vec<Line<'static>> = Vec::with_capacity(ctx.height);
        for (i, hl_line) in highlighted.into_iter().enumerate() {
            let row = scroll_top + i;
            let lnum =
                highlighter::line_number_span(row, ctx.total_lines, self.theme.line_number_fg);
            let content_spans = self.overlay_spans(row, hl_line.spans, search_re.as_ref(), ctx);
            let content_spans = clip_spans(content_spans, self.hscroll, ctx.content_width);

            let mut final_spans = vec![lnum];
            final_spans.extend(content_spans);

            let mut line = Line::from(final_spans);
            if row == ctx.cursor_row {
                line = line.style(Style::default().add_modifier(Modifier::UNDERLINED));
            }
            display_lines.push(line);
        }

        frame.render_widget(Paragraph::new(display_lines), ctx.area);

        let cursor_rel_row = ctx.cursor_row.saturating_sub(scroll_top) as u16;
        self.cursor_line_screen_row = i32::from(cursor_rel_row);
        let cursor_x = ctx.area.x + (ctx.lnum_width + cursor_x - self.hscroll) as u16;
        let cursor_y = ctx.area.y + cursor_rel_row;
        if cursor_x < ctx.area.x + ctx.area.width && cursor_y < ctx.area.y + ctx.area.height {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }

    /// Render with word wrap enabled. `scroll_top` counts visual rows.
    fn render_highlighted_wrapped(&mut self, frame: &mut Frame, ctx: &RenderContext<'_>) {
        let wrap_width = ctx.content_width.max(1);

        // Which visual row of its line the cursor sits on, and where that
        // line starts in the document's visual-row space.
        let cursor_line = &self.textarea.lines()[ctx.cursor_row];
        let cursor_starts = wrap_starts(cursor_line, wrap_width);
        let cursor_sub = cursor_starts
            .partition_point(|&s| s <= ctx.cursor_col)
            .saturating_sub(1);
        let cursor_x =
            display_width_between(cursor_line, cursor_starts[cursor_sub], ctx.cursor_col)
                .min(wrap_width - 1);
        let cursor_visual_abs = self.visual_line_start(ctx.cursor_row, wrap_width) + cursor_sub;

        if cursor_visual_abs < self.scroll_top {
            self.scroll_top = cursor_visual_abs;
        } else if cursor_visual_abs >= self.scroll_top + ctx.height {
            self.scroll_top = cursor_visual_abs + 1 - ctx.height;
        }

        let (first_logical, skip_visual) =
            self.logical_line_at_visual(self.scroll_top, wrap_width, ctx.total_lines);

        // Every logical line takes at least one row, so height+1 lines suffice.
        let highlight_end = std::cmp::min(first_logical + ctx.height + 1, ctx.total_lines);
        let highlighted =
            self.highlighter
                .highlight_range(self.textarea.lines(), first_logical, highlight_end);
        let search_re = Self::search_regex(ctx.search_query);

        let mut display_lines: Vec<Line<'static>> = Vec::with_capacity(ctx.height);
        let mut cursor_screen_y: Option<u16> = None;

        for (i, hl_line) in highlighted.into_iter().enumerate() {
            if display_lines.len() >= ctx.height {
                break;
            }
            let row = first_logical + i;
            let skip = if i == 0 { skip_visual } else { 0 };
            let is_cursor_line = row == ctx.cursor_row;
            if is_cursor_line {
                self.cursor_line_screen_row = display_lines.len() as i32 - skip as i32;
            }

            let content_spans = self.overlay_spans(row, hl_line.spans, search_re.as_ref(), ctx);
            let line_text = &self.textarea.lines()[row];
            let byte_starts = char_starts_to_bytes(line_text, &wrap_starts(line_text, wrap_width));
            let wrapped = split_spans_at(content_spans, &byte_starts);

            for (vi, visual_line_spans) in wrapped.into_iter().enumerate().skip(skip) {
                if display_lines.len() >= ctx.height {
                    break;
                }

                // Line number on the first visual row, blank on continuations
                let lnum = if vi == 0 {
                    highlighter::line_number_span(row, ctx.total_lines, self.theme.line_number_fg)
                } else {
                    Span::raw(" ".repeat(ctx.lnum_width))
                };

                let mut final_spans = vec![lnum];
                final_spans.extend(visual_line_spans);

                let mut line = Line::from(final_spans);
                if is_cursor_line {
                    line = line.style(Style::default().add_modifier(Modifier::UNDERLINED));
                }
                if is_cursor_line && vi == cursor_sub {
                    cursor_screen_y = Some(display_lines.len() as u16);
                }
                display_lines.push(line);
            }
        }

        frame.render_widget(Paragraph::new(display_lines), ctx.area);

        if let Some(cy) = cursor_screen_y {
            let x = ctx.area.x + (ctx.lnum_width + cursor_x) as u16;
            let y = ctx.area.y + cy;
            if x < ctx.area.x + ctx.area.width && y < ctx.area.y + ctx.area.height {
                frame.set_cursor_position((x, y));
            }
        }
    }

    /// Absolute visual row where logical line `row` starts.
    fn visual_line_start(&self, row: usize, wrap_width: usize) -> usize {
        self.textarea.lines()[..row]
            .iter()
            .map(|l| visual_rows(l, wrap_width))
            .sum()
    }

    /// Given an absolute visual row, find which logical line it belongs to
    /// and how many visual rows into that logical line it is.
    fn logical_line_at_visual(
        &self,
        visual_line: usize,
        wrap_width: usize,
        total_lines: usize,
    ) -> (usize, usize) {
        let mut visual = 0;
        for (row, line) in self.textarea.lines().iter().enumerate() {
            let vlines = visual_rows(line, wrap_width);
            if visual + vlines > visual_line {
                return (row, visual_line - visual);
            }
            visual += vlines;
        }
        (total_lines.saturating_sub(1), 0)
    }

    /// Update scroll_top to keep the cursor within the visible viewport.
    fn update_scroll(&mut self, cursor_row: usize, height: usize) {
        if cursor_row < self.scroll_top {
            self.scroll_top = cursor_row;
        } else if cursor_row >= self.scroll_top + height {
            self.scroll_top = cursor_row + 1 - height;
        }
    }
}

/// Terminal cell width of a char. Tabs are rendered as a single space.
fn char_width(c: char) -> usize {
    if c == '\t' {
        1
    } else {
        UnicodeWidthChar::width(c).unwrap_or(0)
    }
}

/// True when every char is printable ASCII (or tab), i.e. one cell per byte.
fn is_simple_ascii(line: &str) -> bool {
    line.bytes()
        .all(|b| (0x20..0x7f).contains(&b) || b == b'\t')
}

/// Char indices at which each visual row of `line` begins when hard-wrapped
/// at `width` cells. Always starts with 0.
fn wrap_starts(line: &str, width: usize) -> Vec<usize> {
    let width = width.max(1);
    if is_simple_ascii(line) {
        let rows = line.len().div_ceil(width).max(1);
        return (0..rows).map(|r| r * width).collect();
    }
    let mut starts = vec![0];
    let mut col = 0;
    for (i, c) in line.chars().enumerate() {
        let w = char_width(c);
        if col + w > width && col > 0 {
            starts.push(i);
            col = 0;
        }
        col += w;
    }
    starts
}

/// Number of visual rows `line` occupies when hard-wrapped at `width` cells.
fn visual_rows(line: &str, width: usize) -> usize {
    if is_simple_ascii(line) {
        line.len().div_ceil(width.max(1)).max(1)
    } else {
        wrap_starts(line, width).len()
    }
}

/// Display width of the first `char_col` chars of `line`.
fn display_width_to(line: &str, char_col: usize) -> usize {
    display_width_between(line, 0, char_col)
}

/// Display width of chars `[from, to)` of `line`.
fn display_width_between(line: &str, from: usize, to: usize) -> usize {
    line.chars()
        .skip(from)
        .take(to.saturating_sub(from))
        .map(char_width)
        .sum()
}

/// Char index within `[from, to]` of `line` that a click at cell `x`
/// (relative to char `from`) lands on.
fn char_at_display_col(line: &str, from: usize, to: usize, x: usize) -> usize {
    let mut col = 0;
    for (i, c) in line.chars().enumerate().skip(from).take(to - from) {
        let w = char_width(c);
        if col + w > x {
            return i;
        }
        col += w;
    }
    to
}

/// Convert char-index row starts into byte offsets.
fn char_starts_to_bytes(line: &str, starts: &[usize]) -> Vec<usize> {
    if starts.len() <= 1 {
        return vec![0];
    }
    let mut out = Vec::with_capacity(starts.len());
    let mut next = 0;
    for (ci, (bi, _)) in line.char_indices().enumerate() {
        if next < starts.len() && starts[next] == ci {
            out.push(bi);
            next += 1;
            if next == starts.len() {
                break;
            }
        }
    }
    out
}

/// Split a line's spans into visual rows at the given byte offsets
/// (`byte_starts[0]` is 0).
fn split_spans_at(spans: Vec<Span<'static>>, byte_starts: &[usize]) -> Vec<Vec<Span<'static>>> {
    let mut rows: Vec<Vec<Span<'static>>> = vec![Vec::new(); byte_starts.len().max(1)];
    let mut pos = 0;
    let mut row = 0;
    for span in spans {
        let content = span.content.as_ref();
        let mut local = 0;
        while local < content.len() {
            while row + 1 < byte_starts.len() && byte_starts[row + 1] <= pos + local {
                row += 1;
            }
            let row_end = byte_starts.get(row + 1).copied().unwrap_or(usize::MAX);
            let take = (row_end - (pos + local)).min(content.len() - local);
            rows[row].push(Span::styled(
                content[local..local + take].to_string(),
                span.style,
            ));
            local += take;
        }
        pos += content.len();
    }
    rows
}

/// Drop the first `skip` cells of a line (horizontal scroll). A wide char cut
/// by the left edge is replaced with spaces so columns stay aligned. The right
/// edge is left to the Paragraph, which truncates at the area width.
fn clip_spans(spans: Vec<Span<'static>>, skip: usize, width: usize) -> Vec<Span<'static>> {
    if skip == 0 {
        return spans;
    }
    let limit = skip + width;
    let mut out = Vec::new();
    let mut col = 0;
    for span in spans {
        if col >= limit {
            break;
        }
        let mut kept = String::new();
        for c in span.content.chars() {
            let w = char_width(c);
            if col >= skip {
                kept.push(if c == '\t' { ' ' } else { c });
            } else if col + w > skip {
                kept.push_str(&" ".repeat(col + w - skip));
            }
            col += w;
        }
        if !kept.is_empty() {
            out.push(Span::styled(kept, span.style));
        }
    }
    out
}

/// Apply a highlight overlay (e.g., selection or search highlight) to a set of spans.
/// Splits spans at byte boundaries `start` and `end`, applying `overlay_style` background
/// to the bytes within that range.
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
            result.push(span);
        } else {
            let content = span.content.to_string();
            let base_style = span.style;

            if span_start < start {
                let before_end = start - span_start;
                result.push(Span::styled(content[..before_end].to_string(), base_style));
            }

            let hl_start = start.saturating_sub(span_start);
            let hl_end = if end < span_end {
                end - span_start
            } else {
                span_len
            };
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

            if span_end > end {
                let after_start = end - span_start;
                result.push(Span::styled(content[after_start..].to_string(), base_style));
            }
        }

        byte_pos = span_end;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::Editor;
    use crate::theme::Theme;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn content_round_trip_preserves_trailing_lf() {
        let editor = Editor::new(Some("alpha\nbeta\n".to_string()), None, Theme::ocean());
        assert_eq!(editor.content(), "alpha\nbeta\n");
    }

    #[test]
    fn content_round_trip_preserves_crlf() {
        let editor = Editor::new(Some("alpha\r\nbeta\r\n".to_string()), None, Theme::ocean());
        assert_eq!(editor.content(), "alpha\r\nbeta\r\n");
    }

    #[test]
    fn content_round_trip_preserves_missing_final_newline() {
        let editor = Editor::new(Some("alpha\nbeta".to_string()), None, Theme::ocean());
        assert_eq!(editor.content(), "alpha\nbeta");
    }

    #[test]
    fn content_preserves_crlf_after_edit() {
        let mut editor = Editor::new(Some("alpha\r\nbeta\r\n".to_string()), None, Theme::ocean());
        let action = editor.handle_key(KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE));
        assert_eq!(action, Some(super::EditorAction::ContentChanged));
        assert_eq!(editor.content(), "!alpha\r\nbeta\r\n");
    }

    fn render_cursor(editor: &mut Editor, width: u16, height: u16) -> (u16, u16) {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|f| editor.render_highlighted(f, f.area(), ""))
            .unwrap();
        let pos = terminal.get_cursor_position().unwrap();
        (pos.x, pos.y)
    }

    fn jump(editor: &mut Editor, row: u16, col: u16) {
        editor
            .textarea_mut()
            .move_cursor(ratatui_textarea::CursorMove::Jump(row, col));
    }

    #[test]
    fn wrap_starts_measure_cells_not_bytes() {
        assert_eq!(super::wrap_starts("abcdefgh", 3), vec![0, 3, 6]);
        assert_eq!(super::wrap_starts("", 3), vec![0]);
        // 日/本/語 are 2 cells each; em dash is 3 bytes but 1 cell.
        assert_eq!(super::wrap_starts("日本語abc", 4), vec![0, 2, 5]);
        assert_eq!(super::wrap_starts("a—b—c", 2), vec![0, 2, 4]);
    }

    #[test]
    fn wrapped_cursor_lands_on_the_right_cell_with_wide_chars() {
        let mut editor = Editor::new(Some("日本語日本語\n".into()), None, Theme::ocean());
        editor.word_wrap = true;
        jump(&mut editor, 0, 3);
        // Gutter is 2 cells ("1 "), leaving 5 for text: rows start at chars
        // [0, 2, 4]; char 3 is the second glyph of row 1.
        assert_eq!(render_cursor(&mut editor, 7, 5), (4, 1));
        assert_eq!(editor.cursor_line_screen_row(), 0);
    }

    #[test]
    fn nowrap_scrolls_horizontally_to_keep_cursor_visible() {
        let mut editor = Editor::new(Some(format!("{}\n", "a".repeat(100))), None, Theme::ocean());
        jump(&mut editor, 0, 100);
        let (x, _) = render_cursor(&mut editor, 22, 3);
        assert!(x < 22, "cursor x {x} off screen");
        // Clicking the first text cell maps back through the scroll offset.
        let (row, col) = editor.visual_click_to_logical(0, 0, 20);
        assert_eq!((row, col), (0, 100 + 1 - 20));
    }

    #[test]
    fn clicks_map_display_columns_to_chars() {
        let mut editor = Editor::new(Some("日本語abc\n".into()), None, Theme::ocean());
        let _ = render_cursor(&mut editor, 40, 3);
        // Cell 3 is the right half of 本 (char 1); cell 6 is 'a' (char 3).
        assert_eq!(editor.visual_click_to_logical(0, 3, 38), (0, 1));
        assert_eq!(editor.visual_click_to_logical(0, 6, 38), (0, 3));
        assert_eq!(editor.visual_click_to_logical(0, 99, 38), (0, 6));
    }
}
