# Phase 3: Polish and Power Features - Research

**Researched:** 2026-03-22
**Domain:** TUI text editing features (search, selection, scroll sync, indent)
**Confidence:** HIGH

## Summary

Phase 3 adds four features to complete the editing experience: scroll sync (preview tracks editor cursor), text search (Ctrl+F), text selection (Shift+arrows), and indent/outdent (Tab/Shift+Tab). All four features integrate into the existing custom render path (`render_highlighted()`) and key routing pattern in `handle_key()`/`handle_editing_key()`.

The critical discovery is that ratatui-textarea 0.8 has built-in support for all four concerns, but the project's custom render path means we must carefully integrate rather than simply enable features. Selection works via `start_selection()` + `move_cursor()` (the private `move_cursor_with_shift` handles shift state internally). Search requires enabling the `search` feature flag in Cargo.toml to get `set_search_pattern`/`search_forward`/`search_back`, but visual highlighting must be done manually in our custom render since we bypass the Widget. Scroll sync is entirely custom code.

**Primary recommendation:** Enable ratatui-textarea's `search` feature, use its selection/search APIs for state management, and implement all visual highlighting in the existing `render_highlighted()` custom path.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Scroll sync uses ratio-based mapping: `preview_scroll = (cursor_line / total_source_lines) * total_preview_lines`. No per-line source-to-rendered mapping.
- **D-02:** Scroll sync active in split mode only. Editor-only and preview-only retain independent scroll.
- **D-03:** Sync on every cursor move (not just edits).
- **D-04:** Center the target region in the preview viewport for comfortable reading.
- **D-05:** Ctrl+F enters search mode. Search prompt in status bar: `Search: ___` with blue background.
- **D-06:** Incremental search -- matches highlight as user types. Editor scrolls to first match from cursor.
- **D-07:** Enter = next match, Shift+Enter = previous match, Esc exits (returns to original position if no match selected, or stays at current match).
- **D-08:** Matches highlighted with yellow background/dark text. Active match with bright cyan background.
- **D-09:** Case-insensitive, plain text only (no regex).
- **D-10:** Match count in status bar: `Search: term [3/17]`.
- **D-11:** Search operates on editor pane only.
- **D-12:** Shift+Arrow keys create/extend selection. Shift+Home/End selects to line start/end. Ctrl+Shift+Left/Right selects word-by-word.
- **D-13:** Selection highlighted with blue/gray background in custom render path.
- **D-14:** Typing replaces selection. Backspace/Delete removes selection.
- **D-15:** Leverage ratatui-textarea's built-in selection support (start_selection, cancel_selection, copy, selection_range).
- **D-16:** No clipboard integration in v1.
- **D-17:** Tab inserts 2 spaces at cursor.
- **D-18:** Shift+Tab removes up to 2 leading spaces (outdent).
- **D-19:** Multi-line indent/outdent with active selection.
- **D-20:** Tab intercepted before input_without_shortcuts().

### Claude's Discretion
- Exact highlight colors for search matches and selection
- Whether to use AppMode::Search or a sub-state of Editing
- Implementation details of proportional scroll mapping
- Selection + undo interaction edge cases

### Deferred Ideas (OUT OF SCOPE)
- Clipboard integration (Ctrl+C/V with OSC 52) -- v2 (PLAT-01)
- Regex search -- v2
- Search in preview pane -- v2
- Replace (Ctrl+H) -- v2
- Adjustable split ratio -- v2 (LAYT-06)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| LAYT-03 | Preview scroll position tracks editor cursor position (scroll sync) | Ratio-based mapping using `cursor_position()` and preview `scroll_offset`. Set in render loop after editor renders. |
| EDIT-07 | Search text with Ctrl+F, highlighted matches, navigate with Enter/Shift+Enter | Enable `search` feature on ratatui-textarea. Use `set_search_pattern`/`search_forward`/`search_back` for cursor movement. Custom highlight in `render_highlighted()`. New `AppMode::Search` with status bar prompt. |
| EDIT-08 | Select text with Shift+arrow keys and Ctrl+Shift+arrow keys | Use `start_selection()`/`cancel_selection()`/`selection_range()`. Intercept Shift+arrow in `handle_key()`, call start_selection then move_cursor. Render selection highlight in `render_highlighted()`. |
| EDIT-09 | Indent/outdent lines with Tab/Shift+Tab | Intercept Tab/Shift+Tab before `input_without_shortcuts()`. Set `tab_length(2)`. Manual implementation for multi-line indent with selection and for outdent. |
</phase_requirements>

## Standard Stack

No new dependencies needed. One feature flag addition:

### Changes to Cargo.toml

| Change | What | Why |
|--------|------|-----|
| Add `search` feature | `ratatui-textarea = { version = "0.8", features = ["crossterm", "search"] }` | Enables `set_search_pattern`, `search_forward`, `search_back` methods and brings in `regex` crate |

The `search` feature adds the `regex` crate as a dependency. This is acceptable -- regex is a standard Rust crate, adds minimal binary size, and the tui-textarea search APIs handle wrapping, forward/back navigation, and match positioning that would be tedious to reimplement.

**Note on D-09 (case-insensitive, plain text):** Although tui-textarea search uses regex internally, we can pass `(?i)` prefix + `regex::escape()` to achieve case-insensitive plain text matching. The `regex::escape()` function converts literal text to a regex-safe pattern.

## Architecture Patterns

### New/Modified Files

```
src/
  app.rs       # Add AppMode::Search, search key routing, scroll sync logic
  editor.rs    # Add selection rendering, search highlight rendering,
               #   indent/outdent methods, search state, tab_length(2)
  status_bar.rs # Add search mode rendering with prompt + match count
```

No new files needed. All features integrate into existing modules.

### Pattern 1: AppMode::Search for Search UI

**What:** Add `AppMode::Search` variant to the existing `AppMode` enum. Search mode captures keystrokes for the search query and routes Enter/Shift+Enter/Esc appropriately.

**When to use:** When Ctrl+F is pressed in Editing mode.

**Recommendation:** Use a dedicated `AppMode::Search` (not a sub-state) because:
1. It follows the established pattern (ConfirmQuit, PromptFilename are both AppMode variants)
2. The status bar already renders differently per AppMode
3. Key routing is cleanly separated in `match self.mode`

**State needed in App:**
```rust
// Search state fields in App
search_query: String,          // Current search text
search_cursor_before: (usize, usize),  // Cursor position when search started (for Esc restore)
search_match_index: usize,     // Current match index (0-based) for [3/17] display
search_match_count: usize,     // Total match count
```

### Pattern 2: Selection via start_selection + move_cursor

**What:** ratatui-textarea 0.8 has public `start_selection()`, `cancel_selection()`, `selection_range()`, and `is_selecting()` methods. The `move_cursor()` method preserves existing selection (it calls private `move_cursor_with_shift(m, self.selection_start.is_some())`). So the pattern is:

```rust
// In Editor::handle_key(), for Shift+arrow:
(modifiers, KeyCode::Right) if modifiers.contains(KeyModifiers::SHIFT) => {
    if !self.textarea.is_selecting() {
        self.textarea.start_selection();
    }
    self.textarea.move_cursor(CursorMove::Forward);
    None
}
```

**Critical insight:** Once `start_selection()` is called, `move_cursor()` will NOT cancel the selection because `move_cursor_with_shift` checks `self.selection_start.is_some()` and passes `shift: true`. Selection only ends when:
- `cancel_selection()` is called explicitly
- A key that doesn't go through `move_cursor()` is pressed (e.g., typing a character calls `insert_char` which calls `delete_selection` first)

**Reading selection range for rendering:**
```rust
// Returns Option<((start_row, start_col), (end_row, end_col))>
// Positions are 0-based, character-wise (not byte-wise)
let sel = self.textarea.selection_range();
```

**Important:** `selection_range()` returns character positions, but the render highlighting in `highlight.rs` uses byte offsets for `selection()`. We need to convert character positions to byte offsets when rendering.

### Pattern 3: Custom Render Highlighting Layers

**What:** `render_highlighted()` already builds `Line` objects from syntect spans. Search and selection highlights must be layered on top.

**Approach:** After syntect highlighting produces spans, apply search/selection highlights as post-processing. For each visible line:
1. Get syntect-highlighted spans (existing)
2. Identify search match byte ranges on this line
3. Identify selection byte range on this line
4. Split spans at highlight boundaries and apply background colors

**Highlight priority (highest wins):**
1. Active search match (bright cyan bg)
2. Other search matches (yellow bg)
3. Selection (blue/gray bg)
4. Syntect syntax colors (existing)

This matches the boundary priority in tui-textarea's own `highlight.rs`: Cursor > Search > Select > End.

### Pattern 4: Scroll Sync (Proportional)

**What:** After each cursor move in split mode, set preview scroll based on editor cursor ratio.

```rust
// In App::render() or App::maybe_update_preview(), after editor renders:
if self.layout_mode == LayoutMode::Split {
    let (cursor_row, _) = self.editor.cursor_position();
    let total_source = self.editor.line_count();
    let total_preview = self.preview_text.lines.len() as u16;
    let viewport_height = preview_area.height;

    if total_source > 0 {
        let ratio = cursor_row as f64 / total_source as f64;
        let target = (ratio * total_preview as f64) as u16;
        // Center in viewport
        let scroll = target.saturating_sub(viewport_height / 2);
        self.preview.set_scroll(scroll);
    }
}
```

**Key detail:** The scroll must be set BEFORE `preview.render()` is called so the clamping in `render()` applies correctly.

### Pattern 5: Tab/Shift+Tab Interception

**What:** Tab must be intercepted in `Editor::handle_key()` BEFORE the `_ =>` fallthrough to `input_without_shortcuts()`.

```rust
// Tab — insert 2 spaces (D-17)
(KeyModifiers::NONE, KeyCode::Tab) => {
    // If selection spans multiple lines, indent all selected lines
    if let Some(((start_row, _), (end_row, _))) = self.textarea.selection_range() {
        if start_row != end_row {
            self.indent_lines(start_row, end_row);
            self.modified = true;
            return Some(EditorAction::ContentChanged);
        }
    }
    // Single line or no selection: insert 2 spaces
    self.textarea.insert_str("  ");
    self.modified = true;
    Some(EditorAction::ContentChanged)
}

// Shift+Tab — outdent (D-18)
(KeyModifiers::SHIFT, KeyCode::BackTab) => {
    // Remove up to 2 leading spaces from current line (or all selected lines)
    // ...
    self.modified = true;
    Some(EditorAction::ContentChanged)
}
```

**Important:** crossterm reports Shift+Tab as `KeyCode::BackTab`, not `KeyCode::Tab` with SHIFT modifier.

### Anti-Patterns to Avoid
- **Passing Shift+arrow to input_without_shortcuts():** This function ignores shift state entirely -- Shift+Left would be treated as regular Left. Selection must be handled before the fallthrough.
- **Using tui-textarea's Widget for rendering:** The project already bypasses Widget for syntect highlighting. Don't try to use the Widget's built-in search/selection rendering -- it would conflict with the custom render path.
- **Byte vs character indexing confusion:** `selection_range()` returns character positions. Line string indexing in Rust uses bytes. Always convert properly.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Search forward/back with wrapping | Custom line-by-line search | `textarea.search_forward()`/`search_back()` | Handles edge cases: wrapping, multi-byte chars, cursor positioning |
| Regex escaping for plain text search | Manual character escaping | `regex::escape()` | Covers all special regex characters correctly |
| Selection state management | Custom selection start/end tracking | `textarea.start_selection()`/`selection_range()` | Already handles multi-line, word boundaries, cursor ordering |
| Match counting | Reimplementing regex iteration | `regex::Regex::find_iter()` on each line | Standard, handles Unicode correctly |

**Key insight:** ratatui-textarea already implements the hard parts (selection state, search navigation with wrapping). Our job is to wire up the keybindings and render the visuals in our custom path.

## Common Pitfalls

### Pitfall 1: Selection Range Character vs Byte Offset
**What goes wrong:** `selection_range()` returns `(usize, usize)` character-wise positions but Rust string slicing uses byte offsets. Using character positions as byte indices on multi-byte strings causes panics.
**Why it happens:** Easy to confuse the two coordinate systems.
**How to avoid:** Always convert character position to byte offset using `.char_indices().nth(col).map(|(i, _)| i)` before string slicing.
**Warning signs:** Panic on `&line[col..]` when editing text with Unicode characters.

### Pitfall 2: Tab Key Intercepted by input_without_shortcuts
**What goes wrong:** `input_without_shortcuts()` explicitly handles `Key::Tab` and calls `insert_tab()` which uses the widget's tab_length (default 4 spaces). If Tab reaches the fallthrough, it inserts 4 spaces instead of 2.
**Why it happens:** Tab is one of the few keys that `input_without_shortcuts` handles directly.
**How to avoid:** Intercept Tab in the explicit `match` arms BEFORE the `_ =>` fallthrough. Set `textarea.set_tab_length(2)` as a safety net.
**Warning signs:** 4 spaces appearing instead of 2 when pressing Tab.

### Pitfall 3: Shift+Tab is KeyCode::BackTab
**What goes wrong:** Matching `(KeyModifiers::SHIFT, KeyCode::Tab)` never fires for Shift+Tab.
**Why it happens:** crossterm translates Shift+Tab as `KeyCode::BackTab` on most terminals.
**How to avoid:** Match `KeyCode::BackTab` explicitly. The modifiers may or may not include SHIFT depending on the terminal.
**Warning signs:** Shift+Tab does nothing.

### Pitfall 4: Search State Leaking After Exit
**What goes wrong:** After Esc from search mode, tui-textarea still has a search pattern set, which affects any future calls to `search_forward`/`search_back`.
**Why it happens:** `set_search_pattern("")` clears the pattern, but this must be called explicitly on search exit.
**How to avoid:** Clear the search pattern when exiting search mode: `textarea.set_search_pattern("").unwrap()`.
**Warning signs:** Ghost search behavior or old highlights persisting.

### Pitfall 5: Scroll Sync Jumpy at Document Extremes
**What goes wrong:** At the very beginning or end of a document, proportional mapping produces 0 or max scroll, causing jarring jumps.
**Why it happens:** Linear ratio mapping doesn't account for viewport centering at extremes.
**How to avoid:** Clamp the scroll target: `let scroll = target.saturating_sub(viewport_height / 2).min(max_scroll)` where `max_scroll = total_preview_lines.saturating_sub(viewport_height)`.
**Warning signs:** Preview jumps to blank space at document start/end.

### Pitfall 6: delete_selection() Called Implicitly by insert_char
**What goes wrong:** When text is typed with an active selection, `insert_char` calls `delete_selection(false)` internally, deleting the selected text. This is CORRECT behavior (D-14). But if Tab is handled manually and doesn't call `delete_selection`, the selection persists after Tab indent.
**Why it happens:** Manual Tab handling bypasses the normal insert path.
**How to avoid:** When inserting 2 spaces for Tab, use `textarea.insert_str("  ")` which calls `delete_selection(false)` internally. Or call `cancel_selection()` explicitly for multi-line indent.

### Pitfall 7: Search Highlight in Custom Render Needs Byte Offsets
**What goes wrong:** `regex::Regex::find_iter()` returns byte offsets (Match.start/end are byte positions). But our syntect spans are also byte-based (they come from processing the string directly). This is actually convenient -- the search match offsets can be used directly for span splitting.
**Why it happens:** Inconsistency awareness is needed -- selection uses character positions while search uses byte positions.
**How to avoid:** Document clearly: search = byte offsets from regex, selection = character offsets from textarea API. Convert selection to bytes before rendering.

## Code Examples

### Enabling Search Feature
```toml
# Cargo.toml change
ratatui-textarea = { version = "0.8", features = ["crossterm", "search"] }
```

### Setting Up Case-Insensitive Plain Text Search
```rust
// When user types in search prompt, update the pattern:
fn update_search_pattern(&mut self, query: &str) {
    if query.is_empty() {
        let _ = self.editor.textarea_mut().set_search_pattern("");
    } else {
        // regex::escape converts literal text to regex-safe pattern
        // (?i) makes it case-insensitive (D-09)
        let pattern = format!("(?i){}", regex::escape(query));
        let _ = self.editor.textarea_mut().set_search_pattern(&pattern);
    }
}
```

### Reading Selection Range for Custom Render
```rust
// In render_highlighted(), get selection in byte offsets:
fn selection_byte_range(&self, line_idx: usize) -> Option<(usize, usize)> {
    let ((sr, sc), (er, ec)) = self.textarea.selection_range()?;
    let line = &self.textarea.lines()[line_idx];

    if line_idx < sr || line_idx > er {
        return None; // Line not in selection
    }

    let start_byte = if line_idx == sr {
        line.char_indices().nth(sc).map(|(i, _)| i).unwrap_or(line.len())
    } else {
        0
    };

    let end_byte = if line_idx == er {
        line.char_indices().nth(ec).map(|(i, _)| i).unwrap_or(line.len())
    } else {
        line.len()
    };

    Some((start_byte, end_byte))
}
```

### Shift+Arrow Selection Handling
```rust
// In Editor::handle_key():

// Shift+Right — extend selection forward
(modifiers, KeyCode::Right) if modifiers == KeyModifiers::SHIFT => {
    if !self.textarea.is_selecting() {
        self.textarea.start_selection();
    }
    self.textarea.move_cursor(CursorMove::Forward);
    None
}

// Ctrl+Shift+Right — extend selection by word
(modifiers, KeyCode::Right)
    if modifiers == KeyModifiers::SHIFT | KeyModifiers::CONTROL =>
{
    if !self.textarea.is_selecting() {
        self.textarea.start_selection();
    }
    self.textarea.move_cursor(CursorMove::WordForward);
    None
}

// Any non-shift movement cancels selection
(KeyModifiers::NONE, KeyCode::Left) => {
    self.textarea.cancel_selection();
    self.textarea.move_cursor(CursorMove::Back);
    None
}
```

### Scroll Sync Implementation
```rust
// In App::render(), after editor area is rendered, before preview renders:
fn sync_preview_scroll(&mut self, preview_area_height: u16) {
    if self.layout_mode != LayoutMode::Split {
        return;
    }
    let (cursor_row, _) = self.editor.cursor_position();
    let total_source = self.editor.line_count();
    let total_preview = self.preview_text.lines.len() as u16;

    if total_source <= 1 {
        self.preview.set_scroll(0);
        return;
    }

    let ratio = cursor_row as f64 / (total_source - 1).max(1) as f64;
    let target_line = (ratio * total_preview as f64) as u16;
    // Center target in viewport (D-04)
    let centered = target_line.saturating_sub(preview_area_height / 2);
    let max_scroll = total_preview.saturating_sub(preview_area_height);
    self.preview.set_scroll(centered.min(max_scroll));
}
```

### Multi-Line Indent
```rust
fn indent_lines(&mut self, start_row: usize, end_row: usize) {
    // Must manipulate lines directly -- no built-in multi-line indent in tui-textarea
    for row in start_row..=end_row {
        // Move cursor to start of each line and insert 2 spaces
        // This is tricky because tui-textarea doesn't have a "insert at position" API
        // Alternative: rebuild lines manually
    }
}
```

**Note on multi-line indent:** tui-textarea does NOT have a multi-line indent API. Implementation options:
1. Move cursor to each line's start and insert spaces (uses undo-able operations but complex cursor management)
2. Get lines, modify them, and set them back via `TextArea::new()` (loses undo history)
3. For each selected line, position cursor at line start and call `insert_str("  ")`

Recommended approach: Option 3 with cursor save/restore. For each line in the selection range, temporarily move cursor to (row, 0), insert "  ", then restore cursor. This preserves undo history for each insertion.

### Outdent Implementation
```rust
fn outdent_line(&mut self, row: usize) {
    let line = &self.textarea.lines()[row];
    let spaces_to_remove = line.chars().take(2).take_while(|c| *c == ' ').count();
    if spaces_to_remove > 0 {
        // Move cursor to (row, 0), delete `spaces_to_remove` characters
        // tui-textarea has delete_next_char() which deletes the character at cursor
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| tui-textarea 0.7.x | ratatui-textarea 0.8.0 (ratatui org fork) | Recent | Renamed package, `search` feature with regex, selection API same |
| Custom selection tracking | Use built-in `start_selection`/`selection_range` | tui-textarea 0.4+ | No need for custom selection state |
| Per-line source-to-rendered mapping | Proportional ratio mapping | Project decision D-01 | Simpler implementation, approximate but sufficient |

## Open Questions

1. **Multi-line indent undo granularity**
   - What we know: Each `insert_str` call creates a separate undo entry. Indenting 10 lines = 10 undo steps.
   - What's unclear: Whether users expect Ctrl+Z to undo all 10 indents at once.
   - Recommendation: Accept per-line undo granularity for v1. Grouping undo operations would require tui-textarea internals modification.

2. **Selection + Undo interaction**
   - What we know: `textarea.undo()` does not restore selection state. After undo, selection is gone.
   - What's unclear: Whether users expect undo to restore the previous selection.
   - Recommendation: Cancel selection on undo/redo. This matches most simple editors.

3. **Search match highlighting performance**
   - What we know: For each render frame, regex matching all visible lines is needed to show highlights.
   - What's unclear: Performance impact on very large files with many matches.
   - Recommendation: Only match visible lines (already limited by `scroll_top` to `scroll_top + height`). This bounds the work regardless of file size.

## Sources

### Primary (HIGH confidence)
- ratatui-textarea 0.8.0 source code at `~/.cargo/registry/src/*/ratatui-textarea-0.8.0/` -- directly read `textarea.rs`, `search.rs`, `highlight.rs`
- Project source: `src/editor.rs`, `src/app.rs`, `src/preview.rs`, `src/highlighter.rs`, `src/status_bar.rs`

### Secondary (MEDIUM confidence)
- crossterm KeyCode documentation for BackTab behavior (verified in crossterm source)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- reading actual source code of dependencies
- Architecture: HIGH -- building on established codebase patterns, verified API availability
- Pitfalls: HIGH -- identified from actual source code analysis (byte vs char offsets, BackTab, input_without_shortcuts Tab handling)

**Research date:** 2026-03-22
**Valid until:** 2026-04-22 (stable -- dependencies pinned, no expected breaking changes)
