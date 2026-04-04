use ratatui::style::Color;
use ratatui::text::Text;
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::buffer::Buffer;

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
        highlight_line: Option<u16>,
    ) {
        let total_lines = text.lines.len() as u16;
        let visible = area.height;
        let max_scroll = total_lines.saturating_sub(visible);
        let clamped_offset = self.scroll_offset.min(max_scroll);

        let widget = Paragraph::new(text.clone())
            .scroll((clamped_offset, 0))
            .wrap(Wrap { trim: false });

        frame.render_widget(widget, area);

        // Draw sync indicator line
        if let Some(target) = highlight_line {
            if target >= clamped_offset && target < clamped_offset + visible {
                let y = area.y + (target - clamped_offset);
                let buf = frame.buffer_mut();
                draw_indicator(buf, area.x, y, area.width);
            }
        }
    }
}

/// Draw a subtle left-edge indicator marker on a preview line.
/// Uses a thin colored bar on the left edge without overwriting content.
fn draw_indicator(buf: &mut Buffer, x: u16, y: u16, width: u16) {
    if width == 0 {
        return;
    }
    let indicator_color = Color::Rgb(88, 166, 255); // blue accent

    // Draw a small marker on the left edge
    if let Some(cell) = buf.cell_mut((x, y)) {
        cell.set_char('\u{258F}'); // left 1/8 block
        cell.set_fg(indicator_color);
    }

    // Subtle background tint across the whole line
    for col in x..x + width {
        if let Some(cell) = buf.cell_mut((col, y)) {
            cell.set_bg(Color::Rgb(20, 30, 45));
        }
    }
}
