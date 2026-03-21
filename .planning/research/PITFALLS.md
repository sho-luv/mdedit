# Pitfalls Research

**Domain:** Terminal-based markdown editor with live preview (Rust/ratatui)
**Researched:** 2026-03-21
**Confidence:** HIGH (most pitfalls verified across multiple sources and known Rust TUI ecosystem patterns)

## Critical Pitfalls

### Pitfall 1: Using String/Vec<String> as the Text Buffer Data Structure

**What goes wrong:**
Developers start with a simple `Vec<String>` (one string per line) as the backing store. This works fine for small files but degrades badly for insertions/deletions in large documents because every edit in the middle requires shifting all subsequent bytes or lines. Since tui-textarea uses this model internally, the risk is accepting its internal model as "good enough" without understanding the ceiling.

**Why it happens:**
tui-textarea abstracts away the buffer, so developers never think about the underlying data structure. For files under ~10KB (most markdown), performance is fine. The problem surfaces only when someone opens a large README, a long blog post, or pastes a big chunk of text.

**How to avoid:**
For v1, tui-textarea's internal `Vec<String>` is acceptable -- markdown files are typically small. But design the editor layer with a clean abstraction boundary so the buffer can be swapped to ropey (a Rust rope library with O(log n) edits) in v2 if needed. Do NOT build any logic that assumes line-indexed `Vec<String>` access patterns.

**Warning signs:**
- Noticeable lag when editing files >50KB
- Paste operations taking >16ms (one frame at 60fps)
- Profiler showing time spent in string reallocation

**Phase to address:**
Phase 1 (foundation) -- design the abstraction boundary. Phase 2+ -- swap to rope if benchmarks warrant it.

---

### Pitfall 2: Byte Offset vs Character Index vs Grapheme Cluster Confusion

**What goes wrong:**
Rust strings are UTF-8 byte sequences. A cursor at "position 5" could mean byte 5, char 5, or grapheme cluster 5 -- and these diverge for any non-ASCII text. CJK characters are multi-byte. Emoji with ZWJ sequences (family emoji, skin tone modifiers) are multiple Unicode codepoints but one visual "character." Indexing by bytes panics on non-ASCII. Indexing by chars breaks cursor movement on combined emoji. Indexing by grapheme clusters is the only correct approach for cursor movement.

**Why it happens:**
ASCII-only testing. Rust's `str::chars()` counts codepoints, not graphemes, which passes basic tests but fails on real-world text. tui-textarea's documentation does not explicitly address grapheme cluster handling, making it unclear whether cursor movement is grapheme-aware.

**How to avoid:**
Use the `unicode-segmentation` crate for all cursor movement logic. Use `unicode-width` for display width calculations (CJK characters are 2 columns wide). Never index strings by byte position for user-facing operations. Test with CJK text, emoji, and combining characters from day one -- not as an afterthought.

**Warning signs:**
- Cursor "skipping" characters or landing in the middle of a character
- Misaligned display when CJK or emoji are present
- Panics on non-ASCII input (Rust will panic on invalid byte index into &str)

**Phase to address:**
Phase 1 (text editing foundation) -- must be correct from the start, retrofitting is painful.

---

### Pitfall 3: Terminal State Not Restored on Panic/Crash

**What goes wrong:**
The application enters raw mode and switches to the alternate screen. If it panics or crashes without cleanup, the user's terminal is left in raw mode: no echo, no line buffering, control characters broken. The user must run `reset` or close the terminal. This is the single most user-hostile failure mode for any TUI application.

**Why it happens:**
Developers handle clean shutdown but forget that Rust panics unwind the stack and skip normal cleanup unless a panic hook is installed. Drop implementations on the Terminal struct may not run if the panic occurs in certain contexts.

**How to avoid:**
Install a custom panic hook at startup that disables raw mode and leaves the alternate screen BEFORE printing the panic message. Use `std::panic::set_hook()` to wrap the default handler. The pattern is:

```rust
let original_hook = std::panic::take_hook();
std::panic::set_hook(Box::new(move |panic_info| {
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);
    original_hook(panic_info);
}));
```

Also handle SIGINT/SIGTERM gracefully with the same cleanup.

**Warning signs:**
- Terminal stays in raw mode after a crash during development
- Panic backtraces appear garbled (no carriage returns in raw mode)

**Phase to address:**
Phase 1 (very first thing) -- this must be the first code written, before any feature work.

---

### Pitfall 4: tui-markdown is Experimental -- Treating It as Production-Ready

**What goes wrong:**
tui-markdown explicitly describes itself as an "experimental Proof of Concept." Developers build their entire rendering pipeline on it, then discover it cannot handle certain markdown constructs, has layout bugs with complex nested structures, or produces incorrect widget sizing. Swapping it out late requires rewriting the entire preview pane.

**Why it happens:**
It is the most obvious ratatui-compatible markdown rendering crate. It works for simple cases. The PoC label is easy to overlook when you are focused on getting something on screen.

**How to avoid:**
Use tui-markdown as the starting point but wrap it behind an abstraction (`trait MarkdownRenderer`). Expect to either fork it or replace it. The actual markdown parsing should use pulldown-cmark directly (which is mature and battle-tested), and the conversion to ratatui `Text`/`Spans` widgets is the part that may need custom work. Keep the "parse markdown" and "render to TUI widgets" steps separate.

**Warning signs:**
- Markdown constructs that render correctly in other tools but break in the preview
- Nested blockquotes or complex tables rendering incorrectly
- No clear path to fix rendering bugs without forking the crate

**Phase to address:**
Phase 1 (architecture) -- define the abstraction. Phase 2 (preview) -- implement and validate against real markdown files.

---

### Pitfall 5: Re-parsing Entire Document on Every Keystroke

**What goes wrong:**
The naive approach: on every keystroke, take the full document text, parse it through pulldown-cmark, convert to ratatui widgets, and render the preview. For large documents, this creates visible lag. pulldown-cmark processes ~500K chars/second, but combining parsing + widget construction + layout + rendering can blow the 16ms frame budget on documents >20KB.

**Why it happens:**
It is the simplest implementation and works fine for small files. pulldown-cmark's streaming iterator API gives the illusion of efficiency, but you still process the entire document each time.

**How to avoid:**
Implement debounced re-parsing: do not re-parse on every keystroke. Instead, mark the preview as dirty and re-parse after a short idle period (50-100ms of no input). This is the approach used by VS Code's markdown preview and similar tools. For v1 this is sufficient. True incremental parsing (only re-parsing changed sections) is a v2 optimization if needed.

**Warning signs:**
- Preview lag when typing in documents >5KB
- CPU usage spiking during fast typing
- Dropped frames visible as preview "stuttering"

**Phase to address:**
Phase 2 (live preview) -- implement debounced rendering from the start, not as a fix later.

---

### Pitfall 6: Scroll Sync Between Editor and Preview is Deceptively Hard

**What goes wrong:**
Developers assume scroll sync means "editor at 50% scroll = preview at 50% scroll." This is wrong. Markdown source and rendered output have different heights -- a single-line heading renders as one line plus spacing, a table's source is many lines but renders compactly, code blocks may wrap differently. Percentage-based sync produces disorienting jumps.

**Why it happens:**
It looks simple until you try it. Every markdown editor that has attempted this (VS Code, Joplin, Markdown Monster) has iterated through multiple approaches. Even VS Code's implementation is not perfect.

**How to avoid:**
Use source-map-based sync: pulldown-cmark's `into_offset_iter()` provides source byte ranges for each parsed element. Map editor cursor line to the corresponding markdown element, then scroll the preview to show that element. This is element-level sync, not pixel-level. Accept that perfect sync is impossible and aim for "the preview shows what you are editing." Start with a simple approach (first visible line mapping) and refine.

**Warning signs:**
- Preview jumping erratically when scrolling the editor
- Preview and editor showing completely different sections of the document
- Users reporting motion sickness or disorientation

**Phase to address:**
Phase 3 (polish) -- get basic editing and preview working first, then add sync. Do not try to ship perfect sync in v1.

---

### Pitfall 7: Wide Character Display Width Miscalculation

**What goes wrong:**
CJK characters, emoji, and certain Unicode symbols occupy 2 terminal columns but are 1 character (or even 1 grapheme cluster). If the editor calculates line width by counting characters instead of display columns, text alignment breaks: the cursor appears in the wrong position, line wrapping happens at the wrong point, and the side-by-side layout calculation is wrong.

**Why it happens:**
ASCII characters are 1 byte = 1 char = 1 column. This assumption is deeply embedded in naive implementations. Even experienced developers forget that terminal column width is a separate concept from string length.

**How to avoid:**
Use `unicode-width` crate for ALL display width calculations. Never use `str::len()` (bytes) or `str::chars().count()` (codepoints) for layout purposes. ratatui itself uses unicode-width internally, but any custom layout code must also use it. Test with mixed ASCII/CJK text from the start.

**Warning signs:**
- Cursor misaligned after CJK characters
- Line numbers not lining up with their lines
- Side-by-side split appearing uneven with certain content

**Phase to address:**
Phase 1 (text editing) -- use unicode-width from the start in all layout calculations.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Using tui-textarea as-is without abstraction | Fast to get editing working | Locked into its API; hard to swap buffer or add features | Never -- always wrap it, even thinly |
| Full document re-parse on keystroke | Simple implementation | Lag on large files, wasted CPU | MVP only, must add debouncing before release |
| Hardcoded terminal width assumptions | Avoid layout calculation complexity | Breaks on resize, small terminals, tmux splits | Never -- ratatui provides terminal size, use it |
| Skipping alternate screen / raw mode cleanup | Fewer lines of code | Corrupted terminal on any crash | Never -- implement panic hook first |
| String-based undo (storing full document snapshots) | Simple undo implementation | Memory explosion on large files with many edits | MVP only if tui-textarea's built-in undo is insufficient |

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| tui-textarea + ratatui | Version mismatch between tui-textarea's ratatui dependency and your direct ratatui dependency | Pin exact same ratatui version; check tui-textarea's Cargo.toml for its ratatui version |
| pulldown-cmark + tui-markdown | Parsing markdown twice (once in tui-markdown, once for source mapping) | Use pulldown-cmark directly and convert events to ratatui widgets yourself, using tui-markdown as reference |
| crossterm event handling | Blocking on event read, causing preview not to update | Use crossterm's `poll()` with timeout, then `read()` only when events are available |
| File I/O + raw mode | Writing to stdout while in raw mode garbles output | All file I/O goes through the file system, never stdout; status messages go through ratatui widgets |

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Re-rendering full preview on every keystroke | Preview lag, CPU spike during fast typing | Debounce preview updates (50-100ms idle) | Documents >5KB with fast typists |
| Allocating new String for every frame | GC pressure (Rust: allocator pressure), frame drops | Reuse buffers between frames where possible | High frame rate + large documents |
| Parsing markdown synchronously on main thread | UI freezes during parse of large document | For v1, debouncing is sufficient; v2 could use async parsing | Documents >50KB |
| Not using ratatui's built-in diffing | Sending entire screen to terminal every frame | Let ratatui handle the diff -- do not call terminal clear/reset manually | Always; ratatui's diff is efficient for TUI-sized screens |
| Excessive widget nesting in preview | Layout calculation becomes expensive | Flatten widget tree where possible; avoid deeply nested Block wrappers | Complex markdown with deep nesting |

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Preview flashing/flickering on every keystroke | Visually distracting, feels broken | Debounce updates; ratatui's diffing helps but parsing delay still causes flash |
| No visual feedback for unsaved changes | Users lose work, no idea file is modified | Status bar must show modified indicator (asterisk or [+]) from day one |
| Ctrl+C kills app without save prompt | Data loss | Catch SIGINT, prompt to save if modified, then exit |
| Editor cursor invisible or hard to find | Users lose track of where they are editing | Use cursor line highlighting (tui-textarea supports this) and visible cursor style |
| Preview stealing focus from editor | Keystrokes go to wrong pane | Editor pane must always own keyboard focus; preview is display-only |
| No indication of current mode/file | Users forget what file they are editing | Status bar with filename, line:col, modified indicator |
| Side-by-side not working in narrow terminals | Layout breaks, text unreadable | Detect terminal width; auto-switch to stacked/single-pane below ~80 columns |

## "Looks Done But Isn't" Checklist

- [ ] **Text editing:** Works with ASCII but test with CJK, emoji, combining diacritics, RTL text
- [ ] **Undo/redo:** Works for single character edits but test with paste, delete-line, and rapid sequences
- [ ] **File save:** Writes file but verify it preserves original line endings (LF vs CRLF) and trailing newline
- [ ] **Markdown preview:** Renders headings and bold but test with nested blockquotes, tables with alignment, fenced code blocks with language tags, and raw HTML blocks
- [ ] **Terminal resize:** App does not crash or corrupt display when terminal is resized mid-edit
- [ ] **SSH compatibility:** Works locally but test over SSH with latency -- no Kitty/iTerm-specific escape codes in v1
- [ ] **Large file:** Opens 1KB test file fine but test with 100KB+ markdown files
- [ ] **Empty file:** Handles opening a new/empty file without crashing
- [ ] **Binary file:** Does not corrupt terminal if user accidentally opens a binary file

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Wrong text buffer data structure | MEDIUM | Abstract behind trait, swap implementation. If tui-textarea is wrapped, swap the widget. If logic leaked, refactor all buffer access. |
| Unicode handling broken | HIGH | Must audit every place strings are indexed/measured. Retrofitting grapheme-awareness touches cursor movement, display, selection, undo -- essentially everything. |
| Terminal not restored on crash | LOW | Add panic hook + signal handlers. Isolated fix, no architecture change. |
| tui-markdown insufficient | MEDIUM | If abstracted behind trait, swap renderer. If tightly coupled, rewrite preview pane. Using pulldown-cmark directly is the fallback. |
| Performance death by re-parsing | LOW | Add debounce timer. Isolated change to the event loop, does not affect architecture. |
| Scroll sync broken | LOW | Scroll sync is an additive feature. Can ship without it and add later. No architectural impact. |
| Display width wrong | MEDIUM | Find-and-replace all width calculations with unicode-width calls. Tedious but mechanical. |

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Terminal crash cleanup | Phase 1 (first code written) | Deliberately panic in the app; terminal should restore cleanly |
| Unicode/grapheme handling | Phase 1 (text editing) | Test suite with CJK, emoji, combining chars from the start |
| Display width calculation | Phase 1 (text editing) | Render a line with mixed ASCII/CJK; cursor must align correctly |
| tui-textarea abstraction | Phase 1 (architecture) | Editor logic does not import tui-textarea types directly |
| tui-markdown abstraction | Phase 1 (architecture) | Preview logic does not import tui-markdown types directly |
| Buffer data structure | Phase 1 (architecture) | Clean trait boundary exists even if Vec<String> is used initially |
| Debounced preview parsing | Phase 2 (live preview) | Type rapidly in a 20KB file; preview should not stutter |
| Re-parse optimization | Phase 2 (live preview) | Profile CPU during typing; parse should not dominate |
| Scroll sync | Phase 3 (polish) | Editor cursor at heading X; preview shows heading X area |
| Narrow terminal fallback | Phase 3 (polish) | Resize terminal to 60 columns; layout should adapt |

## Sources

- [tui-textarea GitHub](https://github.com/rhysd/tui-textarea) -- widget capabilities and limitations
- [tui-markdown GitHub](https://github.com/joshka/tui-markdown) -- experimental/PoC status confirmed
- [pulldown-cmark GitHub](https://github.com/pulldown-cmark/pulldown-cmark) -- parsing performance and into_offset_iter() for source mapping
- [Text showdown: Gap Buffers vs Ropes (coredumped.dev)](https://coredumped.dev/2023/08/09/text-showdown-gap-buffers-vs-ropes/) -- data structure performance comparison
- [Ratatui rendering docs](https://ratatui.rs/concepts/rendering/under-the-hood/) -- diffing algorithm and frame rendering model
- [ratatui discussion #579](https://github.com/ratatui/ratatui/discussions/579) -- rendering best practices
- [unicode-width crate](https://docs.rs/unicode-width/latest/unicode_width/) -- display width calculations
- [Pretty Rust backtraces in raw terminal mode](https://werat.dev/blog/pretty-rust-backtraces-in-raw-terminal-mode/) -- panic handling in raw mode
- [xi-editor retrospective (Raph Levien)](https://raphlinus.github.io/xi/2020/06/27/xi-retrospective.html) -- lessons from building a Rust text editor
- [Ropey crate](https://github.com/cessen/ropey) -- Rust rope library for future buffer upgrade

---
*Pitfalls research for: terminal markdown editor with live preview (Rust/ratatui)*
*Researched: 2026-03-21*
