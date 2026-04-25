---
slug: preview-line-sync
status: resolved
trigger: |
  Editor cursor and preview line indicator are out of sync. When cursor is on
  "## Vision" heading in editor (source line 15), the preview highlights a
  different line ("or a children's book") instead of the Vision heading line in
  the rendered output. Source-to-preview line mapping is incorrect.
created: 2026-04-24
updated: 2026-04-24
---

# Debug: preview-line-sync

## Symptoms

- **Expected**: When cursor is on `## Vision` heading in editor (source line 15), the preview's highlighted line indicator should be on the corresponding `Vision` heading in the rendered output.
- **Actual**: Preview highlight box is on "or a children's book" — a line that belongs to a different source section (the bullet under Problem about "Expensive illustrators").
- **Reproduction**: Open a markdown file with multiple sections; place editor cursor on a heading line; observe that preview highlight is positioned on a different line than the rendered analog of the source line.

## Resolution

**Root cause**: `src/preview.rs::wrapped_visual_line_count` was a hand-rolled
re-implementation of ratatui's `WordWrapper::process_input` algorithm (the wrap
counter that drives Paragraph rendering). The local copy carried at least one
divergence from the upstream logic — after a wrap was emitted, when the next
grapheme was whitespace and `pending_whitespace` was empty, the local copy
reset `non_whitespace_previous = false` while `WordWrapper` left the flag
untouched. That altered the next iteration's `word_found` branch and changed
the wrapped-line count by one row in specific multi-word boundary cases.

The drift cascaded: every later `logical_line_starts[i]` was off, so
`PreviewMetrics::visual_line_for_logical(...)` returned a row index that
landed inside the wrapped continuation of an earlier paragraph instead of the
heading the cursor was actually on.

**Fix**: replace the local wrap counter with ratatui's own
`Paragraph::line_count(width)` (gated behind the
`unstable-rendered-line-info` feature). That call is implemented on top of the
exact same `WordWrapper` Paragraph uses to render, so the metric and the
rendered output are guaranteed to agree by construction.

**Files changed**:
- `Cargo.toml` — enable `unstable-rendered-line-info` on the ratatui dep.
- `src/preview.rs` — delete `wrapped_visual_line_count` (and unused imports);
  add `visual_lines_for` that wraps each `Line` in a single-line `Paragraph`
  and asks ratatui for its row count. Add regression test
  `preview_metrics_match_paragraph_for_every_logical_line` that, for a
  representative document and seven widths (30/40/50/60/80/100/120), verifies
  every logical line's metric-computed row equals the row a marker-suffixed
  Paragraph actually places it on.

**Verification**: `cargo test` — 16 tests pass, including the new exhaustive
parity test and the existing `preview_metrics_match_actual_wrapped_heading_row`
end-to-end check.
