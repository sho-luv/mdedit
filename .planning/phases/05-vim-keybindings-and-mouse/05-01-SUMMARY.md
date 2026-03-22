---
phase: 05-vim-keybindings-and-mouse
plan: 01
subsystem: editor
tags: [vim, keybindings, state-machine, modal-editing, ratatui, crossterm]

requires:
  - phase: 04-config-and-themes
    provides: Theme struct, EditingMode enum, config loading

provides:
  - VimHandler state machine with VimMode, VimCommand, Motion, InsertPosition enums
  - AppMode expanded with Normal, Insert, Visual, Command variants
  - Vim key routing in event loop (handle_vim_key, handle_vim_insert_key, handle_vim_visual_key, handle_vim_command_key)
  - Mode indicator in status bar with per-theme colors
  - Cursor shape changes per mode (block/bar)
  - Command mode parsing (:w, :q, :q!, :wq, :x)

affects: [05-02, 05-03, 05-04]

tech-stack:
  added: []
  patterns: [vim-state-machine-layer, modal-key-routing, cursor-shape-per-mode]

key-files:
  created: [src/vim.rs]
  modified: [src/app.rs, src/main.rs, src/theme.rs, src/config.rs, src/status_bar.rs]

key-decisions:
  - "VimHandler returns VimCommand enum for app.rs to interpret, keeping editor logic in app layer"
  - "CursorMoveCmd wrapper enum around CursorMove to enable PartialEq/Eq derives"
  - "Status bar vim_mode parameter is Option to keep nano mode completely unchanged"
  - "Task 1 and Task 2 merged because status_bar signature change was needed for cargo check to pass"

patterns-established:
  - "Vim commands: VimHandler.handle_key() returns VimCommand, app.rs matches and executes"
  - "Mode transitions: VimHandler sets internal mode, app.rs mirrors to AppMode"
  - "return_to_editing_mode() helper for mode-aware escape from shared modes"

requirements-completed: [VIM-01, VIM-02, VIM-08]

duration: 3min
completed: 2026-03-22
---

# Phase 05 Plan 01: Vim Handler Foundation Summary

**VimHandler state machine with modal key routing, mode indicator status bar, and cursor shape changes per vim mode**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-22T19:00:34Z
- **Completed:** 2026-03-22T19:04:00Z
- **Tasks:** 2 (merged into 1 commit)
- **Files modified:** 6

## Accomplishments

- VimHandler state machine in src/vim.rs with VimMode, VimCommand, Motion, InsertPosition, CursorMoveCmd enums
- AppMode expanded with Normal/Insert/Visual/Command variants and full key routing in event loop
- Mode switching works: Normal->Insert (i/a/I/A/o/O), Insert->Normal (Esc), Normal->Command (:), Normal->Visual (v/V), Normal->Search (/)
- Status bar displays mode indicator (-- NORMAL --, -- INSERT --, -- VISUAL --) with per-theme colors
- Command mode renders :{buffer}_ in status bar and parses :w, :q, :q!, :wq, :x
- Cursor shape is block in Normal/Visual/Command, bar in Insert/Editing
- Nano mode completely unaffected (vim_handler is None, AppMode::Editing unchanged)
- All four built-in themes have mode indicator colors

## Task Commits

1. **Task 1+2: VimHandler state machine, AppMode expansion, vim key routing, status bar mode indicator** - `d5227f4` (feat)

**Plan metadata:** pending (docs: complete plan)

## Files Created/Modified

- `src/vim.rs` - VimHandler state machine with VimMode, VimCommand, Motion, InsertPosition, CursorMoveCmd enums
- `src/app.rs` - Expanded AppMode, vim key routing methods, cursor shape changes, mode indicator rendering
- `src/main.rs` - Added mod vim declaration
- `src/theme.rs` - Added mode_normal_bg, mode_insert_bg, mode_visual_bg, mode_command_bg to Theme and ThemeColors
- `src/config.rs` - Added mode color fields to CustomThemeColors and to_theme_colors()
- `src/status_bar.rs` - Updated render() to accept optional vim_mode parameter with mode label and color

## Decisions Made

- VimHandler returns VimCommand enum for app.rs to interpret, keeping editor mutation logic in the app layer rather than inside vim.rs
- Created CursorMoveCmd wrapper enum because ratatui_textarea::CursorMove does not derive PartialEq/Eq needed for VimCommand derives
- Status bar vim_mode parameter uses Option<(&str, Color)> to keep nano mode path completely unchanged
- Merged Task 1 and Task 2 because the status_bar signature change was required for cargo check to pass in Task 1

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Merged Task 2 (status bar) into Task 1**
- **Found during:** Task 1 verification (cargo check)
- **Issue:** Task 1 passes vim_mode parameter to status_bar.render() but Task 2 was supposed to update the signature. cargo check fails without the signature update.
- **Fix:** Updated status_bar.rs render() signature and implementation in Task 1 commit
- **Files modified:** src/status_bar.rs
- **Verification:** cargo check passes, cargo build --release succeeds
- **Committed in:** d5227f4 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Task merge was necessary for compilation. No scope creep -- both tasks' acceptance criteria are met.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- VimHandler skeleton ready for Plan 02 (normal mode motions: h/j/k/l, w/b/e, 0/$, gg/G, operators d/c/y)
- VimHandler ready for Plan 03 (visual mode operations, text objects)
- All mode switching infrastructure in place
- Count prefix accumulation implemented and ready for motion repetition

---
*Phase: 05-vim-keybindings-and-mouse*
*Completed: 2026-03-22*
