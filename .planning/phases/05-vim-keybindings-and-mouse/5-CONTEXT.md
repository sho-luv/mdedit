# Phase 5: Vim Keybindings and Mouse - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning
**Mode:** Auto (recommended defaults selected)

<domain>
## Phase Boundary

Replace the current nano-style keybinding system with vim-style modal editing as the default. Implement Normal, Insert, Visual, and Command modes with standard vim motions and operators. Add mouse support for cursor placement, scrolling, text selection, and split ratio adjustment. The config `mode` field (from Phase 4) selects vim vs nano. No new rendering features, no new markdown capabilities.

</domain>

<decisions>
## Implementation Decisions

### Vim mode architecture
- **D-01:** Build a `VimHandler` state machine in `src/vim.rs` that sits between key events and editor operations. It receives `KeyEvent`, maintains mode state, and returns a `VimCommand` enum that the editor executes. The existing `Editor::handle_key()` becomes the nano-mode handler; vim mode bypasses it entirely.
- **D-02:** `AppMode::Editing` splits into `AppMode::Normal`, `AppMode::Insert`, `AppMode::Visual`, `AppMode::Command` when in vim mode. `AppMode::ConfirmQuit`, `AppMode::PromptFilename`, and `AppMode::Search` remain unchanged.
- **D-03:** In Insert mode, keys go through `textarea.input_without_shortcuts()` just like current nano mode, except Esc returns to Normal mode. This preserves all existing text input behavior.
- **D-04:** VimHandler tracks: current mode, pending operator (d/c/y), pending count prefix, partial key sequence (e.g., `g` waiting for `g`), and yank register (single string buffer for Phase 6 clipboard).

### Supported vim motions (Normal mode)
- **D-05:** Navigation motions: `h` (left), `j` (down), `k` (up), `l` (right), `w` (word forward), `b` (word back), `e` (end of word), `0` (line start), `$` (line end), `gg` (file start), `G` (file end), `{` (paragraph up), `}` (paragraph down).
- **D-06:** Operators: `d` (delete), `c` (change — delete + enter Insert), `y` (yank — copy to register), `p` (paste after), `P` (paste before).
- **D-07:** Operator + motion combos: `dw` (delete word), `dd` (delete line), `yy` (yank line), `cc` (change line), `d$`/`D` (delete to end), `c$`/`C` (change to end), `d0` (delete to start).
- **D-08:** Count prefixes: `3j` (down 3), `5dd` (delete 5 lines), `2w` (2 words forward). VimHandler accumulates digits before dispatching.
- **D-09:** Insert mode entry: `i` (before cursor), `a` (after cursor), `o` (new line below), `O` (new line above), `A` (end of line), `I` (start of line).
- **D-10:** Other Normal mode keys: `x` (delete char), `u` (undo), `Ctrl+R` (redo), `/` (enter search — reuses existing Search mode), `:` (enter Command mode).

### Command mode
- **D-11:** `:w` (save), `:q` (quit), `:wq` (save and quit), `:q!` (force quit without saving). Typed in status bar area with `:` prefix. Enter executes, Esc cancels.
- **D-12:** No ex-mode commands beyond save/quit in v2. No `:set`, no `:%s`, no `:!`.

### Visual mode
- **D-13:** `v` enters character-wise Visual mode. Selection extends with same motions as Normal mode (h/j/k/l/w/b/e/0/$). Uses ratatui-textarea's built-in `start_selection()` / `cancel_selection()`.
- **D-14:** `V` enters line-wise Visual mode — entire lines selected. Selection shown with same overlay as current Shift+arrow selection.
- **D-15:** In Visual mode: `d` deletes selection, `c` deletes + enters Insert, `y` yanks selection, `>` indents, `<` outdents. Esc cancels selection and returns to Normal.

### Mode indicator
- **D-16:** Status bar shows mode in left section: `-- NORMAL --`, `-- INSERT --`, `-- VISUAL --`, `-- COMMAND --`. Colored per theme: Normal uses theme.status_bar_bg, Insert uses green-tinted, Visual uses blue-tinted.
- **D-17:** Cursor shape changes via crossterm: block cursor in Normal mode, line cursor in Insert mode. `SetCursorStyle::SteadyBlock` and `SetCursorStyle::SteadyBar`.

### Mouse support
- **D-18:** Enable mouse capture via `crossterm::event::EnableMouseCapture` at terminal init. Disable on exit.
- **D-19:** Left click in editor pane: position cursor at clicked row/col (accounting for line numbers and scroll offset).
- **D-20:** Mouse wheel in editor pane: scroll editor. Mouse wheel in preview pane: scroll preview independently.
- **D-21:** Left click + drag in editor: start selection (enters Visual mode in vim, uses Shift-selection in nano).
- **D-22:** Left click on divider column + drag horizontally: adjust split ratio. Store as percentage in App struct (replacing the current hardcoded `Constraint::Percentage(50)`).
- **D-23:** Mouse events are handled at the App level before mode dispatch. Mouse position determines which pane receives the event.

### Nano mode preservation
- **D-24:** When `config.mode == EditingMode::Nano`, the current `Editor::handle_key()` and all Ctrl+key bindings remain exactly as they are. Vim mode is completely separate — no shared key dispatch beyond mouse events.
- **D-25:** Search (Ctrl+F) works identically in both modes. In vim mode, `/` also enters search (same Search AppMode).

### Claude's Discretion
- Exact VimCommand enum variants and internal state machine transitions
- How to map textarea CursorMove to vim motions (most are direct: CursorMove::Forward = l, etc.)
- Paragraph motion ({/}) implementation (scan for blank lines)
- How to handle edge cases (e.g., `d` with no following motion — cancel after timeout or wait indefinitely)
- Mouse coordinate → editor row/col mapping math
- Divider drag sensitivity and minimum pane width

</decisions>

<specifics>
## Specific Ideas

- User explicitly wants vim keybindings as default: "I would prefer the editing be similar to yazi... it seems to use a vim like editing style by default"
- Vim should be the default — nano available as config option but not the primary experience
- The ratatui-textarea vim example (in the crate's examples/) demonstrates the state machine pattern — use it as reference
- `/` for search should feel natural to vim users — it reuses the existing Ctrl+F search infrastructure
- Mouse support should "just work" — no config needed to enable it

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project context
- `.planning/PROJECT.md` — Core value, constraints, key decisions
- `.planning/REQUIREMENTS.md` — VIM-01 through VIM-10, MOUSE-01 through MOUSE-04

### Research
- `.planning/research/STACK.md` — No new crates needed for vim; crossterm mouse capture built-in
- `.planning/research/ARCHITECTURE.md` — VimHandler state machine design, AppMode expansion
- `.planning/research/PITFALLS.md` — Vim key dispatch pitfalls, cursor shape rendering, multi-key sequences

### Prior phase context
- `.planning/phases/04-configuration-and-themes/4-CONTEXT.md` — Config/theme decisions, EditingMode enum
- `src/config.rs` — EditingMode::Vim/Nano, config loading
- `src/theme.rs` — Theme struct with mode_indicator colors (if added) or status bar colors
- `src/app.rs` — Current AppMode enum, handle_editing_key(), event loop structure
- `src/editor.rs` — Current handle_key() nano bindings, input_without_shortcuts() pattern, render_highlighted()

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `Editor::handle_key()` — Becomes the nano-mode handler, vim mode bypasses entirely
- `textarea.input_without_shortcuts()` — Reused in vim Insert mode for text input
- `textarea.move_cursor(CursorMove::*)` — Direct mapping to vim motions (Forward=l, Back=h, Up=k, Down=j, WordForward=w, WordBack=b, Head=0, End=$, Top=gg, Bottom=G)
- `textarea.start_selection()` / `cancel_selection()` / `selection_range()` — Reused for Visual mode
- `textarea.delete_str()` / `textarea.insert_str()` — Used by vim operators
- `textarea.undo()` / `textarea.redo()` — Mapped to `u` and `Ctrl+R`
- `Editor::render_highlighted()` — No changes needed; selection overlay already works for Visual mode
- `AppMode::Search` with search_query — Reused for vim `/` search

### Established Patterns
- Key routing via `match self.mode` in App::run() event loop — extend with new vim modes
- Ctrl+P intercepted at App level before editor — same pattern for mouse events
- Editor returns `EditorAction` enum to App — VimHandler can return similar action types
- `content_dirty` flag + debounce — works with vim commands that change content

### Integration Points
- `App::run()` event loop — Add mouse event handling, route to vim or nano based on config.mode
- `App::handle_editing_key()` — In vim mode, delegate to VimHandler instead of Editor::handle_key()
- `App::new()` — Accept EditingMode, construct VimHandler if vim mode
- Status bar render — Show mode indicator (NORMAL/INSERT/VISUAL/COMMAND)
- `ratatui::run()` in main.rs — May need to replace with manual terminal init to add EnableMouseCapture
- Split layout in App::render() — Replace hardcoded Percentage(50) with variable split_ratio

</code_context>

<deferred>
## Deferred Ideas

- Visual Block mode (Ctrl+V) — v3+ (VIM-F01, complex column selection)
- Macros (q recording) — v3+ (VIM-F02)
- Marks and jumps — v3+ (VIM-F03)
- Custom keybinding remapping — v3+ (VIM-F04)
- Search and replace (:%s/old/new/g) — v3+ (EDIT-F01)
- Count prefix for all operators — start with common ones, extend later

</deferred>

---

*Phase: 05-vim-keybindings-and-mouse*
*Context gathered: 2026-03-22*
