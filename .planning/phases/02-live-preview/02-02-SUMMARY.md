---
phase: 02-live-preview
plan: 02
subsystem: ui
tags: [syntect, syntax-highlighting, markdown, ratatui, tui]

# Dependency graph
requires:
  - phase: 02-live-preview/01
    provides: "Split layout, preview rendering, tui-textarea editor"
provides:
  - "Markdown-aware syntax highlighting in the editor pane via syntect"
  - "Custom editor rendering path with per-span coloring, line numbers, and cursor"
  - "MarkdownHighlighter module reusable for future highlighting needs"
affects: [03-polish]

# Tech tracking
tech-stack:
  added: [syntect 5.3 (default-fancy)]
  removed: [syntect-tui 3.0 (version mismatch with ratatui 0.30)]
  patterns: [manual syntect-to-ratatui style conversion, custom editor rendering bypassing tui-textarea Widget]

key-files:
  created: [src/highlighter.rs]
  modified: [src/editor.rs, src/app.rs, src/main.rs, Cargo.toml]

key-decisions:
  - "Dropped syntect-tui due to ratatui 0.28 vs 0.30 type mismatch; manual style conversion instead"
  - "Custom render path: bypasses tui-textarea Widget rendering for per-span highlighting while keeping tui-textarea for input handling"
  - "Skip syntect background colors to keep terminal default background (D-10 subtle)"
  - "Underline cursor line instead of background highlight for visibility in custom renderer"

patterns-established:
  - "Manual syntect style conversion: convert_syntect_style() handles fg/modifier without background"
  - "Custom editor rendering: Editor::render_highlighted() owns scroll state and renders Paragraph + cursor"

requirements-completed: [PREV-06]

# Metrics
duration: 6min
completed: 2026-03-21
---

# Phase 2 Plan 2: Editor Syntax Highlighting Summary

**Markdown-aware syntax highlighting in editor pane using syntect with base16-ocean.dark, rendered via custom Paragraph path bypassing tui-textarea's widget**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-21T15:46:03Z
- **Completed:** 2026-03-21T15:52:18Z
- **Tasks:** 2 (1 auto + 1 auto-approved checkpoint)
- **Files modified:** 6

## Accomplishments
- Editor pane now has markdown-aware syntax highlighting: headings, bold/italic markers, code fences, link syntax, and list markers are colored distinctly
- Custom rendering path renders syntect-highlighted lines as a Paragraph widget while keeping tui-textarea for input handling (keystrokes, undo/redo, cursor management)
- Highlighting uses the base16-ocean.dark theme (terminal-compatible) with foreground colors only (no background override) for subtlety
- Complete Phase 2 implementation: side-by-side split with rendered preview, Ctrl+P toggle, debounced updates, scrollable preview, editor highlighting, status bar hints

## Task Commits

Each task was committed atomically:

1. **Task 1: Create Highlighter module and integrate with Editor** - `a2b328b` (feat)
2. **Task 2: Visual verification of complete Phase 2** - auto-approved (checkpoint, no code changes)

## Files Created/Modified
- `src/highlighter.rs` - MarkdownHighlighter using syntect for per-span markdown coloring
- `src/editor.rs` - Added render_highlighted() custom render path with scroll tracking
- `src/app.rs` - Switched to render_highlighted() for editor rendering
- `src/main.rs` - Added mod highlighter declaration
- `Cargo.toml` - Removed syntect-tui dependency (version mismatch)
- `Cargo.lock` - Updated lock file

## Decisions Made
- **Dropped syntect-tui:** ratatui version mismatch (syntect-tui targets 0.28, we use 0.30). Manual `convert_syntect_style()` function handles the conversion directly with ~20 lines of code.
- **Custom render path:** tui-textarea has no per-span styling API (confirmed by reading source). The approach renders highlighted lines ourselves while keeping tui-textarea purely for input handling.
- **No background colors:** Syntect theme backgrounds tend to clash with terminal backgrounds. Only foreground colors and font modifiers (bold/italic/underline) are applied per D-10 "keep it subtle."
- **Persistent scroll state:** Editor tracks its own `scroll_top` to keep cursor visible during custom rendering, matching tui-textarea's scroll behavior.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Dropped syntect-tui due to ratatui version mismatch**
- **Found during:** Task 1 (Highlighter integration)
- **Issue:** syntect-tui 3.0 depends on ratatui 0.28.1; our project uses ratatui 0.30. The `Style` types from different ratatui versions are incompatible, causing `From<Style>` trait errors.
- **Fix:** Removed syntect-tui dependency entirely. Wrote manual `convert_syntect_style()` function (~20 lines) that converts syntect's Style to ratatui 0.30's Style directly.
- **Files modified:** Cargo.toml, src/highlighter.rs
- **Verification:** cargo build succeeds with no type errors
- **Committed in:** a2b328b

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary to resolve version incompatibility. The manual conversion is straightforward and avoids a transitive dependency problem.

## Issues Encountered
None beyond the syntect-tui version mismatch documented above.

## User Setup Required
None - no external service configuration required.

## Known Stubs
None - all functionality is wired and operational.

## Next Phase Readiness
- Complete Phase 2 implementation ready for Phase 3 (polish)
- Editor has syntax highlighting, preview rendering, layout toggles, and status bar
- Scroll sync between editor and preview is Phase 3 scope (LAYT-03)

---
*Phase: 02-live-preview*
*Completed: 2026-03-21*

## Self-Check: PASSED
- src/highlighter.rs: FOUND
- src/editor.rs: FOUND
- 02-02-SUMMARY.md: FOUND
- Commit a2b328b: FOUND
