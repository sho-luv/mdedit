# Requirements: mdedit v2.0

**Defined:** 2026-03-22
**Core Value:** Edit markdown and see the rendered result side-by-side in a single terminal app, with zero external dependencies.

## v2.0 Requirements

Requirements for v2.0 Power User milestone. Each maps to roadmap phases.

### Configuration

- [x] **CONF-01**: User can configure settings via `~/.config/mdedit/config.toml`
- [x] **CONF-02**: User can select color theme by name in config (`theme = "dracula"`)
- [x] **CONF-03**: User can define custom color themes in TOML
- [x] **CONF-04**: User can set default editing mode in config (`mode = "vim"` or `mode = "nano"`)
- [x] **CONF-05**: Editor respects terminal color capability (256-color and truecolor detection)

### Vim Keybindings

- [x] **VIM-01**: Editor starts in Normal mode by default (vim-style)
- [x] **VIM-02**: User can switch between Normal, Insert, and Visual modes
- [x] **VIM-03**: Normal mode supports motions: h/j/k/l, w/b/e, 0/$, gg/G, {/}
- [x] **VIM-04**: Normal mode supports operators: d (delete), c (change), y (yank), p/P (paste)
- [x] **VIM-05**: Insert mode entered via i/a/o/O/A/I, exited via Esc
- [x] **VIM-06**: Visual mode supports character selection (v) and line selection (V)
- [x] **VIM-07**: Command mode supports :w (save), :q (quit), :wq, :q!
- [x] **VIM-08**: Status bar shows current mode (NORMAL/INSERT/VISUAL/COMMAND)
- [x] **VIM-09**: Normal mode supports count prefixes (e.g., 3j, 5dd, 2w)
- [x] **VIM-10**: Undo/redo via u and Ctrl+R in Normal mode

### Clipboard

- [ ] **CLIP-01**: User can copy selected text to system clipboard via vim yank (y) or Ctrl+C
- [ ] **CLIP-02**: User can paste from system clipboard via vim paste (p/P) or Ctrl+V
- [ ] **CLIP-03**: Clipboard works over SSH via OSC 52 escape sequence
- [ ] **CLIP-04**: Clipboard falls back to platform-native (pbcopy/xclip) when available locally

### Mouse Support

- [x] **MOUSE-01**: User can click to position cursor in editor pane
- [x] **MOUSE-02**: User can scroll editor and preview panes with mouse wheel
- [x] **MOUSE-03**: User can click-drag to select text in editor pane
- [x] **MOUSE-04**: User can resize split ratio by dragging the divider

### Browser Companion

- [ ] **BROW-01**: User can launch browser preview with `mdedit --browser <file>`
- [ ] **BROW-02**: Browser renders markdown with GitHub-accurate CSS styling
- [ ] **BROW-03**: Browser preview auto-refreshes when file is saved
- [ ] **BROW-04**: Browser companion runs as local HTTP server (localhost only)
- [ ] **BROW-05**: Terminal editor and browser companion can run simultaneously

### WYSIWYG Mode

- [ ] **WYS-01**: User can launch WYSIWYG mode with `mdedit --wysiwyg <file>`
- [ ] **WYS-02**: Markdown renders inline in the terminal (headings styled, bold rendered, etc.)
- [ ] **WYS-03**: Cursor line reveals raw markdown syntax for editing
- [ ] **WYS-04**: User can edit in WYSIWYG mode with same keybindings (vim/nano)
- [ ] **WYS-05**: WYSIWYG mode saves the underlying markdown source (not rendered form)

## Future Requirements

Deferred beyond v2.0.

### Advanced Vim
- **VIM-F01**: Visual Block mode (Ctrl+V)
- **VIM-F02**: Macros (q recording)
- **VIM-F03**: Marks and jumps
- **VIM-F04**: Custom keybinding remapping in config

### Advanced Editing
- **EDIT-F01**: Search and replace (:%s/old/new/g)
- **EDIT-F02**: Regex search mode
- **EDIT-F03**: Multi-cursor editing
- **EDIT-F04**: Auto-pair brackets and markdown syntax

### Browser
- **BROW-F01**: Server-sent events for instant refresh (no polling)
- **BROW-F02**: Scroll sync between terminal and browser
- **BROW-F03**: Dark/light mode toggle in browser

## Out of Scope

| Feature | Reason |
|---------|--------|
| File browser/picker | Open files via CLI, not in-app navigator |
| Multiple open files/tabs | One file at a time, keep it simple |
| Image rendering in terminal | Terminal image protocols too fragmented |
| Plugin system | Premature abstraction |
| Async runtime (tokio) | Synchronous event loop sufficient, adds complexity |
| Full vim compatibility | Support ~30 core keybindings, not all of vim |
| LSP integration | Out of scope for a markdown editor |
| Collaborative editing | Far beyond current scope |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| CONF-01 | Phase 4 | Complete |
| CONF-02 | Phase 4 | Complete |
| CONF-03 | Phase 4 | Complete |
| CONF-04 | Phase 4 | Complete |
| CONF-05 | Phase 4 | Complete |
| VIM-01 | Phase 5 | Complete |
| VIM-02 | Phase 5 | Complete |
| VIM-03 | Phase 5 | Complete |
| VIM-04 | Phase 5 | Complete |
| VIM-05 | Phase 5 | Complete |
| VIM-06 | Phase 5 | Complete |
| VIM-07 | Phase 5 | Complete |
| VIM-08 | Phase 5 | Complete |
| VIM-09 | Phase 5 | Complete |
| VIM-10 | Phase 5 | Complete |
| CLIP-01 | Phase 6 | Pending |
| CLIP-02 | Phase 6 | Pending |
| CLIP-03 | Phase 6 | Pending |
| CLIP-04 | Phase 6 | Pending |
| MOUSE-01 | Phase 5 | Complete |
| MOUSE-02 | Phase 5 | Complete |
| MOUSE-03 | Phase 5 | Complete |
| MOUSE-04 | Phase 5 | Complete |
| BROW-01 | Phase 7 | Pending |
| BROW-02 | Phase 7 | Pending |
| BROW-03 | Phase 7 | Pending |
| BROW-04 | Phase 7 | Pending |
| BROW-05 | Phase 7 | Pending |
| WYS-01 | Phase 8 | Pending |
| WYS-02 | Phase 8 | Pending |
| WYS-03 | Phase 8 | Pending |
| WYS-04 | Phase 8 | Pending |
| WYS-05 | Phase 8 | Pending |

**Coverage:**
- v2.0 requirements: 33 total
- Mapped to phases: 33
- Unmapped: 0

---
*Requirements defined: 2026-03-22*
*Last updated: 2026-03-22 after roadmap creation*
