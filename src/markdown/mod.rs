mod layout;
mod renderer;
pub use renderer::PreviewRenderer;

use crate::config::RenderProfile;
use crate::theme::Theme;
use ratatui::text::Line;

/// Rendered preview, laid out for one specific pane width.
pub struct RenderResult {
    /// Final visual rows. Every row fits within `width` cells, so the preview
    /// draws them verbatim without any further wrapping.
    pub rows: Vec<Line<'static>>,
    /// Source line (0-based) that each row was produced from.
    pub row_source: Vec<usize>,
    /// First row for each source line (0-based). Lines that produce no row of
    /// their own (e.g. blank lines) inherit the previous line's row.
    pub source_to_row: Vec<usize>,
    /// Width the rows were laid out for.
    pub width: u16,
}

impl RenderResult {
    pub fn empty() -> Self {
        RenderResult {
            rows: Vec::new(),
            row_source: Vec::new(),
            source_to_row: Vec::new(),
            width: 0,
        }
    }

    /// A plain message (e.g. a render failure) in place of a document.
    pub fn message(lines: &[&str]) -> Self {
        RenderResult {
            rows: lines.iter().map(|l| Line::from(l.to_string())).collect(),
            row_source: vec![0; lines.len()],
            source_to_row: Vec::new(),
            width: 0,
        }
    }

    /// First preview row for a source line, if the line is mapped.
    pub fn row_for_source(&self, source_line: usize) -> Option<usize> {
        self.source_to_row.get(source_line).copied()
    }

    /// Rows that render `source_line`: its first row plus any following rows
    /// that begin with that source line's content (wrapped continuations).
    pub fn rows_for_source(&self, source_line: usize) -> std::ops::Range<usize> {
        let Some(start) = self.row_for_source(source_line) else {
            return 0..0;
        };
        if start >= self.rows.len() {
            return 0..0;
        }
        let mut end = start + 1;
        while end < self.rows.len() && self.row_source[end] == source_line {
            end += 1;
        }
        start..end
    }
}

/// Abstraction over markdown rendering so the renderer can be replaced (D-01).
/// Returns owned rows so the result can be cached in App state.
pub trait MarkdownRenderer {
    fn render(
        &self,
        markdown: &str,
        profile: RenderProfile,
        theme: &Theme,
        width: u16,
    ) -> RenderResult;
}
