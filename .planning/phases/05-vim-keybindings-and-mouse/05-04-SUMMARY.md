---
phase: 05-vim-keybindings-and-mouse
plan: 04
subsystem: ui
tags: [mouse, crossterm, ratatui, tui, drag, scroll, split-ratio]

requires:
  - phase: 05-01
    provides: "Vim mode routing in app.rs event loop, VimHandler state machine"
provides:
  - "Mouse click-to-cursor in editor pane"
  - "Mouse wheel scroll for editor and preview panes"
  - "Drag-to-select text (with vim Visual mode integration)"
  - "Drag divider to resize split panes (20-80% range)"
  - "EnableMouseCapture/DisableMouseCapture in terminal lifecycle"
  - "Configurable split_ratio field on App"
affects: [wysiwyg, browser-companion]

tech-stack:
  added: []
  patterns:
    - "Manual terminal init (replacing ratatui::run()) for mouse capture control"
    - "Area-based mouse hit testing: store Rect from layout, check in event handler"
    - "Mouse events handled at App level before mode dispatch"

key-files:
  created: []
  modified:
    - "src/main.rs"
    - "src/app.rs"
    - "src/editor.rs"
    - "src/vim.rs"

key-decisions:
  - "Manual terminal init replaces ratatui::run() to enable EnableMouseCapture"
  - "Split ratio clamped 20-80% to prevent unusable pane sizes"
  - "Mouse scroll moves cursor (3 lines) rather than viewport-only scroll"
  - "Mouse drag in vim mode enters Visual char-wise mode automatically"

patterns-established:
  - "Area tracking pattern: store Rect from render() for hit testing in event loop"
  - "Divider drag pattern: set flag on MouseDown, update ratio on Drag, clear on MouseUp"

requirements-completed: [MOUSE-01, MOUSE-02, MOUSE-03, MOUSE-04]

duration: 2min
completed: 2026-03-22
---

# Phase 05 Plan 04: Mouse Support Summary

**Full mouse support: click-to-cursor with line number offset, wheel scroll per-pane, drag-select with vim Visual integration, and draggable split divider**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-22T19:07:15Z
- **Completed:** 2026-03-22T19:10:06Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Replaced ratatui::run() with manual terminal init for EnableMouseCapture/DisableMouseCapture
- Click in editor positions cursor correctly accounting for line numbers and scroll offset
- Mouse wheel scrolls editor or preview depending on which pane the cursor is over
- Left drag selects text and enters vim Visual mode when in vim editing mode
- Divider drag adjusts split ratio (clamped 20-80%) for flexible pane sizing
- Added configurable split_ratio field (default 50%) to App struct

## Task Commits

Each task was committed atomically:

1. **Task 1: Enable mouse capture and add split_ratio to App** - `cbcad33` (feat)
2. **Task 2: Handle mouse events - click, scroll, drag-select, divider drag** - `b19fe20` (feat)

## Files Created/Modified
- `src/main.rs` - Manual terminal init with EnableMouseCapture/DisableMouseCapture
- `src/app.rs` - split_ratio, area tracking, handle_mouse_event(), click_to_editor_cursor()
- `src/editor.rs` - Added scroll_top() public getter
- `src/vim.rs` - Added set_mode_visual() for mouse-initiated selection

## Decisions Made
- Manual terminal init replaces ratatui::run() to enable EnableMouseCapture -- ratatui::run() is a convenience wrapper with no mouse support
- Split ratio clamped 20-80% to prevent unusable pane sizes
- Mouse scroll moves cursor by 3 lines rather than viewport-only scroll for consistent behavior
- Mouse drag in vim mode enters Visual char-wise mode automatically via set_mode_visual()

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Mouse support complete for both vim and nano modes
- All four MOUSE requirements fulfilled
- Ready for verification and integration testing

---
*Phase: 05-vim-keybindings-and-mouse*
*Completed: 2026-03-22*
