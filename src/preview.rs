use std::ops::Range;

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::Frame;

use crate::markdown::RenderResult;
use crate::theme::Theme;

/// Scrollable view over a pre-laid-out [`RenderResult`].
///
/// Rows arrive already wrapped to the pane width, so drawing is a direct copy
/// of the visible slice — O(visible rows), independent of document size.
pub struct Preview {
    scroll_offset: usize,
    /// Whether an empty document shows the "nothing to preview" hint. Off
    /// while a render is still in flight, so the hint never flashes.
    pub show_empty_hint: bool,
}

impl Preview {
    pub fn new() -> Self {
        Preview {
            scroll_offset: 0,
            show_empty_hint: true,
        }
    }

    pub fn scroll_up(&mut self, lines: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(usize::from(lines));
    }

    pub fn scroll_down(&mut self, lines: u16) {
        self.scroll_offset = self.scroll_offset.saturating_add(usize::from(lines));
    }

    /// Set the scroll offset programmatically (scroll sync, Home/End).
    /// Values past the end are clamped on the next render.
    pub fn set_scroll(&mut self, offset: usize) {
        self.scroll_offset = offset;
    }

    /// Current scroll offset (rows from the top).
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Render the visible rows into `area`.
    ///
    /// Clamps the scroll offset so the last page stays full (Pitfall 6); the
    /// clamp is persisted so a jump past the end (e.g. `G`) lands exactly on
    /// the last page and subsequent scroll-ups step back from there.
    /// `highlight` tints the rows of the source line under the editor cursor.
    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        doc: &RenderResult,
        theme: &Theme,
        highlight: Option<Range<usize>>,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let visible = usize::from(area.height);
        let max_scroll = doc.rows.len().saturating_sub(visible);
        self.scroll_offset = self.scroll_offset.min(max_scroll);
        let top = self.scroll_offset;

        let buf = frame.buffer_mut();
        buf.set_style(area, Style::default().fg(theme.preview_fg));

        if doc.rows.iter().all(|row| row.width() == 0) {
            // Empty document: a quiet placeholder instead of a blank pane.
            let hint = "Nothing to preview yet";
            if self.show_empty_hint && usize::from(area.width) > hint.len() {
                let x = area.x + (area.width - hint.len() as u16) / 2;
                let y = area.y + area.height / 2;
                buf.set_string(
                    x,
                    y,
                    hint,
                    Style::default()
                        .fg(theme.preview_muted_fg)
                        .add_modifier(ratatui::style::Modifier::ITALIC),
                );
            }
            return;
        }
        for (i, line) in doc.rows.iter().skip(top).take(visible).enumerate() {
            buf.set_line(area.x, area.y + i as u16, line, area.width);
        }

        if let Some(rows) = highlight {
            let start = rows.start.max(top);
            let end = rows.end.min(top + visible);
            for row in start..end {
                let y = area.y + (row - top) as u16;
                // Tint the background only — never overwrite glyphs (an earlier
                // left-edge marker destroyed the first character of the line).
                buf.set_style(
                    Rect::new(area.x, y, area.width, 1),
                    Style::default().bg(theme.preview_indicator_bg),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Preview;
    use crate::config::RenderProfile;
    use crate::markdown::{MarkdownRenderer, PreviewRenderer};
    use crate::theme::Theme;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    /// The row the preview reports for a source line must be exactly the
    /// screen row where that content is drawn, at every width.
    #[test]
    fn mapped_rows_match_drawn_rows() {
        let markdown = "\
# BookSmith

AI-powered book generation platform. Upload a story, define your characters, and get a publish-ready book with consistent AI-generated illustrations.

## Problem

Creating illustrated books today requires either:
- **Expensive illustrators** -- $2K-$10K+ for a children's book
- **Inconsistent AI tools** -- every image generation is a roll of the dice; characters look different on every page
- **Limited export options** -- most tools don't produce files that meet publisher specs (B&N Press, Amazon KDP)

## Vision
";
        let theme = Theme::ocean();
        for width in [20u16, 30, 40, 60, 80, 120] {
            let doc = PreviewRenderer.render(markdown, RenderProfile::Github, &theme, width);
            let height = doc.rows.len() as u16;
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut preview = Preview::new();
            terminal
                .draw(|f| preview.render(f, f.area(), &doc, &theme, None))
                .unwrap();
            let buffer = terminal.backend().buffer().clone();
            let drawn_row = |needle: &str| {
                (0..height)
                    .find(|&y| {
                        let row: String = (0..width).map(|x| buffer[(x, y)].symbol()).collect();
                        row.contains(needle)
                    })
                    .map(usize::from)
            };

            for (src_prefix, needle) in [
                ("## Problem", "Problem"),
                ("## Vision", "Vision"),
                ("- **Limited", "Limited"),
                ("Creating", "Creating"),
            ] {
                let src = markdown
                    .lines()
                    .position(|l| l.starts_with(src_prefix))
                    .unwrap();
                assert_eq!(
                    doc.row_for_source(src),
                    drawn_row(needle),
                    "width {width}, source {src_prefix:?}"
                );
            }
        }
    }

    #[test]
    fn scroll_is_clamped_to_last_page() {
        let theme = Theme::ocean();
        let doc = PreviewRenderer.render("a\n\nb\n\nc\n\nd\n", RenderProfile::Github, &theme, 10);
        let mut terminal = Terminal::new(TestBackend::new(10, 3)).unwrap();
        let mut preview = Preview::new();
        preview.set_scroll(usize::MAX);
        terminal
            .draw(|f| preview.render(f, f.area(), &doc, &theme, None))
            .unwrap();
        assert_eq!(preview.scroll_offset(), doc.rows.len() - 3);
    }
}
