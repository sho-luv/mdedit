# Technology Stack

**Project:** mdedit - Terminal Markdown Editor with Live Preview
**Researched:** 2026-03-21

## Recommended Stack

### Core Framework

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| Rust (edition 2021) | 1.75+ | Language | Single binary, no runtime deps, fast startup, strong TUI ecosystem | HIGH |
| ratatui | 0.30.x | TUI framework | De facto standard for Rust TUIs. Modular workspace since 0.30, no_std support, active development. Maintained by joshka (also maintains tui-markdown). 73M+ downloads on crossterm alone. | HIGH |
| crossterm | 0.29.x | Terminal backend | Pure Rust, cross-platform (macOS/Linux/Windows), no C dependencies. Default backend for ratatui 0.30. Works over SSH. | HIGH |

### Markdown Parsing

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| pulldown-cmark | 0.13.x | Markdown parser | The standard Rust CommonMark parser. Pull-based (streaming/low-memory), supports GFM extensions (tables, task lists, strikethrough, footnotes). Used by tui-markdown under the hood. No real competition in Rust for this role. | HIGH |

### Markdown Terminal Rendering

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| tui-markdown | 0.3.7 | Markdown-to-ratatui Text conversion | Converts pulldown-cmark events into ratatui `Text` widgets. Supports headings, paragraphs, block quotes (nested), bold/italic/strikethrough, ordered/unordered lists, code blocks, tables, task lists, links, rules, footnotes. Maintained by joshka (ratatui core maintainer). | MEDIUM |

**Important note on tui-markdown:** This crate is explicitly labeled "experimental / Proof of Concept." It will likely need patching or forking for production quality. Plan to contribute upstream or maintain a fork. The fact that the maintainer is the same person who leads ratatui is a good sign for long-term viability, but set expectations accordingly.

**Alternative considered:** `the-other-tui-markdown` -- also uses pulldown-cmark, maps to ratatui Spans/Lines. Less ecosystem adoption. Stick with tui-markdown since it has more users (16 dependent crates, ~11K monthly downloads) and is closer to the ratatui core team.

### Text Editing

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| tui-textarea | 0.7.x | Editor widget | Purpose-built multi-line text editor for ratatui. Includes undo/redo, line numbers, cursor line highlighting, regex search, text selection, mouse scrolling, yank buffer. Backend-agnostic. Emacs-like keybindings by default (matches our "no vim mode" decision). | HIGH |

**Why not build from scratch:** tui-textarea handles the hard parts -- cursor management, scrolling, selection, undo/redo history. Building this from scratch is months of work for a v1. Use the widget and customize as needed.

**Why not ropey:** Ropey (1.6.1 stable, 2.0 in beta) is a rope data structure for large text buffers. tui-textarea uses a simpler `Vec<String>` internally. For v1 targeting normal markdown files (not multi-GB files), tui-textarea's approach is fine. If we hit performance issues with very large files later, ropey is the upgrade path.

### Syntax Highlighting

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| syntect | 5.3.0 | Syntax highlighting in editor pane | Uses Sublime Text syntax definitions. Mature (production use at multiple companies), pure-Rust regex backend available (no C deps). Cacheable data structures for incremental re-highlighting. Supports markdown syntax plus embedded code blocks. | HIGH |

**Why not tree-sitter-highlight:** tree-sitter provides better structural parsing but requires grammar binaries per language, increasing binary size significantly. For highlighting markdown in an editor (not a full IDE), syntect is simpler and sufficient. tree-sitter would be overkill for v1.

**Why not tree-painter / syntastica:** Newer, less battle-tested. syntect is the proven choice used by Typst, Yazi, bat, delta, and many other production Rust tools.

### Supporting Libraries

| Library | Version | Purpose | When to Use | Confidence |
|---------|---------|---------|-------------|------------|
| crossterm | 0.29.x | Raw terminal I/O, event handling | Always (bundled via ratatui feature) | HIGH |
| unicode-width | 2.x | Correct character width calculation | Always (CJK characters, emoji in markdown) | HIGH |
| clap | 4.x | CLI argument parsing | Parsing `mdedit <file>` args | HIGH |
| anyhow | 1.x | Error handling | Application-level error handling | HIGH |

## What NOT to Use

| Technology | Why Not |
|------------|---------|
| tui-rs | Abandoned predecessor to ratatui. Do not use. |
| termion | Linux-only backend. crossterm is cross-platform. |
| termwiz | Wez's terminal library -- good but less ecosystem support than crossterm. |
| cursive | Alternative TUI framework. Less widget ecosystem than ratatui, smaller community. |
| comrak | Alternative markdown parser (GFM-focused). Heavier than pulldown-cmark, uses C bindings for speed. pulldown-cmark is pure Rust and fast enough. |
| tree-sitter | Overkill for syntax highlighting in a markdown editor. Adds grammar binaries, increases binary size. |
| tokio / async | No need for async in a single-file TUI editor. crossterm's event polling is synchronous and sufficient. Async adds complexity for zero benefit here. |
| serde / config crates | No config file in v1 (per project constraints). Add later if needed. |
| ratatui-textarea | Fork of tui-textarea. Less established, unclear maintenance. Stick with the original. |

## Architecture-Relevant Stack Notes

### Binary Size
- ratatui + crossterm + pulldown-cmark + tui-textarea: expect ~3-5MB release binary
- syntect with default themes: adds ~1-2MB (theme/syntax definition data)
- Total well under the 10MB constraint
- Use `syntect`'s `default-onig` feature OFF, use `default-fancy` for pure-Rust regex

### Startup Time
- All crates are native Rust, no JIT/interpreter
- syntect loads syntax definitions lazily -- do NOT eagerly load all syntaxes
- Expect <50ms startup easily, well under 100ms constraint

### Incremental Rendering Strategy
- pulldown-cmark is streaming (iterator-based) -- re-parse on every keystroke is fast for typical markdown files
- syntect caches parse states per line -- only re-highlight changed lines
- tui-textarea provides change callbacks to detect what changed

### Terminal Compatibility
- crossterm uses standard VT100/ANSI sequences
- No Kitty graphics protocol, no sixel, no iTerm2 inline images
- Works over SSH by default

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| TUI Framework | ratatui 0.30 | cursive, tui-realm | ratatui has the largest ecosystem, most widgets, most active development |
| Terminal Backend | crossterm | termion, termwiz | crossterm is cross-platform, pure Rust, default for ratatui |
| Markdown Parser | pulldown-cmark | comrak, markdown-rs | pulldown-cmark is pure Rust, streaming, CommonMark compliant, standard choice |
| Markdown Render | tui-markdown | the-other-tui-markdown, custom | tui-markdown is closest to ratatui core team, most users |
| Text Editor Widget | tui-textarea | custom (ropey-based) | tui-textarea has undo/redo/selection/line-numbers built in. Custom is months of work. |
| Syntax Highlighting | syntect | tree-sitter-highlight, syntastica | syntect is proven, pure Rust option available, right complexity level |
| CLI Args | clap | argh, lexopt | clap is the standard, well-documented, derive macro support |

## Installation

```bash
# Create new project
cargo init mdedit

# Core dependencies
cargo add ratatui --features crossterm
cargo add crossterm
cargo add pulldown-cmark --features simd
cargo add tui-textarea --features crossterm
cargo add tui-markdown
cargo add syntect --no-default-features --features default-fancy,html
cargo add unicode-width
cargo add clap --features derive
cargo add anyhow
```

### Cargo.toml (expected)

```toml
[package]
name = "mdedit"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"

[dependencies]
ratatui = { version = "0.30", features = ["crossterm"] }
crossterm = "0.29"
pulldown-cmark = { version = "0.13", features = ["simd"] }
tui-textarea = { version = "0.7", features = ["crossterm"] }
tui-markdown = "0.3"
syntect = { version = "5.3", default-features = false, features = ["default-fancy"] }
unicode-width = "2"
clap = { version = "4", features = ["derive"] }
anyhow = "1"

[profile.release]
opt-level = "z"     # Optimize for binary size
lto = true          # Link-time optimization
codegen-units = 1   # Single codegen unit for better optimization
strip = true        # Strip debug symbols
```

## Risk Assessment

| Component | Risk | Mitigation |
|-----------|------|------------|
| tui-markdown (experimental) | May not render all markdown correctly, API may change | Fork if needed, contribute fixes upstream. Rendering is the core differentiator -- budget time for custom work here. |
| tui-textarea (pre-1.0) | API may change between minor versions | Pin exact version. Widget is mature enough for v1 features. |
| syntect theme integration | Terminal color support varies | Use adaptive themes (detect 256-color vs truecolor). Provide a sensible default that works on most terminals. |
| Scroll sync (editor <-> preview) | No off-the-shelf solution | Must implement custom. Map source line numbers to rendered output positions. This is novel work. |

## Sources

- [ratatui GitHub](https://github.com/ratatui/ratatui) - v0.30.0, modular workspace
- [ratatui v0.30 highlights](https://ratatui.rs/highlights/v030/) - no_std, ratatui::run(), modular crates
- [crossterm GitHub](https://github.com/crossterm-rs/crossterm) - v0.29.0
- [pulldown-cmark GitHub](https://github.com/pulldown-cmark/pulldown-cmark) - v0.13.x, CommonMark 0.31
- [tui-textarea GitHub](https://github.com/rhysd/tui-textarea) - v0.7.0
- [tui-markdown GitHub](https://github.com/joshka/tui-markdown) - v0.3.7, experimental POC
- [syntect GitHub](https://github.com/trishume/syntect) - v5.3.0
- [ropey GitHub](https://github.com/cessen/ropey) - v1.6.1 (future upgrade path)
- [the-other-tui-markdown docs](https://docs.rs/the-other-tui-markdown/latest/the_other_tui_markdown/) - alternative considered
