# Phase 2: Live Preview - Research

**Researched:** 2026-03-21
**Domain:** Markdown preview rendering, split-pane TUI layout, syntax highlighting (Rust/ratatui)
**Confidence:** HIGH

## Summary

Phase 2 transforms mdedit from a plain text editor into its defining feature: a side-by-side markdown editor with live rendered preview. The core work involves three major additions: (1) a markdown rendering pipeline using tui-markdown 0.3.7 (which internally uses pulldown-cmark + syntect + ansi-to-tui for code block highlighting), (2) a split-pane layout system with three modes (split/editor-only/preview-only), and (3) markdown syntax highlighting in the editor pane using syntect.

The existing codebase is well-structured for this addition. `App::render()` currently uses a simple vertical layout (editor + status bar). The split adds a horizontal subdivision of the editor area. `Editor::content()` already exposes the raw markdown string needed for preview rendering. `StatusBar::set_message()` provides the timed message system needed for layout toggle feedback. The event loop's 50ms poll timeout is already suitable for debounced preview updates.

**Primary recommendation:** Start with tui-markdown's `from_str()` for preview rendering (it handles code block syntax highlighting via its `highlight-code` feature by default), add the layout system and Ctrl+P toggle, then layer on editor-pane markdown highlighting with syntect. Wrap tui-markdown behind a `MarkdownRenderer` trait as CONTEXT.md specifies.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Use tui-markdown 0.3.7 for converting pulldown-cmark events to ratatui `Text` widgets. Wrap behind a `MarkdownRenderer` trait so it can be replaced/forked later.
- **D-02:** Use pulldown-cmark 0.13 with GFM extensions enabled (tables, task lists, strikethrough, footnotes).
- **D-03:** Preview renders: headings (h1-h6), bold, italic, strikethrough, code blocks with syntax highlighting, inline code, links (show URL in parentheses), ordered/unordered lists, blockquotes, tables, horizontal rules, task lists.
- **D-04:** Debounce preview rendering -- don't re-parse on every keystroke. Use a 50-100ms idle timer after last edit. This prevents lag on large files.
- **D-05:** Preview pane is a scrollable widget. Initially scroll position is independent (scroll sync comes in Phase 3).
- **D-06:** Use syntect 5.3 for code block highlighting in the preview pane. Detect language from the code fence tag.
- **D-07:** Support at minimum: bash/shell, python, rust, javascript, typescript, json, go, yaml, toml, html, css, sql, c, cpp. Fall back to plain text for unknown languages.
- **D-08:** Use a terminal-compatible theme (e.g., base16-ocean or similar) that works in both 256-color and truecolor terminals.
- **D-09:** Add markdown-aware syntax highlighting to the editor pane using syntect with a markdown grammar.
- **D-10:** Highlight: headings (distinct color), bold/italic markers, code fences and inline code, link syntax, list markers. Keep it subtle.
- **D-11:** Default layout: 50/50 horizontal split. Editor left, preview right. Vertical divider between them (single character).
- **D-12:** Toggle hotkey: Ctrl+P cycles through: split -> editor-only -> preview-only -> split. Show current mode briefly in status bar.
- **D-13:** In editor-only mode: full-width editor (current Phase 1 behavior). In preview-only mode: full-width rendered preview (read-only, scrollable with arrow keys).
- **D-14:** All panes keyboard-accessible. In preview-only mode, arrow keys scroll the preview. Pressing any editing key switches back to split mode.
- **D-15:** Add keybinding hints to the right side of the status bar: `Ctrl+S Save | Ctrl+P Preview | Ctrl+Q Quit`
- **D-16:** When toggling layout, show mode name briefly as a timed status message.

### Claude's Discretion
- Exact syntect theme selection and color mapping
- Preview scroll widget implementation details
- How to structure the MarkdownRenderer trait interface
- Debounce timer implementation (likely use `Instant::elapsed()` in the existing event loop)
- Whether to use a separate preview module or inline in app.rs

### Deferred Ideas (OUT OF SCOPE)
- Scroll sync (preview tracks editor cursor) -- Phase 3 (LAYT-03)
- Adjustable split ratio -- v2 (LAYT-06)
- Stacked layout (top/bottom split) -- v2 (LAYT-05)
- WYSIWYG editing in preview mode -- v2 (PREV-09)
- Selectable markdown flavors -- v2 (PREV-07)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PREV-01 | Preview pane renders markdown as formatted terminal output (headings, bold, italic, strikethrough) | tui-markdown 0.3.7 `from_str()` handles all inline formatting; StyleSheet trait controls heading/bold/italic styles |
| PREV-02 | Preview renders code blocks with syntax highlighting for common languages | tui-markdown's `highlight-code` feature (default-on) uses syntect + ansi-to-tui internally; supports all listed languages via syntect's bundled Sublime syntax definitions |
| PREV-03 | Preview renders links, lists, blockquotes, tables, horizontal rules, task lists | tui-markdown supports all of these; pulldown-cmark with GFM extensions provides the parser events |
| PREV-04 | Preview updates live with no perceptible lag (<100ms) | Debounced re-parsing with 50-100ms idle timer using `Instant::elapsed()` in the existing event loop |
| PREV-05 | Preview uses GitHub Flavored Markdown (GFM) | pulldown-cmark 0.13 with `Options::ENABLE_TABLES \| ENABLE_TASKLISTS \| ENABLE_STRIKETHROUGH \| ENABLE_FOOTNOTES` |
| PREV-06 | Editor pane has markdown-aware syntax highlighting | syntect 5.3 with bundled Markdown.sublime-syntax grammar, mapped to ratatui Spans via syntect-tui |
| LAYT-01 | Default layout is side-by-side (editor left, preview right) | ratatui `Layout::horizontal()` with `[Constraint::Percentage(50), Constraint::Percentage(50)]` |
| LAYT-02 | User can toggle between split, editor-only, and preview-only views | `LayoutMode` enum with Ctrl+P cycling; key routing in `App::handle_editing_key()` |
| LAYT-04 | All features are keyboard-accessible | Arrow keys scroll preview in preview-only mode; editing key press auto-returns to split mode |
| CHRM-02 | Status bar shows available keybinding hints | Right-aligned hints: `Ctrl+S Save \| Ctrl+P Preview \| Ctrl+Q Quit` in StatusBar::render() |
</phase_requirements>

## Standard Stack

### New Dependencies for Phase 2

| Library | Version | Purpose | Why Standard | Confidence |
|---------|---------|---------|--------------|------------|
| tui-markdown | 0.3.7 | Markdown-to-ratatui Text conversion | Maintained by joshka (ratatui core); uses pulldown-cmark + syntect internally; `highlight-code` feature gives code block highlighting for free | MEDIUM |
| pulldown-cmark | 0.13.2 | Markdown parser (GFM) | Standard Rust CommonMark parser; streaming/low-memory; GFM extensions; dependency of tui-markdown anyway | HIGH |
| syntect | 5.3.0 | Syntax highlighting (editor pane + code blocks) | Industry standard; used by bat, delta, Typst; pure-Rust regex backend; bundled Sublime syntax definitions | HIGH |
| syntect-tui | 3.0.6 | Convert syntect highlighted output to ratatui Spans | Purpose-built translation layer; avoids manual style conversion | HIGH |

### Existing Dependencies (no changes)

| Library | Version | Purpose |
|---------|---------|---------|
| ratatui | 0.30 | TUI framework |
| crossterm | 0.29 | Terminal backend |
| ratatui-textarea | 0.8 | Editor widget |
| clap | 4 | CLI args |
| anyhow | 1 | Error handling |
| unicode-width | 0.2 | Display width |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| tui-markdown | Custom pulldown-cmark renderer | Full control but 2-3x more code; start with tui-markdown, replace if limitations appear |
| syntect-tui | ansi-to-tui (manual pipe) | ansi-to-tui is what tui-markdown uses internally; syntect-tui is more direct for editor highlighting |
| syntect-tui | Manual syntect style -> ratatui style mapping | ~30 lines of boilerplate; syntect-tui handles edge cases |

**Installation:**
```bash
cargo add pulldown-cmark --features simd
cargo add tui-markdown
cargo add syntect --no-default-features --features default-fancy
cargo add syntect-tui
```

**Note on tui-markdown:** It depends on pulldown-cmark 0.13.0 and syntect 5.3.0 internally. Adding them as direct dependencies is fine for: (a) configuring pulldown-cmark options ourselves, (b) using syntect for editor-pane highlighting. Cargo will resolve compatible versions.

**Note on `highlight-code` feature:** tui-markdown's default features include `highlight-code`, which pulls in syntect + ansi-to-tui for code block highlighting in the preview. This means PREV-02 and D-06 are handled automatically by tui-markdown -- we do NOT need to manually integrate syntect for preview code blocks. Syntect is needed separately only for editor-pane markdown highlighting (PREV-06/D-09).

## Architecture Patterns

### Recommended Project Structure (Phase 2 additions)
```
src/
  app.rs            # Add LayoutMode, preview state, Ctrl+P handling, split render
  editor.rs         # Add Ctrl+P -> EditorAction::TogglePreview
  status_bar.rs     # Add keybinding hints on the right side
  preview.rs        # NEW: Preview component (scroll state, render cached Text)
  markdown/
    mod.rs          # NEW: MarkdownRenderer trait + TuiMarkdownRenderer impl
    renderer.rs     # NEW: Wraps tui-markdown::from_str_with_options()
  highlighter.rs    # NEW: Editor-pane syntax highlighting via syntect
```

### Pattern 1: LayoutMode Enum
**What:** Simple enum controlling how the main area is split.
**When to use:** Every render call and key event routing decision.
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    Split,        // Editor left (50%), preview right (50%)
    EditorOnly,   // Full-width editor (Phase 1 behavior)
    PreviewOnly,  // Full-width preview (scrollable, read-only)
}

impl LayoutMode {
    pub fn next(self) -> Self {
        match self {
            Self::Split => Self::EditorOnly,
            Self::EditorOnly => Self::PreviewOnly,
            Self::PreviewOnly => Self::Split,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Split => "Split View",
            Self::EditorOnly => "Editor Only",
            Self::PreviewOnly => "Preview Only",
        }
    }
}
```

### Pattern 2: MarkdownRenderer Trait
**What:** Abstraction over tui-markdown so it can be replaced later.
**When to use:** Preview component calls this instead of tui-markdown directly.
```rust
use ratatui::text::Text;

pub trait MarkdownRenderer {
    /// Render markdown source into styled ratatui Text.
    fn render<'a>(&self, markdown: &str) -> Text<'a>;
}

pub struct TuiMarkdownRenderer;

impl MarkdownRenderer for TuiMarkdownRenderer {
    fn render<'a>(&self, markdown: &str) -> Text<'a> {
        tui_markdown::from_str(markdown)
    }
}
```

### Pattern 3: Debounced Preview Update
**What:** Only re-render preview after a short idle period.
**When to use:** In the App tick/render cycle when content is dirty.
```rust
use std::time::Instant;

struct App {
    // ...existing fields...
    layout_mode: LayoutMode,
    preview_text: Text<'static>,
    content_dirty: bool,
    last_edit_time: Option<Instant>,
}

// In the event loop, after handling keys:
fn maybe_update_preview(&mut self) {
    if self.content_dirty {
        if let Some(last_edit) = self.last_edit_time {
            if last_edit.elapsed() >= Duration::from_millis(80) {
                let content = self.editor.content();
                self.preview_text = self.renderer.render(&content);
                self.content_dirty = false;
                self.last_edit_time = None;
            }
        }
    }
}
```

### Pattern 4: Scrollable Preview with Paragraph
**What:** Preview pane renders as a scrollable `Paragraph` widget.
**When to use:** Preview rendering in split and preview-only modes.
```rust
// In render method:
let preview_widget = Paragraph::new(self.preview_text.clone())
    .scroll((self.preview_scroll_offset, 0))
    .block(Block::default()); // No borders -- divider is separate

frame.render_widget(preview_widget, preview_area);
```

### Pattern 5: Split Layout with Divider
**What:** Horizontal split with a thin vertical divider character.
**When to use:** Rendering in Split mode.
```rust
fn render_split(&self, frame: &mut Frame, body_area: Rect) {
    // Reserve 1 column for divider
    let chunks = Layout::horizontal([
        Constraint::Percentage(50),
        Constraint::Length(1),       // Divider
        Constraint::Percentage(50),
    ]).split(body_area);

    // Render editor in chunks[0]
    frame.render_widget(self.editor.widget(), chunks[0]);

    // Render divider
    let divider = Block::default()
        .style(Style::default().fg(Color::DarkGray));
    // Or render a column of '|' characters
    let divider_text: Vec<Line> = (0..chunks[1].height)
        .map(|_| Line::from(Span::styled("|", Style::default().fg(Color::DarkGray))))
        .collect();
    frame.render_widget(Paragraph::new(divider_text), chunks[1]);

    // Render preview in chunks[2]
    let preview = Paragraph::new(self.preview_text.clone())
        .scroll((self.preview_scroll_offset, 0));
    frame.render_widget(preview, chunks[2]);
}
```

### Pattern 6: Key Routing by LayoutMode
**What:** Different key behavior depending on current layout mode.
**When to use:** Event handling in App.
```rust
fn handle_editing_key(&mut self, key: KeyEvent) {
    // Global hotkeys first (work in all modes)
    if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('p') {
        self.layout_mode = self.layout_mode.next();
        self.status_bar.set_message(self.layout_mode.label());
        return;
    }

    match self.layout_mode {
        LayoutMode::PreviewOnly => {
            // Arrow keys scroll preview
            match key.code {
                KeyCode::Up => self.preview_scroll_offset = self.preview_scroll_offset.saturating_sub(1),
                KeyCode::Down => self.preview_scroll_offset += 1,
                KeyCode::PageUp => self.preview_scroll_offset = self.preview_scroll_offset.saturating_sub(20),
                KeyCode::PageDown => self.preview_scroll_offset += 20,
                // Any other key -> switch back to split
                _ => {
                    self.layout_mode = LayoutMode::Split;
                    // Forward key to editor
                    self.forward_to_editor(key);
                }
            }
        }
        LayoutMode::Split | LayoutMode::EditorOnly => {
            // Forward to editor (existing behavior)
            self.forward_to_editor(key);
        }
    }
}
```

### Anti-Patterns to Avoid
- **Calling tui-markdown directly in render():** Always go through the MarkdownRenderer trait. The planner must create the trait abstraction as a prerequisite task.
- **Re-parsing on every frame:** Use the dirty flag + debounce pattern. The 50ms poll timeout in the existing event loop means we check elapsed time every 50ms.
- **Cloning Text every frame:** `Text` contains heap-allocated `Vec<Line<Vec<Span>>>`. Cache it in App state; only regenerate when content changes.
- **Hardcoding preview scroll bounds:** The preview total height depends on content + wrapping. Use `preview_text.lines.len()` for upper bound, but account for word-wrapping making it taller than line count.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Markdown-to-TUI conversion | Custom pulldown-cmark event walker | tui-markdown 0.3.7 `from_str()` | Handles headings, lists, tables, code blocks, inline styles; fork later if needed |
| Code block syntax highlighting | Custom tokenizer | tui-markdown's `highlight-code` feature (uses syntect internally) | Correct language detection, theme support, 50+ languages built-in |
| syntect -> ratatui style conversion | Manual `syntect::highlighting::Style` to `ratatui::style::Style` mapping | syntect-tui 3.0.6 `into_span()` | Handles color space conversion, edge cases |
| Markdown parsing | Regex-based parser | pulldown-cmark 0.13.2 | CommonMark spec compliance, GFM extensions, streaming API |
| Scroll widget | Custom viewport tracking | `Paragraph::new(text).scroll((offset, 0))` | Built into ratatui, handles wrapping correctly |

**Key insight:** tui-markdown with `highlight-code` feature gives us PREV-01, PREV-02, PREV-03, and PREV-05 with essentially one function call. The main engineering work is in the layout system, key routing, editor highlighting, and debounced update loop -- not in markdown rendering.

## Common Pitfalls

### Pitfall 1: tui-markdown Text Widget Sizing
**What goes wrong:** `tui-markdown::from_str()` returns a `Text` value. When rendered via `Paragraph`, the text may be wider than the preview pane, causing horizontal overflow or ugly wrapping.
**Why it happens:** tui-markdown does not know the target width when generating `Text`. Tables and code blocks may produce long lines.
**How to avoid:** Use `Paragraph::new(text).wrap(Wrap { trim: false })` to enable word-wrapping in the preview pane. This prevents horizontal overflow. For code blocks, wrapping may be undesirable -- this is a known limitation to accept for v1.
**Warning signs:** Code blocks or tables extending beyond the preview pane boundary.

### Pitfall 2: Ctrl+P Conflict with tui-textarea
**What goes wrong:** tui-textarea's default Emacs keybindings include Ctrl+P (move cursor up). If the key is passed to `input_without_shortcuts()`, it may be consumed by the textarea.
**Why it happens:** The existing code uses `input_without_shortcuts()` which avoids Emacs shortcuts, so Ctrl+P should NOT be consumed. But this must be verified.
**How to avoid:** Intercept Ctrl+P in `App::handle_editing_key()` BEFORE passing to `Editor::handle_key()`. The current code structure already does this for Ctrl+S and Ctrl+Q at the Editor level, but Ctrl+P should be intercepted at the App level since it's a layout concern, not an editor concern.
**Warning signs:** Pressing Ctrl+P moves cursor up instead of toggling layout.

### Pitfall 3: Preview Flicker During Debounce Window
**What goes wrong:** During the 50-100ms debounce window, the preview shows stale content. If the debounce is too long, users notice the preview is "behind" their typing.
**Why it happens:** Debounce trades immediacy for performance. Too aggressive = lag visible. Too conservative = still re-parsing too often.
**How to avoid:** Use 80ms as the sweet spot. Also, only debounce the PARSE step, not the RENDER step. The cached `Text` is always rendered immediately; only the re-parsing is debounced. This means the preview is never blank -- it just shows slightly stale content during fast typing.
**Warning signs:** Users noticing preview lag; benchmark with the event loop's 50ms poll to ensure total cycle time stays under 100ms.

### Pitfall 4: syntect Lazy Loading for Startup Time
**What goes wrong:** Loading all syntect syntax definitions eagerly at startup adds 20-50ms to startup time.
**Why it happens:** syntect bundles ~200 syntax definitions. `SyntaxSet::load_defaults_newlines()` loads them all.
**How to avoid:** Use `SyntaxSet::load_defaults_newlines()` -- it IS already lazy (syntect uses lazy_static internally). The initial load is on first highlight call, not at import. But do NOT call `find_syntax_by_extension()` in a hot loop -- cache the `SyntaxReference` for the markdown grammar.
**Warning signs:** First render taking noticeably longer than subsequent renders.

### Pitfall 5: Editor Highlighting Interfering with tui-textarea
**What goes wrong:** tui-textarea renders its own content. Adding syntect-based highlighting requires either replacing its renderer or post-processing its output.
**Why it happens:** tui-textarea's `TextArea` widget renders directly; there's no hook to inject custom span styles.
**How to avoid:** tui-textarea has a `set_style()` method for the overall style, and line-level styling can be done via `set_line_number_style()`. For per-line syntax highlighting, use tui-textarea's search highlight mechanism or, more likely, we need to use `TextArea::set_search_style()` creatively. The actual approach: tui-textarea does NOT support per-span highlighting natively. The solution is to NOT use tui-textarea's built-in rendering for the highlighted case. Instead, read the lines, apply syntect highlighting to produce styled `Line` values, and render them manually in a `Paragraph` alongside tui-textarea's cursor management. This is the hardest part of Phase 2.
**Warning signs:** Inability to colorize individual spans within editor lines.

### Pitfall 6: Preview Scroll Offset Overflow
**What goes wrong:** Setting `preview_scroll_offset` higher than the total content height causes the preview to show blank space or panic.
**Why it happens:** `Paragraph::scroll()` takes a `u16` offset. If content changes (gets shorter) while scroll is high, offset is invalid.
**How to avoid:** Clamp scroll offset to `max(0, total_lines - visible_height)` before rendering. Recalculate on every render.
**Warning signs:** Blank preview pane after deleting large sections of content.

## Code Examples

### tui-markdown Basic Usage
```rust
// Source: docs.rs/tui-markdown/0.3.7
use tui_markdown::from_str;

let markdown = "# Hello\n\nThis is **bold** and *italic*.\n\n```rust\nfn main() {}\n```";
let text: ratatui::text::Text = from_str(markdown);
// text is ready to render via Paragraph::new(text)
```

### tui-markdown with Custom Styles
```rust
// Source: docs.rs/tui-markdown/0.3.7
use tui_markdown::{from_str_with_options, Options, DefaultStyleSheet};

let options = Options::default(); // Uses DefaultStyleSheet
let text = from_str_with_options(markdown, options);
```

### pulldown-cmark GFM Options
```rust
// Source: docs.rs/pulldown-cmark/0.13.2
use pulldown_cmark::{Options, Parser};

let mut options = Options::empty();
options.insert(Options::ENABLE_TABLES);
options.insert(Options::ENABLE_TASKLISTS);
options.insert(Options::ENABLE_STRIKETHROUGH);
options.insert(Options::ENABLE_FOOTNOTES);

let parser = Parser::new_ext(markdown_input, options);
// parser is an iterator of pulldown_cmark::Event
```

### syntect Editor Highlighting with syntect-tui
```rust
// Source: github.com/chanq-io/syntect-tui, lib.rs/crates/syntect-tui
use syntect::parsing::SyntaxSet;
use syntect::highlighting::{ThemeSet, Theme};
use syntect::easy::HighlightLines;
use syntect_tui::into_span;

let ss = SyntaxSet::load_defaults_newlines();
let ts = ThemeSet::load_defaults();
let theme = &ts.themes["base16-ocean.dark"];
let syntax = ss.find_syntax_by_extension("md").unwrap();

let mut h = HighlightLines::new(syntax, theme);
let line = "# Hello **world**";
let ranges = h.highlight_line(line, &ss).unwrap();

// Convert to ratatui Spans
let spans: Vec<ratatui::text::Span> = ranges
    .into_iter()
    .map(|(style, text)| into_span((style, text)))
    .collect();
let ratatui_line = ratatui::text::Line::from(spans);
```

### Paragraph with Scroll
```rust
// Source: docs.rs/ratatui/0.30/ratatui/widgets/struct.Paragraph.html
use ratatui::widgets::{Paragraph, Wrap};

let preview = Paragraph::new(cached_text.clone())
    .scroll((scroll_offset, 0))  // (y, x) offset
    .wrap(Wrap { trim: false });

frame.render_widget(preview, preview_area);
```

### Layout Split with Divider
```rust
// Source: ratatui layout docs
use ratatui::layout::{Layout, Constraint, Direction};

let body_chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
        Constraint::Percentage(50),
        Constraint::Length(1),       // Divider column
        Constraint::Percentage(50),
    ])
    .split(body_area);
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| tui-markdown without highlight-code | tui-markdown 0.3.7 with highlight-code (default) | 0.3.x | Code blocks get syntax highlighting automatically |
| Manual syntect-to-ratatui conversion | syntect-tui 3.0.6 | 2024 | Clean `into_span()` API eliminates boilerplate |
| pulldown-cmark 0.12 | pulldown-cmark 0.13.2 | 2025 | Improved GFM compliance, better table handling |
| ratatui Layout::default().constraints() | Layout::horizontal() / Layout::vertical() shorthand | ratatui 0.28+ | Cleaner constraint API |

## Open Questions

1. **Editor-pane syntax highlighting approach**
   - What we know: tui-textarea does NOT expose per-span styling hooks. It renders its own content internally.
   - What's unclear: Whether we can overlay syntect-highlighted spans on tui-textarea's output, or if we need a different approach.
   - Recommendation: Two options: (a) Accept no editor highlighting for the initial implementation and add it as a follow-up within Phase 2, or (b) Render editor lines manually using syntect + Paragraph while keeping tui-textarea only for input handling (cursor, undo/redo, scrolling) but not rendering. Option (b) is complex but achieves D-09. The planner should sequence editor highlighting as the LAST task in Phase 2 so the split layout and preview work first.

2. **tui-markdown rendering quality**
   - What we know: It is labeled "experimental PoC." It supports the major markdown elements we need.
   - What's unclear: How well it handles edge cases (deeply nested blockquotes, complex tables, mixed inline formatting).
   - Recommendation: Implement with tui-markdown first, create a test markdown document covering all required elements, and note any rendering issues. The MarkdownRenderer trait ensures we can swap implementations later.

3. **Preview scroll bounds**
   - What we know: `Paragraph::scroll((y, 0))` scrolls by lines. `Text::lines.len()` gives the line count.
   - What's unclear: Whether wrapped lines count differently for scroll offset purposes (they do -- scroll offset is in rendered lines, not source lines).
   - Recommendation: Implement simple line-based scrolling first. If wrapping causes issues, clamp offset to `content_height.saturating_sub(visible_height)`.

## Sources

### Primary (HIGH confidence)
- tui-markdown 0.3.7 docs.rs -- API: `from_str()`, `from_str_with_options()`, `Options`, `StyleSheet` trait, `highlight-code` feature
- pulldown-cmark 0.13.2 crates.io -- Current version verified
- syntect 5.3.0 crates.io -- Current version verified
- syntect-tui 3.0.6 crates.io -- `into_span()` for syntect-to-ratatui conversion
- ratatui Paragraph widget docs -- `scroll((y, x))` method, `Wrap` configuration
- tui-markdown Cargo.toml (GitHub) -- Confirmed dependencies: pulldown-cmark 0.13.0, syntect 5.3.0, ansi-to-tui 8.0.1

### Secondary (MEDIUM confidence)
- syntect-tui GitHub -- Usage patterns for syntax highlighting in ratatui apps
- tui-markdown StyleSheet trait docs.rs -- 6 required methods: heading, code, link, blockquote, heading_meta, metadata_block

### Tertiary (LOW confidence)
- Editor-pane highlighting approach -- No established pattern found for syntect + tui-textarea integration; this is novel work requiring experimentation

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all crate versions verified on crates.io, tui-markdown internals confirmed via Cargo.toml
- Architecture (layout, preview, debounce): HIGH - standard ratatui patterns with Layout, Paragraph, scroll
- Architecture (editor highlighting): LOW - no established pattern for syntect + tui-textarea; requires experimentation
- Pitfalls: HIGH - well-documented in prior PITFALLS.md research and consistent with ratatui ecosystem patterns

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable ecosystem, no major releases expected)
