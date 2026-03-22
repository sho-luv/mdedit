---
gsd_state_version: 1.0
milestone: v2.0
milestone_name: Power User
status: roadmap_complete
stopped_at: Roadmap created for v2.0
last_updated: "2026-03-22T15:00:00.000Z"
progress:
  total_phases: 5
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-22)

**Core value:** Edit markdown and see the rendered result side-by-side in a single terminal app, with zero external dependencies.
**Current focus:** Phase 4 — Configuration and Themes

## Current Position

Phase: 4 of 8 (Configuration and Themes) — first phase of v2.0
Plan: —
Status: Ready to plan
Last activity: 2026-03-22 — v2.0 roadmap created (5 phases, 33 requirements mapped)

Progress: [██████░░░░] 60% (3/8 phases complete across all milestones)

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

### Pending Todos

None yet.

### Blockers/Concerns

- Vim keybindings require replacing the entire key routing system
- WYSIWYG mode needs cursor mapping between rendered and source positions — novel challenge
- tui-markdown is experimental — may need replacement for WYSIWYG inline rendering
- WYSIWYG confidence is LOW per research — prototype recommended before full commit

## Session Continuity

Last session: 2026-03-22
Stopped at: v2.0 roadmap created, ready to plan Phase 4
Resume file: None
