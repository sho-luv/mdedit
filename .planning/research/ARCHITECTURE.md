# Architecture Patterns

**Domain:** Terminal-based markdown editor with live preview (Rust TUI)
**Researched:** 2026-03-21

## Recommended Architecture

**Pattern: Component Architecture** -- ratatui's officially recommended pattern for modular apps with distinct UI sections. Each component owns its state, handles its own events, and renders itself. This is the right choice over Elm Architecture (better for simpler apps) or Flux (overkill for a two-pane editor).

```
+------------------------------------------------------------------+
|                          App (root)                               |
|  +-----------+  +---------------------------+  +---------------+  |
|  | StatusBar |  |      LayoutManager        |  | CommandBar    |  |
|  +-----------+  |  +----------+ +--------+  |  +---------------+  |
|                 |  | Editor   | |Preview |  |                     |
|                 |  | (left)   | |(right) |  |                     |
|                 |  +----------+ +--------+  |                     |
|                 +---------------------------+                     |
+------------------------------------------------------------------+
|                     Event Loop (crossterm)                        |
+------------------------------------------------------------------+
|                     Terminal Backend                              |
+------------------------------------------------------------------+
```

### Project File Structure

Follow the ratatui component template structure (HIGH confidence -- from official docs):

```
src/
  main.rs           # CLI arg parsing, terminal setup, run loop
  app.rs            # App struct: owns components, routes events, manages mode
  tui.rs            # Terminal init/restore, event polling wrapper
  event.rs          # Event types (Key, Tick, Render, Resize)
  action.rs         # Action enum (all possible state changes)
  components/
    mod.rs          # Component trait definition
    editor.rs       # TextArea wrapper, editing state
    preview.rs      # Markdown rendering, scroll state
    status_bar.rs   # Filename, cursor pos, modified indicator
    layout.rs       # Split pane management, mode toggling
  markdown/
    mod.rs          # Markdown parsing orchestration
    renderer.rs     # pulldown-cmark -> ratatui Text conversion
    highlighter.rs  # Syntax highlighting for code blocks
  file_io.rs        # File reading/writing, path handling
```

**Key principle from ratatui docs:** "Once you have set up the project, you shouldn't need to change the contents of anything outside the `components` folder." Infrastructure code (tui.rs, event.rs, main.rs) is write-once; iteration happens in components and markdown modules.

### Component Boundaries

| Component | Responsibility | Communicates With | Owns |
|-----------|---------------|-------------------|------|
| **App** | Root coordinator. Routes events to focused component. Manages app-level state (mode, quit flag). | All components, Event Loop | Layout mode, quit state, file path |
| **Editor** | Wraps `tui-textarea`. Text editing, cursor movement, undo/redo. Exposes raw markdown content. | App (receives events, returns actions) | TextArea widget, edit buffer, modified flag |
| **Preview** | Renders markdown to styled ratatui Text. Manages preview scroll position. | App (receives markdown text, returns nothing) | Rendered Text cache, scroll offset |
| **StatusBar** | Displays filename, cursor position, modified indicator, current mode. Pure display. | App (receives state snapshot) | Nothing -- stateless renderer |
| **LayoutManager** | Calculates pane rects based on current mode (split/editor-only/preview-only). | App (receives mode, returns Rects) | Nothing -- pure function |
| **FileIO** | Reads/writes files. Not a component -- a utility module. | App (called on open/save) | Nothing |
| **MarkdownRenderer** | Parses markdown string into ratatui Text with styling. Not a component -- a transform. | Preview (called each render when content changes) | Cached parsed output |

### Component Trait

Based on the official ratatui component architecture (HIGH confidence):

```rust
pub trait Component {
    /// One-time initialization
    fn init(&mut self) -> Result<()> { Ok(()) }

    /// Handle a key event, return an optional Action
    fn handle_key_event(&mut self, key: KeyEvent) -> Option<Action>;

    /// Update state based on an Action from any component
    fn update(&mut self, action: Action) -> Option<Action> { None }

    /// Render into the given area
    fn render(&mut self, frame: &mut Frame, area: Rect);
}
```

### Data Flow

```
                    User Input
                        |
                        v
              +-------------------+
              |    Event Loop     |  crossterm polls stdin
              |  (tui.rs/event.rs)|  at ~30fps tick rate
              +-------------------+
                        |
                   KeyEvent / Resize
                        |
                        v
              +-------------------+
              |       App         |  Routes event to focused component
              +-------------------+
                   |           |
          (if editor      (if global
           focused)        hotkey)
                   |           |
                   v           v
            +-----------+  +------------------+
            |  Editor   |  | Mode toggle,     |
            |           |  | Save, Quit       |
            +-----------+  +------------------+
                   |
            Action::ContentChanged
                   |
                   v
            +-----------+
            |  Preview  |  Receives raw markdown string
            |           |  Parses via MarkdownRenderer
            +-----------+  Caches rendered Text
                   |
                   v
              +-------------------+
              |   Render Phase    |  All components render into Frame
              |  (immediate mode) |  Layout -> StatusBar -> Editor -> Preview
              +-------------------+
                        |
                        v
                   Terminal Output
```

**Critical data flow detail:** ratatui uses immediate-mode rendering. Every frame, the entire UI is redrawn from current state. There is no retained widget tree. The `Frame::render_widget()` call takes a widget and a `Rect` and draws it into a buffer. Ratatui diffs the buffer against the previous frame and only sends changed cells to the terminal.

### Event Loop Design

**Use synchronous crossterm, not async/tokio** (MEDIUM confidence -- based on ratatui FAQ guidance).

Rationale: The ratatui FAQ explicitly states "the real question for async architecture is what other parts of your app require or benefit from being async; if not much, it may be simpler to avoid async and tokio." This editor has no network calls, no background tasks, no file watching. A synchronous event loop with `crossterm::event::poll()` with a timeout is simpler and avoids the tokio dependency entirely.

```rust
// Simplified event loop pattern
loop {
    // Render
    terminal.draw(|frame| app.render(frame))?;

    // Poll with timeout (controls tick rate / responsiveness)
    if crossterm::event::poll(Duration::from_millis(33))? {  // ~30fps
        let event = crossterm::event::read()?;
        let action = app.handle_event(event);
        if let Some(Action::Quit) = action {
            break;
        }
    }

    // Tick: update preview if content changed
    app.tick();
}
```

### Layout System

ratatui's `Layout` uses the Cassowary constraint solver. For the split-pane editor:

```rust
// Side-by-side mode
let main_chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
        Constraint::Percentage(50),  // Editor
        Constraint::Percentage(50),  // Preview
    ])
    .split(body_area);

// With status bar
let outer = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Fill(1),         // Main content area
        Constraint::Length(1),       // Status bar
    ])
    .split(frame.area());
```

**Layout modes** are an enum, not a complex state machine:

```rust
enum LayoutMode {
    Split,         // Editor left, preview right (default)
    EditorOnly,    // Full-width editor
    PreviewOnly,   // Full-width preview
}
```

### Scroll Synchronization Strategy

Proportional scroll sync between editor and preview. The editor's scroll position (as a percentage of total lines) maps to the preview's scroll position (as a percentage of total rendered height).

```
editor_scroll_ratio = editor_top_line / editor_total_lines
preview_scroll_offset = editor_scroll_ratio * preview_total_height
```

This is the simplest approach that works. Element-level sync (mapping headings to headings) is more accurate but significantly more complex -- defer to a later phase. Proportional sync is good enough for v1.

**Implementation note:** tui-textarea exposes cursor position and viewport info. The preview component tracks its own total rendered height. App calculates the ratio and passes the target offset to Preview on each tick where content or scroll changed.

### Markdown Rendering Pipeline

```
Raw markdown string (from Editor)
        |
        v
  pulldown-cmark parser  -->  Event stream (heading, paragraph, code, etc.)
        |
        v
  Custom renderer         -->  Maps events to ratatui Spans/Lines with styles
        |                      (handles: bold, italic, headers, lists, code blocks,
        |                       blockquotes, links, tables, horizontal rules)
        |
        v
  syntect (for code blocks) --> Syntax-highlighted spans
        |
        v
  ratatui::text::Text     -->  Ready to render via Paragraph widget
        |
        v
  Paragraph::new(text).scroll((offset, 0))  -->  Rendered in preview area
```

**Why build a custom renderer instead of using tui-markdown directly:** tui-markdown is experimental (described as "Proof of Concept" by its author Josh McKinney). It converts markdown to a single `Text` value, which works, but you lose control over element-level scroll mapping and custom styling. Starting with pulldown-cmark directly gives full control. However, tui-markdown is a reasonable starting point for a first pass -- use it initially, replace with custom renderer when limitations appear.

**Recommended approach:** Start with tui-markdown for the MVP. It supports headings, bold, italic, code blocks, lists, blockquotes, links, tables. Replace with custom pulldown-cmark renderer in a later phase if scroll sync or custom styling demands it.

### Editor Component Detail

**Use tui-textarea** (HIGH confidence -- 490 stars, actively maintained, purpose-built for ratatui).

Key capabilities it provides out-of-the-box:
- Multi-line editing with auto-scroll
- Undo/redo (Ctrl+U / Ctrl+R)
- Line numbers via `set_line_number_style()`
- Cursor line highlighting
- Emacs-style keybindings (Ctrl+A/E/K/N/P/F/B)
- Text selection
- Search with regex (optional feature)
- Mouse scroll support

**What it does NOT provide:**
- Markdown syntax highlighting (you add this via custom style mapping)
- Save/load (you handle file I/O separately)
- Scroll position as a ratio (you calculate from `cursor()` and `lines()`)

Markdown syntax highlighting in the editor pane requires either:
1. A tree-sitter markdown grammar mapped to ratatui styles (complex, accurate)
2. A regex-based line-by-line highlighter (simpler, good enough for v1)
3. Running pulldown-cmark on the content and mapping source ranges to styles (medium complexity, leverages existing parser)

**Recommendation:** Option 3 -- reuse pulldown-cmark. Parse the document, get source ranges from events, map to styles. This avoids adding tree-sitter as a dependency and uses the same parser as the preview.

### File I/O

Simple and synchronous. No async needed.

```rust
pub fn load_file(path: &Path) -> Result<String> {
    fs::read_to_string(path)
        .or_else(|e| if e.kind() == NotFound { Ok(String::new()) } else { Err(e.into()) })
}

pub fn save_file(path: &Path, content: &str) -> Result<()> {
    // Write to temp file first, then rename (atomic write)
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, content)?;
    fs::rename(&tmp, path)?;
    Ok(())
}
```

Atomic write via temp file + rename prevents data loss on crash during save. This is standard practice.

## Patterns to Follow

### Pattern 1: Dirty Flag for Preview Updates

Only re-render markdown when content actually changed. Parsing markdown on every frame is wasteful.

```rust
struct App {
    content_dirty: bool,
    // ...
}

fn tick(&mut self) {
    if self.content_dirty {
        let markdown = self.editor.lines().join("\n");
        self.preview.update_content(&markdown);
        self.content_dirty = false;
    }
}
```

### Pattern 2: Action-Based Communication

Components communicate through an Action enum, never by holding references to each other.

```rust
enum Action {
    Quit,
    Save,
    ToggleLayout,
    ContentChanged,
    ScrollEditor(i32),
    SetMode(LayoutMode),
    Noop,
}
```

### Pattern 3: Focus Management

Only one component receives key events at a time. The App tracks which component is focused.

```rust
enum Focus {
    Editor,
    // Preview is display-only, never focused in v1
}
```

For v1, focus is always on Editor (Preview is read-only). This simplifies event routing but the architecture supports adding focus switching later.

## Anti-Patterns to Avoid

### Anti-Pattern 1: Shared Mutable State Between Components
**What:** Components holding `Rc<RefCell<>>` references to shared state.
**Why bad:** Leads to borrow panics at runtime, makes data flow invisible, hard to debug.
**Instead:** Components return Actions. App mediates all state sharing.

### Anti-Pattern 2: Async for No Reason
**What:** Using tokio + async event stream when there are no async operations.
**Why bad:** Adds ~2MB to binary, increases compile time by ~30%, adds complexity with no benefit.
**Instead:** Synchronous crossterm::event::poll() with timeout.

### Anti-Pattern 3: Re-parsing Markdown Every Frame
**What:** Running pulldown-cmark on every render call (~30fps).
**Why bad:** Wastes CPU, causes visible lag on large documents.
**Instead:** Dirty flag pattern. Only re-parse when editor content changes.

### Anti-Pattern 4: Monolithic app.rs
**What:** Putting all logic in a single app.rs file with a giant match statement.
**Why bad:** Becomes unmaintainable past ~500 lines. Hard to add features.
**Instead:** Component architecture. Each component in its own file.

## Scalability Considerations

| Concern | Small files (<100 lines) | Medium files (<1000 lines) | Large files (>5000 lines) |
|---------|-------------------------|---------------------------|--------------------------|
| Parsing speed | Negligible | ~1ms | ~10-50ms, may need debounce |
| Preview rendering | Instant | Fast | May need viewport-only rendering |
| Editor performance | tui-textarea handles well | tui-textarea handles well | May need to profile |
| Memory | Trivial | Trivial | Still fine (<10MB for huge docs) |

For large files, add a debounce to the dirty flag: only re-parse after 100ms of no typing. This prevents lag while typing rapidly in large documents.

## Suggested Build Order

Based on component dependencies, build in this order:

1. **Terminal infrastructure** (main.rs, tui.rs, event.rs) -- everything else depends on this
2. **App skeleton with layout** (app.rs, layout.rs) -- need somewhere to put components
3. **Editor component** (editor.rs wrapping tui-textarea) -- core editing, no preview yet
4. **File I/O** (file_io.rs) -- open and save files, makes editor usable
5. **Status bar** (status_bar.rs) -- shows file info, makes app feel real
6. **Preview component with tui-markdown** (preview.rs, markdown/) -- the differentiating feature
7. **Scroll sync** -- connects editor and preview scroll positions
8. **Editor syntax highlighting** -- polish, not required for function
9. **Layout mode toggling** -- split/editor-only/preview-only

**Rationale:** Steps 1-5 produce a functional (if basic) terminal text editor. Step 6 adds the core differentiator. Steps 7-9 are polish. This ordering means every milestone produces something usable.

## Sources

- [Ratatui Application Patterns](https://ratatui.rs/concepts/application-patterns/) -- HIGH confidence
- [Ratatui Component Architecture](https://ratatui.rs/concepts/application-patterns/component-architecture/) -- HIGH confidence
- [Ratatui Component Template Project Structure](https://ratatui.rs/templates/component/project-structure/) -- HIGH confidence
- [Ratatui Layout Concepts](https://ratatui.rs/concepts/layout/) -- HIGH confidence
- [Ratatui Terminal and Event Handler Recipe](https://ratatui.rs/recipes/apps/terminal-and-event-handler/) -- HIGH confidence
- [tui-textarea GitHub](https://github.com/rhysd/tui-textarea) -- HIGH confidence
- [tui-markdown GitHub](https://github.com/joshka/tui-markdown) -- MEDIUM confidence (experimental)
- [Ratatui Async Event Stream Tutorial](https://ratatui.rs/tutorials/counter-async-app/async-event-stream/) -- HIGH confidence (used to decide against async)
- [Ratatui FAQ on Async](https://ratatui.rs/faq/) -- HIGH confidence
- [Scroll Sync in Dual-Pane Editors](https://dev.to/woai3c/implementing-synchronous-scrolling-in-a-dual-pane-markdown-editor-5d75) -- MEDIUM confidence
- [tui-scrollview](https://github.com/joshka/tui-scrollview) -- MEDIUM confidence
