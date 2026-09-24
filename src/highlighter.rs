use std::sync::OnceLock;
use std::time::{Duration, Instant};

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use syntect::highlighting::{
    FontStyle, HighlightIterator, HighlightState, Highlighter, Theme, ThemeSet,
};
use syntect::parsing::{ParseState, ScopeStack, SyntaxSet};

/// Process-wide syntect syntax definitions. Deserializing these is the most
/// expensive part of syntect setup, so it happens once and is shared by the
/// editor highlighter and the preview's code-block highlighting.
pub fn syntax_set() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// Process-wide bundled syntect themes (see [`syntax_set`]).
pub fn theme_set() -> &'static ThemeSet {
    static SET: OnceLock<ThemeSet> = OnceLock::new();
    SET.get_or_init(ThemeSet::load_defaults)
}

/// Look up a bundled syntect theme, falling back to base16-ocean.dark.
pub fn syntect_theme(name: &str) -> &'static Theme {
    let themes = &theme_set().themes;
    themes
        .get(name)
        .or_else(|| themes.get("base16-ocean.dark"))
        .expect("bundled syntect theme missing")
}

/// One highlighted source line plus the parser state needed to resume
/// highlighting at the following line.
struct CachedLine {
    source: String,
    line: Line<'static>,
    parse_state: ParseState,
    highlight_state: HighlightState,
}

/// Maximum time one frame may spend extending the highlight cache. Lines the
/// budget doesn't reach are drawn unstyled and filled in on following frames.
const FRAME_BUDGET: Duration = Duration::from_millis(12);

/// Markdown-aware syntax highlighter for the editor pane.
///
/// Highlighting is incremental: the parser state after every line is cached,
/// so a frame only highlights the lines that changed since the last frame
/// (plus any newly scrolled-into-view lines) instead of the whole document.
/// The cache is validated against the current buffer on every call, so any
/// edit — including inserted/deleted lines — invalidates from the first
/// changed line onward. Syntect state flows top-down, so jumping deep into a
/// large, never-highlighted file needs every line above it parsed first; that
/// work is time-sliced across frames (see [`FRAME_BUDGET`]).
pub struct MarkdownHighlighter {
    theme: &'static Theme,
    cache: Vec<CachedLine>,
    /// Whether the last call ran out of budget before reaching its range.
    pending: bool,
}

impl MarkdownHighlighter {
    pub fn new(syntect_theme_name: &str) -> Self {
        MarkdownHighlighter {
            theme: syntect_theme(syntect_theme_name),
            cache: Vec::new(),
            pending: false,
        }
    }

    /// True when the last `highlight_range` left visible lines unstyled and
    /// needs another frame to finish.
    pub fn is_pending(&self) -> bool {
        self.pending
    }

    /// Return highlighted Lines for `lines[start_row..end_row]`.
    pub fn highlight_range(
        &mut self,
        lines: &[String],
        start_row: usize,
        end_row: usize,
    ) -> Vec<Line<'static>> {
        let end_row = end_row.min(lines.len());
        if start_row >= end_row {
            return Vec::new();
        }

        // Validate the cached prefix against the buffer. Syntect state flows
        // top-down, so the first mismatch invalidates everything after it.
        let check_end = end_row.min(self.cache.len());
        if let Some(first_changed) = (0..check_end).find(|&i| self.cache[i].source != lines[i]) {
            self.cache.truncate(first_changed);
        }

        if self.cache.len() < end_row {
            self.extend_cache(lines, end_row, Instant::now() + FRAME_BUDGET);
        }
        self.pending = self.cache.len() < end_row;

        (start_row..end_row)
            .map(|i| match self.cache.get(i) {
                Some(c) => c.line.clone(),
                None => Line::from(lines[i].replace('\t', " ")),
            })
            .collect()
    }

    fn extend_cache(&mut self, lines: &[String], end_row: usize, deadline: Instant) {
        let ss = syntax_set();
        let highlighter = Highlighter::new(self.theme);
        let (mut parse_state, mut highlight_state) = match self.cache.last() {
            Some(c) => (c.parse_state.clone(), c.highlight_state.clone()),
            None => {
                let syntax = ss
                    .find_syntax_by_extension("md")
                    .unwrap_or_else(|| ss.find_syntax_plain_text());
                (
                    ParseState::new(syntax),
                    HighlightState::new(&highlighter, ScopeStack::new()),
                )
            }
        };

        let mut line_buf = String::new();
        let first = self.cache.len();
        for (n, source) in lines[first..end_row].iter().enumerate() {
            if n % 64 == 63 && Instant::now() >= deadline {
                break;
            }
            line_buf.clear();
            line_buf.push_str(source);
            line_buf.push('\n');
            let ops = parse_state.parse_line(&line_buf, ss).unwrap_or_default();
            let ranges: Vec<_> =
                HighlightIterator::new(&mut highlight_state, &ops, &line_buf, &highlighter)
                    .collect();
            self.cache.push(CachedLine {
                source: source.clone(),
                line: Line::from(syntect_ranges_to_spans(&ranges)),
                parse_state: parse_state.clone(),
                highlight_state: highlight_state.clone(),
            });
        }
    }
}

/// Convert syntect highlight ranges to ratatui Spans.
/// Manual conversion to avoid ratatui version mismatch with syntect-tui.
/// Tabs become single spaces so the rendered text keeps the same byte offsets
/// (selection/search overlays are byte-based) and a predictable cell width.
fn syntect_ranges_to_spans(ranges: &[(syntect::highlighting::Style, &str)]) -> Vec<Span<'static>> {
    ranges
        .iter()
        .filter_map(|(style, text)| {
            let content = text.trim_end_matches('\n').replace('\t', " ");
            if content.is_empty() {
                return None;
            }
            Some(Span::styled(content, convert_syntect_style(style)))
        })
        .collect()
}

/// Convert a syntect Style to a ratatui Style (manual, no syntect-tui dependency).
pub fn convert_syntect_style(style: &syntect::highlighting::Style) -> Style {
    let mut ratatui_style = Style::default();

    // Foreground color (skip if alpha is 0 = transparent)
    if style.foreground.a > 0 {
        ratatui_style = ratatui_style.fg(Color::Rgb(
            style.foreground.r,
            style.foreground.g,
            style.foreground.b,
        ));
    }

    // Background: skip to use terminal default (D-10: keep it subtle).
    // Theme backgrounds tend to clash with terminal backgrounds.

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

#[cfg(test)]
mod tests {
    use super::MarkdownHighlighter;

    fn text_of(line: &ratatui::text::Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn incremental_cache_matches_fresh_highlight_after_edits() {
        let mut lines: Vec<String> = "# Title\n\n```rust\nfn main() {}\n```\n\n*em* text"
            .lines()
            .map(String::from)
            .collect();
        let mut cached = MarkdownHighlighter::new("base16-ocean.dark");
        let _ = cached.highlight_range(&lines, 0, lines.len());

        // Edit that changes multi-line state: close the fence earlier.
        lines.insert(3, "```".to_string());
        lines[0] = "## Title".to_string();
        let incremental = cached.highlight_range(&lines, 0, lines.len());
        let fresh =
            MarkdownHighlighter::new("base16-ocean.dark").highlight_range(&lines, 0, lines.len());
        assert_eq!(incremental, fresh);
        for (line, src) in incremental.iter().zip(&lines) {
            assert_eq!(&text_of(line), src);
        }
    }
}
