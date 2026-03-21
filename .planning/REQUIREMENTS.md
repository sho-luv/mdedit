# Requirements: mdedit

**Defined:** 2026-03-21
**Core Value:** Edit markdown and see the rendered result side-by-side in a single terminal app, with zero external dependencies.

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### Foundation

- [x] **FOUND-01**: User can open a .md file by passing it as a CLI argument (`mdedit file.md`)
- [x] **FOUND-02**: User can create a new empty buffer when no file argument is given (`mdedit`)
- [x] **FOUND-03**: App starts in under 100ms with no visible delay
- [x] **FOUND-04**: App compiles to a single binary with no runtime dependencies
- [x] **FOUND-05**: Terminal state is fully restored on exit or crash (panic hook)
- [x] **FOUND-06**: App handles terminal resize events and reflows layout

### Editing

- [x] **EDIT-01**: User can insert, delete, and edit text with standard keyboard input
- [x] **EDIT-02**: User can move cursor with arrow keys, Home/End, Ctrl+Left/Right (word jump)
- [x] **EDIT-03**: User can undo changes with Ctrl+Z and redo with Ctrl+Y
- [x] **EDIT-04**: User can save the file with Ctrl+S and see confirmation in status bar
- [x] **EDIT-05**: User is warned about unsaved changes when attempting to exit
- [x] **EDIT-06**: Editor displays line numbers in a left gutter
- [ ] **EDIT-07**: User can search text with Ctrl+F, see highlighted matches, navigate with Enter/Shift+Enter
- [ ] **EDIT-08**: User can select text with Shift+arrow keys and Ctrl+Shift+arrow keys
- [ ] **EDIT-09**: User can indent/outdent lines with Tab/Shift+Tab
- [x] **EDIT-10**: Editor correctly handles Unicode characters including multi-byte and wide characters

### Preview

- [ ] **PREV-01**: Preview pane renders markdown as formatted terminal output (headings, bold, italic, strikethrough)
- [ ] **PREV-02**: Preview renders code blocks with syntax highlighting for common languages (bash, python, rust, javascript, json, go, typescript)
- [ ] **PREV-03**: Preview renders links, lists (ordered/unordered), blockquotes, tables, horizontal rules, and task lists
- [ ] **PREV-04**: Preview updates live as the user types with no perceptible lag (<100ms)
- [ ] **PREV-05**: Preview uses GitHub Flavored Markdown (GFM) as the rendering standard
- [ ] **PREV-06**: Editor pane has markdown-aware syntax highlighting (headings, bold, code, links colored distinctly)

### Layout

- [ ] **LAYT-01**: Default layout is side-by-side (editor left, preview right)
- [ ] **LAYT-02**: User can toggle between split, editor-only, and preview-only views with a hotkey
- [ ] **LAYT-03**: Preview scroll position tracks the editor cursor position (scroll sync)
- [ ] **LAYT-04**: All features are keyboard-accessible without requiring a mouse

### Chrome

- [x] **CHRM-01**: Status bar shows filename, cursor position (line:col), and modified indicator
- [ ] **CHRM-02**: Status bar shows available keybinding hints
- [x] **CHRM-03**: App works correctly over SSH connections without Nerd Fonts or special terminal features

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Layout Enhancements

- **LAYT-05**: User can switch between horizontal (side-by-side) and vertical (stacked) split
- **LAYT-06**: User can adjust split ratio with keyboard shortcut
- **LAYT-07**: Table of contents sidebar extracted from headings with jump-to navigation

### Editing Enhancements

- **EDIT-11**: Quick-insert snippets for common markdown (table template, link, code block)
- **EDIT-12**: File browser / fuzzy finder to open files without leaving the app
- **EDIT-13**: Multiple file tabs

### Preview Enhancements

- **PREV-07**: Selectable markdown flavors (Obsidian, Lark, etc.) beyond GFM
- **PREV-08**: Image alt-text placeholder rendering (`[image: description]`)

### Editing Experience

- **PREV-09**: WYSIWYG editing in preview mode (edit rendered output directly, changes map back to source markdown)

### Platform

- **PLAT-01**: Clipboard integration (copy/paste via OSC 52 or platform-native)
- **PLAT-02**: Mouse support for scrolling and clicking
- **PLAT-03**: Optional config file for keybindings and theme customization

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Vim/emacs keybindings | Massive scope increase; vim users have render-markdown.nvim |
| Plugin/extension system | Premature abstraction for a compiled binary |
| Cloud sync | Files live on the filesystem; users have git/Dropbox |
| Export to HTML/PDF | Users can pipe through pandoc |
| AI/LLM integration | Out of scope for a focused TUI tool |
| Nerd Font requirement | Breaks SSH and stock terminals |
| Frontmatter/YAML parsing | Irrelevant for general markdown editing |
| Image rendering | Terminal image protocols too fragmented |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| FOUND-01 | Phase 1 | Complete |
| FOUND-02 | Phase 1 | Complete |
| FOUND-03 | Phase 1 | Complete |
| FOUND-04 | Phase 1 | Complete |
| FOUND-05 | Phase 1 | Complete |
| FOUND-06 | Phase 1 | Complete |
| EDIT-01 | Phase 1 | Complete |
| EDIT-02 | Phase 1 | Complete |
| EDIT-03 | Phase 1 | Complete |
| EDIT-04 | Phase 1 | Complete |
| EDIT-05 | Phase 1 | Complete |
| EDIT-06 | Phase 1 | Complete |
| EDIT-07 | Phase 3 | Pending |
| EDIT-08 | Phase 3 | Pending |
| EDIT-09 | Phase 3 | Pending |
| EDIT-10 | Phase 1 | Complete |
| PREV-01 | Phase 2 | Pending |
| PREV-02 | Phase 2 | Pending |
| PREV-03 | Phase 2 | Pending |
| PREV-04 | Phase 2 | Pending |
| PREV-05 | Phase 2 | Pending |
| PREV-06 | Phase 2 | Pending |
| LAYT-01 | Phase 2 | Pending |
| LAYT-02 | Phase 2 | Pending |
| LAYT-03 | Phase 3 | Pending |
| LAYT-04 | Phase 2 | Pending |
| CHRM-01 | Phase 1 | Complete |
| CHRM-02 | Phase 2 | Pending |
| CHRM-03 | Phase 1 | Complete |

**Coverage:**
- v1 requirements: 29 total
- Mapped to phases: 29
- Unmapped: 0

---
*Requirements defined: 2026-03-21*
*Last updated: 2026-03-21 after roadmap creation*
