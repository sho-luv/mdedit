---
quick_id: 260702-tek
slug: btop-style-ui-restyle
created: 2026-07-03
mode: quick
status: in-progress
note: >
  Executed inline by orchestrator (no worktree, no commits) because the working
  tree carries ~640 lines of uncommitted prior work that worktree isolation
  would miss and commits would sweep up. User asked for all changes to remain
  uncommitted for review.
---

# Quick Task 260702-tek: btop-style UI/UX restyle

## Goal

Model mdedit's UI and user experience after btop: bordered panels with
titles embedded in the borders, clickable title-bar buttons, a help overlay,
focus-follows-click between panes, and a contextual key-hint status bar —
while preserving every existing behavior (vim/nano modes, scroll sync, search,
settings overlay, mouse resize/select, layouts).

## Design decisions

- **Panels**: Split layout becomes two adjacent `Block::bordered()` panels with
  `BorderType::Rounded` (btop look). Editor title = ` filename + ` (modified
  marker); preview title = ` preview · <profile> `. Focused panel border and
  title use `theme.preview_accent_primary`; unfocused use `theme.divider_fg`.
  No new theme fields needed.
- **Title-bar buttons** (btop `┤ label ├` style, drawn over the top border of
  the rightmost panel, right-aligned): `F1 help`, `F2 set`, `^P view`,
  `^Q quit`. Key part accent+bold. Button rects recorded each frame for mouse
  hit-testing. Buttons that don't fit are dropped.
- **Help overlay**: `show_help: bool` flag (not an AppMode, so it returns to
  whatever mode was active). Opened via F1 or button; closed by F1/Esc/q/Enter
  or any click. Lists keybindings for the active editing mode + mouse/global
  keys, rendered like the settings overlay (Clear + rounded Block).
- **Focus-follows-click**: new `FocusedPane` {Editor, Preview}. Click or wheel
  in preview focuses it (accent border); while focused, j/k/arrows/PageUp/
  PageDown/g/G/Home/End scroll the preview and cursor scroll-sync is paused
  (this also fixes wheel-scroll of preview being overwritten by sync every
  frame). Esc or any editing key refocuses the editor. Layout cycle sets focus
  to the visible pane; unified handling replaces the nano-only PreviewOnly key
  branch.
- **Status bar**: hints become structured `(key, label)` segments with
  accent-colored keys, btop style; F1 help added everywhere. Ctrl+Q added to
  vim normal mode as request-quit so the quit button's hint holds true; F2
  opens settings from Normal/Editing.
- **Divider drag**: preserved — the two adjacent border columns between the
  panels are the drag target; ratio computed against the stored split body
  area.

## Tasks

1. **app.rs** — FocusedPane + button state, bordered-panel render path for all
   three layouts, title buttons + hit testing, help overlay, focus-aware key
   routing and scroll-sync gating, F1/F2/^Q intercepts, mouse focus handling.
   Verify: `cargo test` render-matrix tests still pass; new tests for button
   registration and help overlay render.
2. **status_bar.rs** — structured hint segments with accent keys; per-mode
   hints supplied by app.rs. Verify: builds, visual check.
3. **Quality gates** — `cargo fmt --check`, `cargo clippy --all-targets -- -D
   warnings`, `cargo test`. NO commits (user constraint).
