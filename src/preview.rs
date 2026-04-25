use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Text};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use crate::theme::Theme;

pub struct PreviewMetrics {
    logical_line_starts: Vec<u16>,
    total_visual_lines: u16,
}

impl PreviewMetrics {
    /// Compute the visual (post-wrap) row index where each logical line begins.
    ///
    /// We delegate to ratatui's `Paragraph::line_count`, which uses the same
    /// `WordWrapper` implementation Paragraph uses when actually rendering.
    /// This guarantees the indicator's computed row matches the row where the
    /// content visually appears (Pitfall: a hand-rolled wrap counter previously
    /// drifted from ratatui's, putting the sync indicator on the wrong line).
    pub fn for_text(text: &Text, width: u16) -> Self {
        if width == 0 {
            return Self {
                logical_line_starts: Vec::new(),
                total_visual_lines: 0,
            };
        }

        let mut logical_line_starts = Vec::with_capacity(text.lines.len());
        let mut visual_line: usize = 0;

        for line in &text.lines {
            logical_line_starts.push(saturating_u16(visual_line));
            visual_line += visual_lines_for(line, text.style, width);
        }

        Self {
            logical_line_starts,
            total_visual_lines: saturating_u16(visual_line),
        }
    }

    pub fn total_visual_lines(&self) -> u16 {
        self.total_visual_lines
    }

    pub fn visual_line_for_logical(&self, logical_line: usize) -> u16 {
        self.logical_line_starts
            .get(logical_line)
            .copied()
            .unwrap_or_else(|| self.total_visual_lines.saturating_sub(1))
    }
}

pub struct Preview {
    scroll_offset: u16,
}

impl Preview {
    pub fn new() -> Self {
        Preview { scroll_offset: 0 }
    }

    pub fn scroll_up(&mut self, lines: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    pub fn scroll_down(&mut self, lines: u16) {
        self.scroll_offset = self.scroll_offset.saturating_add(lines);
    }

    #[allow(dead_code)]
    pub fn reset_scroll(&mut self) {
        self.scroll_offset = 0;
    }

    /// Set the scroll offset programmatically (used by scroll sync).
    pub fn set_scroll(&mut self, offset: u16) {
        self.scroll_offset = offset;
    }

    /// Render the preview text into the given area.
    /// Clamps scroll offset to prevent blank screen (Pitfall 6).
    /// If `highlight_line` is Some, draws a subtle indicator at that preview line.
    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        text: &Text,
        theme: &Theme,
        metrics: &PreviewMetrics,
        highlight_line: Option<u16>,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let visible = area.height;
        let max_scroll = metrics.total_visual_lines().saturating_sub(visible);
        let clamped_offset = self.scroll_offset.min(max_scroll);

        let widget = Paragraph::new(text.clone())
            .style(Style::default().fg(theme.preview_fg))
            .scroll((clamped_offset, 0))
            .wrap(Wrap { trim: false });

        frame.render_widget(widget, area);

        // Draw sync indicator line
        if let Some(target) = highlight_line {
            if target >= clamped_offset && target < clamped_offset + visible {
                let y = area.y + (target - clamped_offset);
                let buf = frame.buffer_mut();
                draw_indicator(buf, area.x, y, area.width, theme);
            }
        }
    }
}

/// How many visual rows a single logical Line occupies when rendered with
/// `Wrap { trim: false }` at the given width. Uses ratatui's own
/// `Paragraph::line_count` so the result matches Paragraph's actual rendering.
fn visual_lines_for(line: &Line, base_style: Style, width: u16) -> usize {
    if width == 0 {
        return 0;
    }
    // line_count counts wrapped rows for the entire text. Empty lines still
    // occupy 1 row, matching how Paragraph renders blank logical lines.
    Paragraph::new(Text {
        lines: vec![line.clone()],
        style: base_style,
        alignment: None,
    })
    .wrap(Wrap { trim: false })
    .line_count(width)
    .max(1)
}

fn saturating_u16(value: usize) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}

/// Draw a subtle left-edge indicator marker on a preview line.
/// Uses a thin colored bar on the left edge without overwriting content.
fn draw_indicator(buf: &mut Buffer, x: u16, y: u16, width: u16, theme: &Theme) {
    if width == 0 {
        return;
    }

    // Draw a small marker on the left edge
    if let Some(cell) = buf.cell_mut((x, y)) {
        cell.set_char('\u{258F}'); // left 1/8 block
        cell.set_fg(theme.preview_indicator_fg);
    }

    // Subtle background tint across the whole line
    for col in x..x + width {
        if let Some(cell) = buf.cell_mut((col, y)) {
            cell.set_bg(theme.preview_indicator_bg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PreviewMetrics;
    use crate::config::RenderProfile;
    use crate::markdown::{MarkdownRenderer, TuiMarkdownRenderer};
    use crate::theme::Theme;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::text::{Line, Text};
    use ratatui::widgets::{Paragraph, Widget, Wrap};

    #[test]
    fn preview_metrics_track_wrapped_visual_lines() {
        let text = Text::from(vec![
            Line::from("ab"),
            Line::from("abcdefghi"),
            Line::from(""),
        ]);

        let metrics = PreviewMetrics::for_text(&text, 4);

        assert_eq!(metrics.visual_line_for_logical(0), 0);
        assert_eq!(metrics.visual_line_for_logical(1), 1);
        assert_eq!(metrics.visual_line_for_logical(2), 4);
        assert_eq!(metrics.total_visual_lines(), 5);
    }

    #[test]
    fn preview_metrics_match_actual_wrapped_heading_row() {
        let markdown = "\
# Intro

BookSmith takes a story and turns it into an illustrated, publish-ready book.

1. **Input a story** -- paste text, upload a file (TXT, PDF, DOCX), or describe a concept and let AI generate one
2. **Characters are identified automatically** -- AI scans the story, extracts characters, and presents them for review
3. **Define character appearances** -- for each character, upload a reference image, describe them, or let AI generate a look. The user can edit/override at any point.
4. **AI generates page illustrations** -- each page gets an illustration where characters are rendered consistently using their reference images, adapted to the scene context
5. **Review and edit** -- flip through the book, regenerate pages you don't like, adjust text/layout
6. **Export** -- download as:
   - Barnes & Noble Press PDF (their spec)
   - Amazon KDP PDF (their spec)
   - Generic print-ready PDF
   - Digital audiobook (AI-narrated)

## Core Design Principles

### Character consistency is the hard problem
";

        let renderer = TuiMarkdownRenderer;
        let result = renderer.render(markdown, RenderProfile::Github, &Theme::ocean());
        let width = 40;
        let metrics = PreviewMetrics::for_text(&result.text, width);

        let source_line = markdown
            .lines()
            .position(|line| line == "## Core Design Principles")
            .expect("heading should be present in source");
        let mapped_visual_row =
            metrics.visual_line_for_logical(result.source_to_preview[source_line]);

        let area = Rect::new(0, 0, width, 200);
        let mut buffer = Buffer::empty(area);
        Paragraph::new(result.text.clone())
            .wrap(Wrap { trim: false })
            .render(area, &mut buffer);

        let actual_visual_row = (0..area.height as usize)
            .find(|&row| row_contains(&buffer, area, row, "Core Design Principles"))
            .expect("heading should be visible in rendered buffer")
            as u16;

        assert_eq!(mapped_visual_row, actual_visual_row);
    }

    /// Regression test for the preview-line-sync bug: the indicator landed on
    /// a wrapped continuation line because the local wrap-count routine drifted
    /// from ratatui's WordWrapper. Verify that for every logical line and every
    /// reasonable preview width, the metric-computed visual row matches the row
    /// where Paragraph actually renders that line.
    #[test]
    fn preview_metrics_match_paragraph_for_every_logical_line() {
        let markdown = "\
# BookSmith

AI-powered book generation platform. Upload a story, define your characters, and get a publish-ready book with consistent AI-generated illustrations.

## Problem

Creating illustrated books today requires either:
- **Expensive illustrators** -- $2K-$10K+ for a children's book
- **Inconsistent AI tools** -- every image generation is a roll of the dice; characters look different on every page
- **Manual layout work** -- hours in InDesign or similar tools to get print-ready output
- **Limited export options** -- most tools don't produce files that meet publisher specs (B&N Press, Amazon KDP)

There's no tool that takes a story, understands its characters, generates consistent illustrations, and outputs a publish-ready book in one workflow.

## Vision

BookSmith takes a story and turns it into an illustrated, publish-ready book.
";

        let renderer = TuiMarkdownRenderer;
        let result = renderer.render(markdown, RenderProfile::Github, &Theme::ocean());

        for &width in &[30u16, 40, 50, 60, 80, 100, 120] {
            let metrics = PreviewMetrics::for_text(&result.text, width);

            // For each logical line, render lines[0..i] followed by a unique marker.
            // The row where the marker appears IS the visual row where lines[i] would start.
            for i in 0..result.text.lines.len() {
                let mut prefix: Vec<Line> = result.text.lines.iter().take(i).cloned().collect();
                prefix.push(Line::from("\u{2603}MARKER\u{2603}"));
                let prefix_text = Text::from(prefix);

                let area = Rect::new(0, 0, width, 1000);
                let mut buf = Buffer::empty(area);
                Paragraph::new(prefix_text)
                    .wrap(Wrap { trim: false })
                    .render(area, &mut buf);

                let marker_row = (0..area.height)
                    .find(|&r| {
                        let mut s = String::new();
                        for c in 0..area.width {
                            if let Some(cell) = buf.cell((c, r)) {
                                s.push_str(cell.symbol());
                            }
                        }
                        s.contains("\u{2603}MARKER\u{2603}")
                    })
                    .expect("marker should be rendered somewhere");

                let metric_row = metrics.visual_line_for_logical(i);
                assert_eq!(
                    metric_row, marker_row,
                    "width={} logical_line={} content={:?}",
                    width,
                    i,
                    result.text.lines[i]
                        .spans
                        .iter()
                        .map(|sp| sp.content.as_ref())
                        .collect::<String>()
                );
            }
        }
    }

    fn row_contains(buffer: &Buffer, area: Rect, row: usize, needle: &str) -> bool {
        let mut rendered = String::new();
        for col in 0..area.width as usize {
            if let Some(cell) = buffer.cell((col as u16, row as u16)) {
                rendered.push_str(cell.symbol());
            }
        }
        rendered.contains(needle)
    }
}
