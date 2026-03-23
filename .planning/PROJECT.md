# mdedit

## What This Is

A terminal-based markdown editor with live rendered preview, syntax highlighting, search, and text selection. A single Rust TUI binary that lets you edit raw markdown on one side and see it rendered on the other — no browser, no bloated IDE, no vault lock-in. Built for people who live in the terminal and write markdown daily.

## Core Value

Edit markdown and see the rendered result side-by-side in a single terminal app, with zero external dependencies.

## Requirements

### Validated

- ✓ Open any .md file from the command line — v1.0
- ✓ Edit markdown with basic text editing (insert, delete, select, undo/redo) — v1.0
- ✓ Line numbers in the editor pane — v1.0
- ✓ Markdown syntax highlighting in the editor pane — v1.0
- ✓ Live rendered preview pane updating as you type — v1.0
- ✓ Side-by-side layout (editor left, preview right) — v1.0
- ✓ Toggle to fullscreen editor or fullscreen preview via hotkey — v1.0
- ✓ Save file (Ctrl+S) — v1.0
- ✓ Create new file if path doesn't exist — v1.0
- ✓ Status bar with filename, cursor position, modified indicator — v1.0
- ✓ Handle common markdown: headings, bold, italic, code blocks, links, lists, blockquotes, tables, horizontal rules — v1.0
- ✓ Scroll sync between editor and preview — v1.0
- ✓ Fast startup (<100ms) — v1.0
- ✓ Single compiled binary, no runtime dependencies — v1.0
- ✓ Works over SSH (no clipboard dependency for basic use) — v1.0
- ✓ Text search with Ctrl+F and match highlighting — v1.0
- ✓ Text selection with Shift+arrows — v1.0
- ✓ Indent/outdent with Tab/Shift+Tab — v1.0

### Active

- [ ] Browser companion with GitHub-accurate rendering (local only, not SSH)
- [ ] WYSIWYG terminal editing mode (`--wysiwyg` flag)

### Validated (v2.0)

- ✓ Vim-style keybindings as default editing mode — Phase 5
- ✓ Configurable color themes — Phase 4
- ✓ Clipboard integration (OSC 52 primary, platform-native fallback) — Phase 6
- ✓ Adjustable split ratio — Phase 5
- ✓ Mouse support for scrolling and clicking — Phase 5

### Out of Scope

- File browser/picker — open files via command line argument, not an in-app navigator
- Multiple open files/tabs — one file at a time
- Image rendering in preview — terminal image protocols are fragmented and unreliable
- Plugin system — premature abstraction
- Frontmatter parsing — not needed for core editing experience
- Blog-specific features — this is a general markdown editor

## Current Milestone: v2.0 Power User

**Goal:** Transform mdedit from a basic editor into a power-user tool with vim-style editing, WYSIWYG mode, configurable themes, and clipboard integration.

**Target features:**
- Vim-style keybindings as default editing mode (modal: normal/insert/visual)
- WYSIWYG terminal editing mode (`--wysiwyg` flag) — edit rendered markdown inline
- Configurable color themes (TOML config file)
- Browser companion with GitHub-accurate rendering (local only)
- Clipboard integration (OSC 52 + platform-native fallback)
- Adjustable split ratio
- Mouse support for scrolling and clicking

## Context

- Born from frustration editing the React2Shell blog post — no good terminal tool combines editing with rendering
- Existing tools: glow (view only), vim (edit only, no render), VS Code (bloated), Obsidian (vault-locked)
- Research confirmed no single compiled binary exists that does both edit + live preview
- v1.0 shipped with 1,508 LOC Rust across 8 source files
- Tech stack: ratatui 0.30, ratatui-textarea 0.8, pulldown-cmark 0.13, tui-markdown 0.3.7, syntect 5.3
- v2.0 focus: vim keybindings as default, WYSIWYG terminal mode, configurable themes, browser companion

## Constraints

- **Tech stack**: Rust with ratatui — best TUI ecosystem, building blocks exist
- **Binary size**: Keep reasonable (<10MB) — no embedding large assets
- **Startup time**: <100ms — must feel instant
- **Terminal compatibility**: Standard VT100/ANSI — no Kitty/iTerm-only features in v1
- **Platform**: macOS and Linux (Windows is nice-to-have, not blocking)

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust with ratatui | Best TUI ecosystem, building blocks exist (tui-markdown, tui-textarea), single binary output | ✓ Good — 1,508 LOC, fast builds |
| Side-by-side default with toggle | Most useful layout, toggle adds flexibility without complexity | ✓ Good — Ctrl+P cycles 3 modes |
| Basic editing only (no vim mode) in v1 | Reduces scope significantly, vim users can use vim | ✓ Good for v1 — switching to vim default in v2 |
| No config file in v1 | Ship sensible defaults, add config when users request it | ✓ Good — config coming in v2 |
| ratatui-textarea 0.8 (not tui-textarea 0.7) | 0.7 incompatible with ratatui 0.30 | ✓ Good — ratatui org fork, well maintained |
| Custom render path for editor | tui-textarea Widget has no per-span styling API | ✓ Good — enables syntax highlighting + selection + search overlays |
| 80ms debounce for preview | Sweet spot between responsiveness and performance | ✓ Good — feels instant |
| tui-markdown behind trait | Experimental library, may need replacement | ✓ Good — MarkdownRenderer trait allows swap |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd:transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd:complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-03-22 after v2.0 milestone start*
