use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::time::{Duration, Instant};

/// Status bar widget that displays filename, cursor position, modified
/// indicator, and timed messages (e.g., "Saved" for 2 seconds).
pub struct StatusBar {
    timed_message: Option<(String, Instant)>,
    message_duration: Duration,
}

impl StatusBar {
    pub fn new() -> StatusBar {
        StatusBar {
            timed_message: None,
            message_duration: Duration::from_secs(2),
        }
    }

    /// Set a timed message that will display for `message_duration` seconds.
    pub fn set_message(&mut self, msg: &str) {
        self.timed_message = Some((msg.to_string(), Instant::now()));
    }

    /// Returns true if a timed message is currently active (not yet expired).
    /// Used by the event loop to know when a redraw is needed for message expiry.
    pub fn is_message_active(&self) -> bool {
        if let Some((_, ref when)) = self.timed_message {
            when.elapsed() < self.message_duration
        } else {
            false
        }
    }

    /// Render the status bar into the given area.
    ///
    /// If a timed message is active, it is displayed. Otherwise, the normal
    /// status line is shown: filename, modified indicator, and cursor position.
    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        filename: &str,
        cursor: (usize, usize),
        modified: bool,
    ) {
        let style = Style::default().bg(Color::DarkGray).fg(Color::White);

        // Check if timed message is still active
        if let Some((ref msg, ref when)) = self.timed_message {
            if when.elapsed() < self.message_duration {
                let paragraph = Paragraph::new(Span::raw(format!(" {}", msg))).style(style);
                frame.render_widget(paragraph, area);
                return;
            }
        }

        // Normal status line: filename + modified indicator on the left,
        // cursor position on the right
        let mod_indicator = if modified { " [+]" } else { "" };
        let (row, col) = cursor;
        let left = format!(" {}{}", filename, mod_indicator);
        let right = format!("Ln {}, Col {} ", row + 1, col + 1);

        // Calculate spacer width to push right-side text to the right edge
        let available = area.width as usize;
        let used = left.len() + right.len();
        let spacer_width = if available > used { available - used } else { 1 };
        let spacer = " ".repeat(spacer_width);

        let line = Line::from(vec![
            Span::raw(left),
            Span::raw(spacer),
            Span::raw(right),
        ]);

        let paragraph = Paragraph::new(line).style(style);
        frame.render_widget(paragraph, area);
    }
}
