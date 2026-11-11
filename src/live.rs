//! Live view: edit inside the rendered document.
//!
//! The whole document is shown rendered (the same rows the preview pane
//! draws), except the block under the cursor — a paragraph, heading, list
//! item, code block, table — which shows its raw markdown so it can be
//! edited. Moving the cursor to another block re-renders the one you left.
//! The editor's text buffer stays the single source of truth; this module
//! only decides what each screen row shows.

use std::ops::Range;

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::Frame;

use crate::editor::{Editor, RawRow};
use crate::markdown::RenderResult;
use crate::theme::Theme;

/// Cells reserved on the left: an accent bar marks the raw (editable) block.
const GUTTER: u16 = 2;

/// What the live view showed on the last frame, kept for scrolling and for
/// mapping mouse clicks back to the document.
#[derive(Default)]
struct Frame0 {
    /// Rendered rows shown before the raw block.
    rows_before: usize,
    /// First rendered row shown after the raw block.
    rows_after_start: usize,
    raw: Vec<RawRow>,
    /// Source lines of the raw block.
    raw_source: Range<usize>,
    total_rows: usize,
    cursor_screen_row: usize,
    area: Rect,
    /// Screen column where raw text starts.
    text_x: u16,
}

#[derive(Default)]
pub struct LiveView {
    scroll: usize,
    last: Frame0,
}

impl LiveView {
    pub fn new() -> Self {
        Self::default()
    }

    /// Width available to document content inside `area`.
    pub fn content_width(area: Rect) -> u16 {
        area.width.saturating_sub(GUTTER)
    }

    /// Scroll position for a scrollbar: (total rows, first visible row).
    pub fn scroll_metrics(&self) -> (usize, usize) {
        (self.last.total_rows, self.scroll)
    }

    /// Source lines `[start, end)` to show raw: every block touched by the
    /// cursor or selection.
    ///
    /// `doc` may be a frame or two behind the buffer (it is rendered in the
    /// background), so line numbers after the cursor are shifted by the
    /// difference in line count: an edit happens at the cursor, and whatever
    /// follows it moved by exactly that much.
    fn raw_range(doc: &RenderResult, editor: &Editor) -> (Range<usize>, Range<usize>) {
        let total = editor.line_count();
        let (first, last) = editor.active_lines();
        if doc.blocks.is_empty() {
            return (0..total, 0..doc.rows.len());
        }
        let doc_lines = doc.source_to_row.len().max(1);
        let grown = total.saturating_sub(doc_lines);
        let shrunk = doc_lines.saturating_sub(total);

        // The same lines in the (possibly stale) document's numbering.
        let stale_first = first.saturating_sub(grown).min(doc_lines - 1);
        let stale_last = (last + shrunk).min(doc_lines - 1).max(stale_first);

        let block_of = |line: usize| {
            doc.blocks
                .partition_point(|b| b.source_start <= line)
                .saturating_sub(1)
        };
        let (b0, b1) = (block_of(stale_first), block_of(stale_last));
        // Lines before the first block (leading blank lines) join it.
        let stale_start = if b0 == 0 {
            0
        } else {
            doc.blocks[b0].source_start
        };
        let stale_end = doc.blocks.get(b1 + 1).map_or(doc_lines, |b| b.source_start);

        let start = stale_start.min(first);
        let end = (stale_end + grown)
            .saturating_sub(shrunk)
            .clamp(last + 1, total);
        let row_start = if b0 == 0 { 0 } else { doc.blocks[b0].row_start };
        let row_end = doc
            .blocks
            .get(b1 + 1)
            .map_or(doc.rows.len(), |b| b.row_start);
        (
            start..end.max(start + 1).min(total),
            row_start..row_end.max(row_start),
        )
    }

    /// Draw the document into `area` and place the terminal cursor.
    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        doc: &RenderResult,
        editor: &Editor,
        theme: &Theme,
        search_query: &str,
    ) {
        if area.width <= GUTTER || area.height == 0 {
            return;
        }
        let height = usize::from(area.height);
        // Raw text shares the rendered text's column (margin and width), so
        // a block doesn't jump sideways when it opens for editing.
        let pane_width = Self::content_width(area);
        let margin = doc.margin.min(pane_width.saturating_sub(1));
        let column = match doc.content_width {
            0 => pane_width - margin,
            w => w.min(pane_width - margin),
        };
        let width = usize::from(column);

        let (raw_source, replaced_rows) = Self::raw_range(doc, editor);
        let (raw, cursor) = editor.raw_rows(raw_source.clone(), width, search_query);
        let rows_before = replaced_rows.start;
        let rows_after_start = replaced_rows.end;
        let total_rows = rows_before + raw.len() + (doc.rows.len() - rows_after_start);
        let (cursor_row, cursor_x) = cursor.unwrap_or((0, 0));
        let cursor_display_row = rows_before + cursor_row;

        // Scrolling. Within a block, scroll only to keep the cursor visible.
        // When the raw block changes, rows above the cursor change height, so
        // re-anchor: keep the cursor one screen row from where it was.
        let block_changed = raw_source.start != self.last.raw_source.start;
        if block_changed && self.last.area == area && self.last.total_rows > 0 {
            let moved_down = raw_source.start > self.last.raw_source.start;
            let wanted = if moved_down {
                self.last.cursor_screen_row + 1
            } else {
                self.last.cursor_screen_row.saturating_sub(1)
            };
            self.scroll = cursor_display_row.saturating_sub(wanted.min(height - 1));
        }
        if cursor_display_row < self.scroll {
            self.scroll = cursor_display_row;
        } else if cursor_display_row >= self.scroll + height {
            self.scroll = cursor_display_row + 1 - height;
        }
        self.scroll = self.scroll.min(total_rows.saturating_sub(height));

        let buf = frame.buffer_mut();
        buf.set_style(area, Style::default().fg(theme.preview_fg));
        let bar = Span::styled(
            "\u{258E}",
            Style::default().fg(theme.preview_accent_primary),
        );
        let text_x = area.x + GUTTER;
        let text_width = area.width - GUTTER;
        let raw_end = rows_before + raw.len();

        for screen_row in 0..height {
            let row = self.scroll + screen_row;
            if row >= total_rows {
                break;
            }
            let y = area.y + screen_row as u16;
            if row < rows_before {
                buf.set_line(text_x, y, &doc.rows[row], text_width);
            } else if row < raw_end {
                let raw_row = &raw[row - rows_before];
                // Raw text uses the editor's colors and a tinted background.
                let style = Style::default()
                    .bg(theme.cursor_line_bg)
                    .fg(theme.editor_fg.unwrap_or(theme.preview_fg));
                buf.set_style(Rect::new(area.x, y, area.width, 1), style);
                buf.set_span(area.x, y, &bar, 1);
                buf.set_line(text_x + margin, y, &raw_row.line, column);
            } else {
                let doc_row = rows_after_start + (row - raw_end);
                buf.set_line(text_x, y, &doc.rows[doc_row], text_width);
            }
        }

        let cursor_screen_row = cursor_display_row - self.scroll;
        if cursor.is_some() {
            frame.set_cursor_position((
                text_x + margin + (cursor_x as u16).min(column.saturating_sub(1)),
                area.y + cursor_screen_row as u16,
            ));
        }

        self.last = Frame0 {
            rows_before,
            rows_after_start,
            raw,
            raw_source,
            total_rows,
            cursor_screen_row,
            area,
            text_x: text_x + margin,
        };
    }

    /// Map a click at screen (`col`, `row`) to a cursor position
    /// (source line, char column), using the last rendered frame.
    pub fn click(
        &self,
        col: u16,
        row: u16,
        doc: &RenderResult,
        editor: &Editor,
    ) -> Option<(usize, usize)> {
        let area = self.last.area;
        if row < area.y || row >= area.y + area.height || col < area.x || col >= area.x + area.width
        {
            return None;
        }
        let display_row = self.scroll + usize::from(row - area.y);
        let x = usize::from(col.saturating_sub(self.last.text_x));
        let raw_end = self.last.rows_before + self.last.raw.len();

        if display_row >= self.last.rows_before && display_row < raw_end {
            // Inside the raw block: exact position.
            let raw_row = &self.last.raw[display_row - self.last.rows_before];
            return Some((raw_row.source_line, editor.char_at_cell(raw_row, x)));
        }
        // On rendered text: go to the start of the source line it came from
        // (that block then opens for editing).
        let doc_row = if display_row < self.last.rows_before {
            display_row
        } else {
            self.last.rows_after_start + (display_row - raw_end)
        };
        let source = *doc.row_source.get(doc_row)?;
        // Lines after the raw block may have shifted relative to the doc.
        let shift = editor.line_count() as isize - doc.source_to_row.len().max(1) as isize;
        let line = if display_row >= raw_end {
            (source as isize + shift).max(0) as usize
        } else {
            source
        };
        Some((line.min(editor.line_count().saturating_sub(1)), 0))
    }
}
