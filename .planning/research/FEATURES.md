# Feature Landscape: v2.0 Power User

**Domain:** Terminal markdown editor -- vim keybindings, WYSIWYG mode, themes, browser companion, clipboard
**Researched:** 2026-03-22
**Supersedes:** v1.0 research (2026-03-21)

---

## Category 1: Vim-Style Keybindings (Modal Editing)

### What This Means

Replace the current nano-style keybindings with vim-style modal editing as the **default** mode. Three modes: Normal (navigation/commands), Insert (text entry), and Visual (selection). The mode indicator shows in the status bar. Escape always returns to Normal mode.

### How TUI Editors Implement This

The ecosystem offers three approaches:

1. **edtui** (recommended path): A purpose-built vim-inspired editor widget for ratatui. Supports Normal/Insert/Visual modes with hjkl navigation, word motions (w/e/b), delete (dd/d), yank (y/yy), paste (p), line operations (J, o/O). Has clipboard integration, syntax highlighting, mouse support, and custom theming. 346 commits, 127 stars. Downside: 134 downloads/month -- small community, and replacing ratatui-textarea means losing all the custom rendering infrastructure built in v1 (syntax highlighting overlay, search highlight overlay, selection overlay).

2. **Custom modal layer on top of ratatui-textarea**: Keep ratatui-textarea for the actual text buffer and input handling via `input_without_shortcuts()`. Build a modal state machine on top that translates vim keystrokes into ratatui-textarea `CursorMove` and editing operations. This preserves the entire v1 custom rendering pipeline (syntax highlighting, search overlays, selection overlays). Most TUI editors with vim bindings use this pattern -- a state machine that maps vim grammar (operator + motion) to underlying editor operations.

3. **modalkit-ratatui**: Heavy-duty framework for building full modal editing apps (used by iamb, a Matrix chat client). Overkill for adding vim keybindings to an existing editor. Designed for building apps from scratch with modal editing as the core abstraction.

### Existing Architecture Impact

The v1 editor uses `ratatui-textarea` with `input_without_shortcuts()` and a custom `handle_key()` that manually maps nano-style bindings to `CursorMove` operations. The custom `render_highlighted()` method bypasses tui-textarea's built-in Widget rendering to layer syntax highlighting, search highlights, and selection overlays via `apply_highlight_overlay()`.

**This is the critical dependency.** The custom rendering path (editor.rs lines 402-505) is ~100 lines of carefully built overlay logic. Switching to edtui would mean rebuilding all of this. Keeping ratatui-textarea and adding a modal layer preserves it entirely.

### Table Stakes for Vim Mode

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Normal/Insert/Visual modes | Fundamental vim contract -- users expect modal editing | Med | State machine with mode enum |
| hjkl navigation in Normal mode | The defining vim interaction | Low | Map to existing CursorMove ops |
| i/a/o/O to enter Insert mode | Standard vim insert entries | Low | i=before cursor, a=after, o=new line below, O=above |
| Escape to return to Normal mode | Universal vim expectation | Low | Already used for search exit |
| dd (delete line), d+motion (delete) | Core vim editing | Med | Compose operator + motion |
| yy (yank line), y+motion (yank), p/P (paste) | Core vim clipboard | Med | Internal yank buffer (not system clipboard) |
| w/e/b word motions | Standard vim navigation | Low | Map to CursorMove::WordForward/WordBack |
| 0/$/^ line position | Line navigation | Low | Map to CursorMove::Head/End |
| gg/G document start/end | Document navigation | Low | Map to CursorMove::Top/Bottom |
| Visual mode (v) with motion-based selection | Text selection by vim grammar | Med | Integrate with existing selection_byte_range() |
| Visual line mode (V) | Select entire lines | Low | Common for delete/yank operations |
| Mode indicator in status bar | Users must know current mode | Low | "NORMAL", "INSERT", "VISUAL" display |
| / for search (in Normal mode) | Vim users search with /, not Ctrl+F | Low | Route to existing search infrastructure |
| : for command mode (at minimum :w, :q, :wq, :q!) | Core vim workflow | Med | Minimal command parser |
| u for undo, Ctrl+R for redo | Vim undo/redo bindings | Low | Map to existing textarea undo/redo |
| Numeric prefixes (5j = move down 5) | Vim users expect counts | Med | Parse digit prefix, repeat motion N times |

### Differentiators for Vim Mode

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Dot repeat (.) | Repeat last change -- huge productivity boost | High | Must record last edit action and replay |
| ci"/ca"/di" (change/delete inside/around) | Text object operations -- power vim feature | High | Requires text object parser |
| % to jump matching bracket | Useful in code blocks | Low | Scan for matching pair |
| Ctrl+D/Ctrl+U half-page scroll | Comfortable vim scrolling | Low | Straightforward |
| Marks (ma, 'a) | Bookmarks in document | Med | Store named positions |

### Anti-Features for Vim Mode

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Full vim compatibility | Vim has 40 years of features. Chasing parity is infinite scope | Implement core 80% that covers 99% of editing. Users wanting full vim should use Neovim |
| Registers (a-z, 0-9) | Complex clipboard system most vim users barely use | Single unnamed register + system clipboard |
| Macros (q recording) | Recording and replay adds significant state management | Defer to v3 if ever |
| Ex commands beyond :w/:q/:wq | Most : commands are rarely used | Support the essential 4-5 commands only |
| Split windows within mdedit | Vim has :split/:vsplit. mdedit already has editor/preview split | The preview IS the split. Don't add vim-style window splits |
| .vimrc compatibility | Reading vim config is unbounded complexity | Own config format in TOML |

### Recommendation

**Build a custom modal layer on top of ratatui-textarea.** Do not switch to edtui. The v1 custom rendering pipeline (syntax highlighting overlays, search highlights, selection overlays) is ~100 lines of carefully tested code that works. edtui would require rebuilding all of it. The modal layer is a state machine that translates vim grammar into CursorMove and editing operations that ratatui-textarea already supports.

**Complexity: HIGH.** The vim state machine (operator-pending mode, motions, counts, text objects) is the single most complex feature in v2. Budget significant time.

---

## Category 2: WYSIWYG Terminal Editing Mode

### What This Means

A `--wysiwyg` flag that renders markdown inline while editing. Instead of raw `## Heading` text on the left and rendered preview on the right, the user sees a single pane where `## Heading` appears as a large bold heading, but the cursor can still navigate into it and edit the raw characters. Think Obsidian's Live Preview or Neovim's render-markdown.nvim.

### How This Works in Practice

No terminal-based WYSIWYG markdown editor exists as a standalone tool. The closest implementations are Neovim plugins:

- **render-markdown.nvim**: Renders markdown in-buffer with conceals. Headings get colored backgrounds, bullet points become Unicode symbols, code blocks get background colors. The raw markdown is hidden via Neovim's conceal mechanism and revealed when the cursor enters that line.
- **markview.nvim**: Similar approach with decorations overlaid on the buffer text.

The key technique is **conceal-on-idle, reveal-on-cursor**: rendered (styled) text is displayed everywhere except the line the cursor is currently on. The cursor line shows raw markdown so the user can edit it directly.

### Implementation Approach

This requires a fundamentally different rendering path from the current split-view:

1. **Single-pane mode**: When `--wysiwyg` is active, no split. Full terminal width is one editable pane.
2. **Line-by-line rendering**: Each line is parsed through pulldown-cmark and rendered with ratatui styles (bold, italic, colored headings, indented lists, etc.).
3. **Cursor line exception**: The line containing the cursor renders as raw markdown text (what the user typed), so editing works normally.
4. **All other lines**: Rendered as styled output -- headings become bold/colored, `**bold**` becomes bold text without asterisks, lists get Unicode bullets, code blocks get backgrounds.
5. **Cursor positioning challenge**: When the cursor moves to a concealed line, the raw text is longer than the displayed text (e.g., `**bold**` displays as `bold`). The cursor column must be translated between raw and rendered positions.

### Table Stakes for WYSIWYG

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Headings rendered as styled text | Most visible markdown element | Med | Bold, colored, sized by level (H1-H6) |
| Bold/italic rendered inline | Core formatting | Med | Conceal markers, apply style |
| Lists with Unicode bullets | Visual improvement over raw `-` | Low | Replace `-`/`*` with bullet characters |
| Code blocks with background | Distinguish code from prose | Med | Apply background color to fenced blocks |
| Cursor line shows raw markdown | Editing must work -- this is the core contract | High | Switch rendering mode per-line based on cursor position |
| Links show text only (conceal URL) | Clean reading experience | Med | `[text](url)` shows as styled `text` |

### Differentiators for WYSIWYG

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Smooth transition animation | Cursor line reveal/conceal feels polished | Med | Cross-fade between raw and rendered |
| Blockquote styling with side border | Visual distinction for quotes | Low | Unicode box-drawing left border |
| Table rendering with aligned columns | Tables readable while editing | High | Column width calculation, alignment |
| Task list checkboxes | `- [ ]` shows as checkbox | Low | Unicode checkbox characters |
| Horizontal rules rendered | `---` shows as a line | Low | Unicode horizontal line character |

### Anti-Features for WYSIWYG

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| True WYSIWYG (editing rendered text) | Editing styled output requires mapping every cursor position between raw and rendered. Cursor in the middle of a "bold" word -- which raw character is that? Unbounded complexity | Conceal-on-idle: render everywhere except cursor line |
| Image rendering | Terminal image protocols (Kitty, Sixel, iTerm) are fragmented | Show `[image: alt-text]` placeholder |
| Nested formatting (bold italic code) | Combinatorial complexity in conceal mapping | Support one level of formatting per span |
| Custom block rendering | Math blocks, diagrams, etc. | Show as styled code blocks |

### Recommendation

**Implement as conceal-on-idle with cursor-line raw reveal.** This is the proven pattern from Neovim plugins. The `--wysiwyg` flag activates a different rendering path that replaces the current custom `render_highlighted()` with a WYSIWYG renderer that applies pulldown-cmark styling per-line, except for the cursor line.

**Complexity: HIGH.** The cursor position mapping between raw and rendered text is the hardest problem. Start with a simpler v1 that only conceals on non-cursor lines and reveals entirely on the cursor line (no partial concealment).

**Dependency:** Requires the vim keybindings to be working first, since WYSIWYG mode is a rendering concern layered on top of the editing model.

---

## Category 3: Configurable Color Themes

### What This Means

A TOML config file (likely `~/.config/mdedit/config.toml`) that lets users customize colors for the editor, preview, and status bar. Ship with 2-3 built-in themes and let users define custom ones.

### How Terminal Apps Do Themes

Two approaches in the ratatui ecosystem:

1. **Syntect themes for syntax highlighting + custom theme struct for UI**: Syntect already bundles several themes (base16-ocean.dark is currently used). The UI chrome (status bar, divider, line numbers, selection highlight) uses hardcoded `Color::DarkGray`, `Color::Rgb(68, 68, 102)`, etc. A theme system would externalize all of these.

2. **Color parsing**: Ratatui's `Color` enum supports named colors (e.g., "Red"), indexed colors (e.g., "10"), and hex colors (e.g., "#FF0000"). The `color-to-tui` crate can parse string representations into ratatui `Color` values. This means TOML config can use human-readable color names.

### Current Hardcoded Colors in v1

From the codebase:

| Element | Current Color | Location |
|---------|--------------|----------|
| Line numbers | `Color::DarkGray` | editor.rs:47 |
| Selection highlight | `Color::Rgb(68, 68, 102)` | editor.rs:444 |
| Search match (active) | `Color::Cyan` bg, `Color::Black` fg | editor.rs:464 |
| Search match (other) | `Color::Yellow` bg, `Color::Black` fg | editor.rs:466 |
| Cursor line | `Modifier::UNDERLINED` | editor.rs:485 |
| Divider | `Color::DarkGray` | app.rs:449 |
| Status bar (confirm quit) | `Color::Red` bg, `Color::White` fg | app.rs:483 |
| Status bar (prompt) | `Color::Blue` bg, `Color::White` fg | app.rs:489 |
| Status bar (search) | `Color::Blue` bg, `Color::White` fg | app.rs:502 |
| Syntax highlighting | base16-ocean.dark theme | highlighter.rs:22 |

### Table Stakes for Themes

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Config file at `~/.config/mdedit/config.toml` | Standard XDG config location | Low | Use `dirs` crate for platform-appropriate path |
| 2-3 built-in themes (dark, light, minimal) | Users expect choice out of the box | Med | Define theme structs, ship as defaults |
| Syntax highlighting theme selection | syntect has ~10 built-in themes | Low | Map theme name in config to syntect ThemeSet |
| Editor chrome colors (line numbers, cursor line, selection) | Everything currently hardcoded needs to be themeable | Med | Extract all Color constants into a Theme struct |
| Status bar colors | Currently hardcoded per-mode | Low | Part of Theme struct |
| Hex color support in config | `fg = "#FF5733"` | Low | ratatui parses hex natively |
| Named color support | `fg = "red"` | Low | ratatui parses named colors |

### Differentiators for Themes

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Theme hot-reload | Change config, see result without restart | Med | Watch config file, reload on change |
| `:theme <name>` command | Switch themes at runtime (vim command mode) | Low | If vim command mode exists, this is easy |
| Preview pane theme (independent) | Style preview differently from editor | Low | Separate section in config |
| Terminal-adaptive defaults | Detect 256-color vs truecolor, adjust theme | Med | Query terminal capabilities via crossterm |
| Community theme repository | Share themes as .toml files | Low | Just documentation, no code needed |

### Anti-Features for Themes

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| GUI theme editor | Way outside scope of a terminal tool | Edit TOML file directly |
| Font configuration | Terminal apps don't control fonts | Document "use a Nerd Font for best results" |
| Per-file-type themes | Only editing markdown, one filetype | Single theme applies to everything |
| CSS-like selectors | Over-engineering configuration | Flat key-value theme struct |

### Recommendation

**Create a `Theme` struct, externalize all hardcoded colors, add TOML deserialization with serde.** This is straightforward and low-risk. Add `serde` and `toml` as dependencies. Ship base16-ocean.dark as default, add a light theme and a minimal/monochrome theme.

**Complexity: MEDIUM.** The work is mostly mechanical -- extract constants, define struct, add serde derives, parse config file. No algorithmic challenges.

**Dependency:** Independent of vim keybindings or WYSIWYG. Can be built in parallel.

---

## Category 4: Browser Companion

### What This Means

A `--browser` flag (or hotkey) that spawns a local HTTP server serving the rendered markdown as HTML with GitHub-flavored styling. Opens in the default browser. Updates live as the user edits. Local-only (not SSH-compatible by design).

### How Existing Tools Do This

- **grip** (Python): The established tool. Runs `grip` on a file, serves at `localhost:6419`. Uses GitHub's Markdown API for pixel-perfect GFM rendering. Requires Python + pip + GitHub API access (rate-limited).
- **mdserve** (Rust): Lightweight local markdown preview server. Serves rendered HTML at a local port with live reload.
- **Textual serve** (Python): Textual framework's approach -- renders the TUI itself to HTML via WebSocket. Not relevant to our use case.
- **VS Code preview**: Built-in markdown preview opens in a side panel, updates live.

### Implementation Approach

1. **Embed a tiny HTTP server**: Use `tiny_http` or `hyper` (minimal subset). Serve on `localhost:<random-port>`.
2. **Render HTML**: Use pulldown-cmark (already a dependency) to render markdown to HTML. Wrap in a page with GitHub CSS (github-markdown-css, ~10KB inlined).
3. **Live reload**: Use Server-Sent Events (SSE) or WebSocket to push updates. SSE is simpler -- the browser listens on `/events`, the server pushes a "reload" event when content changes.
4. **Open browser**: Use the `open` crate (or `std::process::Command` with `open`/`xdg-open`).
5. **Lifecycle**: Server starts when `--browser` is passed or a hotkey is pressed. Server stops when mdedit exits.

### Table Stakes for Browser Companion

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Serve rendered markdown as HTML at localhost | Core value -- see GitHub-accurate rendering | Med | pulldown-cmark HTML output + CSS |
| GitHub-flavored markdown styling | The whole point -- see how it looks on GitHub | Low | Inline github-markdown-css |
| Auto-open browser on launch | Don't make users copy-paste a URL | Low | `open` crate or `xdg-open` |
| Live reload on edit | Preview updates as you type | Med | SSE push from editor content changes |
| GFM features (tables, task lists, strikethrough) | GitHub renders these; terminal can't fully match | Low | pulldown-cmark GFM extensions already enabled |

### Differentiators for Browser Companion

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Scroll sync with editor | Browser scrolls to match editor cursor position | High | Inject JS to scroll to corresponding heading/paragraph |
| Syntax-highlighted code blocks | GitHub uses highlight.js; include it | Med | Embed highlight.js (~30KB) or use syntect server-side |
| Mermaid diagram rendering | GitHub renders Mermaid; valuable for docs | Med | Include mermaid.js (~1MB, could be CDN-loaded) |
| Custom CSS override | Let users provide their own stylesheet | Low | Config option for CSS file path |
| Print-ready output | Nice for exporting to PDF via browser print | Low | Add print CSS media query |

### Anti-Features for Browser Companion

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Two-way sync (edit in browser) | Turns into a web editor -- not our product | Browser is view-only |
| Network-accessible server | Security risk, not needed | Bind to 127.0.0.1 only |
| Electron/webview embedding | Binary size explosion | Use system browser |
| GitHub API rendering | Rate-limited, requires network, requires token | Render locally with pulldown-cmark |
| SSH compatibility | Browser companion inherently requires a local display | Document this is local-only |

### Recommendation

**Embed `tiny_http` (or `warp`/`axum` minimal) with SSE live reload.** Render HTML with pulldown-cmark, wrap in github-markdown-css. This is a well-understood pattern. The main decision is whether to include the HTTP server dependency in the default binary or behind a cargo feature flag.

**Complexity: MEDIUM.** The HTTP server and SSE are well-trodden ground. The main work is wiring content changes from the editor to the server thread.

**Dependency:** Requires a way to signal content changes to a background thread. The existing `ContentChanged` action in EditorAction provides the hook. Needs async or a separate thread (crossbeam channel).

**Binary size concern:** An HTTP server + CSS adds ~500KB-1MB. Consider a `browser` cargo feature flag to keep the base binary lean.

---

## Category 5: Clipboard Integration

### What This Means

Copy and paste between mdedit and the system clipboard. Currently Ctrl+C does nothing (editor.rs:265). Text selection exists (Shift+arrows) but selected text can only be deleted, not copied to the system clipboard.

### How Terminal Apps Handle Clipboard

Three approaches, in order of preference:

1. **OSC 52 escape sequences**: The terminal standard for clipboard access. Write-only (setting clipboard is well-supported; reading clipboard is restricted by most terminals for security). Works over SSH, through tmux (with configuration), and in most modern terminals (Alacritty, WezTerm, iTerm2, kitty, Windows Terminal). crossterm rejected adding OSC 52 to core (PR #697 closed), but a later PR #974 may have revisited this. The `copypasta-ext` crate provides an `Osc52ClipboardContext` for write operations.

2. **Platform-native clipboard** (`arboard` crate): Cross-platform clipboard access (macOS pbcopy/pbpaste, Linux X11/Wayland, Windows). Works locally but NOT over SSH. The `arboard` crate (maintained by 1Password) is the standard Rust clipboard library. Supports text, images, and HTML content.

3. **Spawn pbcopy/xclip/xsel**: Shell out to platform clipboard tools. Fragile but works everywhere those tools exist. Fallback approach.

### Recommended Approach

**OSC 52 for write (copy) + arboard as fallback for local.** This gives the best coverage:
- OSC 52 works over SSH (the primary advantage)
- arboard works locally on all platforms
- Try OSC 52 first, fall back to arboard, fall back to internal-only yank buffer

### Table Stakes for Clipboard

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Copy selected text to system clipboard | The most requested missing feature from v1 | Med | OSC 52 write + arboard fallback |
| Paste from system clipboard | Complete the copy/paste loop | Med | arboard for read (OSC 52 read is restricted) |
| Ctrl+C to copy (or vim y to yank) | Standard keybinding | Low | Wire to clipboard provider |
| Ctrl+V to paste (or vim p to put) | Standard keybinding | Low | Wire to clipboard provider |
| Internal yank buffer (vim mode) | Vim yank/put must work even without system clipboard | Low | Already conceptually needed for vim dd/yy/p |
| Works over SSH (at least copy) | Terminal users SSH constantly | Med | OSC 52 for copy; paste may not work over SSH |

### Differentiators for Clipboard

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Auto-detect clipboard capability | Silently fall back if OSC 52 unsupported | Med | Try OSC 52, check for arboard, degrade gracefully |
| Clipboard indicator in status bar | Show when copy succeeded | Low | Flash "Copied" message |
| Paste with auto-indent | Pasted markdown aligns with context | Med | Detect indent level, adjust pasted text |

### Anti-Features for Clipboard

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Clipboard history | Out of scope, OS-level feature | Single clipboard operation |
| Rich text paste (HTML) | We edit markdown, not HTML | Paste as plain text only |
| Image paste | Terminal-based, no image editing | Ignore image clipboard content |
| X11 primary selection (middle-click) | Fragmented Linux-only feature | Focus on standard clipboard |

### Recommendation

**Use OSC 52 for copy (write to stdout as escape sequence) + arboard for paste (read from system clipboard).** OSC 52 copy is the highest-value feature because it works over SSH. For paste, arboard provides cross-platform read access locally. Add arboard behind a `clipboard` cargo feature flag to avoid pulling in X11/Wayland deps for users who don't need it.

**Complexity: MEDIUM.** The OSC 52 write is ~20 lines of code (base64-encode text, write escape sequence to stdout). arboard is a well-tested crate. The complexity is in the fallback chain and error handling.

**Dependency:** Benefits from vim mode (yank buffer maps naturally to clipboard) but can work independently with the existing selection mechanism.

---

## Category 6: Adjustable Split Ratio

### What This Means

Let users change the editor/preview pane width ratio from the current hardcoded 50/50. Support at minimum 70/30, 50/50, 30/70 presets, or finer-grained adjustment.

### Current Implementation

`app.rs` line 433-438: `Constraint::Percentage(50)` for editor and preview, with a 1-column divider between them. Changing this to a variable percentage is trivial.

### Table Stakes

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Hotkey to adjust ratio (e.g., Ctrl+Left/Right) | Quick adjustment without config | Low | Increment/decrement percentage by 10 |
| Persist ratio in config | Remember user preference | Low | Requires config file (from themes) |
| Minimum pane width | Don't let either pane collapse to 0 | Low | Clamp to minimum 20% |

### Differentiators

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Mouse-draggable divider | Intuitive resizing | Med | Detect mouse click on divider column, track drag |
| Preset ratios (Ctrl+1/2/3) | Quick switch between common layouts | Low | 70/30, 50/50, 30/70 |

### Recommendation

**Store split_ratio as a u16 percentage in App, adjust with hotkeys, persist in config.** This is 30 minutes of work once the config system exists.

**Complexity: LOW.** Nearly trivial. The ratatui Layout system already supports `Constraint::Percentage(n)`.

**Dependency:** Benefits from config file (theme system) for persistence.

---

## Category 7: Mouse Support

### What This Means

Enable crossterm mouse capture so users can scroll with the mouse wheel and click to position the cursor.

### Current Implementation

Mouse events are NOT captured. `ratatui::run()` in main.rs handles terminal setup but does not call `enable_mouse_capture()`. Adding mouse support requires calling `crossterm::execute!(stdout, EnableMouseCapture)` at startup and handling `Event::Mouse` in the event loop.

### Table Stakes

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Mouse wheel scrolling in editor | Universal expectation in 2026 | Low | Map MouseEvent::ScrollUp/Down to cursor movement |
| Mouse wheel scrolling in preview | Scroll preview independently | Low | Map to preview.scroll_up/down |
| Click to position cursor in editor | Standard text editor behavior | Med | Translate (x, y) click to (row, col) accounting for scroll and line numbers |

### Differentiators

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Click-drag text selection | Mouse-based selection | Med | Start selection on mouse down, extend on mouse move |
| Click on divider to drag-resize | Mouse-driven split adjustment | Med | Combine with adjustable split ratio |
| Double-click to select word | Common editor pattern | Low | Detect double-click, select word under cursor |

### Anti-Features

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Mouse-only workflows | Terminal users are keyboard-first | Mouse augments keyboard, never replaces |
| Right-click context menu | Overcomplicating the UI | All actions via keyboard or command mode |
| Mouse hover tooltips | Terminal rendering limitations | Status bar for contextual info |

### Recommendation

**Enable mouse capture, handle scroll and click events.** This is well-supported by crossterm and ratatui. The main complexity is translating click coordinates to editor positions (accounting for line number gutter width and scroll offset).

**Complexity: LOW-MEDIUM.** Scroll is trivial. Click-to-cursor requires coordinate math that already partially exists in `render_highlighted()` (the cursor positioning logic at line 498-504).

**Dependency:** Independent of other features. Can be built in any order.

---

## Feature Dependencies (v2)

```
Config file (TOML) ──> Configurable themes
Config file (TOML) ──> Persist split ratio
Config file (TOML) ──> Persist theme choice

Vim modal layer ──> : command mode ──> :theme command
Vim modal layer ──> Yank buffer ──> Clipboard integration (yank to system clipboard)
Vim modal layer ──> / search (reuses existing search infrastructure)
Vim modal layer ──> Visual mode (reuses existing selection infrastructure)

WYSIWYG mode ──> Vim keybindings (must work in WYSIWYG too)
WYSIWYG mode ──> Cursor position mapping (new infrastructure)

Browser companion ──> Content change signaling (existing EditorAction::ContentChanged)
Browser companion ──> Background thread / channel for HTTP server

Clipboard ──> OSC 52 (write-only, ~20 lines)
Clipboard ──> arboard crate (read + write, platform-native)
Clipboard ──> Vim yank buffer (if vim mode exists)

Mouse support ──> crossterm::EnableMouseCapture (independent)
Adjustable split ──> Config file for persistence (otherwise just a variable)
```

---

## Priority Recommendation for v2

### Phase 1: Foundation
1. **Config file + Theme system** -- unblocks everything else, mechanical work, medium complexity
2. **Mouse support (scroll + click)** -- low complexity, immediate user value, independent
3. **Adjustable split ratio** -- low complexity, pairs with config

### Phase 2: Core Power Feature
4. **Vim-style keybindings** -- the headline feature, highest complexity, highest impact. This is the one users will evaluate mdedit on.

### Phase 3: Clipboard + Browser
5. **Clipboard integration** -- unblocked by vim mode (yank buffer), medium complexity
6. **Browser companion** -- independent, medium complexity, high value for GitHub README authors

### Phase 4: Advanced
7. **WYSIWYG mode** -- highest complexity, depends on vim mode working, experimental/differentiator

**Rationale:** Config/themes first because it's mechanical and unblocks theme persistence and the `:theme` command. Mouse support is cheap and universally wanted. Vim keybindings are the flagship feature but benefit from having config/themes done first (mode indicator colors, keybinding config). WYSIWYG is last because it's the most experimental and depends on everything else being stable.

---

## Sources

- [edtui - GitHub](https://github.com/preiter93/edtui) -- vim-inspired editor widget for ratatui
- [edtui - lib.rs](https://lib.rs/crates/edtui) -- 134 downloads/month, 127 stars
- [modalkit-ratatui - docs.rs](https://docs.rs/modalkit-ratatui/latest/modalkit_ratatui/) -- heavy modal editing framework
- [ratatui-textarea - crates.io](https://crates.io/crates/ratatui-textarea) -- current editor widget
- [crossterm OSC 52 PR #697](https://github.com/crossterm-rs/crossterm/pull/697) -- closed, not merged
- [copypasta-ext OSC 52](https://docs.rs/copypasta-ext/0.3.2/copypasta_ext/osc52/index.html) -- write-only clipboard via escape sequences
- [arboard - GitHub](https://github.com/1Password/arboard) -- cross-platform clipboard (1Password maintained)
- [grip - GitHub](https://github.com/joeyespo/grip) -- GitHub markdown preview via API
- [mdserve](https://jrfernandez.com/mdserve-fast-markdown-preview-terminal-workflows/) -- Rust markdown preview server
- [ratatui Mouse Capture](https://ratatui.rs/concepts/backends/mouse-capture/) -- official docs
- [ratatui Color](https://docs.rs/ratatui/latest/ratatui/style/enum.Color.html) -- hex and named color parsing
- [ratatui Layout](https://ratatui.rs/concepts/layout/) -- constraint-based layout system
- [render-markdown.nvim](https://github.com/MeanderingProgrammer/render-markdown.nvim) -- Neovim WYSIWYG-like markdown
- [markview.nvim](https://github.com/OXY2DEV/markview.nvim) -- Neovim markdown decorations
- [color-to-tui](https://github.com/ratatui/awesome-ratatui) -- parse color strings to ratatui Colors
- [Ratatui Discussion #877](https://github.com/ratatui/ratatui/discussions/877) -- choosing colors for different terminals
