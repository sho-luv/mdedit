# Architecture Patterns: v2.0 Feature Integration

**Domain:** Terminal-based markdown editor with live preview (Rust TUI)
**Researched:** 2026-03-22
**Focus:** How vim keybindings, WYSIWYG mode, configurable themes, browser companion, and clipboard integrate with the existing ~1,500 LOC codebase

## Existing Architecture Summary

The v1 codebase follows a flat component architecture:

```
App (app.rs, ~360 LOC)
  - Owns: Editor, Preview, StatusBar, TuiMarkdownRenderer
  - Event loop: synchronous crossterm::event::poll(50ms)
  - Key routing: match on AppMode (Editing|ConfirmQuit|PromptFilename|Search)
  - Rendering: immediate-mode via match on LayoutMode (Split|EditorOnly|PreviewOnly)

Editor (editor.rs, ~275 LOC)
  - Wraps ratatui-textarea with input_without_shortcuts()
  - Custom render_highlighted() bypasses Widget trait for syntax/selection/search overlays
  - Returns EditorAction enum (Save|RequestQuit|ContentChanged)
  - Owns: TextArea, MarkdownHighlighter, scroll_top, modified flag

Preview (preview.rs, ~42 LOC) - Thin wrapper: scroll_offset + Paragraph render
StatusBar (status_bar.rs, ~83 LOC) - Timed messages + file/cursor info
```

**Key architectural facts that constrain v2 design:**
1. Editor uses `input_without_shortcuts()` -- all keybindings are manually routed in `handle_key()`
2. Custom `render_highlighted()` owns the entire render pipeline (line numbers, syntax, selection, search overlays via `apply_highlight_overlay()`)
3. `AppMode` enum controls key routing -- each mode has its own `handle_*_key()` method
4. Content changes flow through `EditorAction::ContentChanged` -> `content_dirty` flag -> 80ms debounce -> preview re-render
5. No config system, no trait abstraction on Editor, no plugin points

## v2 Integration Architecture

### New Component Map

```
+------------------------------------------------------------------+
|                          App (app.rs)                             |
|  +-------------+  +---------------------------+  +-------------+ |
|  | StatusBar   |  |      LayoutManager        |  | CommandLine | |
|  | (v1, mod)   |  |  +----------+ +--------+  |  | (NEW)       | |
|  +-------------+  |  | Editor   | |Preview |  |  +-------------+ |
|                    |  | (v1,mod) | |(v1)    |  |                  |
|                    |  +----------+ +--------+  |                  |
|                    +---------------------------+                  |
|  +-------------+  +---------------------------+  +-------------+ |
|  | InputMode   |  |      Theme                |  | Clipboard   | |
|  | (NEW)       |  |      (NEW)                |  | (NEW)       | |
|  +-------------+  +---------------------------+  +-------------+ |
|                    +---------------------------+                  |
|                    |  BrowserCompanion (NEW)   |                  |
|                    +---------------------------+                  |
+------------------------------------------------------------------+
```

### Component Inventory: New vs Modified

| Component | Status | File | Purpose |
|-----------|--------|------|---------|
| `InputMode` | **NEW** | `src/input_mode.rs` | Vim modal state machine (Normal/Insert/Visual/Command) |
| `VimHandler` | **NEW** | `src/vim.rs` | Vim keybinding interpreter, maps keys to editor operations |
| `Theme` | **NEW** | `src/theme.rs` | Parsed TOML theme, provides Style lookups for all components |
| `Config` | **NEW** | `src/config.rs` | TOML config loading from `~/.config/mdedit/config.toml` |
| `Clipboard` | **NEW** | `src/clipboard.rs` | OSC 52 copy + platform fallback |
| `BrowserCompanion` | **NEW** | `src/browser.rs` | HTTP+WebSocket server for browser preview |
| `CommandLine` | **NEW** | `src/command_line.rs` | Vim `:` command input and execution |
| `App` | **MODIFIED** | `src/app.rs` | New AppMode variants, vim mode routing, theme injection |
| `Editor` | **MODIFIED** | `src/editor.rs` | Remove nano keybindings, delegate to VimHandler, theme-aware render |
| `StatusBar` | **MODIFIED** | `src/status_bar.rs` | Show vim mode indicator, use theme colors |
| `Highlighter` | **MODIFIED** | `src/highlighter.rs` | Accept theme for color mapping instead of hardcoded base16-ocean |
| `Preview` | UNCHANGED | `src/preview.rs` | No changes needed |
| `MarkdownRenderer` | UNCHANGED | `src/markdown/` | No changes needed |

---

## Feature 1: Vim-Style Keybindings (Default Mode)

### Architecture Decision: Custom Vim Layer on ratatui-textarea, NOT edtui

**Why not switch to edtui:** edtui is a complete replacement for ratatui-textarea with its own buffer, rendering, and event handling. Switching would require rewriting `render_highlighted()`, the search overlay system, the syntax highlighting pipeline, and selection byte-range logic -- essentially a full rewrite of editor.rs. The existing custom render pipeline is a competitive advantage (syntax + selection + search overlays). Preserve it.

**Why not modalkit-ratatui:** Heavy dependency, brings in vim emulation logic that's far more complex than needed for a markdown editor. Targets ratatui ^0.29 (compatibility risk with 0.30).

**Approach:** Build a vim mode state machine that translates vim keystrokes into ratatui-textarea `CursorMove` and editing operations. The existing `input_without_shortcuts()` pattern already separates key handling from the widget -- vim is just a different key interpreter for the same operations.

### Integration Points

**AppMode expansion:**
```rust
// Current
enum AppMode { Editing, ConfirmQuit, PromptFilename, Search }

// v2: Editing becomes a container for input modes
enum AppMode {
    Normal,          // Vim normal mode (was: N/A)
    Insert,          // Vim insert mode (was: Editing)
    Visual,          // Vim visual/selection mode (was: N/A)
    Command,         // Vim : command line (NEW)
    ConfirmQuit,     // Unchanged
    PromptFilename,  // Unchanged
    Search,          // Now triggered by / in Normal mode (was Ctrl+F)
}
```

**Key routing change in app.rs:**
```rust
// Current: handle_editing_key() does everything
// v2: split into mode-specific handlers

fn handle_key(&mut self, key: KeyEvent) {
    match self.mode {
        AppMode::Normal => self.handle_normal_key(key),     // NEW
        AppMode::Insert => self.handle_insert_key(key),     // Replaces handle_editing_key()
        AppMode::Visual => self.handle_visual_key(key),     // NEW
        AppMode::Command => self.handle_command_key(key),   // NEW
        AppMode::Search => self.handle_search_key(key),     // Modified: / trigger
        AppMode::ConfirmQuit => self.handle_confirm_quit_key(key),
        AppMode::PromptFilename => self.handle_prompt_filename_key(key),
    }
}
```

**Editor changes:**
- Remove all Ctrl+ keybindings from `editor.rs::handle_key()` -- move to app-level vim routing
- Editor becomes a dumb buffer: expose `insert_char()`, `delete_char()`, `move_cursor()`, `start_selection()`, `yank_selection()`, `paste()` as atomic operations
- VimHandler calls these operations based on parsed vim commands

**New file: src/vim.rs (~200-300 LOC estimated)**
```rust
pub struct VimHandler {
    pending_count: Option<usize>,   // For 5j, 3dd etc.
    pending_operator: Option<Operator>,  // d, c, y waiting for motion
    register: String,               // Yank register (single register for v1)
}

pub enum VimCommand {
    Move(CursorMove),
    Insert(InsertPoint),   // i, a, o, O, A, I
    Delete(Motion),
    Change(Motion),
    Yank(Motion),
    Paste,
    Undo,
    Redo,
    EnterVisual,
    EnterCommand,
    Search,
    // ...
}

impl VimHandler {
    /// Feed a key in Normal mode, return a command or None (waiting for more input)
    pub fn handle_normal(&mut self, key: KeyEvent) -> Option<VimCommand> { ... }

    /// Feed a key in Visual mode
    pub fn handle_visual(&mut self, key: KeyEvent) -> Option<VimCommand> { ... }
}
```

**Critical: ratatui-textarea already has the building blocks.** The existing `CursorMove` enum covers: `Forward`, `Back`, `Up`, `Down`, `Head`, `End`, `Top`, `Bottom`, `WordForward`, `WordBack`, `ParagraphForward`, `ParagraphBack`, `Jump(row, col)`. The vim layer just maps keystrokes to these.

### What vim commands to implement (v2 scope)

**Must have (table stakes for vim users):**
- Movement: h/j/k/l, w/b/e, 0/$, gg/G, {/}, Ctrl+d/u (half-page)
- Mode transitions: i/a/I/A/o/O (insert), v/V (visual), Esc (normal), : (command)
- Editing: x, dd, D, cc, C, yy, p/P, u/Ctrl+r (undo/redo), . (repeat)
- Count prefix: [count]motion (5j, 3dd)
- Search: /, n/N
- Commands: :w, :q, :wq, :q!

**Defer to later:**
- Macros (q/@ recording)
- Marks (m/')
- Registers (beyond single register)
- Text objects (di", ci(, etc.) -- high complexity, low priority for markdown
- Ex commands beyond :w/:q

### Status bar integration

The status bar must show the current vim mode. This is a small change:

```rust
// status_bar.rs gains a mode parameter
pub fn render(&self, frame, area, filename, cursor, modified, vim_mode: &str) { ... }
// vim_mode: "NORMAL", "INSERT", "VISUAL", "COMMAND"
```

---

## Feature 2: Configurable Color Themes

### Architecture Decision: TOML config with serde, theme as a shared struct

**Why TOML:** Rust ecosystem standard (Cargo.toml itself). `toml` + `serde` crates are tiny, well-tested, zero surprises. YAML/JSON are wrong choices for Rust projects.

**New dependency:** `toml = "0.8"` and `serde = { version = "1", features = ["derive"] }`

### Config file location

Follow XDG on Linux, `~/Library/Application Support/` on macOS:
- `~/.config/mdedit/config.toml` (Linux/macOS unified -- simpler, most tools do this)
- Override: `--config <path>` CLI flag
- No config file = sensible defaults (current v1 appearance)

### Config structure

```toml
# ~/.config/mdedit/config.toml

[keybindings]
mode = "vim"  # "vim" (default) or "nano"

[theme]
name = "ocean"  # Built-in theme name, OR:

[theme.colors]
# Override individual colors
editor_bg = "default"        # "default" = terminal bg
editor_fg = "#c0c5ce"
line_numbers = "#65737e"
cursor_line = "underline"
selection_bg = "#444466"
status_bar_bg = "#343d46"
status_bar_fg = "#c0c5ce"
vim_normal_indicator = "#a3be8c"
vim_insert_indicator = "#8fa1b3"
vim_visual_indicator = "#bf616a"
search_match_bg = "#ebcb8b"
search_active_bg = "#88c0d0"

[theme.syntax]
# Maps to syntect theme
syntax_theme = "base16-ocean.dark"

[editor]
tab_width = 2
line_numbers = true
relative_line_numbers = false

[preview]
# Future: could select markdown renderer
```

### Theme struct integration

```rust
// src/theme.rs
pub struct Theme {
    pub editor_fg: Color,
    pub editor_bg: Option<Color>,  // None = terminal default
    pub line_numbers: Color,
    pub selection_bg: Color,
    pub status_bar: Style,
    pub vim_mode_styles: HashMap<String, Style>,
    pub search_match: Style,
    pub search_active: Style,
    pub syntect_theme_name: String,
}

impl Default for Theme { /* current v1 hardcoded colors */ }
```

### Integration points

1. **App::new()** loads config, constructs Theme, passes to all components
2. **editor.rs `render_highlighted()`** uses `theme.selection_bg` instead of hardcoded `Color::Rgb(68, 68, 102)`
3. **highlighter.rs** accepts `theme.syntect_theme_name` instead of hardcoded `"base16-ocean.dark"`
4. **status_bar.rs** uses `theme.status_bar` instead of hardcoded `Style::default().bg(Color::DarkGray)`
5. **All hardcoded `Color::` references** in the codebase get replaced with theme lookups

**This is a cross-cutting concern.** Theme touches every rendering component. Build it early so other features use themed colors from the start.

---

## Feature 3: Clipboard Integration

### Architecture Decision: crossterm OSC 52 (primary) + arboard fallback

**crossterm 0.29 has OSC 52 built in** via the `osc52` feature flag. Since mdedit already depends on crossterm 0.29, this is nearly free.

**Why OSC 52 primary:** Works over SSH, works in tmux (with `set -g set-clipboard on`), no platform-specific code. This is the correct default for a terminal-first tool.

**Why arboard fallback:** OSC 52 is write-only in most terminals (you can copy TO clipboard but not read FROM it). For paste, you need either: (a) terminal bracketed paste (crossterm already handles this), or (b) platform clipboard read via `arboard` crate.

### New dependency

```toml
crossterm = { version = "0.29", features = ["osc52"] }  # Add osc52 feature
arboard = "3"  # Optional, behind feature flag for platform paste
```

### Integration points

**Vim yank/paste integration:**
```rust
// src/clipboard.rs
pub struct Clipboard {
    internal_register: String,  // Always available (vim yank register)
    osc52_available: bool,      // Detected at startup
}

impl Clipboard {
    pub fn copy(&mut self, text: &str) {
        self.internal_register = text.to_string();
        if self.osc52_available {
            // crossterm::execute!(stdout, CopyToClipboard(text))
        }
    }

    pub fn paste(&self) -> &str {
        // Use internal register (vim p/P uses this)
        // System paste comes via bracketed paste (crossterm handles automatically)
        &self.internal_register
    }
}
```

**Key mapping:**
- `yy` / `y{motion}` in Normal mode -> `clipboard.copy(selected_text)`
- `p` / `P` in Normal mode -> `editor.insert_str(clipboard.paste())`
- System Ctrl+V/Cmd+V -> bracketed paste (already works via crossterm, no code needed)
- Ctrl+C in v1 is a no-op -- in v2, if selection active in Visual mode, copy to clipboard

**No changes to editor.rs rendering.** Clipboard is purely a data-flow concern handled in app.rs key routing.

---

## Feature 4: Browser Companion

### Architecture Decision: Embedded HTTP+WebSocket server using tiny-http + tungstenite

**Why not aurelius:** Unmaintained (last commit 2022), brings in heavy dependencies (tokio, warp). We need something minimal.

**Why not axum/actix:** Full async web frameworks are overkill. We need: serve one HTML page, push markdown updates over WebSocket.

**Why tiny-http + tungstenite:** Both are minimal, synchronous-capable crates. `tiny-http` serves the HTML page (~50 lines). `tungstenite` handles the WebSocket upgrade and message push. Total added binary size: ~500KB.

**Alternative considered: just use a thread + std::net.** For serving a single page and a single WebSocket, raw std::net with manual HTTP handling is viable but fragile. tiny-http adds ~100KB and handles HTTP correctly.

### Architecture

```
                    mdedit process
+---------------------------------------------------+
|  Main thread (TUI event loop)                      |
|    |                                               |
|    | content_dirty? -> channel.send(markdown)      |
|    |                                               |
+---------------------------------------------------+
|  Companion thread (spawned on --browser flag)      |
|    |                                               |
|    | tiny-http: serves /index.html on :3030        |
|    | tungstenite: WebSocket on /ws                 |
|    | Receives markdown via channel, pushes to WS   |
|    |                                               |
+---------------------------------------------------+

         Browser (localhost:3030)
+---------------------------------------------------+
|  GitHub-style CSS + markdown.js                    |
|  WebSocket client receives rendered HTML           |
|  Auto-updates <div> on message                     |
+---------------------------------------------------+
```

### Integration points

1. **CLI flag:** `--browser` or `--companion` starts the server thread
2. **main.rs** spawns the companion thread before entering the TUI event loop
3. **app.rs** sends markdown content through a `std::sync::mpsc::Sender<String>` when `content_dirty` triggers
4. **No impact on TUI rendering** -- the companion runs independently on a background thread
5. **Graceful shutdown:** App drop/quit sends a shutdown signal to the companion thread

### New dependencies

```toml
# Behind a feature flag to keep the default binary small
[features]
browser = ["tiny-http", "tungstenite"]

[dependencies]
tiny-http = { version = "0.12", optional = true }
tungstenite = { version = "0.24", optional = true }
```

### Embedded assets

The HTML page, CSS (GitHub markdown style), and JavaScript WebSocket client are embedded in the binary via `include_str!()`. Total: ~20KB of embedded assets. Well within the 10MB binary constraint.

### GitHub-accurate rendering

Use `comrak` crate (GFM-compliant markdown -> HTML) on the server thread, NOT pulldown-cmark. Reason: the browser preview should match GitHub rendering exactly. comrak is the standard GFM renderer used by GitHub-compatible tools. This is a different concern from the TUI preview (which uses tui-markdown for terminal rendering).

```toml
comrak = { version = "0.36", optional = true, default-features = false, features = ["shortcodes"] }
```

---

## Feature 5: WYSIWYG Terminal Editing

### Architecture Decision: This is a SEPARATE AppMode, not a modification to the existing editor

**WYSIWYG in a terminal means:** The user sees rendered markdown (bold text appears bold, headers appear large, lists are formatted) and edits inline. When the cursor enters a markdown element, the raw syntax is revealed for editing. When the cursor leaves, it re-renders.

**This is the hardest feature by far.** No existing Rust crate does this. It requires:
1. A custom widget that renders markdown AND supports cursor movement within it
2. Mapping cursor positions between rendered output and raw markdown source
3. Revealing/hiding syntax around the cursor position
4. Maintaining a parallel raw-text buffer and rendered-text view

### Integration approach

```rust
// --wysiwyg flag activates a different editor mode
enum EditorMode {
    Raw,      // Current: raw markdown with syntax highlighting
    Wysiwyg,  // New: rendered markdown with inline editing
}
```

**The WYSIWYG editor is a NEW component** (`src/wysiwyg.rs`), not a modification to `editor.rs`. It replaces the Editor in the layout when active. It still produces raw markdown content for saving and for the preview pane.

### Component design

```rust
// src/wysiwyg.rs
pub struct WysiwygEditor {
    raw_content: String,           // Source of truth
    rendered_lines: Vec<RenderedLine>,  // Display representation
    cursor: WysiwygCursor,         // Position in rendered space
    reveal_range: Option<Range>,   // Syntax revealed around cursor
}

struct RenderedLine {
    spans: Vec<Span<'static>>,     // Styled content
    source_range: Range<usize>,    // Maps back to raw_content bytes
}
```

### Bidirectional mapping (the hard part)

Every rendered position must map back to a raw source position, and vice versa. For example:
- `**bold text**` renders as `bold text` (10 chars rendered, 14 chars raw)
- Cursor at rendered position 3 = raw position 5 (after `**bo`)
- When cursor enters the bold region, reveal: `**bold text**`

This requires a source map built during markdown parsing. pulldown-cmark provides source ranges for each event -- this is the building block.

### Why this should be built LAST

1. It depends on vim keybindings (cursor movement model)
2. It depends on theme (rendering styles)
3. It depends on clipboard (copy/paste in rendered view)
4. It is the highest-risk, most novel feature
5. The raw editor with live preview already works -- WYSIWYG is a bonus, not a requirement

---

## Suggested Build Order

Based on dependency analysis between features:

```
Phase 1: Config + Theme (foundation for everything)
    |
    v
Phase 2: Vim Keybindings (changes key routing, depends on theme for mode indicator)
    |
    v
Phase 3: Clipboard (depends on vim yank/paste model)
    |
    v
Phase 4: Browser Companion (independent, but benefits from having vim :browser command)
    |
    v
Phase 5: WYSIWYG (depends on everything above)
```

### Detailed rationale

1. **Config + Theme FIRST** because:
   - Every other feature needs themed colors
   - Config file determines keybinding mode (vim vs nano)
   - Small, low-risk, cross-cutting -- eliminates hardcoded colors early
   - `serde` + `toml` are tiny dependencies

2. **Vim Keybindings SECOND** because:
   - Largest change to app.rs key routing -- do it before adding more modes
   - Must be done before clipboard (vim yank model)
   - Must be done before WYSIWYG (cursor movement model)
   - User's highest priority feature

3. **Clipboard THIRD** because:
   - Small feature, ~100 LOC
   - Depends on vim yank/paste architecture being in place
   - OSC 52 is nearly free (feature flag on existing dep)

4. **Browser Companion FOURTH** because:
   - Runs on a separate thread -- minimal coupling to TUI code
   - Independent of other features
   - Only depends on content being available (already is via `editor.content()`)
   - Can be behind a cargo feature flag, zero cost when unused

5. **WYSIWYG LAST** because:
   - Highest complexity, most novel
   - Depends on all other features being stable
   - Can be cut without affecting other features
   - Needs the most research during implementation

---

## Data Flow Changes (v1 -> v2)

### v1 data flow
```
KeyEvent -> AppMode match -> Editor.handle_key() -> EditorAction -> App state update
```

### v2 data flow
```
KeyEvent -> AppMode match -> VimHandler.handle_*() -> VimCommand
    -> App interprets VimCommand:
        -> Editor atomic operations (move, insert, delete, yank)
        -> Clipboard operations (copy, paste)
        -> Mode transitions (Normal <-> Insert <-> Visual <-> Command)
        -> App-level actions (save, quit, toggle preview, open browser)
    -> content_dirty? -> debounce -> Preview update
                     -> channel.send() -> BrowserCompanion update
```

The key architectural change is **inserting VimHandler between key events and editor operations**. The editor becomes a passive buffer that exposes operations. The vim layer decides which operations to call. This also makes it easy to support nano mode as an alternative -- swap VimHandler for a NanoHandler that maps Ctrl+ keys directly to the same editor operations.

### New inter-component communication

| From | To | Data | Mechanism |
|------|----|------|-----------|
| App | VimHandler | KeyEvent | Direct method call |
| VimHandler | App | VimCommand | Return value |
| App | Editor | Atomic operations | Direct method calls |
| App | Clipboard | Copy/paste text | Direct method calls |
| App | BrowserCompanion | Markdown content | `mpsc::Sender<String>` |
| App | StatusBar | Vim mode string | Method parameter |
| Config | Theme | Color values | Struct field access |
| Config | App | Keybinding mode | Struct field access |

---

## Files to Create/Modify Summary

### New files (~800-1200 LOC total estimated)
```
src/config.rs       ~100 LOC  - TOML loading, Config struct with defaults
src/theme.rs        ~120 LOC  - Theme struct, color resolution, built-in themes
src/vim.rs          ~300 LOC  - Vim state machine, key -> command translation
src/command_line.rs ~80 LOC   - : command parsing and execution
src/clipboard.rs    ~60 LOC   - OSC 52 + internal register
src/browser.rs      ~200 LOC  - HTTP server, WebSocket, HTML template
src/wysiwyg.rs      ~400 LOC  - WYSIWYG editor (highest uncertainty)
assets/preview.html ~50 lines - Browser companion HTML template
assets/github.css   ~200 lines - GitHub markdown CSS (embedded)
```

### Modified files
```
src/main.rs         +20 LOC  - --browser, --wysiwyg, --config flags
src/app.rs          +150 LOC - New AppMode variants, vim routing, companion channel
src/editor.rs       -80 LOC  - Remove nano keybindings, expose atomic operations
src/status_bar.rs   +15 LOC  - Vim mode indicator, theme colors
src/highlighter.rs  +10 LOC  - Accept theme name parameter
Cargo.toml          +6 deps  - serde, toml, arboard, tiny-http, tungstenite, comrak
```

## Anti-Patterns to Avoid

### Anti-Pattern: Giant VimHandler match statement
**What:** Single 500-line match in vim.rs handling all modes.
**Instead:** Separate `handle_normal()`, `handle_visual()`, `handle_operator_pending()` methods. Each mode is a distinct state with its own key table.

### Anti-Pattern: Theme as global static
**What:** `lazy_static!` or `once_cell` for the theme, accessed everywhere.
**Instead:** Pass Theme reference through component constructors. Makes testing possible, avoids hidden dependencies.

### Anti-Pattern: Tight coupling between vim and editor
**What:** VimHandler directly calling `textarea.move_cursor()`.
**Instead:** VimHandler returns VimCommand. App translates to Editor operations. This separation makes it possible to swap input modes and test vim logic without a real editor.

### Anti-Pattern: Synchronous browser companion blocking TUI
**What:** Running tiny-http on the main thread.
**Instead:** Always spawn companion on a background thread with channel communication. The TUI event loop must never block on HTTP requests.

## Sources

- [edtui - vim-inspired editor widget for ratatui](https://github.com/preiter93/edtui) - Evaluated as alternative, rejected (would require rewrite of custom render pipeline)
- [ratatui-textarea vim example](https://github.com/rhysd/tui-textarea) - Pattern for building vim on top of textarea
- [modalkit-ratatui](https://docs.rs/modalkit-ratatui/latest/modalkit_ratatui/) - Evaluated, too heavy for this use case
- [crossterm OSC 52 clipboard (PR #974)](https://github.com/crossterm-rs/crossterm/blob/master/CHANGELOG.md) - Merged in crossterm 0.29
- [crossterm clipboard API](https://docs.rs/crossterm/latest/crossterm/clipboard/index.html) - CopyToClipboard command
- [aurelius - markdown preview server](https://github.com/euclio/aurelius) - Reference architecture for WebSocket preview (unmaintained, not using directly)
- [markdown-live-preview](https://crates.io/crates/markdown-live-preview) - Alternative reference for browser companion pattern
- [Typora](https://typora.io/) - Reference for WYSIWYG inline markdown editing UX
- [toml crate](https://docs.rs/toml/latest/toml/) - TOML parsing for config
- [pulldown-cmark source ranges](https://github.com/pulldown-cmark/pulldown-cmark) - Needed for WYSIWYG source mapping
