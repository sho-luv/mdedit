use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::time::{Duration, Instant};

use crate::theme::Theme;

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
    #[allow(dead_code)]
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
    ///
    /// When `vim_mode` is Some, renders vim mode indicator on the left with the
    /// given background color and mode-appropriate hints. When None (nano mode),
    /// renders the classic nano-style status bar.
    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        filename: &str,
        cursor: (usize, usize),
        modified: bool,
        theme: &Theme,
        vim_mode: Option<(&str, ratatui::style::Color)>,
    ) {
        let style = Style::default().bg(theme.status_bar_bg).fg(theme.status_bar_fg);

        // Check if timed message is still active
        if let Some((ref msg, ref when)) = self.timed_message {
            if when.elapsed() < self.message_duration {
                let paragraph = Paragraph::new(Span::raw(format!(" {}", msg))).style(style);
                frame.render_widget(paragraph, area);
                return;
            }
        }

        let mod_indicator = if modified { " [+]" } else { "" };
        let (row, col) = cursor;

        if let Some((label, bg_color)) = vim_mode {
            // Vim mode status bar: mode label on the left with mode bg color
            let mode_label = format!(" {} ", label);
            let file_info = format!(" {}{}", filename, mod_indicator);

            // Mode-appropriate hints
            let hints = if label.contains("NORMAL") {
                "i Insert | v Visual | :w Save | :q Quit"
            } else if label.contains("INSERT") {
                "Esc Normal | Type to edit"
            } else if label.contains("VISUAL") {
                "d Del | y Yank | Esc Normal"
            } else if label.contains("COMMAND") {
                "Enter Execute | Esc Cancel"
            } else {
                ""
            };

            let right_with_hints = format!("{} | Ln {}, Col {} ", hints, row + 1, col + 1);
            let right_no_hints = format!("Ln {}, Col {} ", row + 1, col + 1);

            let available = area.width as usize;
            let right = if mode_label.len() + file_info.len() + right_with_hints.len() <= available {
                right_with_hints
            } else {
                right_no_hints
            };

            let used = mode_label.len() + file_info.len() + right.len();
            let spacer_width = if available > used { available - used } else { 1 };
            let spacer = " ".repeat(spacer_width);

            let line = Line::from(vec![
                Span::styled(mode_label, Style::default().bg(bg_color).fg(theme.status_bar_fg)),
                Span::raw(file_info),
                Span::raw(spacer),
                Span::raw(right),
            ]);

            let paragraph = Paragraph::new(line).style(style);
            frame.render_widget(paragraph, area);
        } else {
            // Nano mode status bar (unchanged)
            let left = format!(" {}{}", filename, mod_indicator);
            let hints = "Ctrl+S Save | Ctrl+P Preview | Ctrl+Q Quit";
            let right_with_hints = format!("{} | Ln {}, Col {} ", hints, row + 1, col + 1);
            let right_no_hints = format!("Ln {}, Col {} ", row + 1, col + 1);

            let available = area.width as usize;
            let right = if left.len() + right_with_hints.len() <= available {
                right_with_hints
            } else {
                right_no_hints
            };

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
}
