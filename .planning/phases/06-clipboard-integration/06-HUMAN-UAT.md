---
status: partial
phase: 06-clipboard-integration
source: [06-VERIFICATION.md]
started: 2026-03-23T09:20:00Z
updated: 2026-03-23T09:20:00Z
---

## Current Test

[awaiting human testing]

## Tests

### 1. Vim yy populates system clipboard
expected: pbpaste (macOS) or xclip -o (Linux) returns the yanked line
result: [pending]

### 2. External copy → vim p paste round-trip
expected: The externally-copied text is inserted at/after the cursor
result: [pending]

### 3. SSH session: OSC 52 reaches local clipboard
expected: Local clipboard (on the SSH client) receives the yanked text
result: [pending]

### 4. Nano mode: select text, Ctrl+C, paste in another app
expected: The selected text appears in the other application
result: [pending]

### 5. Nano mode: copy in another app, Ctrl+V in mdedit
expected: The external text is inserted at cursor position
result: [pending]

### 6. Terminal bracketed paste (Cmd+V) inserts as single operation
expected: Pasted text appears at cursor in a single operation (not character-by-character)
result: [pending]

### 7. tmux session: yank text with yy
expected: System clipboard is populated (requires tmux allow-passthrough or set-clipboard on)
result: [pending]

## Summary

total: 7
passed: 0
issues: 0
pending: 7
skipped: 0
blocked: 0

## Gaps
