# Phase 1: Terminal Editor - Research

**Researched:** 2026-03-21
**Domain:** Rust TUI text editor (ratatui + crossterm + ratatui-textarea)
**Confidence:** HIGH

## Summary

Phase 1 builds a fully functional terminal text editor for markdown files -- no preview, no syntax highlighting, just solid editing with undo/redo, line numbers, file I/O, status bar, and terminal safety. The core challenge is wiring together ratatui 0.30's new convenience APIs, the ratatui-textarea widget (forked version compatible with 0.30), and crossterm's event loop into a clean component architecture.

The most important finding from this research is a **version incompatibility**: the originally recommended `tui-textarea` 0.7 depends on ratatui 0.29, not 0.30. The correct crate is `ratatui-textarea` 0.8.0 (maintained under the ratatui GitHub org), which uses ratatui-core 0.1 and ratatui-widgets 0.3 -- the modular crates from ratatui 0.30. This is the crate to use.

The second key finding is that `ratatui::run()` (new in 0.30) handles terminal init, panic hooks, and teardown automatically -- simplifying D-16 and D-18 significantly. Signal handling (D-17) still needs manual implementation.

**Primary recommendation:** Use `ratatui::run()` for terminal lifecycle, `ratatui-textarea` 0.8 for the editor widget with `input_without_shortcuts()` for custom keybindings, and build incrementally: panic safety first, then editor widget, then file I/O, then status bar.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** `mdedit file.md` opens the file for editing. If the file doesn't exist, create it on first save (not immediately).
- **D-02:** `mdedit` with no arguments opens an empty buffer titled "[untitled]". On save, prompt for filename.
- **D-03:** Exit codes: 0 for clean exit, 1 for error. No special exit codes needed for v1.
- **D-04:** Use `clap` for argument parsing -- standard Rust CLI crate, minimal overhead.
- **D-05:** Follow nano/micro conventions (most intuitive for non-vim users):
  - `Ctrl+S` -- Save
  - `Ctrl+Q` -- Quit
  - `Ctrl+Z` -- Undo
  - `Ctrl+Y` -- Redo
  - `Ctrl+Left/Right` -- Word jump
  - `Home/End` -- Line start/end
  - `Ctrl+Home/End` -- Document start/end (if supported)
- **D-06:** No keybinding conflicts with common terminal shortcuts (Ctrl+C exits, don't override it for copy).
- **D-07:** Ctrl+C should NOT exit the editor. Instead, it does nothing or copies (if selection exists in a later phase). Ctrl+Q is the explicit quit command.
- **D-08:** Line numbers in a left gutter, right-aligned, dimmed color (not distracting).
- **D-09:** Cursor is a block cursor (standard terminal cursor, not custom rendering).
- **D-10:** Use terminal's default color scheme -- no hardcoded colors. Detect 256-color vs truecolor support.
- **D-11:** No line wrapping for v1 -- long lines scroll horizontally.
- **D-12:** Ctrl+S saves immediately. Status bar shows "Saved" for 2 seconds, then reverts to normal.
- **D-13:** On exit with unsaved changes: show a bar prompt "Unsaved changes. Save? (y/n/Esc)" -- y saves and exits, n exits without saving, Esc cancels exit.
- **D-14:** No auto-save. No backup/swap files. Keep it simple.
- **D-15:** Modified indicator is a dot or `[+]` after the filename in the status bar.
- **D-16:** Panic hook installed before entering raw mode -- uses `std::panic::set_hook` to restore terminal state.
- **D-17:** Also handle SIGINT/SIGTERM gracefully -- restore terminal before exit.
- **D-18:** Use crossterm's `enable_raw_mode` / `disable_raw_mode` and alternate screen.

### Claude's Discretion
- Exact gutter width calculation (auto-sized to line count digits)
- Status bar layout and styling
- Internal module organization
- Error message wording
- ratatui-textarea configuration details

### Deferred Ideas (OUT OF SCOPE)
- WYSIWYG editing in preview mode -- added as v2 requirement (PREV-09)
- Markdown flavor selection (GFM, Obsidian, Lark) -- v2 (PREV-07)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| FOUND-01 | Open .md file by CLI argument | clap derive for arg parsing, ratatui-textarea `insert_str()` to load content |
| FOUND-02 | New empty buffer when no argument | TextArea::default() creates empty buffer |
| FOUND-03 | Start in under 100ms | All native Rust, no lazy-loading needed -- expect <50ms |
| FOUND-04 | Single binary, no runtime deps | Rust + pure-Rust crates, no C dependencies |
| FOUND-05 | Terminal state restored on exit/crash | `ratatui::run()` handles panic hook; add signal handler for SIGINT/SIGTERM |
| FOUND-06 | Handle terminal resize | crossterm emits `Event::Resize`; ratatui re-renders on next `draw()` call |
| EDIT-01 | Insert, delete, edit text | ratatui-textarea handles all basic editing via `input_without_shortcuts()` |
| EDIT-02 | Cursor movement (arrows, Home/End, word jump) | `TextArea::move_cursor(CursorMove::*)` + custom key routing |
| EDIT-03 | Undo (Ctrl+Z) and Redo (Ctrl+Y) | `TextArea::undo()` / `TextArea::redo()` called from custom keybinding handler |
| EDIT-04 | Save with Ctrl+S, status bar confirmation | Atomic file write (temp + rename), timed status message |
| EDIT-05 | Warn on exit with unsaved changes | Track modified flag, intercept Ctrl+Q, show prompt bar |
| EDIT-06 | Line numbers in left gutter | `TextArea::set_line_number_style()` |
| EDIT-10 | Unicode characters (multi-byte, wide) | ratatui-textarea uses unicode-width internally; add `unicode-width` for any custom layout code |
| CHRM-01 | Status bar: filename, position, modified | Custom widget rendered below editor area |
| CHRM-03 | Works over SSH, no Nerd Fonts | crossterm uses standard VT100/ANSI; no special features needed |
</phase_requirements>

## Standard Stack

### Core (Phase 1 only -- no preview/highlighting crates)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| ratatui | 0.30.x | TUI framework | De facto Rust TUI standard. New in 0.30: `ratatui::run()` convenience, modular workspace |
| crossterm | 0.29.x | Terminal backend | Pure Rust, cross-platform, default for ratatui 0.30. Works over SSH |
| ratatui-textarea | 0.8.0 | Editor widget | Fork of tui-textarea maintained under ratatui org. Compatible with ratatui 0.30 (uses ratatui-core 0.1 + ratatui-widgets 0.3). Provides undo/redo, line numbers, cursor management |
| clap | 4.x | CLI argument parsing | Standard Rust CLI crate, derive macro support |
| anyhow | 1.x | Error handling | Application-level error handling, clean error chains |
| unicode-width | 0.2.x | Display width calculation | Correct column width for CJK/emoji in custom layout code |

### CRITICAL: Version Compatibility Fix

The STACK.md research recommended `tui-textarea` 0.7. **This is incompatible with ratatui 0.30.** tui-textarea 0.7.0's Cargo.toml pins `ratatui = "0.29.0"`.

**Use `ratatui-textarea` 0.8.0 instead.** This is the ratatui org's maintained fork, published 2026-02-21, targeting ratatui 0.30's modular crates. Same API surface -- `TextArea`, `input()`, `input_without_shortcuts()`, `undo()`, `redo()`, `move_cursor()`, `set_line_number_style()` all present.

### NOT Needed in Phase 1

| Library | Why Not Yet |
|---------|-------------|
| pulldown-cmark | No markdown parsing in Phase 1 (editor only) |
| tui-markdown | No preview rendering in Phase 1 |
| syntect | No syntax highlighting in Phase 1 |
| unicode-segmentation | ratatui-textarea handles grapheme clusters internally |

### Installation

```bash
cargo init mdedit
cargo add ratatui --features crossterm
cargo add crossterm
cargo add ratatui-textarea --features crossterm
cargo add clap --features derive
cargo add anyhow
cargo add unicode-width
```

### Cargo.toml (Phase 1)

```toml
[package]
name = "mdedit"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"

[dependencies]
ratatui = { version = "0.30", features = ["crossterm"] }
crossterm = "0.29"
ratatui-textarea = { version = "0.8", features = ["crossterm"] }
clap = { version = "4", features = ["derive"] }
anyhow = "1"
unicode-width = "0.2"

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
strip = true
```

## Architecture Patterns

### Project Structure (Phase 1)

```
src/
  main.rs           # CLI parsing (clap), terminal lifecycle (ratatui::run)
  app.rs            # App struct: owns editor + status bar, routes events, manages mode
  editor.rs         # TextArea wrapper, custom keybindings, modified tracking
  status_bar.rs     # Filename, cursor pos, modified indicator, timed messages
  file_io.rs        # Atomic file read/write
```

Phase 1 does NOT need the full component architecture from ARCHITECTURE.md. No preview, no layout manager, no markdown modules. Keep it minimal -- five files total.

### Pattern 1: ratatui::run() for Terminal Lifecycle

**What:** New in ratatui 0.30. Handles terminal init, alternate screen, raw mode, panic hook, and teardown in one call.
**When to use:** Always -- it replaces manual Terminal::new(), enable_raw_mode(), panic hook setup.
**Example:**
```rust
// Source: https://ratatui.rs/highlights/v030/
fn main() -> Result<(), Box<dyn std::error::Error>> {
    ratatui::run(|terminal| {
        let mut app = App::new();
        app.run(terminal)
    })
}
```

**Important:** `ratatui::run()` handles panic hooks but does NOT handle SIGINT/SIGTERM signals. Signal handling must be added separately (see Pattern 4).

### Pattern 2: Custom Keybindings with input_without_shortcuts()

**What:** `TextArea::input_without_shortcuts()` processes only basic input (chars, Tab, Enter, Backspace, Delete). All shortcut keys (Ctrl+*, Alt+*) are ignored, letting you implement custom bindings.
**When to use:** Always -- the default `input()` uses Emacs keybindings (Ctrl+U=undo, Ctrl+R=redo) which conflict with D-05's nano-style bindings.
**Example:**
```rust
// Source: https://docs.rs/ratatui-textarea/0.8.0/
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
    match (key.modifiers, key.code) {
        // Custom keybindings (D-05)
        (KeyModifiers::CONTROL, KeyCode::Char('s')) => Some(Action::Save),
        (KeyModifiers::CONTROL, KeyCode::Char('q')) => Some(Action::RequestQuit),
        (KeyModifiers::CONTROL, KeyCode::Char('z')) => {
            self.textarea.undo();
            Some(Action::ContentChanged)
        }
        (KeyModifiers::CONTROL, KeyCode::Char('y')) => {
            self.textarea.redo();
            Some(Action::ContentChanged)
        }
        // Word jump
        (KeyModifiers::CONTROL, KeyCode::Left) => {
            self.textarea.move_cursor(CursorMove::WordBack);
            None
        }
        (KeyModifiers::CONTROL, KeyCode::Right) => {
            self.textarea.move_cursor(CursorMove::WordForward);
            None
        }
        // Home/End
        (KeyModifiers::NONE, KeyCode::Home) => {
            self.textarea.move_cursor(CursorMove::Head);
            None
        }
        (KeyModifiers::NONE, KeyCode::End) => {
            self.textarea.move_cursor(CursorMove::End);
            None
        }
        // Ctrl+C does nothing (D-07)
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => None,
        // All other keys: default basic input handling
        _ => {
            let changed = self.textarea.input_without_shortcuts(key);
            if changed { Some(Action::ContentChanged) } else { None }
        }
    }
}
```

### Pattern 3: Atomic File Write

**What:** Write to temp file, then rename. Prevents data loss if the process crashes mid-write.
**When to use:** Every save operation.
**Example:**
```rust
// Source: ARCHITECTURE.md research
use std::fs;
use std::path::Path;

pub fn save_file(path: &Path, content: &str) -> anyhow::Result<()> {
    let tmp = path.with_extension("mdedit-tmp");
    fs::write(&tmp, content)?;
    fs::rename(&tmp, path)?;
    Ok(())
}
```

### Pattern 4: Signal Handling for SIGINT/SIGTERM (D-17)

**What:** `ratatui::run()` handles panics but NOT signals. Ctrl+C sends SIGINT. We need to catch it so the editor doesn't exit without cleanup.
**When to use:** At startup, before entering the event loop.
**Example:**
```rust
// Approach: crossterm already captures Ctrl+C as a KeyEvent when in raw mode.
// In raw mode, SIGINT is NOT generated by Ctrl+C -- crossterm intercepts it.
// So D-07 (Ctrl+C does nothing) is handled by the keybinding router.
//
// For actual SIGTERM (kill signal from OS), ratatui::run()'s Drop-based
// cleanup handles terminal restoration. No additional signal handler needed
// for the common case.
```

**Key insight:** In raw mode, the terminal does NOT send SIGINT for Ctrl+C. crossterm intercepts all keyboard input as events. So D-07 and D-17 are largely handled by being in raw mode + ratatui::run()'s cleanup. Only a `kill -TERM` from another process could leave the terminal dirty, and ratatui::run()'s drop guard handles that.

### Pattern 5: Status Bar with Timed Messages (D-12)

**What:** Show "Saved" message for 2 seconds after save, then revert to normal status.
**When to use:** After every save operation.
**Example:**
```rust
use std::time::{Duration, Instant};

struct StatusBar {
    filename: String,
    modified: bool,
    timed_message: Option<(String, Instant)>,
    message_duration: Duration,
}

impl StatusBar {
    fn set_message(&mut self, msg: &str) {
        self.timed_message = Some((msg.to_string(), Instant::now()));
    }

    fn display_text(&self, cursor: (usize, usize)) -> String {
        // Check if timed message is still active
        if let Some((ref msg, when)) = self.timed_message {
            if when.elapsed() < self.message_duration {
                return msg.clone();
            }
        }
        // Normal status: filename [+] | Ln X, Col Y
        let modified = if self.modified { " [+]" } else { "" };
        format!(" {}{} | Ln {}, Col {}",
            self.filename, modified, cursor.0 + 1, cursor.1 + 1)
    }
}
```

### Pattern 6: Unsaved Changes Prompt (D-13)

**What:** When user presses Ctrl+Q with unsaved changes, show a prompt bar instead of exiting.
**When to use:** Exit flow.
**Example:**
```rust
enum AppMode {
    Editing,
    ConfirmQuit,  // "Unsaved changes. Save? (y/n/Esc)"
    PromptFilename, // For new files: "Save as: ___"
}

// In event handler during ConfirmQuit mode:
match key.code {
    KeyCode::Char('y') | KeyCode::Char('Y') => {
        self.save()?;
        return Ok(true); // signal quit
    }
    KeyCode::Char('n') | KeyCode::Char('N') => {
        return Ok(true); // quit without save
    }
    KeyCode::Esc => {
        self.mode = AppMode::Editing; // cancel quit
    }
    _ => {} // ignore other keys
}
```

### Anti-Patterns to Avoid
- **Using `TextArea::input()` instead of `input_without_shortcuts()`:** Default keybindings are Emacs-style (Ctrl+U=undo) which conflicts with nano-style decisions. Always use `input_without_shortcuts()` and route all shortcuts manually.
- **Monolithic main.rs:** Even for Phase 1's simple scope, separate concerns into 5 files. Phase 2 adds preview/markdown modules; if Phase 1 is a monolith, Phase 2 becomes a rewrite.
- **Calling `terminal.clear()` manually:** Let ratatui's double-buffering handle diffs. Manual clearing causes full-screen flicker.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Multi-line text editing | Custom cursor/buffer management | ratatui-textarea 0.8 | Cursor movement, scrolling, undo/redo, line numbers are months of work |
| Terminal lifecycle | Manual raw mode + alternate screen + panic hook | `ratatui::run()` | Handles all edge cases including panic recovery |
| CLI argument parsing | Manual `std::env::args()` parsing | clap 4 derive | Handles edge cases, generates help text, future-proof for more args |
| Display width for CJK/emoji | Character counting | unicode-width | CJK chars are 2 columns, emoji widths vary. Wrong width = cursor misalignment |

**Key insight:** Phase 1's value is in the assembly and keybinding customization, not in reimplementing solved problems. The only novel code is the keybinding router, status bar, file I/O integration, and the unsaved-changes prompt flow.

## Common Pitfalls

### Pitfall 1: tui-textarea vs ratatui-textarea Version Confusion
**What goes wrong:** Using `tui-textarea` 0.7 with ratatui 0.30 causes compile errors due to incompatible ratatui versions.
**Why it happens:** The STACK.md research recommended tui-textarea 0.7. But tui-textarea 0.7 depends on ratatui 0.29. ratatui-textarea 0.8 is the correct crate for ratatui 0.30.
**How to avoid:** Use `cargo add ratatui-textarea` (not `tui-textarea`). The crate is `ratatui-textarea` on crates.io.
**Warning signs:** Compile errors about mismatched ratatui types.

### Pitfall 2: Using input() Instead of input_without_shortcuts()
**What goes wrong:** Default Emacs keybindings fire. Ctrl+U deletes line instead of nothing. Ctrl+Z does nothing (not mapped by default). Ctrl+R triggers redo instead of nothing.
**Why it happens:** `TextArea::input()` includes all default Emacs keybindings. The nano-style bindings in D-05 conflict.
**How to avoid:** Always use `input_without_shortcuts()` for basic input, then manually route all Ctrl+* and special keys.
**Warning signs:** Unexpected behavior when pressing Ctrl+key combinations.

### Pitfall 3: Forgetting to Track Modified State
**What goes wrong:** User makes edits, presses Ctrl+Q, and the app exits without prompting because modified flag was never set.
**Why it happens:** `input_without_shortcuts()` returns `bool` indicating if content changed, but it's easy to ignore the return value.
**How to avoid:** Every call to `input_without_shortcuts()`, `undo()`, `redo()`, and direct TextArea mutation methods must check the return value and update the modified flag.
**Warning signs:** Status bar never shows `[+]`. Ctrl+Q exits without prompt after editing.

### Pitfall 4: Ctrl+Home/End Not Being a Standard crossterm Event
**What goes wrong:** D-05 specifies Ctrl+Home/End for document start/end. crossterm may report these differently across terminals.
**Why it happens:** Not all terminals send Ctrl+Home/End as distinct key events. Some send escape sequences that crossterm may not parse.
**How to avoid:** Test on target terminals. Have a fallback (e.g., Ctrl+Up/Down or other binding). CursorMove::Top and CursorMove::Bottom are the TextArea methods to call.
**Warning signs:** Ctrl+Home/End does nothing in certain terminal emulators.

### Pitfall 5: New File Save Prompt Complexity
**What goes wrong:** D-02 says `mdedit` with no args opens "[untitled]" and prompts for filename on save. This requires a text input mode within the status bar -- essentially a mini text editor inside the status bar.
**Why it happens:** It seems simple but requires: switching app mode, capturing text input, rendering an input field in the status bar area, handling Enter/Esc, then performing the save.
**How to avoid:** Use a simple approach: render a Paragraph widget in the status bar area with an inline text buffer. Don't create a second TextArea for this -- just a String + cursor position.
**Warning signs:** Overcomplicating the filename prompt with a full widget.

## Code Examples

### Minimal App Skeleton

```rust
// Source: ratatui 0.30 docs + ratatui-textarea 0.8 docs
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui_textarea::{TextArea, CursorMove};
use std::time::Duration;

struct App<'a> {
    textarea: TextArea<'a>,
    filepath: Option<std::path::PathBuf>,
    modified: bool,
    should_quit: bool,
    // ... status bar, mode
}

impl<'a> App<'a> {
    fn run(&mut self, terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
        loop {
            terminal.draw(|frame| self.render(frame))?;

            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key(key)?;
                }
                if let Event::Resize(_, _) = event::read().unwrap_or(Event::FocusLost) {
                    // ratatui handles resize automatically on next draw()
                }
            }

            if self.should_quit {
                break;
            }
        }
        Ok(())
    }

    fn render(&self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Fill(1),    // Editor
                Constraint::Length(1),  // Status bar
            ])
            .split(frame.area());

        // Render editor
        frame.render_widget(&self.textarea, chunks[0]);

        // Render status bar
        // ... (status_bar.render)
    }
}

fn main() -> Result<()> {
    ratatui::run(|terminal| {
        let mut app = App::new(/* parsed args */);
        app.run(terminal)
    })?;
    Ok(())
}
```

### Line Number Setup

```rust
// Source: https://docs.rs/ratatui-textarea/0.8.0/
use ratatui::style::{Color, Style};

let mut textarea = TextArea::default();
textarea.set_line_number_style(Style::default().fg(Color::DarkGray));
```

### Loading File Content

```rust
fn load_file(path: &std::path::Path) -> Result<String> {
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(content),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.into()),
    }
}

// Set content into TextArea
let content = load_file(&path)?;
let lines: Vec<String> = content.lines().map(String::from).collect();
let textarea = TextArea::new(lines);
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual Terminal init/restore + panic hook | `ratatui::run()` | ratatui 0.30 (2026) | Eliminates ~20 lines of boilerplate, handles edge cases |
| `tui-textarea` 0.7 | `ratatui-textarea` 0.8 | Feb 2026 | Proper ratatui 0.30 compatibility via modular crates |
| `Block::title(Title::from(...))` | `Block::title("...")` (Into<Line>) | ratatui 0.30 | Simpler API, but breaking change from 0.29 |
| `Alignment` enum | `HorizontalAlignment` | ratatui 0.30 | Renamed -- will cause compile errors if using old name |

## Open Questions

1. **Ctrl+Home/End terminal support**
   - What we know: CursorMove::Top and CursorMove::Bottom exist in ratatui-textarea
   - What's unclear: Whether crossterm reliably reports Ctrl+Home/End across terminals (macOS Terminal.app, iTerm2, Alacritty, SSH)
   - Recommendation: Implement it, test on target terminals, document as "may not work in all terminals" if issues arise

2. **ratatui-textarea 0.8 feature flag name for crossterm**
   - What we know: tui-textarea 0.7 used `--features crossterm`. ratatui-textarea 0.8 should be similar.
   - What's unclear: Exact feature flag name (could be `crossterm` or `ratatui-crossterm`)
   - Recommendation: Check `cargo add ratatui-textarea --features crossterm` output; if it fails, check the crate's Cargo.toml on docs.rs

3. **TextArea rendering syntax with ratatui 0.30**
   - What we know: Deprecated `widget()` method; `&TextArea` can be rendered directly
   - What's unclear: Exact `frame.render_widget()` call syntax with ratatui 0.30
   - Recommendation: Use `frame.render_widget(&self.textarea, area)` -- the `Widget` trait is implemented for `&TextArea`

## Sources

### Primary (HIGH confidence)
- [ratatui v0.30 highlights](https://ratatui.rs/highlights/v030/) - `ratatui::run()`, modular workspace, breaking changes
- [ratatui-textarea 0.8.0 docs](https://docs.rs/ratatui-textarea/0.8.0/ratatui_textarea/struct.TextArea.html) - Full API reference
- [tui-textarea 0.7.0 Cargo.toml](https://docs.rs/crate/tui-textarea/latest/source/Cargo.toml.orig) - Confirmed ratatui 0.29 dependency (incompatible with 0.30)
- [ratatui-textarea crate](https://docs.rs/crate/ratatui-textarea/latest) - v0.8.0, published 2026-02-21, targets ratatui-core 0.1

### Secondary (MEDIUM confidence)
- [ratatui panic hooks recipe](https://ratatui.rs/recipes/apps/panic-hooks/) - Manual panic hook setup (superseded by `ratatui::run()`)
- [tui-textarea GitHub](https://github.com/rhysd/tui-textarea) - Custom keybinding patterns, `input_without_shortcuts()` documentation
- [ratatui-textarea GitHub](https://github.com/ratatui/ratatui-textarea) - Fork relationship confirmed, maintained by ratatui org

### Tertiary (LOW confidence)
- Ctrl+Home/End terminal compatibility -- no authoritative source found, needs runtime testing

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Versions verified via crates.io docs, compatibility confirmed
- Architecture: HIGH - Patterns from official ratatui docs, adapted for Phase 1 scope
- Pitfalls: HIGH - Version mismatch discovered and documented, keybinding approach verified via API docs
- Keybinding remapping: HIGH - `input_without_shortcuts()` + manual routing confirmed via docs
- Unicode handling: MEDIUM - ratatui-textarea uses unicode-width internally, but unclear on grapheme cluster edge cases

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable ecosystem, 30-day window)
