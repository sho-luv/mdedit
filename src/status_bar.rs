use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::time::{Duration, Instant};
use unicode_width::UnicodeWidthStr;

use crate::theme::Theme;

pub struct StatusBarContent<'a> {
    pub filename: &'a str,
    pub cursor: (usize, usize),
    pub modified: bool,
    /// Mode pill: label and fill color (vim modes; `None` in nano mode).
    pub mode: Option<(&'a str, Color)>,
    /// Contextual key hints: accent-colored key + dim label.
    pub hints: &'a [(&'a str, &'a str)],
    /// Word count of the document.
    pub words: usize,
}

/// An input prompt shown in place of the status line (`:` commands, search,
/// save-as, confirmations).
pub struct Prompt<'a> {
    /// Pill label, e.g. "SEARCH".
    pub label: &'a str,
    /// Pill fill color.
    pub color: Color,
    /// Text typed so far (or the question, for confirmations).
    pub text: &'a str,
    /// Dim right-aligned hint, e.g. "Enter next · Esc cancel".
    pub hint: &'a str,
    /// Whether to place the terminal cursor after `text`.
    pub cursor: bool,
}

/// Status bar widget: mode pill, filename, modified marker, key hints,
/// document stats, and timed messages (e.g. "Saved" for 2 seconds).
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

    /// The active timed message, if it hasn't expired.
    fn message(&self) -> Option<&str> {
        self.timed_message
            .as_ref()
            .filter(|(_, when)| when.elapsed() < self.message_duration)
            .map(|(msg, _)| msg.as_str())
    }

    /// Render the status line.
    ///
    /// Left: mode pill, filename, modified dot. Right: key hints (or the
    /// active timed message in their place), word count, cursor position.
    /// Right-hand segments are dropped, least important first, when the
    /// terminal is too narrow.
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
        let base = Style::default()
            .bg(theme.status_bar_bg)
            .fg(theme.status_bar_fg);
        let dim = Style::default().fg(theme.preview_muted_fg);
        let key_style = Style::default()
            .fg(theme.preview_accent_primary)
            .add_modifier(Modifier::BOLD);

        let mut left: Vec<Span> = Vec::new();
        if let Some((label, color)) = content.mode {
            left.push(pill(label, color, theme));
        }
        left.push(Span::styled(
            format!(" {}", content.filename),
            Style::default().add_modifier(Modifier::BOLD),
        ));
        if content.modified {
            left.push(Span::styled(
                " \u{25CF}",
                Style::default().fg(theme.preview_accent_warning),
            ));
        }

        // Right-hand segments, most droppable first.
        let middle: Vec<Span> = match self.message() {
            Some(msg) => vec![Span::raw(format!("{}   ", msg))],
            None => content
                .hints
                .iter()
                .flat_map(|(key, label)| {
                    [
                        Span::styled((*key).to_string(), key_style),
                        Span::styled(format!(" {}  ", label), dim),
                    ]
                })
                .collect(),
        };
        let words = vec![Span::styled(
            format!(
                " {} {}  ",
                group_thousands(content.words),
                if content.words == 1 { "word" } else { "words" }
            ),
            dim,
        )];
        let (row, col) = content.cursor;
        let position = vec![Span::raw(format!("Ln {}, Col {} ", row + 1, col + 1))];

        let width = |spans: &[Span]| spans.iter().map(|s| s.content.width()).sum::<usize>();
        let available = usize::from(area.width);
        let mut right: Vec<Vec<Span>> = vec![middle, words, position];
        // A timed message outranks the word count; hints do not.
        let drop_order: &[usize] = if self.message().is_some() {
            &[1, 0, 2]
        } else {
            &[0, 1, 2]
        };
        for &i in drop_order {
            let total = width(&left) + 1 + right.iter().map(|s| width(s)).sum::<usize>();
            if total <= available {
                break;
            }
            right[i].clear();
        }

        let right: Vec<Span> = right.into_iter().flatten().collect();
        let gap = available.saturating_sub(width(&left) + width(&right));
        let mut spans = left;
        spans.push(Span::raw(" ".repeat(gap)));
        spans.extend(right);
        frame.render_widget(Paragraph::new(Line::from(spans)).style(base), area);
    }

    /// Render an input prompt in place of the status line.
    pub fn render_prompt(&self, frame: &mut Frame, area: Rect, theme: &Theme, prompt: Prompt<'_>) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let base = Style::default()
            .bg(theme.status_bar_bg)
            .fg(theme.status_bar_fg);
        let dim = Style::default().fg(theme.preview_muted_fg);

        let label = pill(prompt.label, prompt.color, theme);
        let text = format!(" {}", prompt.text);
        let used = label.content.width() + text.width();
        let available = usize::from(area.width);
        let hint = format!("{} ", prompt.hint);
        let show_hint = used + 2 + hint.width() <= available;

        let mut spans = vec![label, Span::raw(text)];
        if show_hint {
            spans.push(Span::raw(" ".repeat(available - used - hint.width())));
            spans.push(Span::styled(hint, dim));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)).style(base), area);

        if prompt.cursor {
            let x = area.x + (used as u16).min(area.width.saturating_sub(1));
            frame.set_cursor_position((x, area.y));
        }
    }
}

/// A filled label: ` NORMAL `.
fn pill(label: &str, color: Color, theme: &Theme) -> Span<'static> {
    Span::styled(
        format!(" {} ", label),
        Style::default()
            .bg(color)
            .fg(theme.chip_fg)
            .add_modifier(Modifier::BOLD),
    )
}

/// Format a count with thousands separators: 12345 -> "12,345".
fn group_thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn draw(width: u16, bar: &StatusBar, content: StatusBarContent<'_>) -> String {
        let theme = Theme::ocean();
        let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
        terminal
            .draw(|f| bar.render(f, f.area(), &theme, content))
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        (0..width).map(|x| buf[(x, 0)].symbol()).collect()
    }

    fn content<'a>(hints: &'a [(&'a str, &'a str)]) -> StatusBarContent<'a> {
        StatusBarContent {
            filename: "notes.md",
            cursor: (11, 2),
            modified: true,
            mode: Some(("NORMAL", Color::Blue)),
            hints,
            words: 12345,
        }
    }

    #[test]
    fn shows_mode_file_modified_dot_stats_and_position() {
        let row = draw(100, &StatusBar::new(), content(&[("i", "insert")]));
        assert!(row.starts_with(" NORMAL  notes.md \u{25CF}"), "{row:?}");
        assert!(row.contains("i insert"));
        assert!(row.contains("12,345 words"));
        assert!(row.trim_end().ends_with("Ln 12, Col 3"));
    }

    #[test]
    fn narrow_terminals_drop_hints_then_stats_but_keep_position() {
        let hints = [("i", "insert"), ("v", "visual"), ("/", "find")];
        let row = draw(52, &StatusBar::new(), content(&hints));
        assert!(!row.contains("insert"));
        assert!(row.contains("12,345 words"));
        let row = draw(36, &StatusBar::new(), content(&hints));
        assert!(!row.contains("words"));
        assert!(row.contains("Ln 12, Col 3"));
    }

    #[test]
    fn timed_message_takes_the_place_of_hints() {
        let mut bar = StatusBar::new();
        bar.set_message("Saved");
        let row = draw(100, &bar, content(&[("i", "insert")]));
        assert!(row.contains("Saved"));
        assert!(!row.contains("insert"));
        assert!(row.starts_with(" NORMAL  notes.md"));
    }

    #[test]
    fn thousands_are_grouped() {
        assert_eq!(group_thousands(0), "0");
        assert_eq!(group_thousands(999), "999");
        assert_eq!(group_thousands(1000), "1,000");
        assert_eq!(group_thousands(1234567), "1,234,567");
    }
}
