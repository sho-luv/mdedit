use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Theme, ThemeSet};
use syntect::parsing::SyntaxSet;

/// Markdown-aware syntax highlighter using syntect with a configurable theme.
///
/// Converts raw markdown text into syntax-highlighted ratatui `Line`s suitable for
/// rendering in a `Paragraph` widget. Highlights headings, bold/italic markers,
/// code fences, link syntax, and list markers with subtle, readable colors (D-09, D-10).
pub struct MarkdownHighlighter {
    syntax_set: SyntaxSet,
    theme: Theme,
}

impl MarkdownHighlighter {
    pub fn new(syntect_theme_name: &str) -> Self {
        let ss = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();
        let theme = match ts.themes.get(syntect_theme_name) {
            Some(t) => t.clone(),
            None => {
                eprintln!(
                    "Warning: syntect theme '{}' not found, falling back to base16-ocean.dark",
                    syntect_theme_name
                );
                ts.themes["base16-ocean.dark"].clone()
            }
        };
        MarkdownHighlighter {
            syntax_set: ss,
            theme,
        }
    }

    /// Highlight markdown text, returning styled Lines.
    /// Each input line becomes one output Line with per-token coloring.
    #[allow(dead_code)]
    pub fn highlight_lines(&self, text: &str) -> Vec<Line<'static>> {
        let syntax = self
            .syntax_set
            .find_syntax_by_extension("md")
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let mut h = HighlightLines::new(syntax, &self.theme);

        text.lines()
            .map(|line| {
                let line_with_nl = format!("{}\n", line);
                let ranges = h
                    .highlight_line(&line_with_nl, &self.syntax_set)
                    .unwrap_or_default();
                let spans = syntect_ranges_to_spans(&ranges);
                Line::from(spans)
            })
            .collect()
    }

    /// Highlight a subset of lines (by index range) for efficient rendering.
    /// Returns highlighted Lines for lines[start_row..end_row].
    /// All lines up to end_row are parsed for correct syntect state tracking.
    pub fn highlight_range(
        &self,
        text: &str,
        start_row: usize,
        end_row: usize,
    ) -> Vec<Line<'static>> {
        let syntax = self
            .syntax_set
            .find_syntax_by_extension("md")
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let mut h = HighlightLines::new(syntax, &self.theme);

        text.lines()
            .enumerate()
            .filter_map(|(i, line)| {
                let line_with_nl = format!("{}\n", line);
                let ranges = h
                    .highlight_line(&line_with_nl, &self.syntax_set)
                    .unwrap_or_default();

                if i >= start_row && i < end_row {
                    let spans = syntect_ranges_to_spans(&ranges);
                    Some(Line::from(spans))
                } else {
                    // Process for state tracking, but discard the output.
                    None
                }
            })
            .collect()
    }
}

/// Convert syntect highlight ranges to ratatui Spans.
/// Manual conversion to avoid ratatui version mismatch with syntect-tui.
fn syntect_ranges_to_spans(
    ranges: &[(syntect::highlighting::Style, &str)],
) -> Vec<Span<'static>> {
    ranges
        .iter()
        .filter_map(|(style, text)| {
            let content = text.trim_end_matches('\n').to_string();
            if content.is_empty() {
                return None;
            }
            let ratatui_style = convert_syntect_style(style);
            Some(Span::styled(content, ratatui_style))
        })
        .collect()
}

/// Convert a syntect Style to a ratatui Style (manual, no syntect-tui dependency).
fn convert_syntect_style(style: &syntect::highlighting::Style) -> Style {
    let mut ratatui_style = Style::default();

    // Foreground color (skip if alpha is 0 = transparent)
    if style.foreground.a > 0 {
        ratatui_style = ratatui_style.fg(Color::Rgb(
            style.foreground.r,
            style.foreground.g,
            style.foreground.b,
        ));
    }

    // Background: skip to use terminal default (D-10: keep it subtle)
    // Only apply background for non-transparent, non-theme-default backgrounds
    // Theme backgrounds tend to clash with terminal backgrounds, so we skip them.

    // Font style modifiers
    if style.font_style.contains(FontStyle::BOLD) {
        ratatui_style = ratatui_style.add_modifier(Modifier::BOLD);
    }
    if style.font_style.contains(FontStyle::ITALIC) {
        ratatui_style = ratatui_style.add_modifier(Modifier::ITALIC);
    }
    if style.font_style.contains(FontStyle::UNDERLINE) {
        ratatui_style = ratatui_style.add_modifier(Modifier::UNDERLINED);
    }

    ratatui_style
}

/// Build a line number span (right-aligned) with a configurable color.
pub fn line_number_span(row: usize, total_lines: usize, line_number_fg: Color) -> Span<'static> {
    let width = digit_count(total_lines);
    let num = format!("{:>width$} ", row + 1, width = width);
    Span::styled(num, Style::default().fg(line_number_fg))
}

/// Return the total width of the line number column (digits + 1 space).
pub fn line_number_width(total_lines: usize) -> usize {
    digit_count(total_lines) + 1
}

/// Count digits needed to display a number.
fn digit_count(n: usize) -> usize {
    if n == 0 {
        return 1;
    }
    let mut count = 0;
    let mut val = n;
    while val > 0 {
        count += 1;
        val /= 10;
    }
    count
}
