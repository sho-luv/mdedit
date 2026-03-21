# Roadmap: mdedit

## Overview

mdedit goes from zero to a polished terminal markdown editor in three phases. Phase 1 builds a fully functional terminal text editor (no preview) — the load-bearing infrastructure everything else depends on. Phase 2 adds the live rendered preview, which is the core differentiator and the reason mdedit exists. Phase 3 adds scroll sync, search, selection, and other polish that makes it competitive with multi-runtime tools like Splitmark and MarkLn.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: Terminal Editor** - A working terminal text editor for markdown files with file I/O and status bar
- [ ] **Phase 2: Live Preview** - Side-by-side layout with live-updating rendered markdown preview
- [ ] **Phase 3: Polish and Power Features** - Scroll sync, search, selection, and UX completeness

## Phase Details

### Phase 1: Terminal Editor
**Goal**: Users can open, edit, and save markdown files in a fast, reliable terminal application
**Depends on**: Nothing (first phase)
**Requirements**: FOUND-01, FOUND-02, FOUND-03, FOUND-04, FOUND-05, FOUND-06, EDIT-01, EDIT-02, EDIT-03, EDIT-04, EDIT-05, EDIT-06, EDIT-10, CHRM-01, CHRM-03
**Success Criteria** (what must be TRUE):
  1. User can run `mdedit file.md` and see the file contents in an editor with line numbers, or run `mdedit` to start with an empty buffer
  2. User can type, delete, move cursor (arrows, Home/End, word-jump), undo with Ctrl+Z, and redo with Ctrl+Y — including with Unicode/emoji characters
  3. User can save with Ctrl+S, see confirmation in the status bar, and is warned about unsaved changes on exit
  4. Status bar shows filename, cursor position (line:col), and modified indicator
  5. App starts in under 100ms, compiles to a single binary, handles terminal resize, restores terminal state on exit/crash, and works over SSH
**Plans:** 2 plans

Plans:
- [x] 01-01-PLAN.md — Project skeleton, editor widget with nano-style keybindings, app event loop
- [x] 01-02-PLAN.md — File I/O (atomic save), status bar, and exit-with-unsaved-changes flow

### Phase 2: Live Preview
**Goal**: Users can see their markdown rendered live alongside the editor — the defining mdedit experience
**Depends on**: Phase 1
**Requirements**: PREV-01, PREV-02, PREV-03, PREV-04, PREV-05, PREV-06, LAYT-01, LAYT-02, LAYT-04, CHRM-02
**Success Criteria** (what must be TRUE):
  1. Editor and preview display side-by-side with editor on the left and rendered preview on the right
  2. Preview renders headings, bold, italic, strikethrough, code blocks (with syntax highlighting), links, lists, blockquotes, tables, horizontal rules, and task lists using GFM
  3. Preview updates live as the user types with no perceptible lag
  4. User can toggle between split view, editor-only, and preview-only with a hotkey
  5. Editor pane has markdown-aware syntax highlighting and keybinding hints are visible in the status bar
**Plans:** 1/2 plans executed

Plans:
- [x] 02-01-PLAN.md — Preview component, MarkdownRenderer trait, split layout, Ctrl+P toggle, debounced preview, keybinding hints
- [x] 02-02-PLAN.md — Editor-pane markdown syntax highlighting via syntect, visual verification

### Phase 3: Polish and Power Features
**Goal**: Users have scroll sync, search, and text selection — completing the editing experience for daily use
**Depends on**: Phase 2
**Requirements**: LAYT-03, EDIT-07, EDIT-08, EDIT-09
**Success Criteria** (what must be TRUE):
  1. Preview scroll position tracks the editor cursor position so the user sees the rendered version of what they are editing
  2. User can search text with Ctrl+F, see highlighted matches, and navigate between matches with Enter/Shift+Enter
  3. User can select text with Shift+arrow keys and Ctrl+Shift+arrow keys
  4. User can indent lines with Tab and outdent with Shift+Tab
**Plans**: TBD

Plans:
- [ ] 03-01: Scroll sync and text interaction polish

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Terminal Editor | 2/2 | Complete | 2026-03-21 |
| 2. Live Preview | 1/2 | In Progress|  |
| 3. Polish and Power Features | 0/1 | Not started | - |
