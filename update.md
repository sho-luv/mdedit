# mdedit - Log

## Project Overview
- **Created**: 2026-03-20
- **Location**: ~/home/projects/mine/mdedit/
- **Goal**: Build a terminal-based markdown editor with live rendered preview. Single TUI app that lets you edit raw markdown and toggle/split to see it rendered. Lightweight, no browser dependency. Publish to GitHub as open source.

---

## Updates

### 2026-09-24
- Rewrote the preview pipeline for speed and exact alignment. Per-frame work on a 5,200-line file dropped from ~100ms to ~0.1ms; editor syntax highlighting from ~75ms/keystroke to ~1.2ms.
  - Preview is laid out once per pane width into pre-wrapped rows with an exact row→source-line map; drawing a frame is now a copy of the visible slice (`src/markdown/layout.rs`, new).
  - Editor highlighting is incremental (caches syntect state per line, only re-does from the first edited line) and time-sliced across frames for cold jumps into huge files.
  - Event loop drains pending input before redrawing and sleeps when idle (was redrawing every 50ms).
  - Preview re-renders on every keystroke for normal files; debounced only when a render exceeds 8ms.
- Fixed alignment: scroll-sync is now exact line-by-line (incl. inside code blocks and tables), all widths measured in terminal cells (correct for CJK/emoji/em-dashes), code blocks/tables/rules/headings fit the pane. Fixed inline code escaping table cells, nested ordered-list numbering, tab handling; editor scrolls horizontally when wrap is off.
- New **clean** render profile (now the default), Warp-inspired: plain bold headings, outlined code blocks with language label in the border, borderless tables (record layout when narrow), circle/square nested bullets, checkbox task lists, reading margins with a 100-column cap. github/obsidian/commonmark unchanged.
- Switched `opt-level` z→3 (binary 3.6MB, well under 10MB). Removed unused `tui-markdown` dep. Added `.github/workflows/ci.yml` (fmt/clippy/test).
- Removed GSD workflow enforcement from project CLAUDE.md per preference.
- 36 tests (was 20), fmt/clippy/test all green. Researched Warp's open-source client to inform the clean profile (design only; their markdown crates are AGPL, so nothing copied).

### 2026-07-03
- Restyled the UI/UX after btop (quick task 260702-tek, left uncommitted for review)
- Rounded bordered panels with titles in the border (filename + modified marker, preview profile)
- Clickable title-bar buttons: ^P view, F2 set, F1 help, ^Q quit
- F1 help overlay listing keybindings for the active mode (vim or nano)
- Focus-follows-click: focused pane gets accent border; preview scrolls with keys when focused, scroll-sync pauses (fixes manual preview scroll being overwritten)
- btop-style status bar with accent-colored contextual key hints
- 4 new tests (20 total), fmt/clippy/test all green

### 2026-03-20
- Project created
- Born from frustration editing the React2Shell blog post — no good terminal tool exists that combines markdown editing with live rendering
- Existing tools: glow (view only), vim (edit only, no render), VS Code (bloated), Obsidian (vault-locked)
- Target: Rust or Go, single binary, fast startup, works over SSH

---
