# Milestones

## v1.0 MVP (Shipped: 2026-03-22)

**Phases completed:** 3 phases, 6 plans, 10 tasks

**Key accomplishments:**

- Rust TUI editor with ratatui-textarea 0.8, nano-style keybindings (Ctrl+S/Q/Z/Y), line numbers, modified tracking, and modal quit/save-as prompts
- Atomic file save with temp+rename, status bar with timed messages and cursor tracking, complete save-before-quit flow with filename prompting for untitled buffers.
- Side-by-side markdown preview with tui-markdown rendering, Ctrl+P layout toggle, 80ms debounced updates, and scrollable preview pane
- Markdown-aware syntax highlighting in editor pane using syntect with base16-ocean.dark, rendered via custom Paragraph path bypassing tui-textarea's widget
- Text selection with Shift+arrows, Tab/Shift+Tab indent/outdent, and proportional editor-to-preview scroll sync
- Ctrl+F incremental search with highlighted matches, forward/backward navigation, and [current/total] match count in status bar

---
