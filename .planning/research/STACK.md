# Technology Stack: v2.0 Additions

**Project:** mdedit v2.0 Power User
**Researched:** 2026-03-22
**Focus:** New dependencies only. Existing stack (ratatui 0.30, crossterm 0.29, ratatui-textarea 0.8, pulldown-cmark 0.13, tui-markdown 0.3.7, syntect 5.3, clap 4, anyhow 1, unicode-width, regex) is validated and unchanged.

## New Dependencies by Feature

### Vim-Style Keybindings (Modal Editing)

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| **None (custom)** | N/A | Modal editing state machine | Build on top of ratatui-textarea 0.8's existing operation methods. The upstream tui-textarea repo has a working vim example that implements a state machine with Normal/Insert/Visual/Operator-Pending modes. Our ratatui-textarea 0.8 fork exposes the same `TextArea::input()` bypass pattern -- call operation methods directly instead. No new crate needed. | HIGH |

**Rationale for not using edtui:** edtui (the vim-inspired ratatui editor widget) requires ratatui ^1.0, which does not exist yet (ratatui is still at 0.30). Even if it were compatible, edtui would replace ratatui-textarea entirely, requiring a rewrite of the custom render path, syntax highlighting integration, search overlays, and selection rendering already built in v1.0. Not worth it.

**Rationale for not using modalkit-ratatui:** Adds a heavy abstraction layer (full command parsing, register management) when we only need Normal/Insert/Visual modes with ~30 keybindings. Overkill for a markdown editor.

**Implementation approach:**
- Create a `VimState` enum: `Normal`, `Insert`, `Visual`, `OperatorPending(Operator)`
- Map key events to state transitions and ratatui-textarea operations
- Change cursor shape per mode (block/bar/underline) via crossterm's `SetCursorStyle`
- Display current mode in status bar

### Configurable Color Themes (TOML Config)

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| serde | 1.x | Serialization framework | Required by `toml` for deserializing config structs. Industry standard, 1B+ downloads. Use `features = ["derive"]` for `#[derive(Deserialize)]`. | HIGH |
| toml | 0.9.x | TOML parser/deserializer | Latest stable (0.9.11 as of 2026-01). Supports TOML 1.1 spec. Simple API: `toml::from_str()` to deserialize into typed structs. No need for `toml_edit` (we read config, never write it). | HIGH |
| dirs | 6.x | Platform config directory | Returns `~/.config/` on Linux (XDG), `~/Library/Application Support/` on macOS. 150M+ downloads, actively maintained. Use `dirs::config_dir()` to find `mdedit/config.toml`. | HIGH |

**Config file location:** `$XDG_CONFIG_HOME/mdedit/config.toml` (Linux) or `~/Library/Application Support/mdedit/config.toml` (macOS).

**Theme struct shape:**
```toml
[theme]
name = "dark"  # or path to custom theme

[theme.colors]
editor_bg = "#1e1e2e"
editor_fg = "#cdd6f4"
preview_bg = "#1e1e2e"
heading = "#89b4fa"
# ... etc
```

**Why not the `config` crate:** The `config` crate adds layered configuration (env vars, multiple files, etc.) which is unnecessary complexity. A single TOML file with `serde` + `toml` is simpler, faster to compile, and sufficient for our needs.

**Why not `figment`:** Same over-engineering problem. We have one file, one format.

### Browser Companion (Local HTTP Server)

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| tiny_http | 0.12.x | Lightweight HTTP server | Synchronous, no async runtime needed (no tokio). Minimal API: create server, handle requests. Serves our single HTML page with rendered markdown. 15M+ downloads, battle-tested. Adds ~200KB to binary. | HIGH |

**Rationale for tiny_http over axum/actix/warp:** All major web frameworks require tokio (async runtime). We explicitly decided against async in v1 -- adding tokio for a localhost-only file server would double compile time and binary size. tiny_http is synchronous, runs in a spawned thread, and does exactly what we need.

**Implementation approach:**
- Spawn a `std::thread` running tiny_http on `127.0.0.1:0` (OS-assigned port)
- Serve a single HTML page: GitHub CSS + rendered markdown content
- Use pulldown-cmark's built-in `html::push_html()` to render markdown to HTML (already in dependency tree, zero new crates)
- Embed [sindresorhus/github-markdown-css](https://github.com/sindresorhus/github-markdown-css) (~15KB) as a const string via `include_str!()` at compile time
- On each editor change, re-render HTML and update shared state (server thread reads latest via `Arc<Mutex<String>>`)
- Print the URL to stderr on startup: `Browser companion: http://127.0.0.1:{port}`
- Auto-refresh via `<meta http-equiv="refresh" content="1">` or simple JS polling -- avoids needing SSE/WebSocket crates

### Clipboard Integration (OSC 52)

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| **crossterm (existing, add feature)** | 0.29.x | OSC 52 clipboard | crossterm 0.29 added `crossterm::clipboard` behind the `osc52` feature flag. We already depend on crossterm 0.29 -- just enable the feature. Zero new crates. Works over SSH. | HIGH |

**What changes in Cargo.toml:**
```toml
crossterm = { version = "0.29", features = ["osc52"] }
```

**Limitation:** OSC 52 is write-only (copy to clipboard). Reading/pasting from clipboard is not supported via OSC 52. For paste, rely on the terminal's native paste (Ctrl+Shift+V / Cmd+V), which crossterm already receives as regular key input in raw mode.

**Platform-native fallback:** NOT adding `arboard` or `copypasta` crates. These require system libraries (X11/Wayland on Linux, pbcopy on macOS) and add platform-specific build complexity. OSC 52 works everywhere the terminal supports it (most modern terminals: iTerm2, Alacritty, WezTerm, kitty, Windows Terminal, tmux with `set-clipboard on`). If a terminal doesn't support OSC 52, the copy command simply does nothing -- acceptable degradation.

### Mouse Support

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| **crossterm (existing)** | 0.29.x | Mouse events | Already in dependency tree. Use `crossterm::event::EnableMouseCapture` at startup and handle `MouseEvent` variants (scroll, click, drag) in the event loop. ratatui-textarea 0.8 already handles mouse events when passed through `TextArea::input()`. | HIGH |

**No new dependencies.** Mouse support is entirely within crossterm's existing API.

**Implementation:**
- Enable mouse capture: `execute!(stdout, EnableMouseCapture)` at init
- Disable at cleanup: `execute!(stdout, DisableMouseCapture)`
- Handle `Event::Mouse(MouseEvent { kind, column, row, .. })` in event loop
- Map clicks to pane focus (editor vs preview)
- Map scroll wheel to scroll in the focused pane
- ratatui-textarea handles mouse clicks for cursor positioning automatically

### Adjustable Split Ratio

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| **None** | N/A | Split ratio | Pure layout logic using ratatui's `Layout::constraints()`. Change `Constraint::Percentage(50)` to `Constraint::Percentage(ratio)` where `ratio` is adjustable. No crate needed. | HIGH |

**Implementation:** Store a `split_ratio: u16` (0-100) in app state. Adjust with keybinding (e.g., `Ctrl+Left`/`Ctrl+Right` in Normal mode, or configurable). Persist in config file.

### WYSIWYG Terminal Editing

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| **None (custom)** | N/A | WYSIWYG editing | This is a novel rendering mode, not a library problem. Use tui-markdown's rendered output as the display, but map cursor positions and edits back to the raw markdown source. No off-the-shelf solution exists for this in any TUI ecosystem. | MEDIUM |

**Why MEDIUM confidence:** WYSIWYG terminal editing is the hardest feature in v2.0. The core challenge is bidirectional mapping between rendered markdown positions and source positions. tui-markdown renders markdown to ratatui `Text` widgets but doesn't provide source-position mapping. This will require:
1. A custom rendering pass that tracks source spans alongside rendered output
2. Cursor translation logic (rendered position <-> source position)
3. Inline editing that modifies raw markdown but displays rendered result

**No new crates help here.** This is application-level logic.

## Summary: Cargo.toml Changes

### New Dependencies
```toml
# Config file support
serde = { version = "1", features = ["derive"] }
toml = "0.9"
dirs = "6"

# Browser companion
tiny_http = "0.12"
```

### Modified Dependencies
```toml
# Add osc52 feature for clipboard
crossterm = { version = "0.29", features = ["osc52"] }
```

### Full Expected Cargo.toml
```toml
[package]
name = "mdedit"
version = "0.2.0"
edition = "2021"
rust-version = "1.75"

[dependencies]
# Existing (unchanged)
ratatui = { version = "0.30", features = ["crossterm"] }
ratatui-textarea = { version = "0.8", features = ["crossterm", "search"] }
pulldown-cmark = { version = "0.13", features = ["simd"] }
tui-markdown = "0.3.7"
syntect = { version = "5.3", default-features = false, features = ["default-fancy"] }
clap = { version = "4", features = ["derive"] }
anyhow = "1"
unicode-width = "0.2"
regex = "1"

# Modified (added osc52 feature)
crossterm = { version = "0.29", features = ["osc52"] }

# New for v2.0
serde = { version = "1", features = ["derive"] }
toml = "0.9"
dirs = "6"
tiny_http = "0.12"

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
strip = true
```

## What NOT to Add

| Technology | Why Not |
|------------|---------|
| tokio / async-std | No async needed. tiny_http is synchronous. Adding async runtime for a localhost server is absurd bloat. |
| edtui | Requires ratatui ^1.0 (doesn't exist yet, ratatui is at 0.30). Would replace our entire editing layer. |
| modalkit-ratatui | Heavy vim emulation framework. Overkill for ~30 keybindings in a markdown editor. |
| arboard / copypasta | Platform-native clipboard adds C dependencies and build complexity. OSC 52 via crossterm is sufficient. |
| config (crate) | Layered config system is overkill. We have one TOML file. |
| figment | Another config framework. Same over-engineering problem. |
| toml_edit | We read config, never write it. toml (deserialize-only) is simpler. |
| notify (file watcher) | Was considered for browser companion auto-reload. Not needed -- we control when content changes (on every keystroke) and can push updates directly. |
| warp / actix-web / axum | Full web frameworks requiring tokio async. tiny_http serves one page on localhost. |
| tungstenite / tokio-tungstenite | WebSocket for browser live-reload. Simple polling or meta-refresh is sufficient for a local companion. |

## Binary Size Impact

| Addition | Estimated Size | Notes |
|----------|---------------|-------|
| serde + toml | ~300-400KB | serde is heavy but well-optimized with LTO |
| dirs | ~20KB | Tiny crate, platform API calls only |
| tiny_http | ~200KB | Minimal HTTP implementation |
| github-markdown-css (embedded) | ~15KB | Compile-time `include_str!()` |
| OSC 52 (crossterm feature) | ~5KB | Just base64 encoding added |
| **Total new** | **~550-650KB** | Well within 10MB constraint |

Current binary (release, stripped, LTO): ~3-5MB. With additions: ~4-6MB. Comfortable margin.

## Startup Time Impact

| Addition | Impact | Notes |
|----------|--------|-------|
| Config loading | ~1-2ms | Read one small TOML file from disk |
| tiny_http server spawn | ~1ms | Thread spawn + bind, non-blocking to main thread |
| Mouse capture enable | <1ms | Single escape sequence |
| OSC 52 init | 0ms | Nothing to initialize, used on-demand |
| **Total added** | **~3ms** | Well within 100ms constraint |

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Vim keybindings | Custom state machine on ratatui-textarea | edtui widget | edtui requires ratatui ^1.0 (unreleased), would replace entire editing layer |
| Vim keybindings | Custom state machine | modalkit-ratatui | Heavy framework, overkill for markdown editor |
| Config format | TOML (toml 0.9) | YAML, JSON, RON | TOML is Rust ecosystem standard, human-readable, Cargo.toml uses it |
| Config crate | serde + toml | config, figment | One file, one format -- layered config is overkill |
| Config directory | dirs 6.x | directories, hardcoded | dirs is the standard, handles XDG/macOS correctly |
| HTTP server | tiny_http 0.12 | axum, warp, actix | No async needed for localhost single-page server |
| Clipboard | crossterm OSC 52 feature | arboard, copypasta | OSC 52 works over SSH, no C dependencies, already in our dep tree |
| Browser live-reload | meta-refresh / JS polling | WebSocket (tungstenite) | Adds async complexity for marginal UX improvement |

## Integration Points with Existing Stack

| New Feature | Integrates With | How |
|-------------|----------------|-----|
| Vim keybindings | ratatui-textarea 0.8 | Bypass `input()`, call operation methods directly per vim state |
| Vim cursor style | crossterm 0.29 | `SetCursorStyle::BlinkingBlock` (normal), `BlinkingBar` (insert) |
| Vim mode display | status_bar.rs | Show `NORMAL`, `INSERT`, `VISUAL` in status bar |
| Config themes | syntect 5.3 | Map theme colors to syntect `Style` values for code highlighting |
| Config themes | ratatui 0.30 | Apply `Color::Rgb(r,g,b)` from config to all widget styles |
| Browser companion | pulldown-cmark 0.13 | Use `html::push_html()` (already a dependency) to render HTML |
| Browser companion | app.rs | Spawn server thread, share content via `Arc<Mutex<String>>` |
| Clipboard (copy) | crossterm 0.29 + osc52 | `clipboard::SetClipboardContent` with selected text from ratatui-textarea |
| Mouse events | crossterm 0.29 | `EnableMouseCapture` + handle `Event::Mouse` in event loop |
| Mouse clicks | ratatui-textarea 0.8 | Pass mouse events to `TextArea::input()` for cursor positioning |
| Split ratio | ratatui 0.30 | Adjust `Layout::constraints()` percentages dynamically |
| Split ratio | config (new) | Persist preferred ratio in TOML config |

## Risk Assessment for New Stack

| Component | Risk | Mitigation |
|-----------|------|------------|
| Custom vim state machine | Incomplete vim emulation frustrates power users | Scope to essential motions only (hjkl, w/b/e, d/c/y, i/a/o, v, gg/G, /, :w/:q). Document supported commands. |
| WYSIWYG mode (custom) | Source-position mapping is hard, may produce bugs | Ship as `--wysiwyg` flag (opt-in). Mark as experimental. Iterate based on feedback. |
| tiny_http thread safety | Shared state between TUI thread and HTTP thread | Use `Arc<Mutex<String>>` for HTML content. Lock contention is minimal (write on keystroke, read on HTTP request). |
| OSC 52 terminal support | Some terminals don't support OSC 52 | Degrade gracefully (copy does nothing). Document supported terminals. |
| toml 0.9 (relatively new) | API may differ from widely-documented 0.8 examples | Pin version. Core API (`from_str`) is stable between 0.8 and 0.9. |

## Sources

- [ratatui-textarea vim example](https://github.com/rhysd/tui-textarea) - Modal editing state machine pattern
- [ratatui-textarea (ratatui org fork)](https://github.com/ratatui/ratatui-textarea) - v0.8, ratatui 0.30 compatible
- [edtui](https://github.com/preiter93/edtui) - Requires ratatui ^1.0, not compatible with 0.30
- [modalkit-ratatui](https://docs.rs/modalkit-ratatui/latest/modalkit_ratatui/) - Heavy vim framework, considered and rejected
- [crossterm 0.29 OSC 52](https://docs.rs/crate/crossterm/latest) - Clipboard feature flag added in 0.29
- [crossterm mouse events](https://ratatui.rs/concepts/event-handling/) - EnableMouseCapture documentation
- [tiny_http](https://github.com/tiny-http/tiny-http) - v0.12, synchronous HTTP server
- [toml crate](https://docs.rs/toml) - v0.9.11, TOML 1.1 support
- [dirs crate](https://github.com/xdg-rs/dirs) - Platform config directories, XDG compliant
- [serde derive](https://serde.rs/derive.html) - Derive-based deserialization
- [github-markdown-css](https://github.com/sindresorhus/github-markdown-css) - Standalone GitHub CSS for browser companion
- [pulldown-cmark HTML rendering](https://docs.rs/pulldown-cmark/latest/pulldown_cmark/html/) - Built-in HTML output, already in deps
