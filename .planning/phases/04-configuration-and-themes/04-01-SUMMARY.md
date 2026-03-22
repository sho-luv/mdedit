---
phase: 04-configuration-and-themes
plan: 01
subsystem: config
tags: [serde, toml, themes, ratatui, color-detection]

requires:
  - phase: none
    provides: n/a
provides:
  - Config struct with TOML deserialization and XDG config loading
  - Theme struct with 16 color fields and 4 built-in themes
  - ColorCapability detection and 256-color fallback
  - Custom theme overlay from TOML
  - CLI --theme and --mode flags
  - EditingMode enum (Vim/Nano)
affects: [04-02-theme-wiring, 05-vim-keybindings]

tech-stack:
  added: [serde, toml, dirs]
  patterns: [config-load-with-defaults, theme-by-name-resolution, color-fallback-256]

key-files:
  created: [src/config.rs, src/theme.rs]
  modified: [Cargo.toml, src/main.rs, src/app.rs]

key-decisions:
  - "Vim as default EditingMode (user preference from memory)"
  - "Ocean theme matches all original hardcoded colors for zero visual change"
  - "256-color fallback uses euclidean distance to 6x6x6 cube"
  - "CustomThemeColors duplicated in config.rs to keep serde boundary clean"

patterns-established:
  - "Config loading: dirs::config_dir() + graceful default fallback"
  - "Theme resolution: built-in -> custom -> fallback with warning"
  - "CLI override: config loaded first, then CLI args override fields"

requirements-completed: [CONF-01, CONF-02, CONF-03, CONF-04, CONF-05]

duration: 3min
completed: 2026-03-22
---

# Phase 04 Plan 01: Configuration and Theme Infrastructure Summary

**Config and theme infrastructure with 4 built-in themes (ocean/dracula/solarized-light/gruvbox-dark), TOML config loading from XDG path, custom theme overlay, 256-color fallback, and CLI --theme/--mode flags**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-22T14:47:13Z
- **Completed:** 2026-03-22T14:49:44Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Theme struct with 16 color fields covering every hardcoded color in the codebase
- 4 built-in themes available by name with case-insensitive lookup
- Config loading from ~/.config/mdedit/config.toml with graceful defaults on missing/malformed files
- CLI --theme and --mode flags that override config file values
- Terminal color capability detection from $COLORTERM with automatic 256-color fallback
- Custom theme support via TOML sections with overlay on any base theme

## Task Commits

Each task was committed atomically:

1. **Task 1: Create Theme struct, built-in themes, color detection, and custom theme support** - `917394b` (feat)
2. **Task 2: Create Config struct, config loading, and wire CLI args** - `2657a6e` (feat)

## Files Created/Modified
- `src/theme.rs` - Theme struct, 4 built-in themes, ColorCapability, 256-color fallback, ThemeColors serde, parse_color, from_custom overlay
- `src/config.rs` - Config struct, EditingMode enum, load_config from XDG, resolve_theme with fallback chain
- `Cargo.toml` - Added serde, toml, dirs dependencies
- `src/main.rs` - Added mod config/theme, CLI --theme/--mode args, config loading and theme resolution
- `src/app.rs` - Added theme and editing_mode fields to App struct, updated new() signature

## Decisions Made
- Vim as default EditingMode per user preference stored in memory
- Ocean theme uses exact same values as original hardcoded colors for zero visual change on upgrade
- 256-color fallback uses euclidean distance to the 6x6x6 color cube (indices 16-231)
- CustomThemeColors struct duplicated in config.rs (not reusing ThemeColors directly) to keep the serde deserialization boundary clean from theme.rs internals

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Theme and config infrastructure ready for Plan 02 (theme wiring to all UI components)
- App struct stores theme and editing_mode, ready to replace hardcoded colors
- All 4 built-in themes tested via cargo check

---
*Phase: 04-configuration-and-themes*
*Completed: 2026-03-22*
