# mdedit - Log

## Project Overview
- **Created**: 2026-03-20
- **Location**: ~/home/projects/mine/mdedit/
- **Goal**: Build a terminal-based markdown editor with live rendered preview. Single TUI app that lets you edit raw markdown and toggle/split to see it rendered. Lightweight, no browser dependency. Publish to GitHub as open source.

---

## Updates

### 2026-11-11
- Polish and performance pass after evaluating the app end to end (screenshots of every screen, whole-app benchmarks). Uncommitted, for review.
- Fixed data loss: edits made in vim mode never set the modified flag, so `:q` discarded unsaved work without warning and the modified marker never showed.
- Fixed search in vim mode: Enter now accepts and returns to Normal (`n`/`N` step, Esc clears highlights); before, the next keys were typed into the search box.
- Startup: editor on screen in ~1ms (was 56-82ms). Replaced syntect in the editor pane with a small hand-written markdown tokenizer and moved preview rendering to a background thread. Jump to end of a 5,200-line file: 0.6ms (was ~200ms). Keystroke + full frame on that file: ~1.2ms.
- UI: new truecolor palettes (ocean no longer uses 16-color ANSI greys/blues), themes with their own background now paint it (solarized-light was unreadable on dark terminals), current-line highlight instead of underline, padded gutter, colored mode pill, modified dot, word count, unified prompts with a real cursor, scrollbars, rebuilt settings panel, empty-preview hint, syntax colors derived from the app theme.
- Editor wraps at word boundaries and word wrap is on by default. Preview: reading column centered in wide panes, bare URLs linked, images marked, clean callouts without emoji.
- Config now lives at `~/.config/mdedit/config.toml` on every platform (legacy macOS location still honored). Added `--version`, `:help`, `:noh`, plain-language file errors, and saves that keep permissions and symlinks. Better 256-color fallback and truecolor detection.
- **Live view** (new default, `--view live`, Ctrl+P cycles): edit inside the rendered document, glow-style. The document is drawn from the preview's rendered rows; the block under the cursor or selection (paragraph, heading, list item, code block, table) is swapped for its raw, highlighted source and re-renders when the cursor leaves. Renderer now reports top-level block spans (source lines <-> rows); `src/live.rs` composes the view and maps clicks. 0.17ms per cursor move and 0.3ms per keystroke on a 5,200-line file.
- Security review: save now creates its temp file exclusively and private (it briefly exposed private files and would write through a planted symlink); crash log moved from the shared temp dir to the user's cache dir; dependencies updated and unused syntect features dropped (`cargo audit`: 4 advisories -> 0); installer verifies `SHA256SUMS` (published by the release workflow from the next release on); CI audits dependencies on push and weekly; Dependabot added. Test proves control characters in a document never reach the terminal.
- README rewritten around the live view; demo GIFs and MP4 re-recorded from the current build with VHS (`demo/demo.tape`, `demo/screenshots/*.tape`, sample doc `demo/sample.md`).
- 55 tests (was 36), fmt/clippy (-D warnings)/test green.

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
