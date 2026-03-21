<!-- GSD:project-start source:PROJECT.md -->
## Project

**mdedit**

A terminal-based markdown editor with live rendered preview. A single Rust TUI binary that lets you edit raw markdown on one side and see it rendered on the other — no browser, no bloated IDE, no vault lock-in. Built for people who live in the terminal and write markdown daily.

**Core Value:** Edit markdown and see the rendered result side-by-side in a single terminal app, with zero external dependencies.

### Constraints

- **Tech stack**: Rust with ratatui — best TUI ecosystem, building blocks exist
- **Binary size**: Keep reasonable (<10MB) — no embedding large assets
- **Startup time**: <100ms — must feel instant
- **Terminal compatibility**: Standard VT100/ANSI — no Kitty/iTerm-only features in v1
- **Platform**: macOS and Linux (Windows is nice-to-have, not blocking)
<!-- GSD:project-end -->

<!-- GSD:stack-start source:research/STACK.md -->
## Technology Stack

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
### Text Editing
| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| tui-textarea | 0.7.x | Editor widget | Purpose-built multi-line text editor for ratatui. Includes undo/redo, line numbers, cursor line highlighting, regex search, text selection, mouse scrolling, yank buffer. Backend-agnostic. Emacs-like keybindings by default (matches our "no vim mode" decision). | HIGH |
### Syntax Highlighting
| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| syntect | 5.3.0 | Syntax highlighting in editor pane | Uses Sublime Text syntax definitions. Mature (production use at multiple companies), pure-Rust regex backend available (no C deps). Cacheable data structures for incremental re-highlighting. Supports markdown syntax plus embedded code blocks. | HIGH |
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
# Create new project
# Core dependencies
### Cargo.toml (expected)
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
<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->
## Conventions

Conventions not yet established. Will populate as patterns emerge during development.
<!-- GSD:conventions-end -->

<!-- GSD:architecture-start source:ARCHITECTURE.md -->
## Architecture

Architecture not yet mapped. Follow existing patterns found in the codebase.
<!-- GSD:architecture-end -->

<!-- GSD:workflow-start source:GSD defaults -->
## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:
- `/gsd:quick` for small fixes, doc updates, and ad-hoc tasks
- `/gsd:debug` for investigation and bug fixing
- `/gsd:execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->



<!-- GSD:profile-start -->
## Developer Profile

> Profile not yet configured. Run `/gsd:profile-user` to generate your developer profile.
> This section is managed by `generate-claude-profile` -- do not edit manually.
<!-- GSD:profile-end -->
