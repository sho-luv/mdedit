use ratatui::text::Text;
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;
use ratatui::layout::Rect;

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
    pub fn render(&self, frame: &mut Frame, area: Rect, text: &Text) {
        let total_lines = text.lines.len() as u16;
        let visible = area.height;
        let max_scroll = total_lines.saturating_sub(visible);
        let clamped_offset = self.scroll_offset.min(max_scroll);

        let widget = Paragraph::new(text.clone())
            .scroll((clamped_offset, 0))
            .wrap(Wrap { trim: false }); // Prevent horizontal overflow (Pitfall 1)

        frame.render_widget(widget, area);
    }
}
