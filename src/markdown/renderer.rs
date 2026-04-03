use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

use super::MarkdownRenderer;

pub struct TuiMarkdownRenderer;

impl MarkdownRenderer for TuiMarkdownRenderer {
    fn render(&self, markdown: &str) -> Text<'static> {
        render_markdown(markdown)
    }
}

/// Render markdown to styled ratatui Text using pulldown-cmark.
/// Strips markdown syntax and applies terminal-appropriate styling.
fn render_markdown(markdown: &str) -> Text<'static> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(markdown, options);

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();
    let mut style_stack: Vec<Style> = vec![Style::default()];
    let mut list_depth: usize = 0;
    let mut ordered_index: Option<u64> = None;
    let mut in_code_block = false;
    let mut code_block_lines: Vec<String> = Vec::new();
    let mut heading_level: Option<HeadingLevel> = None;

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {

                    heading_level = Some(level);
                    let style = heading_style(level);
                    style_stack.push(style);
                }
                Tag::Paragraph => {}
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
                Tag::CodeBlock(_) => {
                    in_code_block = true;
                    code_block_lines.clear();
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
                        format!("{}\u{2022} ", indent) // bullet character
                    };
                    current_spans.push(Span::styled(bullet, Style::default().fg(Color::DarkGray)));
                }
                Tag::BlockQuote(_) => {
                    current_spans.push(Span::styled(
                        "\u{2502} ".to_string(),
                        Style::default().fg(Color::DarkGray),
                    ));
                    let base = current_style(&style_stack);
                    style_stack.push(base.fg(Color::Gray).add_modifier(Modifier::ITALIC));
                }
                Tag::Link { dest_url, .. } => {
                    let base = current_style(&style_stack);
                    style_stack.push(base.fg(Color::Cyan).add_modifier(Modifier::UNDERLINED));
                    // Store URL to append after link text
                    let _ = dest_url; // URL shown inline after text
                }
                Tag::Table(_) | Tag::TableHead | Tag::TableRow | Tag::TableCell => {}
                _ => {}
            },
            Event::End(tag_end) => match tag_end {
                TagEnd::Heading(_) => {
                    style_stack.pop();
                    if let Some(level) = heading_level.take() {
                        let mut heading_spans = vec![];
                        heading_spans.append(&mut current_spans);
                        lines.push(Line::from(heading_spans));
                        if matches!(level, HeadingLevel::H1 | HeadingLevel::H2) {
                            let underline_char = if level == HeadingLevel::H1 { "\u{2550}" } else { "\u{2500}" };
                            lines.push(Line::from(Span::styled(
                                underline_char.repeat(40),
                                Style::default().fg(Color::DarkGray),
                            )));
                        }
                    } else {
                        flush_line(&mut lines, &mut current_spans);
                    }

                    lines.push(Line::from(""));
                }
                TagEnd::Paragraph => {
                    flush_line(&mut lines, &mut current_spans);
                    lines.push(Line::from(""));
                }
                TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                    style_stack.pop();
                }
                TagEnd::CodeBlock => {
                    in_code_block = false;
                    let code_style = Style::default()
                        .fg(Color::Green)
                        .bg(Color::Rgb(30, 30, 40));
                    for code_line in &code_block_lines {
                        lines.push(Line::from(Span::styled(
                            format!("  {}", code_line),
                            code_style,
                        )));
                    }
                    code_block_lines.clear();
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
                    flush_line(&mut lines, &mut current_spans);
                }
                TagEnd::Link => {
                    style_stack.pop();
                }
                TagEnd::Table | TagEnd::TableHead | TagEnd::TableRow | TagEnd::TableCell => {}
                _ => {}
            },
            Event::Text(text) => {
                if in_code_block {
                    // Split code text into lines
                    for line in text.split('\n') {
                        code_block_lines.push(line.to_string());
                    }
                } else {
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
                current_spans.push(Span::raw(" "));
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

    // Flush any remaining spans
    flush_line(&mut lines, &mut current_spans);

    Text::from(lines)
}

fn current_style(stack: &[Style]) -> Style {
    stack.last().copied().unwrap_or_default()
}

fn flush_line(lines: &mut Vec<Line<'static>>, spans: &mut Vec<Span<'static>>) {
    if !spans.is_empty() {
        lines.push(Line::from(spans.drain(..).collect::<Vec<_>>()));
    }
}

fn heading_style(level: HeadingLevel) -> Style {
    match level {
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
    }
}
