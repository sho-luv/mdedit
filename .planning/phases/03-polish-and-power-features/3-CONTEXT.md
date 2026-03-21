# Phase 3: Polish and Power Features - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Add scroll sync (preview tracks editor cursor), text search (Ctrl+F with match highlighting and navigation), text selection (Shift+arrow keys), and indent/outdent (Tab/Shift+Tab). These are the final v1 features that make mdedit competitive for daily use. No new rendering features, no new layout modes.

</domain>

<decisions>
## Implementation Decisions

### Scroll sync (LAYT-03)
- **D-01:** Map editor cursor line to a proportional scroll position in the preview pane. Use ratio-based mapping: `preview_scroll = (cursor_line / total_source_lines) * total_preview_lines`. This avoids needing a per-line source-to-rendered mapping which tui-markdown doesn't expose.
- **D-02:** Scroll sync is active in split mode only. Editor-only and preview-only modes retain independent scroll.
- **D-03:** Sync on every cursor move (not just edits). When the user arrows through the document, the preview follows.
- **D-04:** Smooth the scroll position — don't jump to exact proportional line, center the target region in the preview viewport for comfortable reading.

### Search (EDIT-07)
- **D-05:** Ctrl+F enters search mode. A search prompt appears in the status bar area: `Search: ___` with blue background (matching the filename prompt style).
- **D-06:** Search is incremental — matches highlight as the user types. The editor scrolls to the first match from the current cursor position.
- **D-07:** Enter jumps to the next match. Shift+Enter jumps to the previous match. Esc exits search mode and returns cursor to where it was before search (if no match was selected) or leaves cursor at the current match.
- **D-08:** Matches are highlighted with a distinct background color (e.g., yellow background, dark text) in the editor pane. The current/active match has a different color (e.g., bright cyan background) to distinguish it from other matches.
- **D-09:** Search is case-insensitive by default. No regex support in v1 — plain text matching only.
- **D-10:** Match count shown in status bar: `Search: term [3/17]` (current match / total matches).
- **D-11:** Search operates on the editor pane only, not the preview. This keeps it simple and avoids rendered-vs-source position mapping issues.

### Text selection (EDIT-08)
- **D-12:** Shift+Arrow keys (up/down/left/right) create and extend a selection. Shift+Home/End selects to start/end of line. Ctrl+Shift+Left/Right selects word-by-word.
- **D-13:** Selection is highlighted with a distinct background color (e.g., blue/gray background) in the custom `render_highlighted()` path.
- **D-14:** Typing any character while a selection is active replaces the selection. Backspace/Delete removes the selection. This is standard text editor behavior.
- **D-15:** Leverage ratatui-textarea's built-in selection support if available (it has `start_selection()`, `cancel_selection()`, `copy()`). The custom render path must read the selection range from textarea and apply visual highlighting.
- **D-16:** No clipboard integration in v1 (that's PLAT-01, v2). Selection is for delete/overwrite operations only.

### Indent/outdent (EDIT-09)
- **D-17:** Tab inserts 2 spaces at the current cursor position (consistent with common markdown conventions).
- **D-18:** Shift+Tab removes up to 2 leading spaces from the current line (outdent).
- **D-19:** When a selection spans multiple lines, Tab indents all selected lines by 2 spaces. Shift+Tab outdents all selected lines.
- **D-20:** Tab key is intercepted before `input_without_shortcuts()` to prevent tui-textarea's default tab behavior.

### Claude's Discretion
- Exact highlight colors for search matches and selection
- Whether to use a separate AppMode::Search or handle search as a sub-state of Editing
- Implementation details of the proportional scroll mapping
- How to handle edge cases in selection (e.g., selection + undo interaction)

</decisions>

<specifics>
## Specific Ideas

- Search should feel like nano's Ctrl+W or VS Code's Ctrl+F — simple, fast, no modal complexity
- Scroll sync doesn't need to be pixel-perfect; approximate "same region" tracking is fine
- Selection should feel native — same behavior users expect from any text editor

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project context
- `.planning/PROJECT.md` — Core value, constraints, key decisions
- `.planning/REQUIREMENTS.md` — EDIT-07, EDIT-08, EDIT-09, LAYT-03

### Research
- `.planning/research/STACK.md` — ratatui-textarea 0.8 capabilities (selection, search built-ins)
- `.planning/research/ARCHITECTURE.md` — Component architecture, event routing pattern
- `.planning/research/PITFALLS.md` — tui-textarea limitations, custom render path implications

### Prior phase context
- `.planning/phases/01-terminal-editor/1-CONTEXT.md` — Keybinding decisions, nano-style approach
- `.planning/phases/02-live-preview/2-CONTEXT.md` — Layout decisions, debounce, preview rendering
- `src/app.rs` — Event loop, LayoutMode, AppMode, key routing, debounced preview
- `src/editor.rs` — Custom `render_highlighted()`, `handle_key()`, `update_scroll()`
- `src/preview.rs` — Preview scroll state, `scroll_up/down/reset`, `render()` with clamping
- `src/highlighter.rs` — MarkdownHighlighter with syntect, `highlight_range()`

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `Editor::render_highlighted()` — Custom render path that can be extended to show search highlights and selection highlights
- `Editor::cursor_position()` — Returns (row, col), directly usable for scroll sync mapping
- `Preview::scroll_offset` / `scroll_up/down` — Preview scroll state ready for sync control
- `Editor::handle_key()` — Explicit match arms pattern makes it easy to add Ctrl+F, Shift+arrows, Tab
- `StatusBar::set_message()` — Timed messages for feedback, but search prompt needs a persistent mode
- `AppMode` enum — Extensible for a Search mode
- `MarkdownHighlighter::highlight_range()` — Already does per-line highlighting; can layer search/selection highlights on top
- `input_without_shortcuts()` — Used for all text input; Tab must be intercepted before this

### Established Patterns
- Key routing via `match self.mode` in the event loop — add Search mode here
- Ctrl+key intercepted at App level before editor (Ctrl+P pattern) — reuse for Ctrl+F
- Custom render bypasses tui-textarea Widget — search highlights and selection highlights added in same path
- `content_dirty` flag + debounce — extend pattern for search match recalculation

### Integration Points
- `App::handle_editing_key()` — Add Ctrl+F interception, Shift+arrow routing, Tab/Shift+Tab interception
- `Editor::render_highlighted()` — Add selection range highlighting and search match highlighting
- `App::render()` — Scroll sync: after rendering editor, set preview scroll based on editor cursor
- `App::maybe_update_preview()` — Update scroll sync here alongside debounced re-render
- Status bar render in `App::render()` — Add Search mode rendering with prompt and match count

</code_context>

<deferred>
## Deferred Ideas

- Clipboard integration (Ctrl+C/V with OSC 52) — v2 (PLAT-01)
- Regex search — v2
- Search in preview pane — v2
- Replace (Ctrl+H) — v2
- Adjustable split ratio — v2 (LAYT-06)

</deferred>

---

*Phase: 03-polish-and-power-features*
*Context gathered: 2026-03-21*
