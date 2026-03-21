# mdedit

## What This Is

A terminal-based markdown editor with live rendered preview. A single Rust TUI binary that lets you edit raw markdown on one side and see it rendered on the other — no browser, no bloated IDE, no vault lock-in. Built for people who live in the terminal and write markdown daily.

## Core Value

Edit markdown and see the rendered result side-by-side in a single terminal app, with zero external dependencies.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Open any .md file from the command line
- [ ] Edit markdown with basic text editing (insert, delete, select, undo/redo)
- [ ] Line numbers in the editor pane
- [ ] Markdown syntax highlighting in the editor pane
- [ ] Live rendered preview pane updating as you type
- [ ] Side-by-side layout (editor left, preview right)
- [ ] Toggle to fullscreen editor or fullscreen preview via hotkey
- [ ] Save file (Ctrl+S or equivalent)
- [ ] Create new file if path doesn't exist
- [ ] Status bar with filename, cursor position, modified indicator
- [ ] Handle common markdown: headings, bold, italic, code blocks, links, lists, blockquotes, tables, horizontal rules
- [ ] Scroll sync between editor and preview
- [ ] Fast startup (<100ms)
- [ ] Single compiled binary, no runtime dependencies
- [ ] Works over SSH (no clipboard dependency for basic use)

### Out of Scope

- Vim/emacs keybindings — adds complexity, basic editing is sufficient for v1
- File browser/picker — open files via command line argument, not an in-app navigator
- Multiple open files/tabs — one file at a time for v1
- Image rendering in preview — terminal image protocols are fragmented and unreliable
- Plugin system — premature abstraction
- Config file — sensible defaults only for v1
- Clipboard integration — fragmented across terminals/SSH, defer to v2
- Frontmatter parsing — not needed for core editing experience
- Blog-specific features — this is a general markdown editor

## Context

- Born from frustration editing the React2Shell blog post — no good terminal tool combines editing with rendering
- Existing tools: glow (view only), vim (edit only, no render), VS Code (bloated), Obsidian (vault-locked)
- Research confirmed no single compiled binary exists that does both edit + live preview
- Closest tools: Splitmark (Node.js, not single binary), MarkLn (Python, not single binary)
- Building blocks exist in Rust: ratatui (TUI framework), tui-markdown (rendering), tui-textarea (editing)
- Target user: developers who write markdown in the terminal and want to see what it looks like without leaving

## Constraints

- **Tech stack**: Rust with ratatui — best TUI ecosystem, building blocks exist
- **Binary size**: Keep reasonable (<10MB) — no embedding large assets
- **Startup time**: <100ms — must feel instant
- **Terminal compatibility**: Standard VT100/ANSI — no Kitty/iTerm-only features in v1
- **Platform**: macOS and Linux (Windows is nice-to-have, not blocking)

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust with ratatui | Best TUI ecosystem, building blocks exist (tui-markdown, tui-textarea), single binary output | — Pending |
| Side-by-side default with toggle | Most useful layout, toggle adds flexibility without complexity | — Pending |
| Basic editing only (no vim mode) | Reduces scope significantly, vim users can use vim | — Pending |
| No config file in v1 | Ship sensible defaults, add config when users request it | — Pending |

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
*Last updated: 2026-03-21 after initialization*
