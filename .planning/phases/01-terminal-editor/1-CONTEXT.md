# Phase 1: Terminal Editor - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Build a fully functional terminal text editor for markdown files. Users can open files, edit text, save, and navigate — with line numbers, undo/redo, Unicode support, status bar, and crash recovery. No preview rendering in this phase — that's Phase 2.

</domain>

<decisions>
## Implementation Decisions

### CLI behavior
- **D-01:** `mdedit file.md` opens the file for editing. If the file doesn't exist, create it on first save (not immediately).
- **D-02:** `mdedit` with no arguments opens an empty buffer titled "[untitled]". On save, prompt for filename.
- **D-03:** Exit codes: 0 for clean exit, 1 for error. No special exit codes needed for v1.
- **D-04:** Use `clap` for argument parsing — standard Rust CLI crate, minimal overhead.

### Keybinding scheme
- **D-05:** Follow nano/micro conventions (most intuitive for non-vim users):
  - `Ctrl+S` — Save
  - `Ctrl+Q` — Quit
  - `Ctrl+Z` — Undo
  - `Ctrl+Y` — Redo
  - `Ctrl+Left/Right` — Word jump
  - `Home/End` — Line start/end
  - `Ctrl+Home/End` — Document start/end (if tui-textarea supports it)
- **D-06:** No keybinding conflicts with common terminal shortcuts (Ctrl+C exits, don't override it for copy).
- **D-07:** Ctrl+C should NOT exit the editor. Instead, it does nothing or copies (if selection exists in a later phase). Ctrl+Q is the explicit quit command.

### Editor appearance
- **D-08:** Line numbers in a left gutter, right-aligned, dimmed color (not distracting).
- **D-09:** Cursor is a block cursor (standard terminal cursor, not custom rendering).
- **D-10:** Use terminal's default color scheme — no hardcoded colors. Detect 256-color vs truecolor support.
- **D-11:** No line wrapping for v1 — long lines scroll horizontally. Soft wrapping is a v2 feature.

### Save and exit flow
- **D-12:** Ctrl+S saves immediately. Status bar shows "Saved" for 2 seconds, then reverts to normal.
- **D-13:** On exit with unsaved changes: show a bar prompt "Unsaved changes. Save? (y/n/Esc)" — y saves and exits, n exits without saving, Esc cancels exit.
- **D-14:** No auto-save. No backup/swap files. Keep it simple.
- **D-15:** Modified indicator is a dot or `[+]` after the filename in the status bar.

### Terminal safety
- **D-16:** Panic hook installed before entering raw mode — uses `std::panic::set_hook` to restore terminal state.
- **D-17:** Also handle SIGINT/SIGTERM gracefully — restore terminal before exit.
- **D-18:** Use crossterm's `enable_raw_mode` / `disable_raw_mode` and alternate screen.

### Claude's Discretion
- Exact gutter width calculation (auto-sized to line count digits)
- Status bar layout and styling
- Internal module organization
- Error message wording
- tui-textarea configuration details

</decisions>

<specifics>
## Specific Ideas

- Should feel like opening `nano` or `micro` — instant, no confusion, just start typing
- Status bar at the bottom like nano — filename on the left, position on the right
- The editor IS the full screen in Phase 1 (no split, no preview) — preview comes in Phase 2

</specifics>

<canonical_refs>
## Canonical References

No external specs — requirements are fully captured in decisions above and in:

### Project context
- `.planning/PROJECT.md` — Core value, constraints, key decisions
- `.planning/REQUIREMENTS.md` — FOUND-01 through FOUND-06, EDIT-01 through EDIT-06, EDIT-10, CHRM-01, CHRM-03

### Research
- `.planning/research/STACK.md` — ratatui 0.30, tui-textarea 0.7, crossterm crate recommendations
- `.planning/research/ARCHITECTURE.md` — Component architecture pattern, synchronous event loop, build order
- `.planning/research/PITFALLS.md` — Panic hook priority, Unicode handling with unicode-segmentation + unicode-width

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- None — greenfield project, no existing code

### Established Patterns
- None yet — this phase establishes the patterns

### Integration Points
- Phase 2 will consume the editor component's text content for preview rendering
- Phase 2 will split the layout to add a preview pane alongside the editor
- The editor's cursor position will be needed by Phase 3 for scroll sync

</code_context>

<deferred>
## Deferred Ideas

- WYSIWYG editing in preview mode — added as v2 requirement (PREV-09)
- Markdown flavor selection (GFM, Obsidian, Lark) — v2 (PREV-07)

</deferred>

---

*Phase: 01-terminal-editor*
*Context gathered: 2026-03-21*
