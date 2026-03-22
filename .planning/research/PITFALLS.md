# Domain Pitfalls: mdedit v2.0 Power User Features

**Domain:** Adding vim modal editing, WYSIWYG mode, configurable themes, browser companion, and clipboard integration to an existing ratatui-textarea TUI editor
**Researched:** 2026-03-22
**Confidence:** HIGH (pitfalls derived from codebase analysis + ecosystem research + known patterns in ratatui/vim/terminal editors)

---

## Critical Pitfalls

Mistakes that cause rewrites or major architectural issues.

### Pitfall 1: Vim Mode Fights the Existing Key Dispatch Architecture

**What goes wrong:** The current `editor.rs` handles keys via a flat `match (key.modifiers, key.code)` block in `handle_key()`, then falls through to `input_without_shortcuts()` for unmatched keys. Adding vim modes means every single key changes meaning based on mode (Normal, Insert, Visual, Operator-pending). Developers typically try to wrap the existing match block with `if mode == Insert { existing_logic } else { vim_logic }` -- this creates an unmaintainable monster because vim Normal mode reinterprets literally every key (`h`, `j`, `w`, `d`, `y`, `0`, `$`, etc.) and multi-key sequences (`dd`, `dw`, `gg`, `ci"`) require pending-state tracking that does not exist in the current architecture.

**Why it happens:** The v1 architecture assumes every keypress is either a Ctrl-chord (intercepted) or a character to insert (passed through via `input_without_shortcuts()`). Vim breaks this assumption fundamentally: in Normal mode, `d` is not a character to insert -- it is the start of a delete operator waiting for a motion.

**Consequences:**
- The match block becomes 300+ lines of nested conditionals
- Multi-key sequences (operator + motion) require state that does not exist in the current `EditorAction` enum
- `input_without_shortcuts()` must NEVER be called in Normal mode (it would insert `h`, `j`, `k`, `l` as characters)
- Every existing Ctrl-chord (Ctrl+S, Ctrl+Q, Ctrl+F, Ctrl+P) must be re-evaluated for each vim mode

**Prevention:**
1. **Replace the key dispatch entirely.** Create a `VimStateMachine` struct with a `Mode` enum (`Normal`, `Insert`, `Visual`, `OperatorPending(char)`) and a `Transition` enum for state changes. The ratatui-textarea vim example uses exactly this pattern -- a `Vim` struct with `mode`, `pending` fields, and a `transition()` method that returns `Nop`, `Mode(Mode)`, `Pending(Input)`, or `Quit`.
2. **Route ALL keys through the state machine first.** The state machine decides whether to call `input_without_shortcuts()` (Insert mode only), `move_cursor()` (Normal mode motions), or accumulate pending state (operator-pending).
3. **Extract current Ctrl-chords into a separate layer** that runs BEFORE the vim state machine. Ctrl+S/Q/P/F are app-level commands that work regardless of vim mode. This is the `AppMode` layer in `app.rs` that already exists -- keep it.
4. **Do NOT try to add vim as "another AppMode."** Vim modes are EDITING sub-modes, not app-level modes like `Search` or `ConfirmQuit`. The vim state machine lives inside the `Editor` struct, not in `App`.

**Detection:** If you find yourself writing `if self.vim_mode == Normal` inside the existing `handle_key()` match arms, you are going down the wrong path. The entire `handle_key()` method body should be replaced.

**Specific to this codebase:** The `EditorAction` enum currently has 3 variants (`Save`, `RequestQuit`, `ContentChanged`). Vim needs to signal additional actions: `ModeChanged(Mode)` for status bar updates, and potentially `YankedText(String)` for clipboard integration. Extend `EditorAction` rather than adding mode checks in `App`.

---

### Pitfall 2: Custom Render Path (render_highlighted) Ignores Vim Cursor Shape

**What goes wrong:** The current `render_highlighted()` method in `editor.rs` places the cursor via `frame.set_cursor_position()` at line 503 -- this produces a blinking line cursor (the terminal default). Vim users expect a block cursor in Normal mode, a line cursor in Insert mode, and a highlighted range in Visual mode. Since `render_highlighted()` completely bypasses `TextArea::widget()` (the standard ratatui-textarea rendering), none of the cursor shape APIs from ratatui-textarea apply.

**Why it happens:** The custom render path was built for syntax highlighting overlay (v1 decision D-09). It manually constructs `Paragraph` widgets from highlighted spans and manually positions the cursor. There is no mechanism in this custom path to render a block cursor (a character with inverted colors) vs. a line cursor.

**Consequences:**
- Vim Normal mode looks identical to Insert mode -- users cannot tell what mode they are in by looking at the cursor
- Visual mode selection needs both a highlighted range AND a cursor position, which the current `apply_highlight_overlay()` function could handle but `frame.set_cursor_position()` cannot show both
- Terminal cursor shape escape sequences (`\e[1 q` for block, `\e[5 q` for bar) are unreliable across terminals and conflict with ratatui's cursor management

**Prevention:**
1. **Render the block cursor as an inverted-color character in the Paragraph.** In Normal mode, instead of calling `frame.set_cursor_position()`, style the character under the cursor with foreground/background color swap (inversion). This works universally because it is just styled text, not terminal cursor escape sequences.
2. **In Insert mode, continue using `frame.set_cursor_position()`** for the blinking line cursor (current behavior).
3. **In Visual mode, use `apply_highlight_overlay()`** (already exists in `editor.rs`!) for the selection range, plus the inverted block cursor for the cursor position.
4. **Add a `cursor_style()` method to `Editor`** that returns the rendering strategy based on vim mode. The `render_highlighted()` function checks this instead of always calling `set_cursor_position()`.

**Detection:** If Normal mode editing feels visually identical to Insert mode, the cursor shape is wrong.

---

### Pitfall 3: WYSIWYG Source-to-Rendered Position Mapping Is Unsolved

**What goes wrong:** WYSIWYG terminal editing requires the user to edit rendered markdown (bold text appears bold, headers are styled) while the underlying storage remains raw markdown. This means every cursor position in the rendered view must map bidirectionally to a byte offset in the source markdown. No existing Rust crate provides this mapping. This is the hardest problem in the entire v2.0 feature set.

**Why it happens:** Markdown rendering is a many-to-one transformation:
- `**bold**` (8 source chars) renders as `bold` (4 rendered chars with bold styling)
- `## Title` (8 chars) renders as `Title` (5 chars) with heading style
- A table's `| cell |` pipes have no visual presence in rendered output
- Wrapped lines: a single source line may become 3 rendered lines at terminal width
- Nested formatting `***bold italic***` (19 chars) renders as `bold italic` (11 chars)

**Consequences:**
- Pressing Backspace on rendered `bold` must delete in `**bold**` at the correct source offset
- Pressing Enter must insert `\n` at the correct source position, not the rendered position
- Cursor up/down in rendered view must skip over collapsed markup characters
- Undo/redo must work on source offsets, not rendered offsets
- If the mapping is off by even one byte, edits corrupt the document silently

**Prevention:**
1. **Build a bidirectional position map during rendering.** pulldown-cmark events carry source byte offsets via `into_offset_iter()`. Build a `Vec<SourceMapping>` that records `(rendered_line, rendered_col) -> (source_byte_start, source_byte_end)` for every rendered span.
2. **Start with a "reveal on cursor" approach** rather than true WYSIWYG. When the cursor enters a formatted region, temporarily show the raw markdown syntax (like Obsidian's live preview mode). This sidesteps the full bidirectional mapping problem and still feels WYSIWYG to users.
3. **Do NOT attempt WYSIWYG editing of tables.** Table source format is too structurally different from rendered format. Show tables rendered but switch to source editing when the cursor enters a table region.
4. **Treat WYSIWYG as a separate rendering mode, not a modification of the existing editor.** The existing `Editor` struct edits source markdown. WYSIWYG needs a `WysiwygEditor` that wraps it with a position-mapping layer.

**Detection:** If you are trying to make `tui-markdown`'s output editable, stop. `tui-markdown` is read-only and explicitly experimental. WYSIWYG requires a completely different rendering pipeline that preserves source mappings.

**Specific to this codebase:** The `MarkdownRenderer` trait in `markdown/mod.rs` returns `Text<'static>` with no position information. WYSIWYG needs a different trait method that returns `Text` plus a `PositionMap`. Do not retrofit position mapping into `TuiMarkdownRenderer`.

---

### Pitfall 4: Adding tokio for Browser Companion Breaks the Synchronous Event Loop

**What goes wrong:** The current `main.rs` uses `ratatui::run()` which owns the terminal and runs a synchronous event loop with `crossterm::event::poll()` (50ms timeout) and `event::read()`. Adding a browser companion requires an HTTP server (serving rendered HTML) and a WebSocket server (live updates). Both need async I/O. Developers add `#[tokio::main]`, and discover that `crossterm::event::read()` blocks the tokio runtime, or that `ratatui::run()` does not compose with a tokio context, or that the async runtime's thread pool fights with TUI terminal ownership.

**Why it happens:** `ratatui::run()` is designed for synchronous single-threaded apps. It initializes the terminal, runs the closure, then restores the terminal. The spotify-tui project documented this exact problem when migrating from sync to async.

**Consequences:**
- `event::read()` blocks the entire tokio thread, starving the HTTP server of CPU time
- If you spawn the HTTP server on a separate OS thread, sharing state requires `Arc<Mutex<>>` on document content, and Mutex contention causes dropped frames
- If the server panics (port already in use), it can leave the terminal in raw mode
- Binary size increases: tokio runtime adds ~1-2MB to the stripped binary (currently under 10MB constraint)

**Prevention:**
1. **Run the HTTP server on a separate OS thread** using `std::thread::spawn()` with its own single-threaded tokio runtime (`tokio::runtime::Builder::new_current_thread()`). The TUI event loop stays fully synchronous.
2. **Communicate via `std::sync::mpsc` channels, not shared mutable state.** When content changes, the TUI sends the rendered HTML string through the channel. The server thread picks it up and pushes to WebSocket clients. No Mutex needed.
3. **Use a lightweight HTTP server** gated behind a `browser` Cargo feature flag. `tiny_http` is pure sync and needs no tokio at all. If using axum, isolate tokio as a dev-dependency of only the browser feature.
4. **Handle server startup failure gracefully.** If the port is taken, show a status bar message ("Browser companion: port 8234 in use") and continue without the server. Never let server failure crash the TUI.
5. **Make the browser companion opt-in** (`--browser` flag). Do not start an HTTP server by default for a terminal editor.

**Detection:** If you find yourself adding `#[tokio::main]` to the existing `main()`, you are coupling the async runtime to the TUI lifecycle. Keep them on separate threads.

**Specific to this codebase:** The `ratatui::run()` call in `main.rs` is a convenience wrapper. To spawn the server thread before entering the event loop, replace it with manual `Terminal::new()` + `enable_raw_mode()` + cleanup, which gives you control over initialization order.

---

## Moderate Pitfalls

### Pitfall 5: OSC 52 Clipboard Is a Minefield of Terminal Incompatibility

**What goes wrong:** OSC 52 is the escape sequence for clipboard access, crucial for SSH use. But terminal support is wildly inconsistent, detection is unreliable, and size limits silently truncate.

**Known compatibility matrix (verified across Neovim issues, real-world reports, and terminal docs):**

| Terminal | OSC 52 Write (Copy) | OSC 52 Read (Paste) | Notes |
|----------|---------------------|---------------------|-------|
| iTerm2 | Yes (must enable) | No | Preferences > General > Selection > "Apps may access clipboard" |
| macOS Terminal.app | No | No | No support at all |
| Kitty | Yes (default) | No (must enable) | `clipboard_control` config for read |
| Alacritty | Yes | No | Write only |
| GNOME Terminal / VTE | No | No | No support |
| Windows Terminal | Yes | No | Write only since v1.x |
| tmux | Forwarded if `set-clipboard on` | No | Stripped by default! |
| mosh | Partial | No | tmux + mosh = broken unless specific options |

**Size limit:** OSC 52 maximum is 100,000 bytes (~74,994 bytes payload after base64).

**Detection is broken:** Neovim uses XTGETTCAP to detect OSC 52 support, but this query writes garbage on older terminals. A newer DA1 feature-52 approach exists but adoption is very low.

**Prevention:**
1. **Use a fallback chain:** Try OSC 52 for write, fall back to platform-native (`pbcopy`/`pbpaste` on macOS, `xclip`/`xsel`/`wl-copy`/`wl-paste` on Linux). Never rely on OSC 52 alone.
2. **Do NOT attempt to READ the clipboard via OSC 52.** Reading is disabled by default in most terminals for security. For paste, rely on the terminal's native paste which arrives as bracketed paste events via crossterm.
3. **Handle the size limit.** If the user copies >74KB, fall back to platform clipboard or show a warning.
4. **Do not auto-detect OSC 52 support.** Let users configure clipboard method in TOML config: `clipboard = "osc52" | "native" | "auto"`.
5. **Test with tmux explicitly.** Most SSH users run tmux. If OSC 52 does not work in default tmux config, document the required `set -g set-clipboard on` setting.

**Specific to this codebase:** Ctrl+C is currently a no-op (editor.rs line 265). Vim mode will use `y` in Visual mode for yank. Clipboard integration must be ready BEFORE vim yank is implemented, or yank will silently discard text with nowhere to put it.

---

### Pitfall 6: Theme Configuration That Ignores Terminal Color Capabilities

**What goes wrong:** Adding TOML color themes sounds simple. But terminals have three color tiers (16, 256, truecolor/24-bit), and themes specifying `Color::Rgb(68, 68, 102)` (already used in current codebase for selection highlight) render as garbage on 16-color terminals.

**Current codebase exposure:** The following hardcoded colors already break on 16-color terminals:
- `editor.rs`: `Color::Rgb(68, 68, 102)` for selection background
- `editor.rs`: `Color::Cyan`, `Color::Yellow` for search highlights
- `highlighter.rs`: `base16-ocean.dark` theme emits RGB colors via `convert_syntect_style()`
- `app.rs`: `Color::DarkGray` for divider, `Color::Red`/`Color::Blue`/`Color::White` for status bar prompts

**Prevention:**
1. **Detect terminal color capability at startup.** Check `$COLORTERM` env var (`truecolor` or `24bit`), check `$TERM` for `256color`, fall back to 16-color baseline.
2. **Ship three theme tiers:** 16-color (ANSI named colors only), 256-color (indexed), truecolor (RGB). Auto-select based on detected capability.
3. **For TOML format, accept both named ANSI colors and hex RGB.** If user specifies hex and terminal is 16-color, map to nearest ANSI color at load time.
4. **Separate syntect themes from UI themes.** Syntect controls syntax highlighting colors. UI theme controls borders, status bar, selection, cursor. These must be independently configurable -- a single "theme" object that tries to do both becomes confusing.
5. **All current hardcoded colors must become theme values.** Extract every `Color::Rgb(...)` and `Color::Cyan` etc. into a `Theme` struct that is loaded from config.

**Detection:** SSH into a `TERM=xterm` (16-color) session and check if the editor is usable.

---

### Pitfall 7: Vim Visual Mode Selection Conflicts with Existing Shift-Arrow Selection

**What goes wrong:** The v1 editor has Shift+Arrow selection (editor.rs lines 102-153) using `textarea.start_selection()` and `textarea.move_cursor()`. Vim Visual mode ALSO does selection, triggered by `v` in Normal mode with `hjkl` extending. Both use the same underlying `TextArea` selection API. If both coexist without coordination, entering Visual mode while a Shift-selection is active (or vice versa) produces ghost selections, double-selections, or panics from conflicting selection state.

**Prevention:**
1. **When vim mode is active, disable Shift-arrow selection entirely.** Vim users use Visual mode (`v`, `V`, `Ctrl+V`), not Shift+Arrow. Gate all Shift-arrow code on `if !vim_mode_active`.
2. **When entering Visual mode (`v`), call `textarea.start_selection()`.** When exiting (Esc, completing an operation), call `textarea.cancel_selection()`.
3. **Visual Line mode (`V`) is not the same as character Visual mode.** It selects entire lines. `TextArea` has no "line selection" mode -- you must manually select from column 0 of start line to end of end line.
4. **Visual Block mode (`Ctrl+V`) is extremely complex** and not supported by `TextArea` at all. Defer to a later version or omit entirely.

---

### Pitfall 8: Debounced Preview Breaks Under Rapid Vim Operations

**What goes wrong:** The current preview uses 80ms debounce in `maybe_update_preview()`. In vim, operations like `dd` (delete line), `p` (paste), `5dd` (delete 5 lines) happen as instant bursts. A user doing `5dd` followed immediately by `u` (undo) may see the preview flash the deleted state before showing the restored state, creating a jarring visual artifact.

**Prevention:**
1. **Trigger immediate preview update for "bulk" operations** (delete line, paste, undo, redo) by having those operations set a `force_preview_update` flag instead of relying on the debounce timer. The debounce remains for character-by-character Insert mode typing.
2. **Or reduce debounce to 30-40ms for Normal mode.** Normal mode operations are complete immediately (no more keystrokes coming in the next few ms), unlike Insert mode where the user is typing continuously.

---

## Minor Pitfalls

### Pitfall 9: Vim Count Prefix Handling Requires Accumulator State

**What goes wrong:** Vim commands accept numeric prefixes (`5dd` = delete 5 lines, `3w` = move 3 words). Users expect this immediately. The count must accumulate across keystrokes (pressing `5` then `d` then `d`), requiring a `count: Option<usize>` field in the state machine.

**Prevention:** Add count accumulator to the vim state machine from day one. When a digit is pressed in Normal mode (and it is not `0` at the start of a new command, since `0` is "go to line start"), accumulate into the count. Apply count to subsequent motion or operation. The ratatui-textarea vim example handles this.

---

### Pitfall 10: Browser Companion WebSocket Reconnection Sends Blank Page

**What goes wrong:** The browser shows rendered HTML via WebSocket. If the browser disconnects (tab sleep, network glitch) and reconnects, it receives nothing until the next edit. The browser shows a blank page or stale content.

**Prevention:** On every WebSocket connect, immediately send the current rendered HTML. Do not wait for the next content change. Keep the latest rendered HTML cached on the server thread for this purpose.

---

### Pitfall 11: Config File Location Platform Differences

**What goes wrong:** The TOML config needs a standard location. `~/.config/mdedit/config.toml` works on Linux but is non-standard on macOS (which uses `~/Library/Application Support/`).

**Prevention:** Use the `dirs` crate (`dirs::config_dir()`) to get the platform-appropriate directory. Also support `$MDEDIT_CONFIG` env var for override. Document the default path for each platform.

---

### Pitfall 12: Vim Registers Scope Creep

**What goes wrong:** Vim has 26 named registers, default register, yank register, 9 numbered delete registers, and special registers (`"+` for clipboard, `"*` for selection). Implementing all of them is months of work.

**Prevention:** Implement ONLY: (1) the unnamed register (default yank/delete buffer), (2) `"+` mapped to system clipboard via the clipboard integration. Add named registers later if users request them. This covers 95% of real usage.

---

### Pitfall 13: Vim Command-Line Mode (:w, :q, :wq) Conflicts with AppMode

**What goes wrong:** Vim users expect `:w` to save, `:q` to quit, `:wq` to save-and-quit. This requires a command-line input mode (type `:` in Normal mode, status bar becomes a text input). The current `AppMode` enum has `PromptFilename` which is similar but not the same -- `:` commands need parsing, argument handling, and potentially tab completion.

**Prevention:**
1. **Add a new `AppMode::CommandLine` variant** -- do not overload `PromptFilename`.
2. **Start with just `:w`, `:q`, `:wq`, `:q!`.** These map directly to existing `EditorAction::Save` and `EditorAction::RequestQuit`. Defer `:set`, `:s/find/replace/`, etc.
3. **Ctrl+S and Ctrl+Q should continue to work** even in vim mode. Many vim users in terminal apps expect both interfaces.

---

### Pitfall 14: Mouse Support Interacts Badly with Vim Modes

**What goes wrong:** Mouse click to position cursor is natural in Insert mode but confusing in Normal mode (should it enter Insert mode? stay in Normal?). Mouse drag to select conflicts with Visual mode. Mouse scroll needs to work the same in all modes.

**Prevention:**
1. **Mouse click positions cursor but does NOT change vim mode.** Clicking in Normal mode moves cursor, stays in Normal mode.
2. **Mouse drag creates a Visual mode selection.** If in Normal mode, mouse drag enters Visual mode. If already in Visual mode, mouse drag adjusts selection.
3. **Mouse scroll always scrolls the viewport** regardless of mode, matching terminal convention.

---

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation | Severity |
|-------------|---------------|------------|----------|
| Vim keybindings | #1 (architecture), #2 (cursor shape), #7 (selection conflict) | Build state machine first, replace key dispatch entirely, render block cursor as inverted text | Critical |
| Vim keybindings | #9 (count prefixes), #12 (registers), #13 (command-line) | Include count from start, scope registers minimal, add :w/:q only | Minor |
| WYSIWYG mode | #3 (position mapping) | Start with "reveal on cursor" approach, defer true WYSIWYG for tables | Critical |
| Theme configuration | #6 (color tiers), #11 (config location) | Detect capabilities, ship multi-tier themes, use `dirs` crate | Moderate |
| Browser companion | #4 (async vs sync), #10 (reconnection) | Separate OS thread with channel, send current state on connect | Critical |
| Clipboard integration | #5 (OSC 52 compatibility) | Fallback chain, write-only OSC 52, config toggle | Moderate |
| Preview rendering | #8 (debounce under vim ops) | Force-update flag for bulk operations | Minor |
| Mouse support | #14 (mouse + vim mode interaction) | Click positions cursor only, drag enters Visual, scroll is mode-independent | Minor |

---

## Sources

- [ratatui-textarea vim example](https://github.com/ratatui/ratatui-textarea) - Modal editing state machine pattern with Mode/Transition/pending architecture
- [edtui - vim-inspired editor widget](https://github.com/preiter93/edtui) - EditorState/EditorView separation pattern
- [Neovim OSC 52 detection issue #34472](https://github.com/neovim/neovim/issues/34472) - DA1 feature 52 detection proposal, XTGETTCAP problems
- [OSC 52 journey (miek.nl)](https://miek.nl/2024/january/31/osc52-my-cut-paste-journey/) - Real-world OSC 52 terminal compatibility matrix
- [Clipboards, Terminals, and Linux (dev.to)](https://dev.to/djmitche/clipboards-terminals-and-linux-3pk5) - Terminal clipboard fragmentation overview
- [Windows Terminal OSC 52 issue #2946](https://github.com/microsoft/terminal/issues/2946) - Write-only support, no paste via OSC 52
- [Windows Terminal paste OSC 52 issue #9479](https://github.com/microsoft/terminal/issues/9479) - Paste feature request still open
- [tmux + mosh OSC 52 hack (GitHub gist)](https://gist.github.com/yudai/95b20e3da66df1b066531997f982b57b) - Multiplexer interop broken by default
- [tmux OSC 52 clipboard guide (sunaku.github.io)](https://sunaku.github.io/tmux-yank-osc52.html) - set-clipboard configuration requirement
- [Ratatui async event stream tutorial](https://ratatui.rs/tutorials/counter-async-app/async-event-stream/) - Async integration patterns for ratatui
- [Ratatui + Axum todo app](https://dev.to/sebyx07/building-a-multi-interface-todo-app-with-rust-ratatui-and-axum-1cke) - TUI + HTTP server on separate threads
- [spotify-tui async migration](https://keliris.dev/articles/improving-spotify-tui) - Blocking event::read() starving async runtime
- [Yazi theme override issue #1407](https://github.com/sxyazi/yazi/issues/1407) - syntect theme precedence/override problems
- [syntect-tui](https://github.com/chanq-io/syntect-tui) - syntect-to-ratatui style translation layer
- [osc52pty workaround](https://github.com/roy2220/osc52pty) - Terminal.app OSC 52 workaround via pty proxy

---
*Pitfalls research for: mdedit v2.0 power user features*
*Researched: 2026-03-22*
