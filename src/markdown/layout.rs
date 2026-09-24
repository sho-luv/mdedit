//! Width-aware layout for the rendered preview.
//!
//! The renderer emits logical preview lines ([`PLine`]) tagged with the source
//! line each span came from. [`layout`] wraps them to the pane width *once*,
//! producing final visual rows plus an exact row <-> source-line map. Drawing
//! a frame is then just copying the visible rows into the buffer: no per-frame
//! re-wrapping, measuring, or cloning of the whole document, and scroll sync
//! is an index lookup that can never drift from what is on screen.

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::RenderResult;

/// Marker source for spans that don't carry content from the document
/// (continuation indents, padding), so they never anchor scroll sync.
pub const NO_SRC: usize = usize::MAX;

/// How a logical line is fitted to the pane width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fit {
    /// Prose: word-wrap at whitespace, hard-breaking words wider than a row.
    Flow,
    /// Preformatted (code, tables, rules): break at the exact cell limit and,
    /// when `fill` is set, pad every row to the full width with that style so
    /// block backgrounds form a clean rectangle.
    Pre { fill: Option<Style> },
    /// Preformatted content inside a box: rows are hard-wrapped to the width
    /// minus the frame and drawn as `│ content │` in the `border` style
    /// (the top and bottom edges are separate lines).
    Framed { border: Style },
}

/// Page geometry for a layout pass.
#[derive(Clone, Copy, Debug)]
pub struct Page {
    /// Full pane width the result is cached for.
    pub pane_width: u16,
    /// Width available to content (excludes margins).
    pub content_width: usize,
    /// Blank cells added to the left of every row.
    pub margin: usize,
}

/// One logical preview line before wrapping.
pub struct PLine {
    pub spans: Vec<Span<'static>>,
    /// Source line of each span (parallel to `spans`).
    pub srcs: Vec<usize>,
    /// Source line of the line as a whole (used when it has no spans).
    pub src: usize,
    /// Prefix repeated on wrapped continuation rows (list indent, quote bar).
    pub hang: Vec<Span<'static>>,
    pub fit: Fit,
    /// Whether this line's rows may serve as the scroll-sync target for its
    /// source lines. Decoration (heading rules, table borders, blank spacing)
    /// is not an anchor.
    pub anchor: bool,
}

impl PLine {
    pub fn empty(src: usize, fit: Fit, anchor: bool) -> Self {
        PLine {
            spans: Vec::new(),
            srcs: Vec::new(),
            src,
            hang: Vec::new(),
            fit,
            anchor,
        }
    }

    /// A line whose spans all come from the same source line.
    pub fn uniform(spans: Vec<Span<'static>>, src: usize, fit: Fit, anchor: bool) -> Self {
        let srcs = vec![src; spans.len()];
        PLine {
            spans,
            srcs,
            src,
            hang: Vec::new(),
            fit,
            anchor,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }
}

/// Terminal cell width of a grapheme, matching ratatui's `Buffer::set_stringn`
/// (control characters are dropped, i.e. zero width).
fn grapheme_width(g: &str) -> usize {
    if g.contains(char::is_control) {
        0
    } else {
        g.width()
    }
}

/// Terminal cell width of a string as ratatui will draw it.
pub fn text_width(s: &str) -> usize {
    if s.bytes().all(|b| (0x20..0x7f).contains(&b)) {
        return s.len();
    }
    s.graphemes(true).map(grapheme_width).sum()
}

/// Total cell width of a list of spans.
pub fn spans_width(spans: &[Span<'_>]) -> usize {
    spans.iter().map(|s| text_width(&s.content)).sum()
}

/// A visual row under construction.
pub struct RowBuf {
    pieces: Vec<(String, Style, usize)>,
    pub width: usize,
    /// Width of the hang prefix; a row is "empty" until it exceeds this.
    base: usize,
}

impl RowBuf {
    fn new(hang: &[Span<'static>], hang_w: usize) -> Self {
        let mut row = RowBuf {
            pieces: Vec::new(),
            width: 0,
            base: hang_w,
        };
        for h in hang {
            row.push(&h.content, h.style, NO_SRC, 0);
        }
        row.width = hang_w;
        row
    }

    fn push(&mut self, text: &str, style: Style, src: usize, w: usize) {
        match self.pieces.last_mut() {
            Some(last) if last.1 == style && last.2 == src => last.0.push_str(text),
            _ => self.pieces.push((text.to_string(), style, src)),
        }
        self.width += w;
    }

    fn has_content(&self) -> bool {
        self.width > self.base
    }

    /// Source lines present in this row, in order of appearance.
    fn srcs(&self) -> impl Iterator<Item = usize> + '_ {
        self.pieces.iter().map(|p| p.2).filter(|&s| s != NO_SRC)
    }

    pub fn into_spans(self) -> Vec<Span<'static>> {
        self.pieces
            .into_iter()
            .map(|(text, style, _)| Span::styled(text, style))
            .collect()
    }
}

/// A run of text that must stay together (a word) or may be dropped at a
/// break (whitespace). Holds (text, style, src, width) pieces.
struct Unit<'a> {
    whitespace: bool,
    width: usize,
    pieces: Vec<(&'a str, Style, usize, usize)>,
}

fn units<'a>(spans: &'a [Span<'static>], srcs: &[usize]) -> Vec<Unit<'a>> {
    let mut out: Vec<Unit<'a>> = Vec::new();
    for (i, span) in spans.iter().enumerate() {
        let src = srcs.get(i).copied().unwrap_or(NO_SRC);
        let content: &'a str = span.content.as_ref();
        let mut start = 0;
        let mut run_ws: Option<bool> = None;
        let mut run_w = 0;
        for (idx, g) in content.grapheme_indices(true) {
            let ws = g.chars().all(char::is_whitespace);
            if run_ws.is_some_and(|r| r != ws) {
                push_piece(
                    &mut out,
                    &content[start..idx],
                    span.style,
                    src,
                    run_w,
                    run_ws.unwrap(),
                );
                start = idx;
                run_w = 0;
            }
            run_ws = Some(ws);
            run_w += grapheme_width(g);
        }
        if let Some(ws) = run_ws {
            push_piece(&mut out, &content[start..], span.style, src, run_w, ws);
        }
    }
    out
}

fn push_piece<'a>(
    out: &mut Vec<Unit<'a>>,
    text: &'a str,
    style: Style,
    src: usize,
    w: usize,
    whitespace: bool,
) {
    match out.last_mut() {
        Some(u) if u.whitespace == whitespace => {
            u.width += w;
            u.pieces.push((text, style, src, w));
        }
        _ => out.push(Unit {
            whitespace,
            width: w,
            pieces: vec![(text, style, src, w)],
        }),
    }
}

/// Word-wrap spans to `width` cells. Continuation rows start with `hang`.
pub fn wrap_flow(
    spans: &[Span<'static>],
    srcs: &[usize],
    width: usize,
    hang: &[Span<'static>],
) -> Vec<RowBuf> {
    let width = width.max(1);
    let (hang, hang_w) = usable_hang(hang, width);
    let mut rows = vec![RowBuf::new(&[], 0)];
    let mut at_break = false;

    for unit in units(spans, srcs) {
        let row = rows.last_mut().expect("rows is never empty");
        if unit.whitespace {
            if at_break {
                continue; // swallow whitespace at an automatic line break
            }
            if row.width + unit.width <= width {
                for (text, style, src, w) in unit.pieces {
                    row.push(text, style, src, w);
                }
            } else {
                rows.push(RowBuf::new(hang, hang_w));
                at_break = true;
            }
            continue;
        }

        at_break = false;
        if row.width + unit.width <= width {
            for (text, style, src, w) in unit.pieces {
                row.push(text, style, src, w);
            }
        } else if row.has_content() && hang_w + unit.width <= width {
            let mut row = RowBuf::new(hang, hang_w);
            for (text, style, src, w) in unit.pieces {
                row.push(text, style, src, w);
            }
            rows.push(row);
        } else {
            // Word wider than a row: hard-break it grapheme by grapheme.
            for (text, style, src, _) in unit.pieces {
                push_hard(&mut rows, text, style, src, width, hang, hang_w);
            }
        }
    }
    rows
}

/// Break spans at the exact cell limit (no word boundaries).
pub fn wrap_hard(
    spans: &[Span<'static>],
    srcs: &[usize],
    width: usize,
    hang: &[Span<'static>],
) -> Vec<RowBuf> {
    let width = width.max(1);
    let (hang, hang_w) = usable_hang(hang, width);
    let mut rows = vec![RowBuf::new(&[], 0)];
    for (i, span) in spans.iter().enumerate() {
        let src = srcs.get(i).copied().unwrap_or(NO_SRC);
        push_hard(
            &mut rows,
            &span.content,
            span.style,
            src,
            width,
            hang,
            hang_w,
        );
    }
    rows
}

fn push_hard(
    rows: &mut Vec<RowBuf>,
    text: &str,
    style: Style,
    src: usize,
    width: usize,
    hang: &[Span<'static>],
    hang_w: usize,
) {
    for g in text.graphemes(true) {
        let w = grapheme_width(g);
        let row = rows.last_mut().expect("rows is never empty");
        if row.width + w > width && row.has_content() {
            rows.push(RowBuf::new(hang, hang_w));
        }
        rows.last_mut()
            .expect("rows is never empty")
            .push(g, style, src, w);
    }
}

/// Drop the hang prefix when it would leave no room for content.
fn usable_hang<'a>(hang: &'a [Span<'static>], width: usize) -> (&'a [Span<'static>], usize) {
    let hang_w = spans_width(hang);
    if hang_w * 2 > width {
        (&[], 0)
    } else {
        (hang, hang_w)
    }
}

/// Wrap every logical line to `width` and build the row/source maps.
pub fn layout(plines: Vec<PLine>, page: Page, total_source_lines: usize) -> RenderResult {
    let w = page.content_width.max(1);
    let margin = " ".repeat(page.margin);
    let mut rows: Vec<Line<'static>> = Vec::with_capacity(plines.len() + plines.len() / 4);
    let mut row_source: Vec<usize> = Vec::with_capacity(rows.capacity());
    let mut first_row: Vec<Option<usize>> = vec![None; total_source_lines];

    for pl in plines {
        let (bufs, fill, frame) = match pl.fit {
            Fit::Flow => (wrap_flow(&pl.spans, &pl.srcs, w, &pl.hang), None, None),
            Fit::Pre { fill } => (wrap_hard(&pl.spans, &pl.srcs, w, &pl.hang), fill, None),
            Fit::Framed { border } => {
                let inner = w.saturating_sub(4).max(1);
                (
                    wrap_hard(&pl.spans, &pl.srcs, inner, &pl.hang),
                    None,
                    Some((border, inner)),
                )
            }
        };

        for (k, mut buf) in bufs.into_iter().enumerate() {
            let r = rows.len();
            let mut row_src = None;
            for s in buf.srcs() {
                row_src.get_or_insert(s);
                if pl.anchor && s < total_source_lines && first_row[s].is_none() {
                    first_row[s] = Some(r);
                }
            }
            if pl.anchor && k == 0 && pl.src < total_source_lines && first_row[pl.src].is_none() {
                first_row[pl.src] = Some(r);
            }
            if let Some(style) = fill {
                if buf.width < w {
                    let pad = " ".repeat(w - buf.width);
                    buf.push(&pad, style, NO_SRC, w - buf.width);
                }
            }
            row_source.push(row_src.unwrap_or(pl.src));

            let mut spans = Vec::with_capacity(buf.pieces.len() + 3);
            if !margin.is_empty() {
                spans.push(Span::raw(margin.clone()));
            }
            match frame {
                Some((border, inner)) => {
                    let pad = inner.saturating_sub(buf.width);
                    spans.push(Span::styled("\u{2502} ", border));
                    spans.extend(buf.into_spans());
                    spans.push(Span::raw(" ".repeat(pad)));
                    spans.push(Span::styled(" \u{2502}", border));
                }
                None => spans.extend(buf.into_spans()),
            }
            rows.push(Line::from(spans));
        }
    }

    // Source lines that produce no anchored row (blank lines, fences of
    // unterminated blocks) inherit the previous line's row.
    let mut source_to_row = Vec::with_capacity(total_source_lines);
    let mut last = 0;
    for slot in first_row {
        if let Some(r) = slot {
            last = r;
        }
        source_to_row.push(last);
    }

    RenderResult {
        rows,
        row_source,
        source_to_row,
        width: page.pane_width,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(rows: &[RowBuf]) -> Vec<String> {
        rows.iter()
            .map(|r| r.pieces.iter().map(|p| p.0.as_str()).collect())
            .collect()
    }

    #[test]
    fn flow_wraps_at_words_and_drops_break_whitespace() {
        let spans = vec![Span::raw("aaa bbb ccc")];
        let rows = wrap_flow(&spans, &[0], 7, &[]);
        assert_eq!(texts(&rows), vec!["aaa bbb", "ccc"]);
    }

    #[test]
    fn flow_uses_hang_on_continuation_rows() {
        let spans = vec![Span::raw("- "), Span::raw("one two three")];
        let hang = vec![Span::raw("  ")];
        let rows = wrap_flow(&spans, &[0, 0], 9, &hang);
        assert_eq!(texts(&rows), vec!["- one two", "  three"]);
    }

    #[test]
    fn wide_graphemes_never_overflow_the_row() {
        let spans = vec![Span::raw("日本語のテキストです ⚠\u{fe0f} emoji 🎉🎉🎉")];
        for width in 1..20 {
            for rows in [
                wrap_flow(&spans, &[0], width, &[]),
                wrap_hard(&spans, &[0], width, &[]),
            ] {
                for row in &rows {
                    let text: String = row.pieces.iter().map(|p| p.0.as_str()).collect();
                    // A single grapheme wider than the row is allowed alone.
                    assert!(
                        row.width <= width || text.graphemes(true).count() == 1,
                        "width {width}: {text:?} is {} cells",
                        row.width
                    );
                    assert_eq!(row.width, text_width(&text));
                }
            }
        }
    }
}
