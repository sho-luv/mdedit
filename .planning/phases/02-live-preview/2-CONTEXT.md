# Phase 2: Live Preview - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Add side-by-side live rendered markdown preview to the existing terminal editor. Users see their markdown rendered in real-time as they type. Includes layout toggle (split/editor-only/preview-only), markdown syntax highlighting in the editor pane, and keybinding hints in the status bar. Scroll sync is NOT in this phase — that's Phase 3.

</domain>

<decisions>
## Implementation Decisions

### Preview rendering
- **D-01:** Use tui-markdown 0.3.7 for converting pulldown-cmark events to ratatui `Text` widgets. Wrap behind a `MarkdownRenderer` trait so it can be replaced/forked later.
- **D-02:** Use pulldown-cmark 0.13 with GFM extensions enabled (tables, task lists, strikethrough, footnotes).
- **D-03:** Preview renders: headings (h1-h6), bold, italic, strikethrough, code blocks with syntax highlighting, inline code, links (show URL in parentheses), ordered/unordered lists, blockquotes, tables, horizontal rules, task lists.
- **D-04:** Debounce preview rendering — don't re-parse on every keystroke. Use a 50-100ms idle timer after last edit. This prevents lag on large files.
- **D-05:** Preview pane is a scrollable widget. Initially scroll position is independent (scroll sync comes in Phase 3).

### Code block syntax highlighting
- **D-06:** Use syntect 5.3 for code block highlighting in the preview pane. Detect language from the code fence tag (```rust, ```python, etc.).
- **D-07:** Support at minimum: bash/shell, python, rust, javascript, typescript, json, go, yaml, toml, html, css, sql, c, cpp. Fall back to plain text for unknown languages.
- **D-08:** Use a terminal-compatible theme (e.g., base16-ocean or similar) that works in both 256-color and truecolor terminals.

### Editor syntax highlighting
- **D-09:** Add markdown-aware syntax highlighting to the editor pane using syntect with a markdown grammar.
- **D-10:** Highlight: headings (distinct color), bold/italic markers, code fences and inline code, link syntax, list markers. Keep it subtle — the editor should be readable, not a Christmas tree.

### Layout
- **D-11:** Default layout: 50/50 horizontal split. Editor left, preview right. Vertical divider between them (single character `│`).
- **D-12:** Toggle hotkey: `Ctrl+P` cycles through: split → editor-only → preview-only → split. Show current mode briefly in status bar.
- **D-13:** In editor-only mode: full-width editor (current Phase 1 behavior). In preview-only mode: full-width rendered preview (read-only, scrollable with arrow keys).
- **D-14:** All panes keyboard-accessible. In preview-only mode, arrow keys scroll the preview. Pressing any editing key (character input, etc.) switches back to split mode.

### Status bar updates
- **D-15:** Add keybinding hints to the right side of the status bar: `Ctrl+S Save | Ctrl+P Preview | Ctrl+Q Quit`
- **D-16:** When toggling layout, show mode name briefly: "Split View", "Editor Only", "Preview Only" as a timed status message (reuse existing StatusBar timed message system).

### Claude's Discretion
- Exact syntect theme selection and color mapping
- Preview scroll widget implementation details
- How to structure the MarkdownRenderer trait interface
- Debounce timer implementation (likely use `Instant::elapsed()` in the existing event loop)
- Whether to use a separate preview module or inline in app.rs

</decisions>

<specifics>
## Specific Ideas

- The split should feel like VS Code's markdown preview — editor on left, rendered on right, same height
- Preview should look clean — use terminal colors tastefully, not overwhelming
- Code blocks in preview should be visually distinct (background color or indent) with syntax-highlighted content
- The divider between panes should be subtle (dimmed `│` character)

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project context
- `.planning/PROJECT.md` — Core value, constraints, key decisions
- `.planning/REQUIREMENTS.md` — PREV-01 through PREV-06, LAYT-01, LAYT-02, LAYT-04, CHRM-02

### Research
- `.planning/research/STACK.md` — tui-markdown 0.3.7 (experimental), pulldown-cmark 0.13, syntect 5.3 recommendations
- `.planning/research/ARCHITECTURE.md` — Component architecture, dirty-flag pattern for preview updates
- `.planning/research/PITFALLS.md` — Debounced preview rendering, tui-markdown limitations

### Phase 1 context
- `.planning/phases/01-terminal-editor/1-CONTEXT.md` — Prior decisions (keybindings, editor appearance, etc.)
- `src/app.rs` — Existing App struct, event loop, render method, AppMode enum
- `src/editor.rs` — Editor wrapper, EditorAction enum, content() method for preview consumption
- `src/status_bar.rs` — StatusBar with timed messages (reuse for layout toggle feedback)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `Editor::content()` — returns full text as String, ready for markdown parsing
- `Editor::cursor_position()` — returns (row, col), useful for future scroll sync
- `StatusBar::set_message()` — timed 2-second messages, reuse for layout toggle feedback
- `AppMode` enum — extend with preview-only mode or use a separate `LayoutMode`
- Event loop with 50ms poll timeout — already suitable for debounced rendering

### Established Patterns
- Component architecture: Editor owns TextArea, App owns Editor + StatusBar
- Vertical Layout with `Constraint::Fill(1)` + `Constraint::Length(1)` for status bar
- Key routing via `match self.mode` in the event loop

### Integration Points
- `App::render()` currently renders editor + status bar. Split layout adds a preview pane alongside the editor.
- `App::handle_editing_key()` needs to route Ctrl+P to layout toggle
- New dependencies: pulldown-cmark, tui-markdown, syntect added to Cargo.toml

</code_context>

<deferred>
## Deferred Ideas

- Scroll sync (preview tracks editor cursor) — Phase 3 (LAYT-03)
- Adjustable split ratio — v2 (LAYT-06)
- Stacked layout (top/bottom split) — v2 (LAYT-05)
- WYSIWYG editing in preview mode — v2 (PREV-09)
- Selectable markdown flavors — v2 (PREV-07)

</deferred>

---

*Phase: 02-live-preview*
*Context gathered: 2026-03-21*
