# Project Research Summary

**Project:** mdedit — Terminal Markdown Editor with Live Preview
**Domain:** Rust TUI application, single-binary terminal tool
**Researched:** 2026-03-21
**Confidence:** HIGH

## Executive Summary

mdedit occupies a genuinely vacant niche: no single compiled binary currently exists that combines markdown editing with live preview. The closest competitors (glow, md-tui) are view-only; the tools that do offer edit+preview (Splitmark, MarkLn) require Node.js or Python runtimes. The Rust TUI ecosystem in 2026 is mature enough to build this cleanly — ratatui 0.30 is the de facto standard with active maintenance, crossterm provides cross-platform terminal support, and tui-textarea handles the hardest parts of text editing out of the box. The recommended architecture is ratatui's Component pattern: each UI pane (Editor, Preview, StatusBar) owns its state and communicates through an Action enum rather than shared mutable references.

The biggest technical risk is tui-markdown, which is explicitly labeled experimental/PoC by its author. The mitigation is to wrap it behind a `MarkdownRenderer` trait from day one, start with tui-markdown for the MVP, and replace it with a custom pulldown-cmark renderer when limitations surface. A second significant risk is scroll synchronization between editor and preview — it looks simple but every mature markdown editor has iterated through multiple approaches. The recommendation is to ship v1 without scroll sync or with a basic proportional approximation, and treat element-level sync as a Phase 3 enhancement.

The build order is well-defined by the dependency graph: terminal infrastructure first, then editor, then file I/O, then preview. Every milestone should produce something runnable. The single-binary zero-dependency story is the #1 differentiator and requires no extra work — it is inherent in choosing Rust.

## Key Findings

### Recommended Stack

The stack is all-Rust with no C dependencies (using syntect's `default-fancy` pure-Rust regex backend). The core dependency chain is ratatui (0.30) + crossterm (0.29) + tui-textarea (0.7) + pulldown-cmark (0.13) + tui-markdown (0.3.7) + syntect (5.3). Expected release binary size is 4–7MB, well under any practical constraint. Startup time will be under 50ms with lazy syntect loading.

**Core technologies:**
- **Rust (edition 2021, 1.75+):** Language — single binary, no runtime, strong TUI ecosystem
- **ratatui 0.30:** TUI framework — de facto standard, 73M+ crossterm downloads, immediate-mode rendering with built-in diff
- **crossterm 0.29:** Terminal backend — pure Rust, cross-platform (macOS/Linux/Windows), SSH-compatible
- **tui-textarea 0.7:** Editor widget — undo/redo, line numbers, cursor highlighting, selection, search built in
- **pulldown-cmark 0.13:** Markdown parser — CommonMark + GFM, streaming/low-memory, source offset mapping via `into_offset_iter()`
- **tui-markdown 0.3.7:** Markdown-to-ratatui bridge — experimental PoC; use as starting point, plan to fork or replace
- **syntect 5.3:** Syntax highlighting — proven in production (bat, delta, Typst), pure-Rust regex option
- **unicode-width 2.x:** Display width — required for correct CJK/emoji column calculations
- **clap 4.x:** CLI args — standard Rust CLI argument parser

Do NOT use: tokio/async (no async operations exist), tui-rs (abandoned), termion (Linux-only), comrak (C bindings), tree-sitter (overkill for highlighting).

### Expected Features

The competitive landscape confirms a clear gap: mdedit is the first Rust single-binary edit+preview tool. Six tools were analyzed; the feature bar is set by Splitmark (Node.js) and MarkLn (Python) for edit+preview, and glow/md-tui (Rust) for view-only polish.

**Must have (table stakes):**
- Open file from CLI argument (`mdedit file.md`)
- Basic text editing: insert, delete, newline, cursor movement including word-jump
- Undo/Redo (Ctrl+Z / Ctrl+Y) — users panic without this
- Save file (Ctrl+S) with confirmation in status bar
- Side-by-side editor + preview layout (left=editor, right=preview)
- Live preview updating as you type (<50ms render after keystroke)
- Markdown syntax highlighting in editor pane
- Rendered preview: headings, bold, italic, code blocks, links, lists, blockquotes, tables, horizontal rules
- Code block syntax highlighting in preview (bash, python, rust, js, json, go, ts minimum)
- Line numbers in editor
- Status bar: filename, line:col, modified indicator, keybinding hints
- Exit with unsaved changes warning
- Responsive to terminal resize
- Fast startup (<100ms)

**Should have (competitive, Phase 2):**
- Scroll sync between editor and preview — biggest UX "wow" after split view works
- Layout toggle: editor-only / preview-only / split (single hotkey cycle)
- Search in editor (Ctrl+F)
- Text selection with Shift+arrows
- Indent/outdent with Tab/Shift+Tab

**Defer to v2+:**
- Table of contents sidebar — useful but significant UI work
- Adjustable split ratio — 50/50 default is sufficient for v1
- Stacked (top/bottom) layout option
- Markdown snippet/tag insertion
- File browser/picker

**Explicit anti-features (never build):**
- Vim/emacs keybinding modes — target "I just want to edit markdown" users, not Vim users
- Config file — ship sensible defaults; add in v2 if demanded
- Image rendering — terminal image protocols are too fragmented
- Plugin system — premature abstraction
- Clipboard integration — too fragmented across SSH/tmux/platforms
- Nerd Font requirement — must work in any monospace font

### Architecture Approach

Use ratatui's Component Architecture pattern. Each component (Editor, Preview, StatusBar) owns its state, handles its own events, and renders itself. Components communicate through an Action enum — never by holding references to each other. The App struct is the root coordinator that routes events and mediates state sharing. This avoids shared mutable state, which causes runtime borrow panics in Rust.

**Major components:**
1. **App** — Root coordinator; routes events to focused component; owns layout mode, quit state, file path
2. **Editor** — Wraps tui-textarea; owns text buffer, cursor state, modified flag; exposes raw markdown content
3. **Preview** — Renders markdown to ratatui Text via MarkdownRenderer; owns scroll position and render cache
4. **StatusBar** — Pure display component; stateless; receives state snapshot from App each frame
5. **LayoutManager** — Pure function; calculates pane Rects from current LayoutMode (Split/EditorOnly/PreviewOnly)
6. **MarkdownRenderer** (trait + impl) — pulldown-cmark → ratatui Text conversion; abstracted so tui-markdown can be swapped
7. **FileIO** — Utility module; synchronous atomic writes (write to .tmp, rename); not a component

The event loop is synchronous (no tokio). `crossterm::event::poll()` with a 33ms timeout (~30fps). Markdown re-parsing uses a dirty flag: only re-parse after content changes, with 50-100ms debounce for large documents.

Suggested build order from ARCHITECTURE.md: (1) terminal infrastructure, (2) app skeleton + layout, (3) editor component, (4) file I/O, (5) status bar, (6) preview with tui-markdown, (7) scroll sync, (8) editor syntax highlighting, (9) layout mode toggling.

### Critical Pitfalls

1. **Terminal state not restored on panic** — Install `std::panic::set_hook()` that calls `disable_raw_mode()` + `LeaveAlternateScreen` BEFORE the first feature line is written. Skipping this leaves the user's terminal broken on any crash.

2. **Unicode/grapheme handling** — Use `unicode-segmentation` for cursor movement, `unicode-width` for display width. Never index strings by byte or char for user-facing operations. Retrofitting grapheme-awareness touches everything and is very expensive to fix later.

3. **tui-markdown is experimental** — Wrap all preview rendering behind a `MarkdownRenderer` trait from day one. Start with tui-markdown, but expect to fork or replace it. The pulldown-cmark parser itself is battle-tested; the tui-markdown conversion layer is the fragile part.

4. **Re-parsing entire document on every keystroke** — Implement dirty flag + debounce (50-100ms) from the start of preview work, not as a fix after noticing lag. pulldown-cmark is fast but combined parse + widget construction + layout can exceed 16ms frame budget on documents >20KB.

5. **Scroll sync is deceptively hard** — Do not attempt pixel-perfect sync in v1. Proportional sync (editor scroll % maps to preview scroll %) is acceptable for a first pass. Element-level sync using pulldown-cmark's `into_offset_iter()` is the correct long-term approach. Even VS Code's implementation is imperfect.

## Implications for Roadmap

Based on combined research, the dependency graph and pitfall phase mapping strongly suggest a 3-phase structure:

### Phase 1: Foundation — Functional Terminal Editor

**Rationale:** All UI components depend on the terminal infrastructure. The editor must work before the preview can be wired up. File I/O and status bar provide enough polish to make Phase 1 a genuinely usable (if plain) text editor. Critically, the panic hook and unicode handling MUST be correct from the start — retrofitting either is extremely costly.

**Delivers:** A working terminal text editor for markdown files. No preview yet, but fully usable as a basic editor. Single binary, opens files from CLI, saves files atomically, shows status bar.

**Addresses (from FEATURES.md):** Open file from CLI, basic text editing, undo/redo, save with Ctrl+S, line numbers, status bar, modified indicator, exit with unsaved changes warning, fast startup.

**Avoids (from PITFALLS.md):** Terminal crash/raw mode pitfall (panic hook first), unicode/grapheme pitfall (unicode-segmentation from day one), display width pitfall (unicode-width in all layout code), tui-textarea abstraction (wrap it, never import types directly), buffer data structure abstraction (clean trait boundary even if Vec<String> used initially).

**Must do in this phase:**
- Install panic hook before any other code
- Wrap tui-textarea behind an Editor abstraction
- Use unicode-width for all display calculations
- Implement synchronous event loop with crossterm::event::poll()
- Atomic file save (write .tmp, rename)

### Phase 2: Live Preview — Core Differentiator

**Rationale:** The preview is the feature that makes mdedit distinct from every other terminal markdown editor. It depends on Phase 1 being stable. Start with tui-markdown (fast to integrate), but abstract behind MarkdownRenderer trait from the first line. Debounced parsing must be implemented from the start of this phase, not added later.

**Delivers:** The defining mdedit experience — side-by-side editor and live-updating rendered preview. This is what ships as v1.

**Addresses (from FEATURES.md):** Side-by-side layout, live preview updating, rendered preview of all common markdown elements, code block syntax highlighting in preview, markdown syntax highlighting in editor, responsive terminal resize, layout toggle (editor/preview/split).

**Uses (from STACK.md):** tui-markdown 0.3.7 (initial impl), pulldown-cmark 0.13 (underlying parser), syntect 5.3 (code block highlighting), ratatui Layout with Constraint::Percentage for split.

**Implements (from ARCHITECTURE.md):** Preview component, MarkdownRenderer trait, dirty flag pattern, debounced re-parsing, LayoutManager with LayoutMode enum, Action::ContentChanged flow.

**Avoids (from PITFALLS.md):** Re-parsing on every keystroke (debounce from day one), tui-markdown tight coupling (trait abstraction), hardcoded terminal width assumptions.

### Phase 3: Polish — Scroll Sync and UX Completeness

**Rationale:** Scroll sync is the biggest UX "wow" but is explicitly flagged as deceptively hard. Defer until Phase 2 is stable and real users have validated the core experience. Search, text selection, and indent/outdent complete the editing experience for power users.

**Delivers:** A polished, competitive tool that compares favorably to Splitmark and MarkLn while beating both on the single-binary distribution story.

**Addresses (from FEATURES.md):** Scroll sync editor↔preview, search (Ctrl+F), text selection (Shift+arrows), indent/outdent (Tab/Shift+Tab), stacked layout option, narrow terminal fallback (<80 cols auto-switch).

**Avoids (from PITFALLS.md):** Scroll sync jumping/disorientation (use proportional first, upgrade to element-level with `into_offset_iter()` if needed), preview flashing (debounce + ratatui diff).

**Custom work required:** Scroll sync has no off-the-shelf solution. Must implement custom mapping from editor cursor line to preview scroll offset. Budget 1-2 days of iteration.

### Phase Ordering Rationale

- **Infrastructure before features:** ratatui's Component Architecture requires the event loop, terminal setup, and App skeleton before any component can be built. This is not optional scaffolding — it is the load-bearing structure.
- **Editor before preview:** The preview renders content from the editor. The editor must be stable before the preview can consume it.
- **Pitfalls drive Phase 1 scope:** Unicode handling and panic cleanup are Phase 1 requirements, not polish. The recovery cost for unicode (HIGH) and the user-hostility of broken terminals make them non-negotiable.
- **Abstraction layers protect against experimental deps:** Both tui-textarea and tui-markdown are pre-1.0 or experimental. Wrapping them behind traits in Phase 1 means Phase 2 and 3 can swap implementations without architectural changes.
- **Scroll sync last:** Researched tools all confirm scroll sync requires iteration. Shipping without it is acceptable; shipping it broken damages user trust.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 2 (Preview):** tui-markdown's exact API surface and known limitations should be mapped before the phase starts. The MarkdownRenderer abstraction boundary depends on knowing what tui-markdown can and cannot do. Recommend a spike/prototype before committing to the interface.
- **Phase 3 (Scroll Sync):** The source-map approach using `pulldown-cmark`'s `into_offset_iter()` needs a focused research/prototype spike. The mapping from source byte offsets to ratatui text line numbers is non-trivial.

Phases with standard patterns (skip research-phase):
- **Phase 1 (Foundation):** ratatui Component Architecture is thoroughly documented in official ratatui docs. The event loop, layout, and panic hook patterns are all well-established. No novel work here.
- **Phase 2 (Syntax Highlighting):** syntect integration is well-documented with production examples (bat, delta). Standard patterns apply.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All primary crates verified from official GitHub repos and crates.io. Version numbers confirmed. The one exception is tui-markdown (MEDIUM) due to experimental status. |
| Features | HIGH | Competitive analysis covered 6 tools across 3 categories. Table stakes are well-validated. The "no single binary edit+preview tool exists" claim is the core differentiator. |
| Architecture | HIGH | Based entirely on official ratatui documentation for Component Architecture, Layout system, and event loop design. The scroll sync strategy is MEDIUM confidence based on community sources. |
| Pitfalls | HIGH | Multiple sources corroborate each pitfall. Unicode handling and terminal state pitfalls are well-documented Rust TUI gotchas. tui-markdown experimental status is confirmed by the author. |

**Overall confidence:** HIGH

### Gaps to Address

- **tui-markdown API coverage:** Research confirms it handles common elements (headings, bold, lists, code, tables) but the exact limitations with complex nested structures are not fully mapped. Validate against a diverse markdown test suite early in Phase 2.

- **Scroll sync accuracy:** The proportional approach is confirmed as the starting point, but how well it performs in practice depends on the ratio of source lines to rendered lines in typical markdown documents. May need element-level sync sooner than Phase 3 if proportional is too jarring.

- **tui-textarea undo/redo keybinding defaults:** tui-textarea uses Emacs-style defaults (Ctrl+U / Ctrl+R for undo/redo), not the standard Ctrl+Z / Ctrl+Y. Validate remapping capability early in Phase 1 — if remapping is limited, the UX story for non-Emacs users is compromised.

- **syntect + tui-textarea integration:** Markdown syntax highlighting in the editor pane requires mapping pulldown-cmark source ranges to tui-textarea style spans. This is identified as the recommended approach (Option 3 in ARCHITECTURE.md) but has no off-the-shelf implementation to reference.

## Sources

### Primary (HIGH confidence)
- [ratatui GitHub + docs](https://ratatui.rs) — Component Architecture, Layout, event loop, rendering model
- [crossterm GitHub](https://github.com/crossterm-rs/crossterm) — v0.29.0, event polling API
- [pulldown-cmark GitHub](https://github.com/pulldown-cmark/pulldown-cmark) — v0.13.x, into_offset_iter()
- [tui-textarea GitHub](https://github.com/rhysd/tui-textarea) — v0.7.0, widget capabilities
- [syntect GitHub](https://github.com/trishume/syntect) — v5.3.0, pure-Rust feature flags
- [unicode-width docs](https://docs.rs/unicode-width/latest/unicode_width/) — display width API

### Secondary (MEDIUM confidence)
- [tui-markdown GitHub](https://github.com/joshka/tui-markdown) — v0.3.7, experimental status confirmed
- [glow](https://github.com/charmbracelet/glow), [md-tui](https://github.com/henriklovhaug/md-tui), [Splitmark](https://splitmark.app/), [MarkLn](https://github.com/xqtr/markln) — competitive feature analysis
- [Scroll Sync in Dual-Pane Editors](https://dev.to/woai3c/implementing-synchronous-scrolling-in-a-dual-pane-markdown-editor-5d75) — implementation strategies

### Tertiary (context/background)
- [xi-editor retrospective (Raph Levien)](https://raphlinus.github.io/xi/2020/06/27/xi-retrospective.html) — lessons from Rust text editor development
- [Text showdown: Gap Buffers vs Ropes](https://coredumped.dev/2023/08/09/text-showdown-gap-buffers-vs-ropes/) — data structure tradeoffs for future buffer work
- [Pretty Rust backtraces in raw terminal mode](https://werat.dev/blog/pretty-rust-backtraces-in-raw-terminal-mode/) — panic hook pattern

---
*Research completed: 2026-03-21*
*Ready for roadmap: yes*
