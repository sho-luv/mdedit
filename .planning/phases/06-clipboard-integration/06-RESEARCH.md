# Phase 6: Clipboard Integration - Research

**Researched:** 2026-03-23
**Domain:** Terminal clipboard (OSC 52 + platform-native), Rust TUI integration
**Confidence:** HIGH

## Summary

Clipboard integration for a terminal editor has two independent channels: **writing** (yank/delete to clipboard) and **reading** (paste from clipboard). The write path uses OSC 52 escape sequences as the primary mechanism, with platform-native tools (pbcopy, xclip, wl-copy) as fallback. The read path uses platform-native tools when available; when only OSC 52 is available, paste relies on the terminal's own bracketed paste mode (user presses Cmd+V, terminal sends `Event::Paste`).

The implementation is straightforward because: (1) OSC 52 write is just a stdout write of a base64-encoded escape sequence, (2) platform-native tools are simple subprocess pipes, (3) crossterm already supports bracketed paste events via `Event::Paste(String)`, and (4) all ~15 `set_yank_register()` call sites in app.rs are natural hook points for clipboard write.

**Primary recommendation:** Build a small `clipboard` module (~150-200 lines) with a `ClipboardProvider` trait and three implementations (Osc52, PlatformNative, Noop). No external clipboard crates needed -- the implementation is simpler than any crate's dependency tree. Use the `base64` crate (or inline encoder) for OSC 52 encoding.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- D-01: OSC 52 is the primary clipboard mechanism -- used first in all environments (local and SSH). Platform-native tools are the fallback.
- D-02: Platform-native fallback order: macOS -> `pbcopy`/`pbpaste`, Linux Wayland (`$WAYLAND_DISPLAY` set) -> `wl-copy`/`wl-paste`, Linux X11 -> `xclip`/`xsel`.
- D-03: Fully automatic detection -- no `clipboard` config knob. Provider is selected at startup and cached.
- D-04: If no clipboard mechanism works, show a one-time status bar warning on first yank: "System clipboard unavailable -- using internal register". Internal yank/paste continues to work.
- D-05: Every yank AND delete operation writes to system clipboard -- `y`, `yy`, `dd`, `cc`, `x`, visual yank, visual delete all sync out. Matches vim `clipboard=unnamedplus` behavior.
- D-06: `p`/`P` reads from system clipboard -- text copied in external applications is immediately available via vim paste.
- D-07: Nano mode gets clipboard support: Ctrl+C copies current selection to system clipboard, Ctrl+V pastes from system clipboard.
- D-08: Delete operations (`dd`, `x`, etc.) write to system clipboard. This is standard vim behavior -- deletes overwrite the clipboard.
- D-09: Write-only OSC 52 -- mdedit writes to clipboard via OSC 52 but does NOT attempt OSC 52 read-back. Paste relies on the terminal's own bracketed paste handling.
- D-10: No size limit handling -- payloads are base64-encoded and sent as-is.
- D-11: No OSC 52 capability detection -- just send the sequence.
- D-12: Auto-detect `$TMUX` env var and wrap OSC 52 in tmux passthrough escape.

### Claude's Discretion
- Clipboard provider trait/enum design
- How to structure the subprocess calls to pbcopy/xclip (spawn + pipe stdin, or write to temp file)
- Error handling for failed subprocess calls
- Exact status bar warning message wording and display duration
- How bracketed paste integrates with vim insert mode vs normal mode
- Whether to detect `$TMUX` at startup or per-operation

### Deferred Ideas (OUT OF SCOPE)
- Named registers (`"a`, `"b`, etc.) -- v3+
- Config knob to force clipboard provider -- add if users report issues
- OSC 52 read-back for full round-trip over SSH -- add if terminal support improves
- Clipboard history / ring buffer -- out of scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CLIP-01 | User can copy selected text to system clipboard via vim yank (y) or Ctrl+C | OSC 52 write + platform-native write; hook all `set_yank_register()` call sites; Ctrl+C in nano mode routes to clipboard write |
| CLIP-02 | User can paste from system clipboard via vim paste (p/P) or Ctrl+V | Platform-native read (pbpaste/xclip -o/wl-paste); bracketed paste via `Event::Paste` for OSC 52-only; Ctrl+V in nano mode |
| CLIP-03 | Clipboard works over SSH via OSC 52 escape sequence | OSC 52 write-only; tmux passthrough wrapping; bracketed paste for read |
| CLIP-04 | Clipboard falls back to platform-native (pbcopy/xclip) when available locally | Platform detection via env vars and `which`; subprocess piping via `std::process::Command` |
</phase_requirements>

## Standard Stack

### Core (no new crates needed)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| std::process::Command | (stdlib) | Spawn pbcopy/xclip/wl-copy subprocesses | Zero dependency, pipes stdin/stdout directly |
| std::io::Write | (stdlib) | Write OSC 52 escape to stdout | Raw byte writing to terminal |
| base64 (inline) | N/A | Base64 encode for OSC 52 payload | ~30 lines of code; helix editor does this inline too |

### Why No External Clipboard Crate

| Crate | Why Not |
|-------|---------|
| arboard (3.6.1) | Requires system libraries (X11, Wayland client libs). Adds ~15+ transitive deps. Does NOT support OSC 52. Designed for GUI apps, not TUI. Would need OSC 52 layer on top anyway. |
| cli-clipboard (0.4.0) | Thin wrapper around xclip/xsel subprocess calls. mdedit needs ~20 lines to do the same thing. No OSC 52 support. |
| copypasta / copypasta-ext | copypasta-ext has an `osc52` module but is unmaintained (last update 2021). copypasta itself wraps smithay-clipboard / x11-clipboard with heavy deps. |

**Decision:** No clipboard crate. The total clipboard module is ~150-200 lines of straightforward code. Adding a crate would increase deps for no benefit while missing the OSC 52 primary requirement.

## Architecture Patterns

### Recommended Module Structure
```
src/
├── clipboard.rs       # New: ClipboardProvider trait + implementations
├── app.rs             # Modified: holds ClipboardProvider, hooks yank/paste
├── vim.rs             # Unchanged: yank_register stays as internal buffer
├── editor.rs          # Unchanged
└── main.rs            # Modified: add EnableBracketedPaste/DisableBracketedPaste
```

### Pattern 1: ClipboardProvider Trait

**What:** A trait abstracting clipboard read/write with three implementations.
**When to use:** Always -- created at startup, passed to App.

```rust
/// Clipboard provider abstraction.
pub trait ClipboardProvider {
    /// Write text to system clipboard. Returns Ok(()) on success.
    fn write(&self, text: &str) -> Result<(), String>;
    /// Read text from system clipboard. Returns Ok(text) or Err if unsupported.
    fn read(&self) -> Result<String, String>;
    /// Human-readable name for status bar display.
    fn name(&self) -> &str;
}
```

### Pattern 2: Provider Detection at Startup

**What:** Detect best clipboard provider once at startup, cache for app lifetime.
**When to use:** In `main.rs` or `App::new()`.

```rust
pub fn detect_provider() -> Box<dyn ClipboardProvider> {
    // 1. Always try OSC 52 for write (primary per D-01)
    // 2. Check platform-native for read capability (D-02 fallback order)
    // 3. Build a composite: OSC 52 write + platform-native read
    // 4. If no platform-native available: OSC 52 write-only + read returns Err

    let in_tmux = std::env::var("TMUX").is_ok();

    // Check platform-native availability for READ
    let native = detect_native_provider();

    Box::new(CompositeProvider {
        writer: Osc52Writer { in_tmux },
        reader: native, // Option<NativeProvider>
    })
}

fn detect_native_provider() -> Option<NativeProvider> {
    if cfg!(target_os = "macos") {
        // pbcopy/pbpaste always available on macOS
        return Some(NativeProvider::MacOs);
    }

    // Linux: check Wayland first, then X11
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        if which("wl-copy") { return Some(NativeProvider::Wayland); }
    }
    if std::env::var("DISPLAY").is_ok() {
        if which("xclip") { return Some(NativeProvider::Xclip); }
        if which("xsel") { return Some(NativeProvider::Xsel); }
    }
    None
}
```

### Pattern 3: Composite Provider (OSC 52 write + native read)

**What:** A single provider that combines OSC 52 for write with platform-native for read.
**Why:** D-01 says OSC 52 is primary for write. D-06 says paste reads from system clipboard (needs native read). D-09 says no OSC 52 read-back.

```rust
pub struct CompositeProvider {
    writer: Osc52Writer,
    reader: Option<NativeProvider>,
}

impl ClipboardProvider for CompositeProvider {
    fn write(&self, text: &str) -> Result<(), String> {
        // Always write via OSC 52 (primary)
        self.writer.write(text)?;
        // Also write via native if available (belt-and-suspenders)
        if let Some(ref native) = self.reader {
            let _ = native.write(text); // best-effort
        }
        Ok(())
    }

    fn read(&self) -> Result<String, String> {
        match &self.reader {
            Some(native) => native.read(),
            None => Err("No clipboard read available (OSC 52 is write-only)".into()),
        }
    }

    fn name(&self) -> &str {
        match &self.reader {
            Some(native) => native.name(),
            None => "OSC 52 (write-only)",
        }
    }
}
```

### Pattern 4: Integration with set_yank_register

**What:** After every `set_yank_register()` call, also write to system clipboard.
**Why:** D-05 says every yank/delete syncs to clipboard.

```rust
// In app.rs, create a helper method:
fn yank_to_clipboard(&mut self, text: &str) {
    // Write to internal register
    if let Some(ref mut handler) = self.vim_handler {
        handler.set_yank_register(text.to_string());
    }
    // Write to system clipboard
    if let Err(e) = self.clipboard.write(text) {
        // Show one-time warning (D-04)
        if !self.clipboard_warned {
            self.status_bar.set_message("System clipboard unavailable -- using internal register");
            self.clipboard_warned = true;
        }
    }
}
```

### Pattern 5: Paste reads from system clipboard

**What:** p/P reads from system clipboard instead of (or preferring) internal register.
**Why:** D-06 says text copied externally should be available via vim paste.

```rust
// In PasteAfter/PasteBefore handlers:
let text = match self.clipboard.read() {
    Ok(clip_text) if !clip_text.is_empty() => clip_text,
    _ => {
        // Fallback to internal register
        self.vim_handler.as_ref()
            .map(|h| h.yank_register().to_string())
            .unwrap_or_default()
    }
};
```

### Anti-Patterns to Avoid
- **Using arboard/copypasta for OSC 52:** These crates do NOT support OSC 52. You'd add heavy deps and still need custom code.
- **Attempting OSC 52 read-back:** Most terminals don't support it, and those that do require explicit opt-in. D-09 says write-only.
- **Blocking on subprocess calls:** pbcopy/xclip should complete in <10ms. But always use `Command::output()` with a reasonable approach -- don't use `spawn()` + `wait()` separately. Use `Command::new(...).stdin(Stdio::piped()).spawn()` + write + `wait_with_output()`.
- **Detecting OSC 52 support:** D-11 says don't bother. Just send it.
- **Detecting $TMUX per-operation:** Check once at startup and cache. The user isn't going to start/stop tmux mid-session.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Base64 encoding | A full base64 library | Inline ~30 line encoder OR `base64` crate | Only need standard base64 encode. Helix does inline. Either approach is fine. |
| Platform detection | Complex feature-detection system | Simple env var checks + `which` | `$WAYLAND_DISPLAY`, `$DISPLAY`, `cfg!(target_os)` cover all cases |

**Key insight:** The clipboard module is simple enough that adding crates creates more complexity (dependency management) than it saves. The total implementation is ~150-200 lines.

## Common Pitfalls

### Pitfall 1: tmux Eats OSC 52
**What goes wrong:** OSC 52 sequences silently vanish when running inside tmux.
**Why it happens:** tmux 3.3+ defaults `allow-passthrough` to `off`. Even when passthrough is on, tmux's `set-clipboard` option controls OSC 52 behavior (`external` is default since 2.6, which means tmux processes it itself rather than passing through).
**How to avoid:** Wrap OSC 52 in tmux DCS passthrough when `$TMUX` is set. The wrap format is: `\x1bPtmux;\x1b{osc52_sequence}\x1b\\`. The inner `\x1b` of the OSC 52 sequence must be doubled. Document in README that users may need `set -g allow-passthrough on` in tmux.conf.
**Warning signs:** Clipboard works locally but not in tmux sessions.

### Pitfall 2: Forgetting to Enable Bracketed Paste
**What goes wrong:** Ctrl+V / Cmd+V in terminal doesn't trigger `Event::Paste`, instead sends individual key events.
**Why it happens:** Bracketed paste must be explicitly enabled via crossterm's `EnableBracketedPaste` command. mdedit currently does NOT enable it.
**How to avoid:** Add `EnableBracketedPaste` to the terminal init in `main.rs` alongside `EnterAlternateScreen` and `EnableMouseCapture`. Add `DisableBracketedPaste` to cleanup.
**Warning signs:** Multi-line paste appears one character at a time, or causes unexpected mode switches.

### Pitfall 3: Ctrl+C Conflict in Nano Mode
**What goes wrong:** Ctrl+C traditionally sends SIGINT. In raw terminal mode, crossterm intercepts it as a key event, but users may expect it to quit.
**Why it happens:** Nano itself uses Ctrl+C for "show cursor position." The behavior is terminal-dependent.
**How to avoid:** In raw mode, Ctrl+C is just `KeyCode::Char('c')` with CONTROL modifier. It will NOT send SIGINT. Handle it as copy. This is safe because mdedit already intercepts all Ctrl+ combinations in raw mode.
**Warning signs:** None -- this is a non-issue in raw mode. Just document the behavior.

### Pitfall 4: Subprocess Failure on Minimal Systems
**What goes wrong:** `xclip` or `wl-copy` not installed, `Command::new("xclip")` returns `Err`.
**Why it happens:** Linux minimal installations or containers may not have clipboard tools.
**How to avoid:** The `detect_native_provider()` function should verify the tool exists before caching it (check `which xclip` at startup, not at copy time). If detection fails, fall back to OSC 52-only with the one-time warning from D-04.
**Warning signs:** Clipboard works on dev machine but not in production/container.

### Pitfall 5: Base64 Newlines in OSC 52
**What goes wrong:** Some base64 encoders insert line breaks every 76 characters. Newlines inside an OSC 52 sequence corrupt it.
**Why it happens:** Standard MIME base64 includes line breaks. OSC 52 requires a single continuous base64 string.
**How to avoid:** Use base64 encoding WITHOUT line breaks. If using the `base64` crate, use `base64::engine::general_purpose::STANDARD` which does not add line breaks by default. If inline, don't add `\n`.
**Warning signs:** Large text copies are truncated or corrupted.

## Code Examples

### OSC 52 Write (with tmux passthrough)

```rust
use std::io::Write;

/// Write text to system clipboard via OSC 52 escape sequence.
fn write_osc52(text: &str, in_tmux: bool) -> Result<(), std::io::Error> {
    let encoded = base64_encode(text.as_bytes());

    let mut stdout = std::io::stdout().lock();
    if in_tmux {
        // tmux DCS passthrough: \ePtmux;\e + OSC52 + \e\\
        // The inner \e must be doubled
        write!(stdout, "\x1bPtmux;\x1b\x1b]52;c;{}\x07\x1b\\", encoded)?;
    } else {
        write!(stdout, "\x1b]52;c;{}\x07", encoded)?;
    }
    stdout.flush()?;
    Ok(())
}
```

### Inline Base64 Encoder

```rust
const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(input: &[u8]) -> String {
    let mut result = String::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;

        result.push(BASE64_CHARS[((n >> 18) & 0x3F) as usize] as char);
        result.push(BASE64_CHARS[((n >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(BASE64_CHARS[((n >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(BASE64_CHARS[(n & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}
```

### Platform-Native Write (pbcopy example)

```rust
use std::process::{Command, Stdio};

fn write_pbcopy(text: &str) -> Result<(), String> {
    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to spawn pbcopy: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(text.as_bytes())
            .map_err(|e| format!("Failed to write to pbcopy: {}", e))?;
    }
    // stdin is dropped here, closing the pipe
    let status = child.wait()
        .map_err(|e| format!("Failed to wait for pbcopy: {}", e))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("pbcopy exited with status: {}", status))
    }
}
```

### Platform-Native Read (pbpaste example)

```rust
fn read_pbpaste() -> Result<String, String> {
    let output = Command::new("pbpaste")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("Failed to run pbpaste: {}", e))?;

    if output.status.success() {
        String::from_utf8(output.stdout)
            .map_err(|e| format!("pbpaste output not UTF-8: {}", e))
    } else {
        Err("pbpaste failed".into())
    }
}
```

### Bracketed Paste Handling

```rust
// In main.rs terminal init, add:
use crossterm::event::{EnableBracketedPaste, DisableBracketedPaste};

// Setup:
execute!(stdout, EnterAlternateScreen, EnableMouseCapture, EnableBracketedPaste)?;

// Cleanup:
execute!(terminal.backend_mut(), DisableBracketedPaste, DisableMouseCapture, LeaveAlternateScreen)?;

// In app.rs event loop, add Event::Paste arm:
match event::read()? {
    Event::Key(key) => { /* existing */ }
    Event::Mouse(mouse) => { /* existing */ }
    Event::Paste(text) => {
        // Insert pasted text at cursor position
        self.editor.textarea_mut().insert_str(&text);
        self.mark_content_changed();
    }
    _ => {}
}
```

### Tool Detection (which equivalent)

```rust
fn command_exists(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| xclip/xsel only | OSC 52 primary, native fallback | ~2020-2022 (neovim, helix adopted) | Works over SSH without X forwarding |
| X11 clipboard only | Wayland (wl-copy/wl-paste) + X11 | ~2023 (Wayland default on most distros) | Must support both Wayland and X11 |
| Custom bracketed paste parsing | crossterm `Event::Paste` | crossterm 0.26+ | No manual escape sequence parsing needed |

**Terminal OSC 52 support (2025-2026):**
- iTerm2, Alacritty, kitty, WezTerm, Windows Terminal, foot, ghostty: all support OSC 52
- gnome-terminal/VTE: supports OSC 52 since VTE 0.69 (2023)
- tmux: supports with `set-clipboard on` or `allow-passthrough on`

## Open Questions

1. **Bracketed paste in Normal mode vs Insert mode**
   - What we know: Bracketed paste sends all text at once via `Event::Paste(String)`. In vim Normal mode, paste should probably enter Insert mode, paste, then return to Normal. Or: just insert at cursor regardless of mode (simpler).
   - What's unclear: Whether to match vim behavior (bracketed paste enters insert mode) or treat it as a direct insert.
   - Recommendation: Insert directly at cursor regardless of mode. The user explicitly pasted via Cmd+V; they expect the text to appear. This is simpler and matches helix/zellij behavior. Use `p` for vim-style paste semantics.

2. **Double-writing clipboard (OSC 52 + native)**
   - What we know: D-01 says OSC 52 is primary. If native tools are available, writing to both ensures maximum compatibility.
   - What's unclear: Whether writing to both causes visible latency on large texts (subprocess overhead).
   - Recommendation: Write to both (belt-and-suspenders). pbcopy/xclip complete in <5ms for typical yank sizes. If native write fails, silently ignore -- OSC 52 already succeeded.

## Sources

### Primary (HIGH confidence)
- crossterm 0.29 docs -- `Event::Paste`, `EnableBracketedPaste`, `DisableBracketedPaste` confirmed
- Rust stdlib `std::process::Command` -- subprocess piping pattern
- OSC 52 specification: `\x1b]52;c;{base64}\x07` format confirmed from multiple sources

### Secondary (MEDIUM confidence)
- [Helix editor OSC 52 PR](https://github.com/helix-editor/helix/pull/3220) -- implementation approach, inline base64, tmux considerations
- [tmux OSC 52 passthrough](https://sunaku.github.io/tmux-yank-osc52.html) -- DCS passthrough format `\x1bPtmux;\x1b{seq}\x1b\\`
- [tmux allow-passthrough issue](https://github.com/tmux/tmux/issues/3192) -- tmux 3.3+ passthrough defaults
- [crossterm OSC 52 PR discussion](https://github.com/crossterm-rs/crossterm/pull/697) -- crossterm intentionally does not include clipboard; keep in user space
- [arboard crate](https://crates.io/crates/arboard) -- v3.6.1, does NOT support OSC 52, requires system libs

### Tertiary (LOW confidence)
- [OSC 52 terminal support matrix](https://can-i-use-terminal.github.io/features/osc52copy.html) -- broad terminal support claimed but individual terminal versions not verified

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no external crates needed; stdlib + inline code is well-understood
- Architecture: HIGH -- provider trait pattern is battle-tested (helix, neovim use same approach)
- Pitfalls: HIGH -- tmux passthrough and bracketed paste are well-documented gotchas
- OSC 52 format: HIGH -- escape sequence format confirmed across multiple authoritative sources
- Platform-native tools: HIGH -- pbcopy/xclip/wl-copy subprocess piping is standard Rust pattern

**Research date:** 2026-03-23
**Valid until:** 2026-04-23 (stable domain, unlikely to change)
