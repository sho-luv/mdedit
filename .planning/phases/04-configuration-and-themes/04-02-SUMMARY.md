---
phase: 04-configuration-and-themes
plan: 02
subsystem: ui
tags: [ratatui, theming, syntect, color-themes, tui]

# Dependency graph
requires:
  - phase: 04-configuration-and-themes plan 01
    provides: Theme struct, Config struct, resolve_theme(), built-in themes
provides:
  - Theme wired through all rendering: editor, status bar, divider, prompts, search highlights
  - Configurable syntect theme name in MarkdownHighlighter
  - Zero hardcoded Color:: values outside theme.rs
affects: [05-vim-keybindings, browser-companion, wysiwyg]

# Tech tracking
tech-stack:
  added: []
  patterns: [theme-threading through component hierarchy, configurable syntect theme with fallback]

key-files:
  created: []
  modified:
    - src/editor.rs
    - src/highlighter.rs
    - src/app.rs
    - src/status_bar.rs

key-decisions:
  - "Theme passed as owned clone to Editor (not reference) to avoid lifetime complexity"
  - "Syntect theme fallback: warn on stderr and use base16-ocean.dark if name not found"
  - "Dynamic Color::Rgb in syntect converter is acceptable -- it converts computed values, not hardcoded theme colors"

patterns-established:
  - "Theme threading: App owns Theme, clones to Editor, passes &Theme to StatusBar"
  - "Configurable syntect: MarkdownHighlighter::new(theme_name) with fallback"

requirements-completed: [CONF-01, CONF-02, CONF-03, CONF-04, CONF-05]

# Metrics
duration: 5min
completed: 2026-03-22
---

# Phase 04 Plan 02: Wire Theme Colors Summary

**All hardcoded Color:: values replaced with theme field references across editor, highlighter, status bar, and app rendering**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-22T14:51:30Z
- **Completed:** 2026-03-22T14:56:12Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Wired Theme struct through Editor, MarkdownHighlighter, StatusBar, and App render methods
- Replaced all hardcoded Color:: values (DarkGray, Rgb(68,68,102), Cyan, Yellow, Red, Blue, White) with theme field references
- Made syntect highlighter theme configurable with graceful fallback
- Switching theme in config now changes all colors: line numbers, selection, search highlights, divider, status bar, confirm prompt, filename prompt, search bar

## Task Commits

Each task was committed atomically:

1. **Task 1: Wire theme into Editor and MarkdownHighlighter** - `d8806d4` (feat)
2. **Task 2: Wire theme into App and StatusBar rendering** - `26bd02d` (feat)

## Files Created/Modified
- `src/highlighter.rs` - MarkdownHighlighter::new accepts syntect_theme_name, line_number_span accepts line_number_fg
- `src/editor.rs` - Editor stores Theme, uses theme colors for selection/search/line numbers
- `src/app.rs` - Render uses self.theme for divider, confirm, prompt, search bar colors
- `src/status_bar.rs` - StatusBar::render accepts &Theme, uses theme.status_bar_bg/fg

## Decisions Made
- Theme is cloned into Editor (owned) rather than passed by reference to avoid lifetime complexity with the textarea borrow
- Syntect theme lookup uses get() with fallback rather than panicking on unknown theme names
- The single remaining Color::Rgb in highlighter.rs (syntect style converter) is acceptable as it converts dynamic computed values

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All theming infrastructure complete: config loading, theme resolution, and color wiring
- Phase 04 (configuration-and-themes) is fully complete
- Ready for Phase 05 (vim-keybindings) which will use the EditingMode from config

---
*Phase: 04-configuration-and-themes*
*Completed: 2026-03-22*
