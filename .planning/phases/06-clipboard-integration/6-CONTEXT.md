# Phase 6: Clipboard Integration - Context

**Gathered:** 2026-03-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Users can copy and paste text to/from the system clipboard, including over SSH. Vim yank/paste and nano Ctrl+C/Ctrl+V both sync with the system clipboard. The internal yank register (from Phase 5) becomes the bridge between editor operations and system clipboard. No new editing features, no new keybindings beyond nano-mode Ctrl+C/Ctrl+V.

</domain>

<decisions>
## Implementation Decisions

### Clipboard provider strategy
- **D-01:** OSC 52 is the primary clipboard mechanism — used first in all environments (local and SSH). Platform-native tools are the fallback.
- **D-02:** Platform-native fallback order: macOS → `pbcopy`/`pbpaste`, Linux Wayland (`$WAYLAND_DISPLAY` set) → `wl-copy`/`wl-paste`, Linux X11 → `xclip`/`xsel`.
- **D-03:** Fully automatic detection — no `clipboard` config knob. Provider is selected at startup and cached.
- **D-04:** If no clipboard mechanism works, show a one-time status bar warning on first yank: "System clipboard unavailable — using internal register". Internal yank/paste continues to work.

### Yank/paste integration
- **D-05:** Every yank AND delete operation writes to system clipboard — `y`, `yy`, `dd`, `cc`, `x`, visual yank, visual delete all sync out. Matches vim `clipboard=unnamedplus` behavior.
- **D-06:** `p`/`P` reads from system clipboard — text copied in external applications is immediately available via vim paste.
- **D-07:** Nano mode gets clipboard support: Ctrl+C copies current selection to system clipboard, Ctrl+V pastes from system clipboard.
- **D-08:** Delete operations (`dd`, `x`, etc.) write to system clipboard. This is standard vim behavior — deletes overwrite the clipboard.

### OSC 52 behavior
- **D-09:** Write-only OSC 52 — mdedit writes to clipboard via OSC 52 but does NOT attempt OSC 52 read-back. Paste relies on the terminal's own bracketed paste handling (user presses Cmd+V / Ctrl+Shift+V and the terminal sends the text).
- **D-10:** No size limit handling — payloads are base64-encoded and sent as-is. Terminal truncation is the terminal's problem.
- **D-11:** No OSC 52 capability detection — just send the sequence. Modern terminals support it; document "use a modern terminal" if clipboard doesn't work.
- **D-12:** Auto-detect tmux via `$TMUX` env var and wrap OSC 52 in tmux passthrough escape (`\ePtmux;...\e\\`). Standard practice (neovim, zellij, yazi all do this).

### Claude's Discretion
- Clipboard provider trait/enum design
- How to structure the subprocess calls to pbcopy/xclip (spawn + pipe stdin, or write to temp file)
- Error handling for failed subprocess calls
- Exact status bar warning message wording and display duration
- How bracketed paste integrates with vim insert mode vs normal mode
- Whether to detect `$TMUX` at startup or per-operation

</decisions>

<specifics>
## Specific Ideas

- User doesn't care about supporting old infrastructure — OSC 52 first is fine
- Behavior should match vim `clipboard=unnamedplus` — every yank/delete syncs, paste reads system clipboard
- User wants Wayland and X11 both supported on Linux, not legacy-only

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` — CLIP-01 through CLIP-04 define the four clipboard requirements

### Prior phase context
- `.planning/phases/05-vim-keybindings-and-mouse/5-CONTEXT.md` — D-04 (yank register design), D-06 (yank/paste operators), D-15 (visual mode yank)
- `src/vim.rs` — `VimHandler.yank_register`, `set_yank_register()`, `yank_register()`, `VimCommand::Yank`, `VimCommand::PasteAfter`, `VimCommand::PasteBefore`, `VimCommand::VisualYank`
- `src/app.rs` — `execute_vim_command()` dispatcher, `execute_vim_operator_yank()`, all yank/paste handling logic
- `src/editor.rs` — `yank_current_line()`, `delete_current_line()` return yanked text

### Project context
- `.planning/PROJECT.md` — Constraints (single binary, <10MB, works over SSH)
- `.planning/STATE.md` — Phase 5 decisions on VimCommand architecture

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `VimHandler.yank_register` / `set_yank_register()` — Internal buffer that currently stores yanked text. Phase 6 adds system clipboard sync alongside this.
- `execute_vim_command()` in `app.rs` — Central dispatcher for all vim commands. Every yank/delete path already calls `set_yank_register()` — these are the exact integration points for clipboard write.
- `execute_vim_operator_yank()` in `app.rs` — Yank-specific operator handler, returns text to register.
- `Editor::yank_current_line()` / `Editor::delete_current_line()` — Return the text content, ready to pipe to clipboard.

### Established Patterns
- `VimCommand` enum dispatched in `app.rs` — clipboard write hooks into existing match arms
- `input_without_shortcuts()` for text input — nano-mode Ctrl+C/Ctrl+V will need to be routed before this call
- Status bar already shows mode and messages — clipboard warning fits existing pattern

### Integration Points
- Every `set_yank_register()` call site — add clipboard write after setting register (~15 call sites in app.rs)
- `VimCommand::PasteAfter` / `VimCommand::PasteBefore` — read from system clipboard instead of (or in addition to) internal register
- `Editor::handle_key()` in nano mode — add Ctrl+C and Ctrl+V handling
- `main.rs` terminal init — no changes needed (OSC 52 is just stdout writes)

</code_context>

<deferred>
## Deferred Ideas

- Named registers (`"a`, `"b`, etc.) — v3+ (vim power user feature)
- Config knob to force clipboard provider — add if users report issues
- OSC 52 read-back for full round-trip over SSH — add if terminal support improves
- Clipboard history / ring buffer — out of scope

</deferred>

---

*Phase: 06-clipboard-integration*
*Context gathered: 2026-03-23*
