use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, ThemeSet};
use syntect::parsing::SyntaxSet;

use crate::config::RenderProfile;
use super::{MarkdownRenderer, RenderResult};

pub struct TuiMarkdownRenderer;

impl MarkdownRenderer for TuiMarkdownRenderer {
    fn render(&self, markdown: &str, profile: RenderProfile) -> RenderResult {
        render_markdown(markdown, profile)
    }
}

/// Render markdown to styled ratatui Text using pulldown-cmark.
/// Strips markdown syntax and applies terminal-appropriate styling.
fn render_markdown(markdown: &str, profile: RenderProfile) -> RenderResult {
    let mut options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;

    match profile {
        RenderProfile::Github | RenderProfile::Obsidian => {
            options |= Options::ENABLE_TABLES;
        }
        RenderProfile::CommonMark => {}
    }

    // Pre-process for Obsidian-specific syntax
    let processed = if profile == RenderProfile::Obsidian {
        preprocess_obsidian(markdown)
    } else {
        markdown.to_string()
    };

    let parser = Parser::new_ext(&processed, options).into_offset_iter();

    // Build a byte-offset-to-source-line lookup from the source text
    let source_text = &processed;
    let line_starts: Vec<usize> = std::iter::once(0)
        .chain(source_text.bytes().enumerate()
            .filter(|(_, b)| *b == b'\n')
            .map(|(i, _)| i + 1))
        .collect();
    let byte_to_source_line = |byte_offset: usize| -> usize {
        match line_starts.binary_search(&byte_offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        }
    };

    // Tracks: for each source line, what preview line does it map to.
    // None means not yet mapped.
    let total_source_lines = source_text.lines().count().max(1);
    let mut source_to_preview_opt: Vec<Option<usize>> = vec![None; total_source_lines];
    let mut current_source_line: usize = 0;

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();
    let mut style_stack: Vec<Style> = vec![Style::default()];
    let mut list_depth: usize = 0;
    let mut ordered_index: Option<u64> = None;
    let mut in_code_block = false;
    let mut code_block_lines: Vec<String> = Vec::new();
    let mut code_block_lang: Option<String> = None;
    let mut heading_level: Option<HeadingLevel> = None;

    // Syntect setup for code block highlighting
    let syntax_set = SyntaxSet::load_defaults_newlines();
    let theme_set = ThemeSet::load_defaults();
    let syntect_theme = &theme_set.themes["base16-ocean.dark"];

    // Table state
    let mut _in_table = false;
    let mut _in_table_head = false;
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    let mut current_row: Vec<String> = Vec::new();
    let mut current_cell = String::new();
    let mut in_table_cell = false;
    let mut table_alignments: Vec<pulldown_cmark::Alignment> = Vec::new();

    // Blockquote / callout state
    let mut in_blockquote = false;
    let mut blockquote_callout: Option<String> = None;
    let mut blockquote_color: Color = Color::DarkGray;
    // Buffer to accumulate text fragments at the start of a blockquote
    // (pulldown-cmark splits [!note] into "[", "!note", "]")
    let mut bq_detect_buf: Option<String> = None;

    for (event, range) in parser {
        // Track which source line this event comes from
        let src_line = byte_to_source_line(range.start);
        if src_line != current_source_line {
            current_source_line = src_line;
        }
        // Record the preview line for this source line (first occurrence wins)
        if src_line < total_source_lines && source_to_preview_opt[src_line].is_none() {
            source_to_preview_opt[src_line] = Some(lines.len());
        }

        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    heading_level = Some(level);
                    let style = heading_style(level, profile);
                    style_stack.push(style);
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
                    in_code_block = true;
                    code_block_lines.clear();
                    code_block_lang = match kind {
                        CodeBlockKind::Fenced(lang) => {
                            let l = lang.trim().to_string();
                            if l.is_empty() { None } else { Some(l) }
                        }
                        CodeBlockKind::Indented => None,
                    };
                }
                Tag::List(start) => {
                    list_depth += 1;
                    ordered_index = start;
                }
                Tag::Item => {
                    let indent = "  ".repeat(list_depth.saturating_sub(1));
                    let bullet = if let Some(ref mut idx) = ordered_index {
                        let s = format!("{}{}. ", indent, idx);
                        *idx += 1;
                        s
                    } else {
                        format!("{}\u{2022} ", indent)
                    };
                    current_spans.push(Span::styled(bullet, Style::default().fg(Color::DarkGray)));
                }
                Tag::BlockQuote(_) => {
                    in_blockquote = true;
                    blockquote_callout = None;
                    blockquote_color = Color::DarkGray;
                    bq_detect_buf = None;
                    let base = current_style(&style_stack);
                    style_stack.push(base.fg(Color::Gray));
                }
                Tag::Link { dest_url, .. } => {
                    let base = current_style(&style_stack);
                    style_stack.push(base.fg(Color::Cyan).add_modifier(Modifier::UNDERLINED));
                    let _ = dest_url;
                }
                Tag::Table(alignments) => {
                    _in_table = true;
                    table_rows.clear();
                    table_alignments = alignments;
                }
                Tag::TableHead => {
                    _in_table_head = true;
                    current_row.clear();
                }
                Tag::TableRow => {
                    current_row.clear();
                }
                Tag::TableCell => {
                    in_table_cell = true;
                    current_cell.clear();
                }
                _ => {}
            },
            Event::End(tag_end) => match tag_end {
                TagEnd::Heading(_) => {
                    style_stack.pop();
                    if let Some(level) = heading_level.take() {
                        render_heading(&mut lines, &mut current_spans, level, profile);
                    } else {
                        flush_line(&mut lines, &mut current_spans);
                    }
                    lines.push(Line::from(""));
                }
                TagEnd::Paragraph => {
                    // If we were buffering for callout detection, flush as regular blockquote
                    if let Some(buf) = bq_detect_buf.take() {
                        if !buf.is_empty() {
                            current_spans.push(Span::styled(
                                "\u{2502} ".to_string(),
                                Style::default().fg(Color::DarkGray),
                            ));
                            let style = current_style(&style_stack);
                            current_spans.push(Span::styled(buf, style));
                        }
                    }
                    flush_line(&mut lines, &mut current_spans);
                    lines.push(Line::from(""));
                }
                TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                    style_stack.pop();
                }
                TagEnd::CodeBlock => {
                    in_code_block = false;
                    let bg = Color::Rgb(30, 30, 40);
                    let fallback_style = Style::default().fg(Color::Green).bg(bg);
                    let h_pad = 2;
                    // Use a fixed block width for consistent appearance across all code blocks.
                    // Lines longer than this will overflow; shorter ones get padded.
                    let block_width = 60;
                    let empty_line = " ".repeat(block_width);

                    // Top padding
                    lines.push(Line::from(Span::styled(empty_line.clone(), Style::default().bg(bg))));

                    // Try syntax highlighting if language is specified
                    let syntax = code_block_lang.as_ref().and_then(|lang| {
                        syntax_set.find_syntax_by_token(lang)
                    });

                    if let Some(syntax) = syntax {
                        let mut highlighter = HighlightLines::new(syntax, syntect_theme);
                        for code_line in &code_block_lines {
                            let line_with_nl = format!("{}\n", code_line);
                            let ranges = highlighter
                                .highlight_line(&line_with_nl, &syntax_set)
                                .unwrap_or_default();

                            let mut spans: Vec<Span<'static>> = Vec::new();
                            // Left padding
                            spans.push(Span::styled(" ".repeat(h_pad), Style::default().bg(bg)));
                            // Highlighted tokens
                            for (style, text) in &ranges {
                                let content = text.trim_end_matches('\n').to_string();
                                if content.is_empty() { continue; }
                                let mut rs = Style::default().bg(bg);
                                if style.foreground.a > 0 {
                                    rs = rs.fg(Color::Rgb(
                                        style.foreground.r,
                                        style.foreground.g,
                                        style.foreground.b,
                                    ));
                                }
                                if style.font_style.contains(FontStyle::BOLD) {
                                    rs = rs.add_modifier(Modifier::BOLD);
                                }
                                if style.font_style.contains(FontStyle::ITALIC) {
                                    rs = rs.add_modifier(Modifier::ITALIC);
                                }
                                spans.push(Span::styled(content, rs));
                            }
                            // Right padding — use known line length for consistency
                            let right = block_width.saturating_sub(h_pad + code_line.len());
                            if right > 0 {
                                spans.push(Span::styled(" ".repeat(right), Style::default().bg(bg)));
                            }
                            lines.push(Line::from(spans));
                        }
                    } else {
                        // No syntax found — fallback to plain green
                        for code_line in &code_block_lines {
                            let right = (block_width).saturating_sub(h_pad).saturating_sub(code_line.len());
                            let full = format!("{}{}{}", " ".repeat(h_pad), code_line, " ".repeat(right));
                            lines.push(Line::from(Span::styled(full, fallback_style)));
                        }
                    }

                    code_block_lines.clear();
                    code_block_lang = None;
                    lines.push(Line::from(""));
                }
                TagEnd::List(_) => {
                    list_depth = list_depth.saturating_sub(1);
                    if list_depth == 0 {
                        ordered_index = None;
                        lines.push(Line::from(""));
                    }
                }
                TagEnd::Item => {
                    flush_line(&mut lines, &mut current_spans);
                }
                TagEnd::BlockQuote(_) => {
                    style_stack.pop();
                    in_blockquote = false;
                    blockquote_callout = None;
                    bq_detect_buf = None;
                    flush_line(&mut lines, &mut current_spans);
                }
                TagEnd::Link => {
                    style_stack.pop();
                }
                TagEnd::TableCell => {
                    in_table_cell = false;
                    current_row.push(current_cell.clone());
                    current_cell.clear();
                }
                TagEnd::TableHead => {
                    _in_table_head = false;
                    table_rows.push(current_row.clone());
                    current_row.clear();
                }
                TagEnd::TableRow => {
                    table_rows.push(current_row.clone());
                    current_row.clear();
                }
                TagEnd::Table => {
                    _in_table = false;
                    render_table(&mut lines, &table_rows, &table_alignments);
                    table_rows.clear();
                    table_alignments.clear();
                    lines.push(Line::from(""));
                }
                _ => {}
            },
            Event::Text(text) => {
                if in_table_cell {
                    current_cell.push_str(&text);
                } else if in_code_block {
                    for line in text.split('\n') {
                        code_block_lines.push(line.to_string());
                    }
                } else if let Some(ref mut buf) = bq_detect_buf {
                    // Accumulating text at the start of a blockquote to detect callouts.
                    // pulldown-cmark splits [!note] into "[", "!note", "]" as separate events.
                    buf.push_str(&text);

                    // Check if we have a complete callout marker like [!note] or [!WARNING]
                    if let Some(callout) = parse_callout_marker(buf) {
                        blockquote_callout = Some(callout.clone());
                        let (icon, color) = callout_style(&callout);
                        blockquote_color = color;

                        // Render callout header: thick colored border + icon + title
                        current_spans.push(Span::styled(
                            "\u{2588} ".to_string(),
                            Style::default().fg(color),
                        ));
                        current_spans.push(Span::styled(
                            format!("{} {}", icon, callout.to_uppercase()),
                            Style::default().fg(color).add_modifier(Modifier::BOLD),
                        ));
                        flush_line(&mut lines, &mut current_spans);

                        // Any text after the ] marker
                        if let Some(close) = buf.find(']') {
                            let rest = buf[close + 1..].trim();
                            if !rest.is_empty() {
                                current_spans.push(Span::styled(
                                    "\u{2588} ".to_string(),
                                    Style::default().fg(color),
                                ));
                                let style = current_style(&style_stack);
                                current_spans.push(Span::styled(rest.to_string(), style));
                            }
                        }
                        bq_detect_buf = None;
                    } else if !buf.starts_with('[') || buf.len() > 30 {
                        // Not a callout — flush as regular blockquote text
                        let content = bq_detect_buf.take().unwrap_or_default();
                        current_spans.push(Span::styled(
                            "\u{2502} ".to_string(),
                            Style::default().fg(Color::DarkGray),
                        ));
                        let style = current_style(&style_stack);
                        current_spans.push(Span::styled(content, style));
                    }
                    // else: keep buffering (we have "[" or "[!" but no "]" yet)
                } else {
                    // Regular text or continuation of a callout blockquote
                    if in_blockquote {
                        if blockquote_callout.is_some() {
                            if current_spans.is_empty() {
                                current_spans.push(Span::styled(
                                    "\u{2588} ".to_string(),
                                    Style::default().fg(blockquote_color),
                                ));
                            }
                        } else if current_spans.is_empty() {
                            current_spans.push(Span::styled(
                                "\u{2502} ".to_string(),
                                Style::default().fg(Color::DarkGray),
                            ));
                        }
                    }
                    let style = current_style(&style_stack);
                    current_spans.push(Span::styled(text.to_string(), style));
                }
            }
            Event::Code(code) => {
                let style = Style::default()
                    .fg(Color::Yellow)
                    .bg(Color::Rgb(40, 40, 50));
                current_spans.push(Span::styled(format!(" {} ", code), style));
            }
            Event::SoftBreak => {
                if bq_detect_buf.is_some() {
                    // End of first line in blockquote — finalize detection
                    if let Some(buf) = bq_detect_buf.take() {
                        if let Some(callout) = parse_callout_marker(&buf) {
                            blockquote_callout = Some(callout.clone());
                            let (icon, color) = callout_style(&callout);
                            blockquote_color = color;

                            current_spans.push(Span::styled(
                                "\u{2588} ".to_string(),
                                Style::default().fg(color),
                            ));
                            current_spans.push(Span::styled(
                                format!("{} {}", icon, callout.to_uppercase()),
                                Style::default().fg(color).add_modifier(Modifier::BOLD),
                            ));
                            flush_line(&mut lines, &mut current_spans);
                        } else {
                            // Regular blockquote
                            current_spans.push(Span::styled(
                                "\u{2502} ".to_string(),
                                Style::default().fg(Color::DarkGray),
                            ));
                            let style = current_style(&style_stack);
                            current_spans.push(Span::styled(buf, style));
                            flush_line(&mut lines, &mut current_spans);
                        }
                    }
                } else {
                    if in_blockquote {
                        flush_line(&mut lines, &mut current_spans);
                    } else {
                        current_spans.push(Span::raw(" "));
                    }
                }
            }
            Event::HardBreak => {
                flush_line(&mut lines, &mut current_spans);
            }
            Event::Rule => {
                flush_line(&mut lines, &mut current_spans);
                lines.push(Line::from(Span::styled(
                    "\u{2500}".repeat(50),
                    Style::default().fg(Color::DarkGray),
                )));
                lines.push(Line::from(""));
            }
            Event::TaskListMarker(checked) => {
                let marker = if checked { "\u{2611} " } else { "\u{2610} " };
                current_spans.push(Span::styled(
                    marker.to_string(),
                    Style::default().fg(if checked { Color::Green } else { Color::Gray }),
                ));
            }
            _ => {}
        }
    }

    flush_line(&mut lines, &mut current_spans);

    // Convert Option<usize> to usize, filling gaps by inheriting from the previous line.
    let mut source_to_preview: Vec<usize> = vec![0; total_source_lines];
    let mut last_preview = 0;
    for i in 0..total_source_lines {
        if let Some(p) = source_to_preview_opt[i] {
            source_to_preview[i] = p;
            last_preview = p;
        } else {
            source_to_preview[i] = last_preview;
        }
    }

    RenderResult {
        text: Text::from(lines),
        source_to_preview,
    }
}

/// Render a heading with visual size differentiation.
fn render_heading(
    lines: &mut Vec<Line<'static>>,
    spans: &mut Vec<Span<'static>>,
    level: HeadingLevel,
    profile: RenderProfile,
) {
    let heading_spans: Vec<Span<'static>> = spans.drain(..).collect();
    let text_content: String = heading_spans.iter().map(|s| s.content.as_ref()).collect();

    match level {
        HeadingLevel::H1 => {
            lines.push(Line::from(""));
            let border = "\u{2550}".repeat(text_content.len().max(40));
            lines.push(Line::from(Span::styled(
                border.clone(),
                Style::default().fg(heading_accent(level, profile)),
            )));
            let mut upper_spans: Vec<Span<'static>> = Vec::new();
            for span in &heading_spans {
                upper_spans.push(Span::styled(
                    span.content.to_uppercase(),
                    span.style,
                ));
            }
            lines.push(Line::from(upper_spans));
            lines.push(Line::from(Span::styled(
                border,
                Style::default().fg(heading_accent(level, profile)),
            )));
        }
        HeadingLevel::H2 => {
            lines.push(Line::from(""));
            lines.push(Line::from(heading_spans));
            let underline = "\u{2500}".repeat(text_content.len().max(30));
            lines.push(Line::from(Span::styled(
                underline,
                Style::default().fg(heading_accent(level, profile)),
            )));
        }
        HeadingLevel::H3 => {
            lines.push(Line::from(""));
            let mut h3_spans = vec![Span::styled(
                "\u{25B6} ".to_string(),
                Style::default().fg(heading_accent(level, profile)),
            )];
            h3_spans.extend(heading_spans);
            lines.push(Line::from(h3_spans));
        }
        HeadingLevel::H4 => {
            let mut h4_spans = vec![Span::styled(
                "\u{25B8} ".to_string(),
                Style::default().fg(heading_accent(level, profile)),
            )];
            h4_spans.extend(heading_spans);
            lines.push(Line::from(h4_spans));
        }
        _ => {
            lines.push(Line::from(heading_spans));
        }
    }
}

/// Get the accent color for heading borders/decorations by profile.
fn heading_accent(level: HeadingLevel, profile: RenderProfile) -> Color {
    match profile {
        RenderProfile::Github => match level {
            HeadingLevel::H1 | HeadingLevel::H2 => Color::Rgb(88, 166, 255),
            _ => Color::Rgb(139, 148, 158),
        },
        RenderProfile::Obsidian => match level {
            HeadingLevel::H1 | HeadingLevel::H2 => Color::Rgb(168, 131, 255),
            HeadingLevel::H3 | HeadingLevel::H4 => Color::Rgb(126, 231, 135),
            _ => Color::DarkGray,
        },
        RenderProfile::CommonMark => Color::Cyan,
    }
}

/// Heading text style varies by profile.
fn heading_style(level: HeadingLevel, profile: RenderProfile) -> Style {
    match profile {
        RenderProfile::Github => match level {
            HeadingLevel::H1 | HeadingLevel::H2 | HeadingLevel::H3 => Style::default()
                .fg(Color::Rgb(230, 237, 243))
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H4 => Style::default()
                .fg(Color::Rgb(200, 210, 220))
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H5 => Style::default()
                .fg(Color::Rgb(139, 148, 158))
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H6 => Style::default()
                .fg(Color::Rgb(139, 148, 158)),
        },
        RenderProfile::Obsidian => match level {
            HeadingLevel::H1 | HeadingLevel::H2 => Style::default()
                .fg(Color::Rgb(168, 131, 255))
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H3 => Style::default()
                .fg(Color::Rgb(126, 231, 135))
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H4 => Style::default()
                .fg(Color::Rgb(255, 208, 96))
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H5 => Style::default()
                .fg(Color::Rgb(255, 135, 157))
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H6 => Style::default()
                .fg(Color::Rgb(139, 148, 158)),
        },
        RenderProfile::CommonMark => match level {
            HeadingLevel::H1 => Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            HeadingLevel::H2 => Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H3 => Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H4 => Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H5 => Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H6 => Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        },
    }
}

fn current_style(stack: &[Style]) -> Style {
    stack.last().copied().unwrap_or_default()
}

/// Render a table with borders and aligned columns.
fn render_table(
    lines: &mut Vec<Line<'static>>,
    rows: &[Vec<String>],
    alignments: &[pulldown_cmark::Alignment],
) {
    if rows.is_empty() {
        return;
    }

    // Calculate column widths
    let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut col_widths = vec![0usize; num_cols];
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < num_cols {
                col_widths[i] = col_widths[i].max(cell.len());
            }
        }
    }
    // Minimum column width of 3
    for w in col_widths.iter_mut() {
        *w = (*w).max(3);
    }

    let border_style = Style::default().fg(Color::DarkGray);
    let header_style = Style::default()
        .fg(Color::Rgb(230, 237, 243))
        .add_modifier(Modifier::BOLD);
    let cell_style = Style::default().fg(Color::Gray);

    // Build a separator line: ├───┼───┼───┤
    let build_separator = |left: &str, mid: &str, right: &str, fill: &str| -> Line<'static> {
        let mut spans = vec![Span::styled(left.to_string(), border_style)];
        for (i, &w) in col_widths.iter().enumerate() {
            spans.push(Span::styled(fill.repeat(w + 2), border_style));
            if i < num_cols - 1 {
                spans.push(Span::styled(mid.to_string(), border_style));
            }
        }
        spans.push(Span::styled(right.to_string(), border_style));
        Line::from(spans)
    };

    // Top border
    lines.push(build_separator("\u{250C}", "\u{252C}", "\u{2510}", "\u{2500}"));

    for (row_idx, row) in rows.iter().enumerate() {
        // Data row
        let mut spans = vec![Span::styled("\u{2502} ".to_string(), border_style)];
        for (col_idx, _) in col_widths.iter().enumerate() {
            let content = row.get(col_idx).map(|s| s.as_str()).unwrap_or("");
            let width = col_widths[col_idx];
            let align = alignments.get(col_idx).copied()
                .unwrap_or(pulldown_cmark::Alignment::None);

            let padded = match align {
                pulldown_cmark::Alignment::Right => format!("{:>width$}", content, width = width),
                pulldown_cmark::Alignment::Center => {
                    let total_pad = width.saturating_sub(content.len());
                    let left_pad = total_pad / 2;
                    let right_pad = total_pad - left_pad;
                    format!("{}{}{}", " ".repeat(left_pad), content, " ".repeat(right_pad))
                }
                _ => format!("{:<width$}", content, width = width),
            };

            let style = if row_idx == 0 { header_style } else { cell_style };
            spans.push(Span::styled(padded, style));

            if col_idx < num_cols - 1 {
                spans.push(Span::styled(" \u{2502} ".to_string(), border_style));
            }
        }
        spans.push(Span::styled(" \u{2502}".to_string(), border_style));
        lines.push(Line::from(spans));

        // Separator after header row
        if row_idx == 0 {
            lines.push(build_separator("\u{251C}", "\u{253C}", "\u{2524}", "\u{2500}"));
        }
    }

    // Bottom border
    lines.push(build_separator("\u{2514}", "\u{2534}", "\u{2518}", "\u{2500}"));
}

fn flush_line(lines: &mut Vec<Line<'static>>, spans: &mut Vec<Span<'static>>) {
    if !spans.is_empty() {
        lines.push(Line::from(spans.drain(..).collect::<Vec<_>>()));
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
            while tag_end < len && (chars[tag_end].is_alphanumeric() || chars[tag_end] == '_' || chars[tag_end] == '-' || chars[tag_end] == '/') {
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
fn callout_style(callout_type: &str) -> (&'static str, Color) {
    match callout_type {
        "note" | "info" => ("\u{1f4dd}", Color::Rgb(88, 166, 255)),
        "tip" | "hint" => ("\u{1f4a1}", Color::Rgb(126, 231, 135)),
        "important" => ("\u{2757}", Color::Rgb(168, 131, 255)),
        "warning" | "caution" | "attention" => ("\u{26a0}\u{fe0f}", Color::Rgb(255, 208, 96)),
        "danger" | "error" | "bug" => ("\u{274c}", Color::Rgb(248, 81, 73)),
        "example" => ("\u{1f4cb}", Color::Rgb(168, 131, 255)),
        "quote" | "cite" => ("\u{275d}", Color::Gray),
        "abstract" | "summary" | "tldr" => ("\u{1f4c4}", Color::Cyan),
        "todo" => ("\u{2610}", Color::Rgb(88, 166, 255)),
        "success" | "check" | "done" => ("\u{2705}", Color::Rgb(126, 231, 135)),
        "question" | "help" | "faq" => ("\u{2753}", Color::Rgb(255, 208, 96)),
        "failure" | "fail" | "missing" => ("\u{274c}", Color::Rgb(248, 81, 73)),
        _ => ("\u{25b6}", Color::Gray),
    }
}
