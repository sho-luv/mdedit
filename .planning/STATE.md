---
gsd_state_version: 1.0
milestone: v2.0
milestone_name: Power User
status: unknown
stopped_at: Completed 06-01-PLAN.md
last_updated: "2026-03-23T15:46:49.044Z"
progress:
  total_phases: 5
  completed_phases: 3
  total_plans: 7
  completed_plans: 7
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-22)

**Core value:** Edit markdown and see the rendered result side-by-side in a single terminal app, with zero external dependencies.
**Current focus:** Phase 06 — clipboard-integration

## Current Position

Phase: 06 (clipboard-integration) — EXECUTING
Plan: 1 of 1

## Performance Metrics

**Velocity (v1.0):**

| Phase 01 P01 | 2min | 2 tasks | 5 files |
| Phase 01 P02 | 2min | 2 tasks | 4 files |
| Phase 02 P01 | 3min | 2 tasks | 7 files |
| Phase 02 P02 | 6min | 2 tasks | 6 files |
| Phase 03 P01 | 2min | 2 tasks | 3 files |
| Phase 03 P02 | 2min | 2 tasks | 3 files |

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Carried from v1.0:

- ratatui-textarea 0.8 (ratatui org fork) for ratatui 0.30 compatibility
- input_without_shortcuts() exclusively to avoid Emacs keybinding conflicts
- Custom editor render path: bypass tui-textarea Widget for per-span syntax highlighting
- 80ms debounce timer for preview updates
- tui-markdown behind MarkdownRenderer trait for replaceability

v2.0 roadmap decisions:

- Vim as state machine layer over existing editor ops (not replacing editor widget)
- Config/theme first because it's cross-cutting (every component has hardcoded colors)
- Mouse support grouped with vim (both change interaction model in app.rs)
- WYSIWYG last (highest risk, depends on stable vim + theme infrastructure)
- [Phase 04]: Vim as default EditingMode per user preference
- [Phase 04]: Ocean theme matches original hardcoded colors for zero visual change
- [Phase 04]: 256-color fallback uses euclidean distance to 6x6x6 cube
- [Phase 04]: Theme passed as owned clone to Editor to avoid lifetime complexity
- [Phase 05]: VimHandler returns VimCommand enum for app.rs to interpret, keeping editor logic in app layer
- [Phase 05]: CursorMoveCmd wrapper enum to enable PartialEq/Eq derives on VimCommand
- [Phase 05]: Manual terminal init replaces ratatui::run() for EnableMouseCapture
- [Phase 05]: Split ratio clamped 20-80% to prevent unusable pane sizes
- [Phase 05]: Central execute_vim_command dispatcher replaces per-mode inline handling
- [Phase 05]: Line-wise yank stores trailing newline to distinguish from char-wise paste
- [Phase 05]: Track line-wise state via last_visual_line_wise field rather than checking current mode
- [Phase 06]: OSC 52 as primary clipboard transport with platform-native fallback reader
- [Phase 06]: Inline base64 encoder to avoid adding base64 crate dependency
- [Phase 06]: yank_to_clipboard centralizes all clipboard+register write operations

### Pending Todos

None yet.

### Blockers/Concerns

- Vim keybindings require replacing the entire key routing system
- WYSIWYG mode needs cursor mapping between rendered and source positions — novel challenge
- tui-markdown is experimental — may need replacement for WYSIWYG inline rendering
- WYSIWYG confidence is LOW per research — prototype recommended before full commit

## Session Continuity

Last session: 2026-03-23T15:46:49.041Z
Stopped at: Completed 06-01-PLAN.md
Resume file: None
