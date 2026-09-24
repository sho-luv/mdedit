---
quick_id: 260702-tek
slug: btop-style-ui-restyle
status: complete
completed: 2026-07-03
commit: none (left uncommitted at user request — working tree also carried prior uncommitted work)
---

# Summary: btop-style UI/UX restyle

## What changed

**src/app.rs** (core rework)
- Split layout is now two adjacent rounded-border panels (btop look) instead
  of two flat panes with a 1-column divider. Editor title ` filename + `,
  preview title ` preview · <profile> `, embedded in the top border.
- New `FocusedPane` focus model: click/wheel in a pane focuses it; focused
  panel gets `preview_accent_primary` border. While the preview is focused,
  scroll keys drive it and cursor scroll-sync pauses (also fixes manual
  preview scrolls being overwritten by sync every frame).
- Clickable title-bar buttons drawn over the top border, right-aligned:
  `┤^P view├ ┤F2 set├ ┤F1 help├ ┤^Q quit├`. Hit areas recorded per frame;
  buttons stop short of the panel title and drop lowest-priority first
  (view, set) when narrow.
- Help overlay (`show_help` flag): F1 or button opens; F1/Esc/q/Enter or any
  click closes. Lists vim or nano keybindings + mouse/pane behaviors.
- F2 opens settings; Ctrl+Q added to vim normal mode as request-quit;
  unified `cycle_layout()` moves focus to the visible pane; the old nano-only
  PreviewOnly key branch replaced by a mode-aware `preview_focus_intercept`
  (letters only scroll in vim Normal/Visual — they stay text in Insert/nano).
- Divider drag preserved: the adjacent border columns are the drag target;
  ratio computed against the stored split body area.
- Settings overlay restyled (rounded, accent border, lowercase title).

**src/status_bar.rs** — hints are now structured `(key, label)` segments with
accent-colored bold keys, btop style; per-mode hints (including
preview-focused variants) supplied by app.rs.

**src/preview.rs** — `render` now persists the scroll clamp (so End/G lands on
the last page and scrolling back works); added `scroll_offset()` getter.

**README.md** — documented F1/F2/^Q, clickable buttons, and focus behavior.

## Verification

- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D
  warnings`, `cargo test`: all clean, 20/20 tests (4 new: button
  registration/click, help overlay render matrix, help overlay content,
  preview focus + manual scroll persistence).
- Visual: VHS-rendered screenshots of the real binary confirmed panels,
  titles, buttons, settings overlay, and status-bar hints; help overlay
  verified via TestBackend buffer dump.

## Deviations from plan

- None functional. Executed inline (no gsd-executor worktree) and left
  uncommitted, both per the constraint recorded in the plan.
