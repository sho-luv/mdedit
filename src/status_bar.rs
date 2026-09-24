use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::time::{Duration, Instant};

use crate::theme::Theme;

pub struct StatusBarContent<'a> {
    pub filename: &'a str,
    pub cursor: (usize, usize),
    pub modified: bool,
    pub vim_mode: Option<(&'a str, ratatui::style::Color)>,
    /// Contextual key hints rendered btop-style: accent-colored key + label.
    pub hints: &'a [(&'a str, &'a str)],
}

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

    /// Render the status bar into the given area.
    ///
    /// If a timed message is active, it is displayed. Otherwise, the normal
    /// status line is shown: mode indicator (vim), filename, modified marker,
    /// btop-style key hints, and cursor position.
    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &Theme,
        content: StatusBarContent<'_>,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let style = Style::default()
            .bg(theme.status_bar_bg)
            .fg(theme.status_bar_fg);

        // Check if timed message is still active
        if let Some((ref msg, ref when)) = self.timed_message {
            if when.elapsed() < self.message_duration {
                let paragraph = Paragraph::new(Span::raw(format!(" {}", msg))).style(style);
                frame.render_widget(paragraph, area);
                return;
            }
        }

        let mod_indicator = if content.modified { " [+]" } else { "" };
        let (row, col) = content.cursor;

        let key_style = Style::default()
            .fg(theme.preview_accent_primary)
            .bg(theme.status_bar_bg)
            .add_modifier(Modifier::BOLD);

        // Left segment: vim mode chip (if any) + filename.
        let mut spans: Vec<Span> = Vec::new();
        if let Some((label, bg_color)) = content.vim_mode {
            spans.push(Span::styled(
                format!(" {} ", label),
                Style::default().bg(bg_color).fg(theme.status_bar_fg),
            ));
        }
        spans.push(Span::raw(format!(" {}{}", content.filename, mod_indicator)));
        let left_width: usize = spans.iter().map(|s| s.width()).sum();

        // Right segment: hints (dropped when they don't fit) + cursor position.
        let mut hint_spans: Vec<Span> = Vec::new();
        for (key, label) in content.hints {
            hint_spans.push(Span::styled((*key).to_string(), key_style));
            hint_spans.push(Span::raw(format!(" {}  ", label)));
        }
        let hints_width: usize = hint_spans.iter().map(|s| s.width()).sum();
        let position = format!("Ln {}, Col {} ", row + 1, col + 1);

        let available = area.width as usize;
        let show_hints = left_width + hints_width + position.len() < available;
        let right_width = if show_hints {
            hints_width + position.len()
        } else {
            position.len()
        };

        let used = left_width + right_width;
        let spacer_width = if available > used {
            available - used
        } else {
            1
        };
        spans.push(Span::raw(" ".repeat(spacer_width)));
        if show_hints {
            spans.extend(hint_spans);
        }
        spans.push(Span::raw(position));

        let paragraph = Paragraph::new(Line::from(spans)).style(style);
        frame.render_widget(paragraph, area);
    }
}
