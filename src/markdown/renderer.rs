use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use syntect::easy::HighlightLines;

use super::layout::{layout, spans_width, wrap_flow, Fit, PLine, Page, NO_SRC};
use super::{MarkdownRenderer, RenderResult};
use crate::config::RenderProfile;
use crate::highlighter::{apply_syntect_style, build_syntax_theme, syntax_set};
use crate::theme::Theme;

/// Markdown -> terminal preview renderer built directly on pulldown-cmark.
pub struct PreviewRenderer;

impl MarkdownRenderer for PreviewRenderer {
    fn render(
        &self,
        markdown: &str,
        profile: RenderProfile,
        theme: &Theme,
        width: u16,
    ) -> RenderResult {
        render_markdown(markdown, profile, theme, width)
    }
}

/// Accumulates the spans of the logical line currently being built, tagging
/// each span with the source line of the event that produced it.
struct Builder {
    out: Vec<PLine>,
    spans: Vec<Span<'static>>,
    srcs: Vec<usize>,
    hang: Vec<Span<'static>>,
    /// Source line of the event being processed.
    src: usize,
}

impl Builder {
    fn push(&mut self, span: Span<'static>) {
        if span.content.is_empty() {
            return;
        }
        self.spans.push(span);
        self.srcs.push(self.src);
    }

    /// Push a line prefix (bullet, quote bar) and extend the indent used on
    /// wrapped continuation rows so they align under the content.
    fn push_prefix(&mut self, span: Span<'static>, hang: Span<'static>) {
        self.hang.push(hang);
        self.push(span);
    }

    fn flush(&mut self) {
        if self.spans.is_empty() {
            self.hang.clear();
            return;
        }
        self.out.push(PLine {
            src: self.srcs[0],
            spans: std::mem::take(&mut self.spans),
            srcs: std::mem::take(&mut self.srcs),
            hang: std::mem::take(&mut self.hang),
            fit: Fit::Flow,
            anchor: true,
        });
    }

    /// Take the pending spans without emitting a line (table cells, headings).
    fn take(&mut self) -> (Vec<Span<'static>>, Vec<usize>) {
        self.hang.clear();
        (
            std::mem::take(&mut self.spans),
            std::mem::take(&mut self.srcs),
        )
    }

    /// Blank spacing row. Consecutive blanks collapse into one, and the
    /// document never starts with one.
    fn blank(&mut self) {
        match self.out.last() {
            None => return,
            Some(l) if l.is_empty() && l.fit == Fit::Flow => return,
            _ => {}
        }
        self.out.push(PLine::empty(self.src, Fit::Flow, false));
    }

    fn line(&mut self, line: PLine) {
        self.out.push(line);
    }
}

/// A table cell's inline content.
type Cell = Vec<Span<'static>>;

/// Maximum text width (cells) for the clean profile's reading column.
const CLEAN_MAX_WIDTH: usize = 100;
/// Clean-profile bullets by nesting depth (cycled).
const CLEAN_BULLETS: [&str; 3] = ["\u{2022}", "\u{25E6}", "\u{25AA}"];
/// Columns narrower than this switch a clean table to record layout.
const MIN_TABLE_COLUMN_WIDTH: usize = 3;

/// Render markdown to preview rows laid out for `width` cells.
/// Strips markdown syntax and applies terminal-appropriate styling.
fn render_markdown(
    markdown: &str,
    profile: RenderProfile,
    theme: &Theme,
    width: u16,
) -> RenderResult {
    let mut options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;

    match profile {
        RenderProfile::Clean | RenderProfile::Github | RenderProfile::Obsidian => {
            options |= Options::ENABLE_TABLES;
        }
        RenderProfile::CommonMark => {}
    }

    // Pre-process for Obsidian-specific syntax (never adds or removes lines,
    // so byte offsets still map onto the editor's line numbers).
    let processed = if profile == RenderProfile::Obsidian {
        preprocess_obsidian(markdown)
    } else {
        markdown.to_string()
    };

    // The clean profile reads like a document: a one-cell margin on each
    // side and a capped line length on wide panes (Warp's notebook column).
    let clean = profile == RenderProfile::Clean;
    let pane_cells = usize::from(width.max(1));
    let width_cells = if clean && pane_cells >= 40 {
        (pane_cells - 2).min(CLEAN_MAX_WIDTH)
    } else {
        pane_cells
    };
    // Center the reading column when the pane is wider than the cap.
    let margin = (pane_cells - width_cells) / 2;
    let page = Page {
        pane_width: width,
        content_width: width_cells,
        margin,
    };
    let line_starts: Vec<usize> = std::iter::once(0)
        .chain(memchr_newlines(&processed))
        .collect();
    let total_source_lines = line_starts.len();
    let byte_to_source_line = |byte_offset: usize| -> usize {
        match line_starts.binary_search(&byte_offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        }
    };

    let mut b = Builder {
        out: Vec::new(),
        spans: Vec::new(),
        srcs: Vec::new(),
        hang: Vec::new(),
        src: 0,
    };
    let mut style_stack: Vec<Style> = vec![Style::default()];
    // One entry per open list: Some(next number) for ordered lists.
    let mut list_stack: Vec<Option<u64>> = Vec::new();
    // Per open list item: its indent, and whether it pushed a strike-through
    // style (completed task in the clean profile).
    let mut item_stack: Vec<(String, bool)> = Vec::new();
    let mut heading_level: Option<HeadingLevel> = None;
    // > 0 while inside a link or image (bare-URL detection is skipped there).
    let mut link_depth = 0usize;

    // Code block state
    let mut in_code_block = false;
    let mut code_text = String::new();
    let mut code_block_lang: Option<String> = None;
    let mut code_fenced = false;
    let mut code_start_src = 0;

    // Table state
    let mut table_rows: Vec<(Vec<Cell>, usize)> = Vec::new();
    let mut current_row: Vec<Cell> = Vec::new();
    let mut row_src = 0;
    let mut in_table_cell = false;
    let mut table_alignments: Vec<Alignment> = Vec::new();

    // Blockquote / callout state
    let mut in_blockquote = false;
    let mut blockquote_callout: Option<String> = None;
    let mut blockquote_color = theme.preview_muted_fg;
    // Buffer to accumulate text fragments at the start of a blockquote
    // (pulldown-cmark splits [!note] into "[", "!note", "]")
    let mut bq_detect_buf: Option<String> = None;

    let quote_bar = |color: Color| Span::styled("\u{2502} ", Style::default().fg(color));
    let callout_glyph = if clean { "\u{258E} " } else { "\u{2588} " };
    let callout_bar = |color: Color| Span::styled(callout_glyph, Style::default().fg(color));

    // Top-level blocks for the live view: (source line, first logical line).
    // Items of a top-level list count as blocks of their own, so editing one
    // item doesn't un-render the whole list.
    let mut block_starts: Vec<(usize, usize)> = Vec::new();
    let mut depth = 0usize;

    for (event, range) in Parser::new_ext(&processed, options).into_offset_iter() {
        b.src = byte_to_source_line(range.start);

        let starts_block = match &event {
            Event::Start(Tag::Item) => depth == 1,
            Event::Start(_) | Event::Rule => depth == 0,
            _ => false,
        };
        if starts_block && block_starts.last().map_or(true, |&(src, _)| src < b.src) {
            b.flush();
            block_starts.push((b.src, b.out.len()));
        }
        match &event {
            Event::Start(_) => depth += 1,
            Event::End(_) => depth = depth.saturating_sub(1),
            _ => {}
        }

        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    b.flush();
                    heading_level = Some(level);
                    style_stack.push(heading_style(level, profile, theme));
                }
                Tag::Paragraph => {
                    // If inside a blockquote that hasn't been classified yet,
                    // this is where content starts — begin detection
                    if in_blockquote && blockquote_callout.is_none() && bq_detect_buf.is_none() {
                        bq_detect_buf = Some(String::new());
                    }
                }
                Tag::Emphasis => {
                    let base = current_style(&style_stack);
                    style_stack.push(base.add_modifier(Modifier::ITALIC));
                }
                Tag::Strong => {
                    let base = current_style(&style_stack);
                    style_stack.push(base.add_modifier(Modifier::BOLD));
                }
                Tag::Strikethrough => {
                    let base = current_style(&style_stack);
                    style_stack.push(base.add_modifier(Modifier::CROSSED_OUT));
                }
                Tag::CodeBlock(kind) => {
                    b.flush();
                    in_code_block = true;
                    code_text.clear();
                    code_fenced = matches!(kind, CodeBlockKind::Fenced(_));
                    // Fenced content starts on the line after the fence.
                    code_start_src = if code_fenced { b.src + 1 } else { b.src };
                    code_block_lang = match kind {
                        CodeBlockKind::Fenced(lang) => {
                            let l = lang.trim().to_string();
                            (!l.is_empty()).then_some(l)
                        }
                        CodeBlockKind::Indented => None,
                    };
                }
                Tag::List(start) => {
                    b.flush();
                    list_stack.push(start);
                }
                Tag::Item => {
                    b.flush();
                    let indent = "  ".repeat(list_stack.len().saturating_sub(1));
                    let bullet = match list_stack.last_mut() {
                        Some(Some(idx)) => {
                            let s = format!("{}{}. ", indent, idx);
                            *idx += 1;
                            s
                        }
                        _ if clean => {
                            let glyph = CLEAN_BULLETS[(list_stack.len() - 1) % CLEAN_BULLETS.len()];
                            format!("{}{} ", indent, glyph)
                        }
                        _ => format!("{}\u{2022} ", indent),
                    };
                    item_stack.push((indent, false));
                    let hang = " ".repeat(spans_width(&[Span::raw(bullet.as_str())]));
                    b.push_prefix(
                        Span::styled(bullet, Style::default().fg(theme.preview_muted_fg)),
                        Span::raw(hang),
                    );
                }
                Tag::BlockQuote(_) => {
                    in_blockquote = true;
                    blockquote_callout = None;
                    blockquote_color = theme.preview_muted_fg;
                    bq_detect_buf = None;
                    let base = current_style(&style_stack);
                    style_stack.push(base.fg(theme.preview_muted_fg));
                }
                Tag::Link { .. } => {
                    link_depth += 1;
                    let base = current_style(&style_stack);
                    style_stack.push(
                        base.fg(theme.preview_link_fg)
                            .add_modifier(Modifier::UNDERLINED),
                    );
                }
                Tag::Image { .. } => {
                    // Terminals can't show the picture; mark it and show the
                    // alt text so it doesn't read as ordinary prose.
                    link_depth += 1;
                    let style = Style::default()
                        .fg(theme.preview_muted_fg)
                        .add_modifier(Modifier::ITALIC);
                    b.push(Span::styled("\u{25A3} ", style));
                    style_stack.push(style);
                }
                Tag::Table(alignments) => {
                    b.flush();
                    table_rows.clear();
                    table_alignments = alignments;
                }
                Tag::TableHead | Tag::TableRow => {
                    current_row.clear();
                    row_src = b.src;
                }
                Tag::TableCell => {
                    in_table_cell = true;
                    b.take();
                }
                _ => {}
            },
            Event::End(tag_end) => match tag_end {
                TagEnd::Heading(_) => {
                    style_stack.pop();
                    if let Some(level) = heading_level.take() {
                        let (spans, srcs) = b.take();
                        render_heading(&mut b, spans, srcs, level, profile, theme, width_cells);
                    } else {
                        b.flush();
                    }
                    b.blank();
                }
                TagEnd::Paragraph => {
                    // If we were buffering for callout detection, flush as regular blockquote
                    if let Some(buf) = bq_detect_buf.take() {
                        if !buf.is_empty() {
                            b.push_prefix(
                                quote_bar(theme.preview_muted_fg),
                                quote_bar(theme.preview_muted_fg),
                            );
                            b.push(Span::styled(buf, current_style(&style_stack)));
                        }
                    }
                    b.flush();
                    b.blank();
                }
                TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                    style_stack.pop();
                }
                TagEnd::Link | TagEnd::Image => {
                    link_depth = link_depth.saturating_sub(1);
                    style_stack.pop();
                }
                TagEnd::CodeBlock => {
                    in_code_block = false;
                    let code = CodeBlock {
                        text: &code_text,
                        lang: code_block_lang.as_deref(),
                        start_src: code_start_src,
                        fenced: code_fenced,
                    };
                    if clean && width_cells >= 8 {
                        render_code_block_framed(&mut b, &code, theme, width_cells);
                    } else {
                        render_code_block(&mut b, &code, theme);
                    }
                    code_text.clear();
                    code_block_lang = None;
                    b.blank();
                }
                TagEnd::List(_) => {
                    b.flush();
                    list_stack.pop();
                    if list_stack.is_empty() {
                        b.blank();
                    }
                }
                TagEnd::Item => {
                    b.flush();
                    if let Some((_, true)) = item_stack.pop() {
                        style_stack.pop();
                    }
                }
                TagEnd::BlockQuote(_) => {
                    style_stack.pop();
                    in_blockquote = false;
                    blockquote_callout = None;
                    bq_detect_buf = None;
                    b.flush();
                }
                TagEnd::TableCell => {
                    in_table_cell = false;
                    current_row.push(b.take().0);
                }
                TagEnd::TableHead | TagEnd::TableRow => {
                    table_rows.push((std::mem::take(&mut current_row), row_src));
                }
                TagEnd::Table => {
                    if clean {
                        render_table_clean(
                            &mut b,
                            &table_rows,
                            &table_alignments,
                            theme,
                            width_cells,
                        );
                    } else {
                        render_table(&mut b, &table_rows, &table_alignments, theme, width_cells);
                    }
                    table_rows.clear();
                    table_alignments.clear();
                    b.blank();
                }
                _ => {}
            },
            Event::Text(text) => {
                if in_code_block {
                    code_text.push_str(&text);
                } else if in_table_cell {
                    b.push(Span::styled(
                        text.replace('\t', " "),
                        current_style(&style_stack),
                    ));
                } else if let Some(ref mut buf) = bq_detect_buf {
                    // Accumulating text at the start of a blockquote to detect callouts.
                    buf.push_str(&text);

                    // Check if we have a complete callout marker like [!note] or [!WARNING]
                    if let Some(callout) = parse_callout_marker(buf) {
                        let rest = buf
                            .find(']')
                            .map(|close| buf[close + 1..].trim().to_string())
                            .unwrap_or_default();
                        bq_detect_buf = None;
                        let (icon, color) = callout_style(&callout, theme);
                        blockquote_color = color;
                        push_callout_header(&mut b, &callout, icon, color, clean);
                        blockquote_callout = Some(callout);

                        // Any text after the ] marker
                        if !rest.is_empty() {
                            b.push_prefix(callout_bar(color), callout_bar(color));
                            b.push(Span::styled(rest, current_style(&style_stack)));
                        }
                    } else if !buf.starts_with('[') || buf.len() > 30 {
                        // Not a callout — flush as regular blockquote text
                        let content = bq_detect_buf.take().unwrap_or_default();
                        b.push_prefix(
                            quote_bar(theme.preview_muted_fg),
                            quote_bar(theme.preview_muted_fg),
                        );
                        b.push(Span::styled(content, current_style(&style_stack)));
                    }
                    // else: keep buffering (we have "[" or "[!" but no "]" yet)
                } else {
                    // Regular text or continuation of a callout blockquote
                    if in_blockquote && b.spans.is_empty() {
                        let bar = if blockquote_callout.is_some() {
                            callout_bar(blockquote_color)
                        } else {
                            quote_bar(theme.preview_muted_fg)
                        };
                        b.push_prefix(bar.clone(), bar);
                    }
                    let style = current_style(&style_stack);
                    let text = text.replace('\t', " ");
                    if link_depth == 0 && text.contains("://") {
                        let link_style = style
                            .fg(theme.preview_link_fg)
                            .add_modifier(Modifier::UNDERLINED);
                        for (part, is_url) in split_bare_urls(&text) {
                            b.push(Span::styled(
                                part.to_string(),
                                if is_url { link_style } else { style },
                            ));
                        }
                    } else {
                        b.push(Span::styled(text, style));
                    }
                }
            }
            Event::Code(code) => {
                let style = Style::default()
                    .fg(theme.preview_inline_code_fg)
                    .bg(theme.preview_inline_code_bg);
                // Clean: tight code spans, like Warp's inline-code pills.
                let content = if clean {
                    code.to_string()
                } else {
                    format!(" {} ", code)
                };
                b.push(Span::styled(content, style));
            }
            Event::SoftBreak => {
                if let Some(buf) = bq_detect_buf.take() {
                    // End of first line in blockquote — finalize detection
                    if let Some(callout) = parse_callout_marker(&buf) {
                        let (icon, color) = callout_style(&callout, theme);
                        blockquote_color = color;
                        push_callout_header(&mut b, &callout, icon, color, clean);
                        blockquote_callout = Some(callout);
                    } else {
                        b.push_prefix(
                            quote_bar(theme.preview_muted_fg),
                            quote_bar(theme.preview_muted_fg),
                        );
                        b.push(Span::styled(buf, current_style(&style_stack)));
                        b.flush();
                    }
                } else if in_blockquote {
                    b.flush();
                } else {
                    b.push(Span::raw(" "));
                }
            }
            Event::HardBreak => {
                // Keep the hang (list indent / quote bar) for the next line.
                let hang = b.hang.clone();
                b.flush();
                for h in hang {
                    b.push_prefix(h.clone(), h);
                }
            }
            Event::Rule => {
                b.flush();
                b.line(PLine::uniform(
                    vec![Span::styled(
                        "\u{2500}".repeat(width_cells),
                        Style::default().fg(theme.preview_muted_fg),
                    )],
                    b.src,
                    Fit::Pre { fill: None },
                    true,
                ));
                b.blank();
            }
            Event::TaskListMarker(checked) if clean => {
                // The checkbox replaces the bullet; completed items are
                // struck through and dimmed.
                if let Some(bullet_at) = b.spans.len().checked_sub(1) {
                    b.spans.truncate(bullet_at);
                    b.srcs.truncate(bullet_at);
                    b.hang.pop();
                }
                let indent = item_stack
                    .last()
                    .map(|(i, _)| i.clone())
                    .unwrap_or_default();
                let (marker, color) = if checked {
                    ("\u{2611} ", theme.preview_accent_secondary)
                } else {
                    ("\u{2610} ", theme.preview_muted_fg)
                };
                let hang = " ".repeat(spans_width(&[Span::raw(indent.as_str())]) + 2);
                b.push_prefix(
                    Span::styled(format!("{}{}", indent, marker), Style::default().fg(color)),
                    Span::raw(hang),
                );
                if checked {
                    let base = current_style(&style_stack);
                    style_stack.push(
                        base.fg(theme.preview_muted_fg)
                            .add_modifier(Modifier::CROSSED_OUT),
                    );
                    if let Some(item) = item_stack.last_mut() {
                        item.1 = true;
                    }
                }
            }
            Event::TaskListMarker(checked) => {
                let marker = if checked { "\u{2611} " } else { "\u{2610} " };
                b.push_prefix(
                    Span::styled(
                        marker.to_string(),
                        Style::default().fg(if checked {
                            theme.preview_accent_secondary
                        } else {
                            theme.preview_muted_fg
                        }),
                    ),
                    Span::raw("  "),
                );
            }
            _ => {}
        }
    }

    b.flush();
    layout(b.out, page, total_source_lines, &block_starts)
}

/// Split text into (segment, is_url) pieces, recognizing bare `http://` and
/// `https://` URLs. Trailing sentence punctuation is left outside the URL.
fn split_bare_urls(text: &str) -> Vec<(&str, bool)> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = ["https://", "http://"]
        .iter()
        .filter_map(|scheme| rest.find(scheme))
        .min()
    {
        let tail = &rest[start..];
        let mut end = tail
            .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"'))
            .unwrap_or(tail.len());
        while end > 0 && tail[..end].ends_with(['.', ',', ';', ':', '!', '?', ')', '\'']) {
            end -= 1;
        }
        if tail[..end].find("://").is_some_and(|i| i + 3 < end) {
            if start > 0 {
                out.push((&rest[..start], false));
            }
            out.push((&tail[..end], true));
            rest = &tail[end..];
        } else {
            // Scheme with nothing after it: keep as plain text.
            let skip = start + tail.find("://").map_or(tail.len(), |i| i + 3);
            out.push((&rest[..skip], false));
            rest = &rest[skip..];
        }
    }
    if !rest.is_empty() {
        out.push((rest, false));
    }
    out
}

/// Byte offsets just past each '\n' (i.e. the start of every following line).
fn memchr_newlines(text: &str) -> impl Iterator<Item = usize> + '_ {
    text.bytes()
        .enumerate()
        .filter(|(_, b)| *b == b'\n')
        .map(|(i, _)| i + 1)
}

/// Emit a callout's header line: thick colored bar + icon + title.
fn push_callout_header(b: &mut Builder, callout: &str, icon: &str, color: Color, clean: bool) {
    let label_style = Style::default().fg(color).add_modifier(Modifier::BOLD);
    if clean {
        // The colored bar and label carry the meaning; no icon.
        let mut chars = callout.chars();
        let title: String = chars
            .next()
            .map(|c| c.to_uppercase().chain(chars).collect())
            .unwrap_or_default();
        b.push(Span::styled("\u{258E} ", Style::default().fg(color)));
        b.push(Span::styled(title, label_style));
    } else {
        b.push(Span::styled("\u{2588} ", Style::default().fg(color)));
        b.push(Span::styled(
            format!("{} {}", icon, callout.to_uppercase()),
            label_style,
        ));
    }
    b.flush();
}

/// Expand tabs to 4-column tab stops (terminals can't draw a tab in a cell).
fn expand_tabs(line: &str) -> String {
    if !line.contains('\t') {
        return line.to_string();
    }
    let mut out = String::with_capacity(line.len() + 8);
    let mut col = 0;
    for c in line.chars() {
        if c == '\t' {
            let n = 4 - col % 4;
            out.push_str(&" ".repeat(n));
            col += n;
        } else {
            out.push(c);
            col += 1;
        }
    }
    out
}

/// A fenced or indented code block collected from the parser.
struct CodeBlock<'a> {
    text: &'a str,
    lang: Option<&'a str>,
    /// Source line of the first code line.
    start_src: usize,
    fenced: bool,
}

impl CodeBlock<'_> {
    /// Code lines with the trailing newline removed and tabs expanded.
    fn lines(&self) -> Vec<String> {
        if self.text.is_empty() {
            return Vec::new();
        }
        let body = self.text.strip_suffix('\n').unwrap_or(self.text);
        body.split('\n')
            .map(|l| expand_tabs(l.strip_suffix('\r').unwrap_or(l)))
            .collect()
    }

    /// Source line of the opening fence (or of the block start if indented).
    fn open_src(&self) -> usize {
        if self.fenced {
            self.start_src.saturating_sub(1)
        } else {
            self.start_src
        }
    }
}

/// Syntax-highlight code lines. `base` supplies the fallback foreground and
/// any background; syntect colors override the foreground per token.
fn highlight_code(
    lines: &[String],
    lang: Option<&str>,
    theme: &Theme,
    base: Style,
) -> Vec<Vec<Span<'static>>> {
    let ss = syntax_set();
    let syntax = lang.and_then(|l| ss.find_syntax_by_token(l));
    let Some(syntax) = syntax else {
        return lines
            .iter()
            .map(|l| vec![Span::styled(l.clone(), base)])
            .collect();
    };
    let syntax_theme = build_syntax_theme(theme);
    let mut highlighter = HighlightLines::new(syntax, &syntax_theme);
    lines
        .iter()
        .map(|code_line| {
            let line_with_nl = format!("{}\n", code_line);
            let ranges = highlighter
                .highlight_line(&line_with_nl, ss)
                .unwrap_or_default();
            ranges
                .into_iter()
                .filter_map(|(style, text)| {
                    let content = text.trim_end_matches('\n');
                    if content.is_empty() {
                        return None;
                    }
                    Some(Span::styled(
                        content.to_string(),
                        apply_syntect_style(base, &style),
                    ))
                })
                .collect()
        })
        .collect()
}

/// Render a code block as a full-width shaded rectangle, one preview line per
/// source line so the cursor tracks individual code lines.
fn render_code_block(b: &mut Builder, code: &CodeBlock<'_>, theme: &Theme) {
    let bg = theme.preview_code_bg;
    let fill = Style::default().bg(bg);
    let fit = Fit::Pre { fill: Some(fill) };
    let pad = || Span::styled("  ", fill);
    let lines = code.lines();

    // Top padding stands in for the opening fence.
    b.line(PLine::empty(code.open_src(), fit, code.fenced));

    let base = Style::default().fg(theme.preview_inline_code_fg).bg(bg);
    for (k, tokens) in highlight_code(&lines, code.lang, theme, base)
        .into_iter()
        .enumerate()
    {
        let mut spans = vec![pad()];
        spans.extend(tokens);
        let mut line = PLine::uniform(spans, code.start_src + k, fit, true);
        line.hang = vec![pad()];
        b.line(line);
    }

    // Bottom padding stands in for the closing fence.
    b.line(PLine::empty(code.start_src + lines.len(), fit, code.fenced));
}

/// Render a code block as an outlined box on the page background, with the
/// language label set into the top border (Warp's outlined code blocks):
///
/// ```text
/// ╭─ rust ─────────╮
/// │ fn main() {}   │
/// ╰────────────────╯
/// ```
fn render_code_block_framed(b: &mut Builder, code: &CodeBlock<'_>, theme: &Theme, width: usize) {
    let border = Style::default().fg(theme.preview_muted_fg);
    let lines = code.lines();

    let mut top = vec![Span::styled("\u{256D}\u{2500}", border)];
    let mut used = 2;
    if let Some(lang) = code.lang {
        let label_w = spans_width(&[Span::raw(lang)]);
        if used + label_w + 3 <= width {
            top.push(Span::styled(" ", border));
            top.push(Span::styled(
                lang.to_string(),
                Style::default()
                    .fg(theme.preview_muted_fg)
                    .add_modifier(Modifier::ITALIC),
            ));
            top.push(Span::styled(" ", border));
            used += label_w + 2;
        }
    }
    top.push(Span::styled(
        format!(
            "{}\u{256E}",
            "\u{2500}".repeat(width.saturating_sub(used + 1))
        ),
        border,
    ));
    let pre = Fit::Pre { fill: None };
    b.line(PLine::uniform(top, code.open_src(), pre, code.fenced));

    let fit = Fit::Framed { border };
    let base = Style::default().fg(theme.preview_fg);
    if lines.is_empty() {
        b.line(PLine::empty(code.start_src, fit, false));
    }
    for (k, tokens) in highlight_code(&lines, code.lang, theme, base)
        .into_iter()
        .enumerate()
    {
        b.line(PLine::uniform(tokens, code.start_src + k, fit, true));
    }

    let bottom = format!(
        "\u{2570}{}\u{256F}",
        "\u{2500}".repeat(width.saturating_sub(2))
    );
    b.line(PLine::uniform(
        vec![Span::styled(bottom, border)],
        code.start_src + lines.len(),
        pre,
        code.fenced,
    ));
}

/// Render a heading with visual size differentiation.
fn render_heading(
    b: &mut Builder,
    spans: Vec<Span<'static>>,
    srcs: Vec<usize>,
    level: HeadingLevel,
    profile: RenderProfile,
    theme: &Theme,
    width: usize,
) {
    let src = srcs.first().copied().unwrap_or(b.src);
    if profile == RenderProfile::Clean {
        // No rules, markers, or case changes: hierarchy comes from weight and
        // color alone (see `heading_style`), with a blank row above.
        b.blank();
        b.line(PLine {
            spans,
            srcs,
            src,
            hang: Vec::new(),
            fit: Fit::Flow,
            anchor: true,
        });
        return;
    }
    let accent = Style::default().fg(heading_accent(level, profile, theme));
    let rule = |ch: &str| {
        PLine::uniform(
            vec![Span::styled(ch.repeat(width), accent)],
            src,
            Fit::Pre { fill: None },
            false,
        )
    };
    let content = |spans: Vec<Span<'static>>, srcs: Vec<usize>, hang: Vec<Span<'static>>| PLine {
        spans,
        srcs,
        src,
        hang,
        fit: Fit::Flow,
        anchor: true,
    };
    let with_marker = |marker: &str, spans: Vec<Span<'static>>, srcs: Vec<usize>| {
        let mut s = vec![Span::styled(marker.to_string(), accent)];
        s.extend(spans);
        let mut ss = vec![src];
        ss.extend(srcs);
        content(s, ss, vec![Span::raw("  ")])
    };

    match level {
        HeadingLevel::H1 => {
            b.blank();
            b.line(rule("\u{2550}"));
            let upper = spans
                .into_iter()
                .map(|s| Span::styled(s.content.to_uppercase(), s.style))
                .collect();
            b.line(content(upper, srcs, Vec::new()));
            b.line(rule("\u{2550}"));
        }
        HeadingLevel::H2 => {
            b.blank();
            b.line(content(spans, srcs, Vec::new()));
            b.line(rule("\u{2500}"));
        }
        HeadingLevel::H3 => {
            b.blank();
            b.line(with_marker("\u{25B6} ", spans, srcs));
        }
        HeadingLevel::H4 => b.line(with_marker("\u{25B8} ", spans, srcs)),
        _ => b.line(content(spans, srcs, Vec::new())),
    }
}

/// Get the accent color for heading borders/decorations by profile.
fn heading_accent(level: HeadingLevel, profile: RenderProfile, theme: &Theme) -> Color {
    match profile {
        RenderProfile::Clean => theme.preview_muted_fg,
        RenderProfile::Github => match level {
            HeadingLevel::H1 | HeadingLevel::H2 => theme.preview_accent_primary,
            _ => theme.preview_muted_fg,
        },
        RenderProfile::Obsidian => match level {
            HeadingLevel::H1 | HeadingLevel::H2 => theme.preview_accent_tertiary,
            HeadingLevel::H3 | HeadingLevel::H4 => theme.preview_accent_secondary,
            _ => theme.preview_muted_fg,
        },
        RenderProfile::CommonMark => theme.preview_link_fg,
    }
}

/// Heading text style varies by profile.
fn heading_style(level: HeadingLevel, profile: RenderProfile, theme: &Theme) -> Style {
    match profile {
        RenderProfile::Clean => match level {
            HeadingLevel::H1 => Style::default()
                .fg(theme.preview_accent_primary)
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H2 => Style::default()
                .fg(theme.preview_fg)
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H3 | HeadingLevel::H4 => Style::default()
                .fg(theme.preview_muted_fg)
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H5 | HeadingLevel::H6 => Style::default().fg(theme.preview_muted_fg),
        },
        RenderProfile::Github => match level {
            HeadingLevel::H1 | HeadingLevel::H2 | HeadingLevel::H3 => Style::default()
                .fg(theme.preview_fg)
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H4 => Style::default()
                .fg(theme.preview_fg)
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H5 => Style::default()
                .fg(theme.preview_muted_fg)
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H6 => Style::default().fg(theme.preview_muted_fg),
        },
        RenderProfile::Obsidian => match level {
            HeadingLevel::H1 | HeadingLevel::H2 => Style::default()
                .fg(theme.preview_accent_tertiary)
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H3 => Style::default()
                .fg(theme.preview_accent_secondary)
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H4 => Style::default()
                .fg(theme.preview_accent_warning)
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H5 => Style::default()
                .fg(theme.preview_accent_danger)
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H6 => Style::default().fg(theme.preview_muted_fg),
        },
        RenderProfile::CommonMark => match level {
            HeadingLevel::H1 => Style::default()
                .fg(theme.preview_link_fg)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            HeadingLevel::H2 => Style::default()
                .fg(theme.preview_accent_primary)
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H3 => Style::default()
                .fg(theme.preview_accent_tertiary)
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H4 => Style::default()
                .fg(theme.preview_accent_warning)
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H5 => Style::default()
                .fg(theme.preview_accent_secondary)
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H6 => Style::default()
                .fg(theme.preview_fg)
                .add_modifier(Modifier::BOLD),
        },
    }
}

fn current_style(stack: &[Style]) -> Style {
    stack.last().copied().unwrap_or_default()
}

/// Render a table with box borders, fitted to the pane width.
///
/// Column widths start at their natural (widest cell) size; if the table is
/// wider than the pane, the widest columns shrink first and cell content
/// word-wraps inside its column, so borders always line up. Widths are
/// measured in terminal cells, so non-ASCII content stays aligned.
fn render_table(
    b: &mut Builder,
    rows: &[(Vec<Cell>, usize)],
    alignments: &[Alignment],
    theme: &Theme,
    width: usize,
) {
    let num_cols = rows.iter().map(|(r, _)| r.len()).max().unwrap_or(0);
    if num_cols == 0 {
        return;
    }

    let mut col_widths = vec![3usize; num_cols];
    for (row, _) in rows {
        for (i, cell) in row.iter().enumerate() {
            col_widths[i] = col_widths[i].max(spans_width(cell));
        }
    }
    // "│ " + cells joined by " │ " + " │"
    let chrome = 3 * num_cols + 1;
    let budget = width.saturating_sub(chrome).max(num_cols);
    while col_widths.iter().sum::<usize>() > budget {
        let widest = (0..num_cols)
            .max_by_key(|&i| col_widths[i])
            .expect("num_cols > 0");
        if col_widths[widest] <= 1 {
            break;
        }
        col_widths[widest] -= 1;
    }

    let border_style = Style::default().fg(theme.preview_muted_fg);
    let header_style = Style::default()
        .fg(theme.preview_fg)
        .add_modifier(Modifier::BOLD);
    let cell_style = Style::default().fg(theme.preview_muted_fg);

    let separator = |left: &str, mid: &str, right: &str| -> Vec<Span<'static>> {
        let mut s = left.to_string();
        for (i, &w) in col_widths.iter().enumerate() {
            s.push_str(&"\u{2500}".repeat(w + 2));
            s.push_str(if i + 1 < num_cols { mid } else { right });
        }
        vec![Span::styled(s, border_style)]
    };
    let pre = Fit::Pre { fill: None };

    let first_src = rows[0].1;
    let last_src = rows[rows.len() - 1].1;
    b.line(PLine::uniform(
        separator("\u{250C}", "\u{252C}", "\u{2510}"),
        first_src,
        pre,
        false,
    ));

    for (row_idx, (row, src)) in rows.iter().enumerate() {
        let base = if row_idx == 0 {
            header_style
        } else {
            cell_style
        };
        let wrapped = wrap_row_cells(row, &col_widths, base);
        let height = wrapped.iter().map(Vec::len).max().unwrap_or(1);

        for k in 0..height {
            let mut spans = vec![Span::styled("\u{2502} ", border_style)];
            for (c, cell_rows) in wrapped.iter().enumerate() {
                let (content, used) = cell_rows.get(k).cloned().unwrap_or_default();
                let slack = col_widths[c].saturating_sub(used);
                let (left, right) = match alignments.get(c).copied().unwrap_or(Alignment::None) {
                    Alignment::Right => (slack, 0),
                    Alignment::Center => (slack / 2, slack - slack / 2),
                    _ => (0, slack),
                };
                if left > 0 {
                    spans.push(Span::raw(" ".repeat(left)));
                }
                spans.extend(content);
                if right > 0 {
                    spans.push(Span::raw(" ".repeat(right)));
                }
                let sep = if c + 1 < num_cols {
                    " \u{2502} "
                } else {
                    " \u{2502}"
                };
                spans.push(Span::styled(sep, border_style));
            }
            b.line(PLine::uniform(spans, *src, pre, k == 0));
        }

        if row_idx == 0 {
            // The delimiter row (|---|) sits on the line after the header.
            b.line(PLine::uniform(
                separator("\u{251C}", "\u{253C}", "\u{2524}"),
                src + 1,
                pre,
                true,
            ));
        }
    }

    b.line(PLine::uniform(
        separator("\u{2514}", "\u{2534}", "\u{2518}"),
        last_src,
        pre,
        false,
    ));
}

/// Word-wrap each cell of a table row to its column width, patching `base`
/// under the cell's inline styles. Returns per-column rows of (spans, width).
fn wrap_row_cells(
    row: &[Cell],
    col_widths: &[usize],
    base: Style,
) -> Vec<Vec<(Vec<Span<'static>>, usize)>> {
    col_widths
        .iter()
        .enumerate()
        .map(|(c, &w)| {
            let cell: Vec<Span<'static>> = row
                .get(c)
                .map(|cell| {
                    cell.iter()
                        .map(|s| Span::styled(s.content.clone(), base.patch(s.style)))
                        .collect()
                })
                .unwrap_or_default();
            let srcs = vec![NO_SRC; cell.len()];
            wrap_flow(&cell, &srcs, w, &[])
                .into_iter()
                .map(|r| {
                    let used = r.width;
                    (r.into_spans(), used)
                })
                .collect()
        })
        .collect()
}

/// Render a table the way Warp's terminal UI does: bold header, one dim rule
/// under it, ` │ ` between columns, and no outer box. Columns shrink (widest
/// first) and wrap to fit; when even minimal columns can't fit, the table
/// switches to one `Header: value` line per cell.
fn render_table_clean(
    b: &mut Builder,
    rows: &[(Vec<Cell>, usize)],
    alignments: &[Alignment],
    theme: &Theme,
    width: usize,
) {
    let num_cols = rows.iter().map(|(r, _)| r.len()).max().unwrap_or(0);
    if num_cols == 0 {
        return;
    }
    let chrome = 3 * (num_cols - 1);
    let budget = width.saturating_sub(chrome);
    if budget < MIN_TABLE_COLUMN_WIDTH * num_cols {
        render_table_records(b, rows, theme);
        return;
    }

    let mut col_widths = vec![1usize; num_cols];
    for (row, _) in rows {
        for (i, cell) in row.iter().enumerate() {
            col_widths[i] = col_widths[i].max(spans_width(cell));
        }
    }
    while col_widths.iter().sum::<usize>() > budget {
        let widest = (0..num_cols)
            .max_by_key(|&i| col_widths[i])
            .expect("num_cols > 0");
        if col_widths[widest] <= MIN_TABLE_COLUMN_WIDTH {
            break;
        }
        col_widths[widest] -= 1;
    }

    let rule_style = Style::default().fg(theme.preview_muted_fg);
    let header_style = Style::default()
        .fg(theme.preview_fg)
        .add_modifier(Modifier::BOLD);
    let cell_style = Style::default().fg(theme.preview_fg);
    let pre = Fit::Pre { fill: None };

    for (row_idx, (row, src)) in rows.iter().enumerate() {
        let base = if row_idx == 0 {
            header_style
        } else {
            cell_style
        };
        let wrapped = wrap_row_cells(row, &col_widths, base);
        let height = wrapped.iter().map(Vec::len).max().unwrap_or(1);

        for k in 0..height {
            let mut spans = Vec::new();
            for (c, cell_rows) in wrapped.iter().enumerate() {
                let last = c + 1 == num_cols;
                let (content, used) = cell_rows.get(k).cloned().unwrap_or_default();
                let slack = col_widths[c].saturating_sub(used);
                let (left, right) = match alignments.get(c).copied().unwrap_or(Alignment::None) {
                    Alignment::Right => (slack, 0),
                    Alignment::Center => (slack / 2, slack - slack / 2),
                    _ => (0, slack),
                };
                if left > 0 {
                    spans.push(Span::raw(" ".repeat(left)));
                }
                spans.extend(content);
                if !last {
                    if right > 0 {
                        spans.push(Span::raw(" ".repeat(right)));
                    }
                    spans.push(Span::styled(" \u{2502} ", rule_style));
                }
            }
            b.line(PLine::uniform(spans, *src, pre, k == 0));
        }

        if row_idx == 0 {
            // The delimiter row (|---|) sits on the line after the header.
            let rule = col_widths
                .iter()
                .map(|&w| "\u{2500}".repeat(w))
                .collect::<Vec<_>>()
                .join("\u{2500}\u{253C}\u{2500}");
            b.line(PLine::uniform(
                vec![Span::styled(rule, rule_style)],
                src + 1,
                pre,
                true,
            ));
        }
    }
}

/// Narrow-pane table fallback: each body row becomes a record of
/// `Header: value` lines, with a blank row between records.
fn render_table_records(b: &mut Builder, rows: &[(Vec<Cell>, usize)], theme: &Theme) {
    let Some(((header, _), body)) = rows.split_first() else {
        return;
    };
    let label_style = Style::default()
        .fg(theme.preview_muted_fg)
        .add_modifier(Modifier::BOLD);
    let cell_style = Style::default().fg(theme.preview_fg);

    for (i, (row, src)) in body.iter().enumerate() {
        if i > 0 {
            b.line(PLine::empty(*src, Fit::Flow, false));
        }
        for (c, cell) in row.iter().enumerate() {
            let label: String = header
                .get(c)
                .map(|h| h.iter().map(|s| s.content.as_ref()).collect())
                .unwrap_or_default();
            let mut spans = vec![Span::styled(format!("{}: ", label), label_style)];
            spans.extend(
                cell.iter()
                    .map(|s| Span::styled(s.content.clone(), cell_style.patch(s.style))),
            );
            let mut line = PLine::uniform(spans, *src, Fit::Flow, c == 0);
            line.hang = vec![Span::raw("  ")];
            b.line(line);
        }
    }
}

/// Try to parse a callout marker from accumulated text.
/// Handles the fact that pulldown-cmark splits [!note] into "[", "!note", "]".
fn parse_callout_marker(buf: &str) -> Option<String> {
    let trimmed = buf.trim();
    if trimmed.starts_with("[!") && trimmed.contains(']') {
        let start = 2;
        if let Some(end) = trimmed.find(']') {
            let callout_type = trimmed[start..end].to_lowercase();
            if !callout_type.is_empty() {
                return Some(callout_type);
            }
        }
    }
    None
}

/// Pre-process Obsidian-specific syntax into standard markdown.
fn preprocess_obsidian(markdown: &str) -> String {
    let mut result = String::with_capacity(markdown.len());
    let chars: Vec<char> = markdown.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Detect wikilinks: [[target]] or [[target|display]]
        if i + 1 < len && chars[i] == '[' && chars[i + 1] == '[' {
            let is_embed = i > 0 && chars[i - 1] == '!';
            let start = i + 2;
            if let Some(end) = find_closing_brackets(&chars, start) {
                let inner: String = chars[start..end].iter().collect();
                if is_embed {
                    result.pop();
                    result.push_str(&format!("[{}]({})", inner, inner));
                } else if let Some(pipe_pos) = inner.find('|') {
                    let target = &inner[..pipe_pos];
                    let display = &inner[pipe_pos + 1..];
                    result.push_str(&format!("[{}]({})", display, target));
                } else {
                    result.push_str(&format!("[{}]({})", inner, inner));
                }
                i = end + 2;
                continue;
            }
        }

        // Detect Obsidian tags: #tag
        if chars[i] == '#'
            && (i == 0 || chars[i - 1].is_whitespace())
            && i + 1 < len
            && chars[i + 1].is_alphanumeric()
        {
            let tag_start = i + 1;
            let mut tag_end = tag_start;
            while tag_end < len
                && (chars[tag_end].is_alphanumeric()
                    || chars[tag_end] == '_'
                    || chars[tag_end] == '-'
                    || chars[tag_end] == '/')
            {
                tag_end += 1;
            }
            let tag: String = chars[tag_start..tag_end].iter().collect();
            result.push_str(&format!("`#{}`", tag));
            i = tag_end;
            continue;
        }

        result.push(chars[i]);
        i += 1;
    }

    result
}

fn find_closing_brackets(chars: &[char], start: usize) -> Option<usize> {
    let mut i = start;
    while i + 1 < chars.len() {
        if chars[i] == ']' && chars[i + 1] == ']' {
            return Some(i);
        }
        if chars[i] == '\n' {
            return None;
        }
        i += 1;
    }
    None
}

/// Get icon and color for callout types (GitHub alerts + Obsidian callouts).
fn callout_style(callout_type: &str, theme: &Theme) -> (&'static str, Color) {
    match callout_type {
        "note" | "info" => ("\u{1f4dd}", theme.preview_accent_primary),
        "tip" | "hint" => ("\u{1f4a1}", theme.preview_accent_secondary),
        "important" => ("\u{2757}", theme.preview_accent_tertiary),
        "warning" | "caution" | "attention" => ("\u{26a0}", theme.preview_accent_warning),
        "danger" | "error" | "bug" => ("\u{274c}", theme.preview_accent_danger),
        "example" => ("\u{1f4cb}", theme.preview_accent_tertiary),
        "quote" | "cite" => ("\u{275d}", theme.preview_muted_fg),
        "abstract" | "summary" | "tldr" => ("\u{1f4c4}", theme.preview_link_fg),
        "todo" => ("\u{2610}", theme.preview_accent_primary),
        "success" | "check" | "done" => ("\u{2705}", theme.preview_accent_secondary),
        "question" | "help" | "faq" => ("\u{2753}", theme.preview_accent_warning),
        "failure" | "fail" | "missing" => ("\u{274c}", theme.preview_accent_danger),
        _ => ("\u{25b6}", theme.preview_muted_fg),
    }
}

#[cfg(test)]
mod tests {
    use super::render_markdown;
    use crate::config::RenderProfile;
    use crate::markdown::layout::text_width;
    use crate::markdown::RenderResult;
    use crate::theme::Theme;

    fn row_text(result: &RenderResult, row: usize) -> String {
        result.rows[row]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect()
    }

    fn find_row(result: &RenderResult, needle: &str) -> usize {
        (0..result.rows.len())
            .find(|&r| row_text(result, r).contains(needle))
            .unwrap_or_else(|| panic!("{needle:?} not rendered"))
    }

    fn source_line(markdown: &str, prefix: &str) -> usize {
        markdown
            .lines()
            .position(|l| l.starts_with(prefix))
            .unwrap_or_else(|| panic!("{prefix:?} not in source"))
    }

    const SAMPLE: &str = "\
# Intro Title

BookSmith takes a story — with “curly quotes”, café, 日本語, and emoji 🎉 — and turns it into an illustrated, publish-ready book.
A second source line of the same paragraph.

1. **Input a story** -- paste text, upload a file (TXT, PDF, DOCX), or describe a concept
2. **Export** -- download as:
   - Barnes & Noble Press PDF (their spec)
   - [ ] Amazon KDP PDF

## Core Design Principles

```rust
fn main() {
\tprintln!(\"a very long line of code that will certainly need to wrap in a narrow pane\");
}
```

| Name | Café | Right |
|------|:----:|------:|
| `inline` code | ü | 1 |
| a much longer cell that has to wrap when narrow | 日本 | 22 |

> [!warning]
> Careful: this callout body is long enough to wrap in narrow panes too.

> plain quote

---
### Done
";

    #[test]
    fn every_row_fits_the_width() {
        for (profile, width) in RenderProfile::all()
            .iter()
            .flat_map(|p| [12u16, 20, 30, 40, 57, 80, 120].map(|w| (*p, w)))
        {
            let result = render_markdown(SAMPLE, profile, &Theme::ocean(), width);
            for r in 0..result.rows.len() {
                let text = row_text(&result, r);
                assert!(
                    text_width(&text) <= width as usize,
                    "{profile:?} width {width} row {r} is {} cells: {text:?}",
                    text_width(&text)
                );
            }
        }
    }

    #[test]
    fn source_to_row_is_monotonic_and_in_bounds() {
        for width in [20u16, 50, 100] {
            let result = render_markdown(SAMPLE, RenderProfile::Github, &Theme::ocean(), width);
            assert_eq!(result.source_to_row.len(), SAMPLE.split('\n').count());
            assert!(result.source_to_row.windows(2).all(|w| w[0] <= w[1]));
            assert!(result.source_to_row.iter().all(|&r| r < result.rows.len()));
            assert_eq!(result.row_source.len(), result.rows.len());
        }
    }

    #[test]
    fn headings_list_items_and_paragraph_lines_map_to_their_rows() {
        let result = render_markdown(SAMPLE, RenderProfile::Github, &Theme::ocean(), 60);
        let cases = [
            ("## Core Design", "Core Design Principles"),
            ("# Intro", "INTRO TITLE"),
            ("2. **Export**", "Export"),
            ("   - Barnes", "Barnes & Noble"),
            ("   - [ ] Amazon", "Amazon KDP"),
            ("A second source line", "A second source"),
            ("### Done", "Done"),
        ];
        for (src_prefix, preview) in cases {
            let src = source_line(SAMPLE, src_prefix);
            assert_eq!(
                result.source_to_row[src],
                find_row(&result, preview),
                "source {src_prefix:?}"
            );
        }
    }

    #[test]
    fn each_code_line_maps_to_its_own_row() {
        let result = render_markdown(SAMPLE, RenderProfile::Github, &Theme::ocean(), 100);
        for (src_prefix, preview) in [
            ("fn main", "fn main"),
            ("\tprintln", "println!"),
            ("}", "}"),
        ] {
            let src = source_line(SAMPLE, src_prefix);
            assert_eq!(result.source_to_row[src], find_row(&result, preview));
        }
        // Tabs are expanded, never drawn raw.
        assert!(result
            .rows
            .iter()
            .all(|l| l.spans.iter().all(|s| !s.content.contains('\t'))));
    }

    #[test]
    fn code_block_rows_fill_the_full_width() {
        let theme = Theme::ocean();
        let result = render_markdown(SAMPLE, RenderProfile::Github, &theme, 40);
        let first = find_row(&result, "fn main");
        for r in first - 1..first + 4 {
            assert_eq!(text_width(&row_text(&result, r)), 40, "row {r}");
        }
    }

    #[test]
    fn table_rows_align_and_keep_inline_code_inside_cells() {
        for width in [30u16, 44, 100] {
            let result = render_markdown(SAMPLE, RenderProfile::Github, &Theme::ocean(), width);
            let top = find_row(&result, "\u{250C}");
            let bottom = find_row(&result, "\u{2514}");
            let table_width = text_width(&row_text(&result, top));
            for r in top..=bottom {
                let text = row_text(&result, r);
                assert_eq!(
                    text_width(&text),
                    table_width,
                    "width {width} row {r}: {text:?}"
                );
            }
            let code_row = find_row(&result, "inline");
            assert!(
                code_row > top && code_row < bottom,
                "inline code escaped the table"
            );
            let src = source_line(SAMPLE, "| `inline`");
            assert_eq!(result.source_to_row[src], code_row);
        }
    }

    #[test]
    fn wrapped_list_items_hang_under_their_text() {
        let result = render_markdown(SAMPLE, RenderProfile::Github, &Theme::ocean(), 30);
        let first = find_row(&result, "1. Input");
        assert!(row_text(&result, first + 1).starts_with("   "));
    }

    fn clean(width: u16) -> RenderResult {
        render_markdown(SAMPLE, RenderProfile::Clean, &Theme::ocean(), width)
    }

    #[test]
    fn clean_code_blocks_are_outlined_with_a_language_label() {
        let result = clean(60);
        let top = find_row(&result, "\u{256D}");
        let bottom = find_row(&result, "\u{2570}");
        assert!(
            row_text(&result, top).contains("rust"),
            "label in top border"
        );
        let box_width = text_width(&row_text(&result, top));
        for r in top..=bottom {
            let text = row_text(&result, r);
            assert_eq!(text_width(&text), box_width, "row {r}: {text:?}");
        }
        // No background fill: the box sits on the page background.
        let theme = Theme::ocean();
        assert!(result.rows[top + 1]
            .spans
            .iter()
            .all(|s| s.style.bg != Some(theme.preview_code_bg)));
        // Code lines still map one-to-one; the fence maps to the top edge.
        let fence = source_line(SAMPLE, "```rust");
        assert_eq!(result.source_to_row[fence], top);
        assert_eq!(
            result.source_to_row[fence + 1],
            find_row(&result, "fn main")
        );
    }

    #[test]
    fn clean_tables_are_borderless_and_fall_back_to_records_when_narrow() {
        let result = clean(60);
        assert!(result.rows.iter().all(|l| {
            let t: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
            !t.contains('\u{250C}') && !t.contains('\u{2514}')
        }));
        let header = find_row(&result, "Name");
        assert!(row_text(&result, header + 1).contains("\u{253C}"));
        assert_eq!(
            result.source_to_row[source_line(SAMPLE, "|------")],
            header + 1
        );

        let narrow = clean(12);
        let record = find_row(&narrow, "Name: ");
        assert!(row_text(&narrow, record).contains("inline"));
        assert!(narrow.rows.iter().all(|l| {
            let t: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
            !t.contains('\u{253C}')
        }));
    }

    #[test]
    fn clean_task_checkbox_replaces_bullet_and_completed_is_struck() {
        let md = "- [x] shipped\n- [ ] todo\n";
        let result = render_markdown(md, RenderProfile::Clean, &Theme::ocean(), 30);
        assert_eq!(row_text(&result, 0), "\u{2611} shipped");
        assert_eq!(row_text(&result, 1), "\u{2610} todo");
        let done = result.rows[0].spans.last().unwrap();
        assert!(done
            .style
            .add_modifier
            .contains(ratatui::style::Modifier::CROSSED_OUT));
        let open = result.rows[1].spans.last().unwrap();
        assert!(!open
            .style
            .add_modifier
            .contains(ratatui::style::Modifier::CROSSED_OUT));
    }

    #[test]
    fn clean_caps_the_reading_width_and_centers_the_column() {
        let result = clean(200);
        let margin = " ".repeat(50);
        for r in 0..result.rows.len() {
            let text = row_text(&result, r);
            if text.is_empty() {
                continue;
            }
            assert!(text.starts_with(&margin), "row {r} not centered: {text:?}");
            assert!(text_width(&text) <= 50 + 100, "row {r} too wide");
        }
        assert_eq!(result.width, 200);

        // Panes narrower than the cap keep a one-cell margin.
        let split = clean(60);
        let heading = find_row(&split, "Core Design Principles");
        assert!(row_text(&split, heading).starts_with(" Core"));
    }
}
