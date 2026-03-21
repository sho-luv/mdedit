# Feature Landscape

**Domain:** Terminal markdown editor with live preview
**Researched:** 2026-03-21

## Competitive Landscape Summary

Researched six tools across the spectrum:

| Tool | Type | Language | Edit? | Preview? | Split? | Single Binary? |
|------|------|----------|-------|----------|--------|----------------|
| **glow** | Viewer only | Go | No | Yes | N/A | Yes |
| **md-tui** | Viewer only | Rust | No (opens $EDITOR) | Yes | N/A | Yes |
| **frogmouth** | Viewer/browser | Python (Textual) | No | Yes | N/A | No (pip) |
| **Splitmark** | Editor + preview | Node.js | Yes | Yes | Yes | No (npm) |
| **MarkLn** | Editor + preview | Python | Yes | Yes | Yes | No (pip) |
| **Neovim plugins** | Editor augmentation | Lua | Yes (Neovim) | Yes (in-buffer) | Yes (markview.nvim) | No (requires Neovim) |

**Key gap mdedit fills:** No single compiled binary exists that does both edit + live preview. Splitmark and MarkLn require runtime interpreters. Glow and md-tui are view-only. Neovim plugins require Neovim. mdedit is the first Rust single-binary editor+preview tool.

---

## Table Stakes

Features users expect. Missing any of these and users will close the app immediately.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Open file from CLI argument | Every terminal tool does this (glow, md-tui, Splitmark, MarkLn) | Low | `mdedit file.md` or `mdedit` for new file |
| Basic text editing (insert, delete, newline) | Fundamental editor capability | Med | Use tui-textarea crate as foundation |
| Undo/Redo (Ctrl+Z / Ctrl+Y) | Every editor has this; Splitmark, MarkLn both support it | Med | Must be reliable; users panic without undo |
| Save file (Ctrl+S) | Universal expectation | Low | Show save confirmation in status bar |
| Cursor movement (arrows, Home/End, Ctrl+arrows) | Standard text navigation; Splitmark has all of these | Low | Word-jump (Ctrl+Arrow) is expected, not a bonus |
| Side-by-side editor + preview layout | Core value proposition; Splitmark and MarkLn both do this | Med | Left=editor, right=preview is the standard |
| Live preview updating as you type | Defining feature; Splitmark calls this out as primary feature | High | Must feel instant (<50ms render after keystroke) |
| Markdown syntax highlighting in editor | Splitmark, MarkLn, and all Neovim plugins do this | Med | Color headings, bold, code, links distinctly |
| Rendered preview of common elements | All viewers handle these; glow, md-tui, frogmouth all render them | High | Headings, bold, italic, code blocks, links, lists, blockquotes, tables, horizontal rules |
| Code block syntax highlighting in preview | glow, md-tui, frogmouth, Splitmark all do this | Med | At minimum: bash, python, rust, javascript, json, go, typescript |
| Line numbers in editor | Standard for code/text editors | Low | Left gutter with line numbers |
| Status bar | Splitmark, MarkLn, micro all have this | Low | Filename, line:col, modified indicator, keybinding hints |
| Modified/unsaved indicator | Users need to know if they have unsaved work | Low | Asterisk or dot in status bar + title |
| Exit with unsaved changes warning | Splitmark warns on exit; every serious editor does | Low | "Unsaved changes. Save? (y/n/cancel)" |
| Responsive to terminal resize | All TUI tools handle this; frogmouth, md-tui resize cleanly | Med | Reflow editor and preview on resize |
| Fast startup (<100ms) | Rust binary advantage; glow and md-tui both start instantly | Low | Inherent with Rust + no runtime |
| Keyboard-driven (no mouse required) | Terminal users expect full keyboard operation; md-tui is entirely keyboard-driven | Low | All actions reachable via keyboard |

---

## Differentiators

Features that set mdedit apart. Not expected by users on day one, but create competitive advantage and "wow" moments.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Single compiled binary, zero deps** | No other edit+preview tool achieves this. Glow and md-tui are single binaries but view-only | Low | This is the #1 differentiator -- just download and run |
| **Scroll sync between editor and preview** | Splitmark has this ("preview follows cursor position"); MarkLn syncs views. Users love it | High | Map editor cursor line to approximate preview position; hard to get right |
| **Layout toggle (editor-only / preview-only / split)** | MarkLn has "Synced, Editor, Preview" modes. Lets user focus | Low | Single hotkey to cycle: split -> editor -> preview -> split |
| **Adjustable split ratio** | Splitmark supports 75/25, 50/50, 25/75 ratios | Med | Let user drag or hotkey-adjust the split point |
| **Table of contents sidebar** | frogmouth extracts headings into a Contents panel; md-tui has this too | Med | Parse headings, show in sidebar, jump on select |
| **Search in editor (Ctrl+F)** | md-tui has search (`/` or `f`); any serious editor needs this eventually | Med | Highlight matches, navigate with n/N |
| **Works over SSH** | No viewer or editor in this space explicitly targets SSH. Rust binary + no clipboard = SSH-friendly | Low | Already inherent if avoiding clipboard/image deps |
| **Stacked layout option (top/bottom)** | Splitmark supports both side-by-side and stacked; useful for narrow terminals | Low | Toggle between horizontal and vertical split |
| **Markdown tag/snippet insertion** | MarkLn has F3 tag selector for inserting markdown syntax | Med | Quick-insert for tables, links, code blocks, lists |
| **Text selection (Shift+arrows)** | Splitmark supports this with Shift+Arrow and Ctrl+Shift+Arrow | Med | Visual selection for cut/copy/delete |
| **Indent/outdent (Tab/Shift+Tab)** | Splitmark and most editors support this; critical for lists and code | Low | Indent selected lines or current line |
| **Create new file if path doesn't exist** | Splitmark auto-creates folders for nested paths | Low | `mdedit new/path/file.md` creates directories |

---

## Anti-Features

Features to explicitly NOT build. Each was considered and rejected for clear reasons.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| **Vim/emacs keybindings** | Massive scope increase. Vim users will use Neovim + render-markdown.nvim. mdedit targets the "I just want to edit markdown" crowd | Stick with standard editor keybindings (Ctrl+S, Ctrl+Z, arrows). Revisit as optional mode in v2+ only if demand is overwhelming |
| **File browser/picker** | Splitmark has one, but it adds significant UI complexity. Users already know the filename | Open files via CLI argument. If no arg, open empty buffer. Can revisit in v2 |
| **Multiple files/tabs** | Adds state management complexity (which file is active, per-file undo, tab UI). Splitmark and MarkLn are single-file | One file at a time. Open another instance for another file |
| **Image rendering in preview** | md-tui supports optional image rendering but notes it depends on terminal support. iTerm/Kitty protocols are fragmented, Sixel isn't universal | Show `[image: alt-text]` placeholder. Terminal image protocols are too inconsistent |
| **Plugin/extension system** | frogmouth supports plugins via Textual/Python. Premature abstraction for a compiled binary | Build features directly. If the architecture is modular, plugins can come later |
| **Config file** | Splitmark has `~/.splitmarkrc`, md-tui has `~/.config/mdt/config.toml`. But config adds support burden | Ship sensible defaults. If v1 feedback demands customization, add a config file in v2 |
| **Cloud sync** | Splitmark offers paid cloud sync. Way outside core scope | Files live on the filesystem. Users have git, Dropbox, Syncthing |
| **Export to HTML/PDF** | Common in GUI editors (Typora, Obsidian). Not what terminal users need | Users can pipe through pandoc if they need conversion |
| **Clipboard integration** | Fragmented across terminals, SSH sessions, tmux, and platforms. OSC 52 is gaining support but not universal | Defer to v2. Use terminal's native copy (select + terminal copy shortcut) |
| **Frontmatter/YAML parsing** | Nice for blog tools, irrelevant for general markdown editing | Render frontmatter as a code block in preview |
| **AI/LLM integration** | Trending in GUI editors. Completely out of scope for a focused TUI tool | Not even on the v2 radar |
| **Mouse-driven interface** | md-tui is keyboard-only. frogmouth supports mouse. For v1, keyboard-first is simpler | Keyboard-first. Mouse scrolling can be added cheaply but don't design around it |
| **Nerd Font requirement** | md-tui requires Nerd Fonts. This is a barrier for SSH and stock terminals | Use Unicode box-drawing and standard characters. Work in any monospace font |

---

## Feature Dependencies

```
Basic text editing ──> Undo/Redo
Basic text editing ──> Text selection ──> Cut/Copy/Delete selection
Basic text editing ──> Search (Ctrl+F)
Basic text editing ──> Indent/Outdent

Markdown parser ──> Syntax highlighting (editor)
Markdown parser ──> Rendered preview
Markdown parser ──> Table of contents

Rendered preview ──> Side-by-side layout
Rendered preview ──> Scroll sync
Rendered preview ──> Layout toggle (editor/preview/split)
Rendered preview ──> Stacked layout option
Rendered preview ──> Adjustable split ratio

Side-by-side layout ──> Stacked layout option
Side-by-side layout ──> Adjustable split ratio

File I/O (open/save) ──> Modified indicator
File I/O (open/save) ──> Unsaved changes warning
File I/O (open/save) ──> Create new file/dirs
```

---

## MVP Recommendation

**Prioritize (Phase 1 -- minimum usable product):**

1. Open file from CLI, basic text editing, save file (the foundation)
2. Markdown syntax highlighting in editor pane (makes editing tolerable)
3. Rendered preview of common elements (the core value)
4. Side-by-side layout with live update (the defining experience)
5. Line numbers, status bar, modified indicator (baseline polish)
6. Exit with unsaved changes warning (data safety)
7. Undo/Redo (non-negotiable for any editor)

**Phase 2 -- make it good:**

8. Scroll sync between editor and preview (biggest "wow" after basic split works)
9. Layout toggle (editor-only / preview-only / split)
10. Search (Ctrl+F)
11. Text selection with Shift+arrows
12. Indent/outdent with Tab/Shift+Tab
13. Terminal resize handling

**Defer to v2+:**

- Table of contents sidebar: Useful but significant UI work
- Adjustable split ratio: Nice but 50/50 default is fine for v1
- Stacked layout: Narrow terminal users can cope with split for now
- Markdown snippet insertion: Power user feature, not blocking
- File browser: CLI argument is sufficient

---

## Sources

- [glow - GitHub](https://github.com/charmbracelet/glow)
- [md-tui - GitHub](https://github.com/henriklovhaug/md-tui)
- [frogmouth - GitHub](https://github.com/Textualize/frogmouth)
- [Splitmark - GitHub](https://github.com/Splitmark/splitmark)
- [Splitmark - Official Site](https://splitmark.app/)
- [MarkLn - GitHub](https://github.com/xqtr/markln)
- [render-markdown.nvim - GitHub](https://github.com/MeanderingProgrammer/render-markdown.nvim)
- [markview.nvim - GitHub](https://github.com/OXY2DEV/markview.nvim)
- [md-tui - crates.io](https://crates.io/crates/md-tui)
- [Best Markdown Editors 2026 - Unmarkdown](https://unmarkdown.com/blog/best-markdown-editors-2026)
