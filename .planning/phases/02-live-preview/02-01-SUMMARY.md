---
phase: 02-live-preview
plan: 01
subsystem: ui
tags: [tui-markdown, pulldown-cmark, syntect, ratatui, split-layout, live-preview]

# Dependency graph
requires:
  - phase: 01-terminal-editor
    provides: "Editor component, status bar, file I/O, event loop"
provides:
  - "MarkdownRenderer trait with TuiMarkdownRenderer implementation"
  - "Preview component with scroll state management"
  - "LayoutMode enum (Split/EditorOnly/PreviewOnly) with Ctrl+P cycling"
  - "Debounced preview rendering (80ms idle timer)"
  - "Keybinding hints in status bar"
affects: [02-live-preview, 03-polish]

# Tech tracking
tech-stack:
  added: [pulldown-cmark 0.13, tui-markdown 0.3.7, syntect 5.3, syntect-tui 3.0]
  patterns: [MarkdownRenderer trait abstraction, debounced preview with dirty flag, layout mode enum]

key-files:
  created:
    - src/markdown/mod.rs
    - src/markdown/renderer.rs
    - src/preview.rs
  modified:
    - Cargo.toml
    - src/main.rs
    - src/app.rs
    - src/status_bar.rs

key-decisions:
  - "Owned Text conversion: tui-markdown returns borrowed Text, added text_to_owned() to convert to Text<'static> for caching"
  - "80ms debounce timer for preview updates (D-04 sweet spot per research)"
  - "Ctrl+P intercepted at App level before editor to avoid tui-textarea Emacs conflict"

patterns-established:
  - "MarkdownRenderer trait: abstracts tui-markdown so it can be replaced/forked later"
  - "Debounce pattern: content_dirty flag + last_edit_time Instant for lazy re-rendering"
  - "LayoutMode cycling: enum with next() method for clean state machine transitions"

requirements-completed: [PREV-01, PREV-02, PREV-03, PREV-04, PREV-05, LAYT-01, LAYT-02, LAYT-04, CHRM-02]

# Metrics
duration: 3min
completed: 2026-03-21
---

# Phase 2 Plan 1: Live Preview Summary

**Side-by-side markdown preview with tui-markdown rendering, Ctrl+P layout toggle, 80ms debounced updates, and scrollable preview pane**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-21T15:40:31Z
- **Completed:** 2026-03-21T15:43:44Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Live markdown preview rendering headings, bold, italic, strikethrough, code blocks with syntax highlighting, links, lists, blockquotes, tables, horizontal rules, and task lists via tui-markdown
- Split-pane layout (50/50 with dimmed vertical divider) with three modes: Split, EditorOnly, PreviewOnly
- Debounced preview updates (80ms idle timer) preventing lag during fast typing
- Scrollable preview pane with clamped scroll offset preventing blank screen
- Keybinding hints in status bar with narrow-terminal fallback

## Task Commits

Each task was committed atomically:

1. **Task 1: Add dependencies, create MarkdownRenderer trait and Preview component** - `555ef26` (feat)
2. **Task 2: Integrate split layout, Ctrl+P toggle, debounced preview, and keybinding hints** - `6520a33` (feat)

## Files Created/Modified
- `Cargo.toml` - Added pulldown-cmark, tui-markdown, syntect, syntect-tui dependencies
- `src/markdown/mod.rs` - MarkdownRenderer trait definition and module exports
- `src/markdown/renderer.rs` - TuiMarkdownRenderer wrapping tui-markdown with borrowed-to-owned Text conversion
- `src/preview.rs` - Preview component with scroll state and clamped Paragraph rendering
- `src/main.rs` - Added markdown and preview module declarations
- `src/app.rs` - LayoutMode enum, split rendering, Ctrl+P toggle, debounced preview, preview-only key routing
- `src/status_bar.rs` - Keybinding hints (Ctrl+S Save | Ctrl+P Preview | Ctrl+Q Quit) with narrow-terminal fallback

## Decisions Made
- Used text_to_owned() conversion to make tui-markdown's borrowed Text cacheable as Text<'static> in App state
- Intercepted Ctrl+P at App::handle_editing_key() before forwarding to Editor to avoid Emacs keybinding conflict (Pitfall 2)
- Set 80ms debounce timer as sweet spot between responsiveness and performance (per research D-04)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed lifetime mismatch in MarkdownRenderer trait**
- **Found during:** Task 1 (MarkdownRenderer implementation)
- **Issue:** tui_markdown::from_str() returns Text<'_> borrowed from input, but the trait needs to return Text<'static> for caching in App state
- **Fix:** Added text_to_owned() function that converts all Cow::Borrowed spans to Cow::Owned, and changed trait signature to return Text<'static>
- **Files modified:** src/markdown/mod.rs, src/markdown/renderer.rs
- **Verification:** cargo check succeeds
- **Committed in:** 555ef26 (Task 1 commit)

**2. [Rule 1 - Bug] Fixed Line.alignment type mismatch**
- **Found during:** Task 1 (text_to_owned implementation)
- **Issue:** ratatui Line.alignment is Option<HorizontalAlignment> but Line::alignment() method expects HorizontalAlignment (not Option)
- **Fix:** Added conditional unwrap: only set alignment when Some
- **Files modified:** src/markdown/renderer.rs
- **Verification:** cargo check succeeds
- **Committed in:** 555ef26 (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Both auto-fixes necessary for compilation. No scope creep.

## Issues Encountered
None beyond the auto-fixed deviations above.

## Known Stubs
None - all data sources are wired (editor.content() feeds renderer, renderer output feeds preview).

## Next Phase Readiness
- Preview rendering pipeline complete, ready for editor-pane syntax highlighting (Plan 02)
- MarkdownRenderer trait in place for future replacement/forking of tui-markdown
- syntect and syntect-tui dependencies already added for editor highlighting

---
*Phase: 02-live-preview*
*Completed: 2026-03-21*
