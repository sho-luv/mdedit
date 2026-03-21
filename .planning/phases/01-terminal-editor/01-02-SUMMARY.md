---
phase: 01-terminal-editor
plan: 02
subsystem: editor
tags: [rust, ratatui, file-io, status-bar, atomic-save, terminal-editor]

# Dependency graph
requires:
  - phase: 01-terminal-editor
    provides: "Editor module with nano-style keybindings, App skeleton with event loop"
provides:
  - "Atomic file save/load via temp file + rename"
  - "StatusBar widget with timed messages, filename, cursor position, modified indicator"
  - "Complete exit-with-unsaved-changes flow (y/n/Esc)"
  - "Filename prompt for untitled buffers"
affects: [02-markdown-preview]

# Tech tracking
tech-stack:
  added: []
  patterns: [atomic-file-write, timed-status-messages, modal-prompt-flow]

key-files:
  created:
    - src/file_io.rs
    - src/status_bar.rs
  modified:
    - src/app.rs
    - src/main.rs

key-decisions:
  - "Error display in status bar: save failures shown as timed message rather than separate error mode"
  - "quit_after_save flag: tracks whether to quit after filename prompt + save completes"

patterns-established:
  - "Atomic file write: write to .mdedit-tmp then rename to target, with best-effort cleanup on failure"
  - "Timed status messages: StatusBar stores (message, Instant) pair, checks elapsed < 2s on render"
  - "Modal input flow: AppMode enum routes keys to different handlers, modes can chain (ConfirmQuit -> PromptFilename -> save -> quit)"

requirements-completed: [EDIT-04, EDIT-05, CHRM-01]

# Metrics
duration: 2min
completed: 2026-03-21
---

# Phase 01 Plan 02: File I/O and Status Bar Summary

**Atomic file save with temp+rename, status bar with timed messages and cursor tracking, complete save-before-quit flow with filename prompting for untitled buffers.**

## What Was Built

### src/file_io.rs (new)
- `save_file()`: writes to `.mdedit-tmp` then atomically renames to target path, with best-effort temp file cleanup on error
- `load_file()`: reads file content, returns `None` for not-found (file created on first save per D-01), propagates other errors

### src/status_bar.rs (new)
- `StatusBar` struct with timed message support (message + Instant timestamp)
- `set_message()`: sets a message that displays for 2 seconds
- `render()`: renders either the timed message or normal status line (filename + modified indicator on left, Ln/Col on right)
- `is_message_active()`: utility for event loop to check if message is still showing

### src/app.rs (updated)
- Added `status_bar: StatusBar` field and `quit_after_save: bool` flag
- Wired Ctrl+S to `file_io::save_file()` with filepath check -- prompts for filename if untitled
- ConfirmQuit ('y') now saves before quitting, handles untitled buffers by chaining to PromptFilename
- PromptFilename Enter now calls `do_save()` and respects `quit_after_save` flag
- Render uses `StatusBar::render()` for editing mode, inline Paragraph for ConfirmQuit and PromptFilename modes

### src/main.rs (updated)
- File loading now uses `file_io::load_file()` instead of inline `std::fs::read_to_string`

## Deviations from Plan

None - plan executed exactly as written.

## Known Stubs

None. All functionality is fully wired.

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | 4383fbf | Create file_io and status_bar modules |
| 2 | 3336530 | Wire file I/O, status bar, and exit flow into App |

## Self-Check: PASSED
