use std::sync::OnceLock;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use std::str::FromStr;
use syntect::highlighting::{
    FontStyle, ScopeSelectors, StyleModifier, Theme, ThemeItem, ThemeSettings,
};
use syntect::parsing::SyntaxSet;

use crate::theme::{adapt_color, to_rgb, Theme as AppTheme};

/// Process-wide syntect syntax definitions. Deserializing these is the most
/// expensive part of syntect setup, so it happens once and is shared by the
/// editor highlighter and the preview's code-block highlighting.
pub fn syntax_set() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// Build the code-token highlighting theme from the app theme.
///
/// Code in the preview's code blocks uses the same accent colors as the rest
/// of the UI, so every app theme gets matching syntax colors instead of an
/// unrelated bundled color scheme.
pub fn build_syntax_theme(theme: &AppTheme) -> Theme {
    use FontStyle as F;
    let color =
        |c: Color| to_rgb(c).map(|(r, g, b)| syntect::highlighting::Color { r, g, b, a: 0xFF });
    let fg = theme.editor_fg.and_then(color);
    let muted = color(theme.preview_muted_fg);
    let primary = color(theme.preview_accent_primary);
    let green = color(theme.preview_accent_secondary);
    let purple = color(theme.preview_accent_tertiary);
    let yellow = color(theme.preview_accent_warning);
    let red = color(theme.preview_accent_danger);

    // (scope selector, foreground, font style). More specific selectors win.
    let rules: [(
        &str,
        Option<syntect::highlighting::Color>,
        Option<FontStyle>,
    ); 8] = [
        // Code tokens
        (
            "comment, punctuation.definition.comment",
            muted,
            Some(F::ITALIC),
        ),
        ("string", green, None),
        (
            "constant.numeric, constant.language, constant.character, constant.other",
            yellow,
            None,
        ),
        ("keyword, storage", purple, None),
        (
            "entity.name.function, support.function, variable.function",
            primary,
            None,
        ),
        (
            "entity.name.type, entity.name.class, entity.name.struct, support.type, support.class",
            yellow,
            None,
        ),
        ("entity.name.tag", red, None),
        (
            "entity.other.attribute-name, variable.parameter",
            yellow,
            None,
        ),
    ];

    let scopes = rules
        .into_iter()
        .filter_map(|(selector, foreground, font_style)| {
            Some(ThemeItem {
                scope: ScopeSelectors::from_str(selector).ok()?,
                style: StyleModifier {
                    foreground,
                    background: None,
                    font_style,
                },
            })
        })
        .collect();

    Theme {
        name: None,
        author: None,
        settings: ThemeSettings {
            // Alpha 0 = "no explicit color": unscoped text keeps the
            // terminal's (or the pane's) default foreground.
            foreground: Some(fg.unwrap_or(syntect::highlighting::Color {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            })),
            ..ThemeSettings::default()
        },
        scopes,
    }
}

/// Apply a syntect style on top of `base` (foreground + bold/italic/underline).
pub fn apply_syntect_style(base: Style, style: &syntect::highlighting::Style) -> Style {
    let mut out = base;
    if style.foreground.a > 0 {
        out = out.fg(adapt_color(Color::Rgb(
            style.foreground.r,
            style.foreground.g,
            style.foreground.b,
        )));
    }
    if style.font_style.contains(FontStyle::BOLD) {
        out = out.add_modifier(Modifier::BOLD);
    }
    if style.font_style.contains(FontStyle::ITALIC) {
        out = out.add_modifier(Modifier::ITALIC);
    }
    if style.font_style.contains(FontStyle::UNDERLINE) {
        out = out.add_modifier(Modifier::UNDERLINED);
    }
    out
}

/// Markdown syntax highlighter for the editor pane.
///
/// A small purpose-built tokenizer rather than a general grammar engine: the
/// only state that crosses lines is "inside a fenced code block", so a frame
/// costs a cheap scan for fence markers plus tokenizing the visible lines.
/// There is nothing to warm up and nothing to cache — startup and jumps into
/// huge files are instant. (Syntect is still used for code tokens in the
/// preview's code blocks, where a real grammar earns its cost.)
pub struct MarkdownHighlighter {
    styles: Styles,
}

/// Editor styles derived from the app theme.
#[derive(Clone, Copy)]
struct Styles {
    text: Style,
    muted: Style,
    heading: Style,
    marker: Style,
    code: Style,
    link: Style,
    url: Style,
    task: Style,
}

impl MarkdownHighlighter {
    pub fn new(theme: &AppTheme) -> Self {
        let text = Style::default();
        let muted = Style::default().fg(theme.preview_muted_fg);
        MarkdownHighlighter {
            styles: Styles {
                text,
                muted,
                heading: Style::default()
                    .fg(theme.preview_accent_primary)
                    .add_modifier(Modifier::BOLD),
                marker: Style::default().fg(theme.preview_accent_tertiary),
                code: Style::default().fg(theme.preview_inline_code_fg),
                link: Style::default().fg(theme.preview_link_fg),
                url: Style::default()
                    .fg(theme.preview_link_fg)
                    .add_modifier(Modifier::UNDERLINED),
                task: Style::default().fg(theme.preview_accent_warning),
            },
        }
    }

    /// Return highlighted Lines for `lines[start_row..end_row]`.
    ///
    /// Each returned line's spans concatenate to exactly the source line
    /// (tabs shown as single spaces), so byte offsets into the source remain
    /// valid for selection and search overlays.
    pub fn highlight_range(
        &self,
        lines: &[String],
        start_row: usize,
        end_row: usize,
    ) -> Vec<Line<'static>> {
        let end_row = end_row.min(lines.len());
        if start_row >= end_row {
            return Vec::new();
        }

        // Fence state at `start_row`: replay only the fence markers above it.
        let mut fence: Option<Fence> = None;
        for line in &lines[..start_row] {
            step_fence(&mut fence, line);
        }

        lines[start_row..end_row]
            .iter()
            .map(|line| {
                let was_open = fence.is_some();
                let is_marker = step_fence(&mut fence, line);
                let mut out = Vec::new();
                if is_marker {
                    fence_line(line, was_open, &self.styles, &mut out);
                } else if was_open {
                    push(&mut out, line, self.styles.text);
                } else {
                    block_line(line, &self.styles, &mut out);
                }
                Line::from(out)
            })
            .collect()
    }
}

/// An open fenced code block: its marker character and length.
#[derive(Clone, Copy)]
struct Fence {
    ch: u8,
    len: usize,
}

/// If `line` is a fence marker line, return (indent, marker char, run length).
fn fence_marker(line: &str) -> Option<(usize, u8, usize)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = &line.as_bytes()[indent..];
    let ch = *rest.first()?;
    if ch != b'`' && ch != b'~' {
        return None;
    }
    let len = rest.iter().take_while(|&&b| b == ch).count();
    (len >= 3).then_some((indent, ch, len))
}

/// Advance fence state over `line`. Returns true when the line is itself a
/// fence marker (opening or closing).
fn step_fence(fence: &mut Option<Fence>, line: &str) -> bool {
    let Some((indent, ch, len)) = fence_marker(line) else {
        return false;
    };
    match *fence {
        None => {
            // A backtick fence's info string can't contain backticks.
            if ch == b'`' && line[indent + len..].contains('`') {
                return false;
            }
            *fence = Some(Fence { ch, len });
            true
        }
        Some(open) => {
            let closes = ch == open.ch && len >= open.len && line[indent + len..].trim().is_empty();
            if closes {
                *fence = None;
            }
            closes
        }
    }
}

/// Append a span, mapping tabs to single spaces (same byte length).
fn push(out: &mut Vec<Span<'static>>, text: &str, style: Style) {
    if text.is_empty() {
        return;
    }
    let content = if text.contains('\t') {
        text.replace('\t', " ")
    } else {
        text.to_string()
    };
    match out.last_mut() {
        Some(last) if last.style == style => last.content.to_mut().push_str(&content),
        _ => out.push(Span::styled(content, style)),
    }
}

/// ```` ```rust ````: dim marker, accented language.
fn fence_line(line: &str, closing: bool, st: &Styles, out: &mut Vec<Span<'static>>) {
    let Some((indent, _, len)) = fence_marker(line) else {
        return push(out, line, st.muted);
    };
    push(out, &line[..indent + len], st.muted);
    push(
        out,
        &line[indent + len..],
        if closing { st.muted } else { st.marker },
    );
}

/// Tokenize one line outside a code fence.
fn block_line(line: &str, st: &Styles, out: &mut Vec<Span<'static>>) {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();

    // ATX heading: `## Title`
    let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
    if (1..=6).contains(&hashes)
        && indent <= 3
        && trimmed[hashes..]
            .chars()
            .next()
            .map_or(true, char::is_whitespace)
    {
        push(
            out,
            &line[..indent + hashes],
            st.muted.add_modifier(Modifier::BOLD),
        );
        inline(&line[indent + hashes..], st.heading, st, out);
        return;
    }

    // Thematic break / setext underline: `---`, `***`, `===`
    if trimmed.len() >= 3
        && indent <= 3
        && [b'-', b'*', b'_', b'=']
            .iter()
            .any(|&c| trimmed.bytes().all(|b| b == c || b == b' ') && trimmed.as_bytes()[0] == c)
    {
        return push(out, line, st.muted);
    }

    // Block quote: `> text` (possibly nested)
    if trimmed.starts_with('>') {
        let markers = trimmed
            .bytes()
            .take_while(|&b| b == b'>' || b == b' ')
            .count();
        push(out, &line[..indent + markers], st.marker);
        let body = &line[indent + markers..];
        // GitHub alert / Obsidian callout marker: `[!NOTE]`
        if body.starts_with("[!") {
            if let Some(close) = body.find(']') {
                push(out, &body[..=close], st.task.add_modifier(Modifier::BOLD));
                inline(
                    &body[close + 1..],
                    st.muted.add_modifier(Modifier::ITALIC),
                    st,
                    out,
                );
                return;
            }
        }
        inline(body, st.muted.add_modifier(Modifier::ITALIC), st, out);
        return;
    }

    // Table row: `| a | b |` and delimiter rows `|---|:-:|`
    if trimmed.starts_with('|') {
        if trimmed
            .bytes()
            .all(|b| matches!(b, b'|' | b'-' | b':' | b' '))
        {
            return push(out, line, st.muted);
        }
        push(out, &line[..indent], st.text);
        let mut rest = trimmed;
        while let Some(pipe) = find_unescaped(rest, '|') {
            inline(&rest[..pipe], st.text, st, out);
            push(out, "|", st.muted);
            rest = &rest[pipe + 1..];
        }
        inline(rest, st.text, st, out);
        return;
    }

    // List item: `- `, `* `, `+ `, `1. `, `1) ` with optional task box
    if let Some(marker_len) = list_marker_len(trimmed) {
        push(out, &line[..indent], st.text);
        push(out, &trimmed[..marker_len], st.marker);
        let body = &trimmed[marker_len..];
        let lower = body.as_bytes().get(..4).map(|b| b.to_ascii_lowercase());
        if matches!(lower.as_deref(), Some(b"[ ] ") | Some(b"[x] ")) {
            push(out, &body[..3], st.task);
            inline(&body[3..], st.text, st, out);
        } else {
            inline(body, st.text, st, out);
        }
        return;
    }

    // HTML comment on one line
    if trimmed.starts_with("<!--") && trimmed.trim_end().ends_with("-->") {
        return push(out, line, st.muted.add_modifier(Modifier::ITALIC));
    }

    inline(line, st.text, st, out);
}

/// Byte length of a list marker (including its trailing space) at the start
/// of `text`, if there is one.
fn list_marker_len(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    match bytes.first()? {
        b'-' | b'*' | b'+' => (bytes.get(1) == Some(&b' ')).then_some(2),
        b'0'..=b'9' => {
            let digits = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
            (digits <= 9
                && matches!(bytes.get(digits), Some(b'.') | Some(b')'))
                && bytes.get(digits + 1) == Some(&b' '))
            .then_some(digits + 2)
        }
        _ => None,
    }
}

/// Position of the first `needle` not preceded by a backslash.
fn find_unescaped(text: &str, needle: char) -> Option<usize> {
    let mut escaped = false;
    for (i, c) in text.char_indices() {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == needle {
            return Some(i);
        }
    }
    None
}

/// Tokenize inline markdown in `text`, appending spans that cover every byte.
/// `base` is the style of plain text here (e.g. heading or bold context).
fn inline(text: &str, base: Style, st: &Styles, out: &mut Vec<Span<'static>>) {
    let bytes = text.as_bytes();
    let mut plain_from = 0;
    let mut i = 0;

    while i < bytes.len() {
        let consumed = match bytes[i] {
            b'\\' if i + 1 < bytes.len() => {
                // Escaped character: skip it (stay on a char boundary).
                let next = text[i + 1..].chars().next().map_or(1, char::len_utf8);
                i += 1 + next;
                continue;
            }
            b'`' => code_span(&text[i..], st),
            b'*' | b'_' => emphasis(text, i, base, st),
            b'~' => strike(&text[i..], base, st),
            b'!' | b'[' => link(&text[i..], st),
            b'<' => autolink(&text[i..], st),
            b'h' => bare_url(text, i, st),
            _ => None,
        };
        match consumed {
            Some((len, spans)) => {
                push(out, &text[plain_from..i], base);
                for span in spans {
                    push(out, &span.content, span.style);
                }
                i += len;
                plain_from = i;
            }
            None => i += 1,
        }
    }
    push(out, &text[plain_from..], base);
}

type Token = Option<(usize, Vec<Span<'static>>)>;

fn spans_of(parts: &[(&str, Style)]) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    for (text, style) in parts {
        push(&mut out, text, *style);
    }
    out
}

/// `` `code` `` (any backtick run length).
fn code_span(text: &str, st: &Styles) -> Token {
    let ticks = text.bytes().take_while(|&b| b == b'`').count();
    let fence = &text[..ticks];
    let rest = &text[ticks..];
    let mut search = 0;
    while let Some(pos) = rest[search..].find(fence) {
        let at = search + pos;
        let run = rest[at..].bytes().take_while(|&b| b == b'`').count();
        if run == ticks {
            return Some((
                ticks + at + ticks,
                spans_of(&[(fence, st.muted), (&rest[..at], st.code), (fence, st.muted)]),
            ));
        }
        search = at + run;
    }
    None
}

/// `*em*`, `**strong**`, `***both***` (and the `_` forms).
fn emphasis(text: &str, at: usize, base: Style, st: &Styles) -> Token {
    let bytes = text.as_bytes();
    let ch = bytes[at];
    let run = bytes[at..].iter().take_while(|&&b| b == ch).count().min(3);
    let delim = &text[at..at + run];
    let after = &text[at + run..];
    let word_char = |c: Option<char>| c.is_some_and(char::is_alphanumeric);

    // Must open against text, and `_` must not be inside a word (snake_case).
    if after.chars().next().map_or(true, char::is_whitespace)
        || (ch == b'_' && word_char(text[..at].chars().next_back()))
    {
        return None;
    }

    let mut search = 0;
    while let Some(pos) = after[search..].find(delim) {
        let close = search + pos;
        let before_ok = close > 0
            && !after[..close]
                .chars()
                .next_back()
                .is_some_and(char::is_whitespace);
        let tail = &after[close + run..];
        let after_ok =
            !(tail.starts_with(ch as char) || ch == b'_' && word_char(tail.chars().next()));
        if before_ok && after_ok {
            let style = match run {
                1 => base.add_modifier(Modifier::ITALIC),
                2 => base.add_modifier(Modifier::BOLD),
                _ => base.add_modifier(Modifier::BOLD | Modifier::ITALIC),
            };
            let mut spans = Vec::new();
            push(&mut spans, delim, st.muted);
            inline(&after[..close], style, st, &mut spans);
            push(&mut spans, delim, st.muted);
            return Some((run + close + run, spans));
        }
        search = close + run;
    }
    None
}

/// `~~strike~~`
fn strike(text: &str, base: Style, st: &Styles) -> Token {
    let rest = text.strip_prefix("~~")?;
    let close = rest.find("~~")?;
    if close == 0 {
        return None;
    }
    let mut spans = Vec::new();
    push(&mut spans, "~~", st.muted);
    inline(
        &rest[..close],
        base.add_modifier(Modifier::CROSSED_OUT),
        st,
        &mut spans,
    );
    push(&mut spans, "~~", st.muted);
    Some((close + 4, spans))
}

/// `[text](url)`, `![alt](url)`, `[text][ref]`, `[[wikilink]]`.
fn link(text: &str, st: &Styles) -> Token {
    let bang = usize::from(text.starts_with('!'));
    let body = &text[bang..];
    if !body.starts_with('[') {
        return None;
    }
    if let Some(inner) = body.strip_prefix("[[") {
        let close = inner.find("]]")?;
        return Some((
            bang + close + 4,
            spans_of(&[
                (&text[..bang + 2], st.muted),
                (&inner[..close], st.link),
                ("]]", st.muted),
            ]),
        ));
    }
    let close = find_unescaped(&body[1..], ']')? + 1;
    let label = &body[1..close];
    let after = &body[close + 1..];
    let (open, shut) = match after.as_bytes().first()? {
        b'(' => ('(', ')'),
        b'[' => ('[', ']'),
        _ => return None,
    };
    let end = after.find(shut)?;
    let target = &after[1..end];
    let mut open_buf = [0u8; 4];
    let mut shut_buf = [0u8; 4];
    Some((
        bang + close + 1 + end + 1,
        spans_of(&[
            (&text[..bang + 1], st.muted),
            (label, st.link),
            ("]", st.muted),
            (open.encode_utf8(&mut open_buf), st.muted),
            (target, st.url),
            (shut.encode_utf8(&mut shut_buf), st.muted),
        ]),
    ))
}

/// `<https://example.com>`
fn autolink(text: &str, st: &Styles) -> Token {
    let close = text.find('>')?;
    let target = &text[1..close];
    (target.starts_with("http://") || target.starts_with("https://") || target.contains('@'))
        .then(|| {
            (
                close + 1,
                spans_of(&[("<", st.muted), (target, st.url), (">", st.muted)]),
            )
        })
        .filter(|_| !target.contains(char::is_whitespace))
}

/// A bare `http(s)://` URL in running text.
fn bare_url(text: &str, at: usize, st: &Styles) -> Token {
    let rest = &text[at..];
    if !(rest.starts_with("http://") || rest.starts_with("https://")) {
        return None;
    }
    if text[..at]
        .chars()
        .next_back()
        .is_some_and(char::is_alphanumeric)
    {
        return None;
    }
    let mut end = rest
        .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '`'))
        .unwrap_or(rest.len());
    while end > 0 && rest[..end].ends_with(['.', ',', ';', ':', '!', '?', ')', '\'', '*', '_']) {
        end -= 1;
    }
    (rest[..end].find("://").is_some_and(|i| i + 3 < end))
        .then(|| (end, spans_of(&[(&rest[..end], st.url)])))
}

/// Build the gutter cell for a line: ` <right-aligned number>  `. The
/// current line's number is drawn in `current_fg` so the eye finds the cursor.
pub fn line_number_span(row: usize, total_lines: usize, style: Style) -> Span<'static> {
    let width = digit_count(total_lines);
    Span::styled(format!(" {:>width$}  ", row + 1, width = width), style)
}

/// Total width of the gutter (1 pad + digits + 2 pad).
pub fn line_number_width(total_lines: usize) -> usize {
    digit_count(total_lines) + 3
}

/// Digits reserved for line numbers (at least 2, so the gutter doesn't
/// jump when a short file grows past 9 lines).
fn digit_count(n: usize) -> usize {
    let mut count = 1;
    let mut val = n;
    while val >= 10 {
        count += 1;
        val /= 10;
    }
    count.max(2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme as AppTheme;

    fn highlight(src: &str) -> Vec<Line<'static>> {
        let lines: Vec<String> = src.split('\n').map(String::from).collect();
        MarkdownHighlighter::new(&AppTheme::ocean()).highlight_range(&lines, 0, lines.len())
    }

    fn text_of(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn style_of(line: &Line, needle: &str) -> Style {
        line.spans
            .iter()
            .find(|s| s.content.contains(needle))
            .unwrap_or_else(|| panic!("{needle:?} not in {:?}", text_of(line)))
            .style
    }

    const TRICKY: &str = "\
# Heading with `code` and *em*
plain **bold** and __also bold__, *it* _it_ ***both*** ~~gone~~
snake_case_name stays plain, 2 * 3 * 4 too, and a lone ` backtick
[link](http://x.y) ![img](a.png) [ref][1] [[wiki]] <https://a.b> https://c.d/e.
> quote with **bold**
> [!WARNING] careful
- item `x`
  * nested
1. first
12) twelfth
- [ ] todo
- [X] done
| a | `b` | c \\| d |
|---|:-:|--:|
---
```rust
let s = \"**not bold**\"; // # not heading
```
~~~
- not a list
~~~
\ttab\tseparated — ünïcödé 日本語 🎉 *wide* `ü`
<!-- comment -->
\\*escaped\\* stars and trailing backslash \\
unclosed **bold and [link(
";

    /// Overlays (selection, search) index into lines by byte, so the spans
    /// must reproduce the source exactly (tabs become spaces, same length).
    #[test]
    fn spans_always_reproduce_the_source_line() {
        let highlighted = highlight(TRICKY);
        for (line, src) in highlighted.iter().zip(TRICKY.split('\n')) {
            assert_eq!(text_of(line), src.replace('\t', " "));
        }
        assert_eq!(highlighted.len(), TRICKY.split('\n').count());
    }

    #[test]
    fn styles_follow_markdown_structure() {
        let theme = AppTheme::ocean();
        let lines = highlight(TRICKY);
        let bold = |s: Style| s.add_modifier.contains(Modifier::BOLD);
        let italic = |s: Style| s.add_modifier.contains(Modifier::ITALIC);

        // Heading: accent + bold, inline code inside keeps the code color.
        assert_eq!(
            style_of(&lines[0], "Heading").fg,
            Some(theme.preview_accent_primary)
        );
        assert!(bold(style_of(&lines[0], "Heading")));
        assert_eq!(
            style_of(&lines[0], "code").fg,
            Some(theme.preview_inline_code_fg)
        );

        assert!(bold(style_of(&lines[1], "bold")));
        assert!(bold(style_of(&lines[1], "also bold")));
        assert!(italic(style_of(&lines[1], "it")));
        let both = style_of(&lines[1], "both");
        assert!(bold(both) && italic(both));
        assert!(style_of(&lines[1], "gone")
            .add_modifier
            .contains(Modifier::CROSSED_OUT));

        // No false emphasis in snake_case or arithmetic.
        assert_eq!(lines[2].spans.len(), 1);

        assert_eq!(style_of(&lines[3], "link").fg, Some(theme.preview_link_fg));
        assert!(style_of(&lines[3], "https://c.d/e")
            .add_modifier
            .contains(Modifier::UNDERLINED));
        // Sentence punctuation after a bare URL is not part of the link.
        let last = lines[3].spans.last().unwrap();
        assert_eq!((last.content.as_ref(), last.style), (".", Style::default()));

        assert!(bold(style_of(&lines[4], "bold")) && italic(style_of(&lines[4], "bold")));
        assert_eq!(
            style_of(&lines[6], "- ").fg,
            Some(theme.preview_accent_tertiary)
        );
        assert_eq!(
            style_of(&lines[10], "[ ]").fg,
            Some(theme.preview_accent_warning)
        );
    }

    #[test]
    fn fenced_code_is_left_alone_and_state_survives_scrolling() {
        let lines = highlight(TRICKY);
        let code = TRICKY
            .split('\n')
            .position(|l| l.starts_with("let s"))
            .unwrap();
        assert_eq!(lines[code].spans.len(), 1, "no markdown inside a fence");
        let tilde = TRICKY
            .split('\n')
            .position(|l| l == "- not a list")
            .unwrap();
        assert_eq!(lines[tilde].spans.len(), 1);

        // Highlighting a window that starts inside the fence gives the same
        // result as highlighting from the top.
        let src: Vec<String> = TRICKY.split('\n').map(String::from).collect();
        let window =
            MarkdownHighlighter::new(&AppTheme::ocean()).highlight_range(&src, code, code + 3);
        assert_eq!(window[..], lines[code..code + 3]);
    }
}
