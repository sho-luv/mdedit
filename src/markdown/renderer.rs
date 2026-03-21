use ratatui::text::{Line, Span, Text};
use super::MarkdownRenderer;

pub struct TuiMarkdownRenderer;

impl MarkdownRenderer for TuiMarkdownRenderer {
    fn render(&self, markdown: &str) -> Text<'static> {
        let text = tui_markdown::from_str(markdown);
        // Convert borrowed Text to owned Text<'static> so it can be cached
        text_to_owned(text)
    }
}

/// Convert a `Text<'_>` with potentially borrowed content into `Text<'static>`
/// by converting all `Cow::Borrowed` spans to `Cow::Owned`.
fn text_to_owned(text: Text<'_>) -> Text<'static> {
    let lines: Vec<Line<'static>> = text
        .lines
        .into_iter()
        .map(|line| {
            let spans: Vec<Span<'static>> = line
                .spans
                .into_iter()
                .map(|span| Span::styled(span.content.into_owned(), span.style))
                .collect();
            let mut new_line = Line::from(spans);
            if let Some(align) = line.alignment {
                new_line = new_line.alignment(align);
            }
            new_line
        })
        .collect();
    let mut owned = Text::from(lines);
    owned.alignment = text.alignment;
    owned
}
