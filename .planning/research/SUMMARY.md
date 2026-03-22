# Research Summary: mdedit v2.0 Power User

**Domain:** Terminal-based markdown editor -- feature expansion milestone
**Researched:** 2026-03-22
**Overall confidence:** MEDIUM (vim integration is HIGH confidence, WYSIWYG is LOW)

## Executive Summary

The v2.0 milestone adds five features to the existing 1,508 LOC Rust codebase: vim-style keybindings (default), configurable themes, clipboard integration, browser companion rendering, and WYSIWYG terminal editing. Research focused on how these integrate with the existing app.rs event loop and editor.rs custom render architecture.

The most important architectural finding is that vim keybindings should be built as a **state machine layer between key events and editor operations**, not by replacing the editor widget. The existing `input_without_shortcuts()` pattern and custom `render_highlighted()` pipeline are strengths to preserve -- they already separate input handling from rendering, which is exactly the seam vim needs. edtui (a vim-ready editor widget) was evaluated and rejected because it would require rewriting the entire custom render pipeline (~100 LOC of overlay logic for syntax + selection + search).

The second key finding is build order. Config/theme must come first because it's cross-cutting (every component references hardcoded colors). Vim must come second because clipboard depends on vim's yank model, and WYSIWYG depends on vim's cursor model. Browser companion is independent and can slot in anywhere after config. WYSIWYG should be last -- it's the highest-risk, most novel feature with no existing Rust crate to lean on.

The browser companion is simpler than expected. crossterm 0.29 already has OSC 52 clipboard via a feature flag. A minimal HTTP server (tiny-http) on a background thread, communicating via mpsc channel, keeps the TUI event loop unaffected. Use pulldown-cmark's built-in `html::push_html()` for HTML rendering -- it's already in the dependency tree.

## Key Findings

**Stack:** Add serde + toml + dirs for config, crossterm osc52 feature for clipboard, tiny-http for browser companion. No new editor widget needed. Total new binary size: ~550-650KB.
**Architecture:** Vim as a state machine returning VimCommand enum. Editor becomes a passive buffer exposing atomic operations. Theme as a shared struct passed through constructors. Browser companion on background thread with mpsc channel.
**Critical pitfall:** WYSIWYG is the highest-risk feature -- no existing Rust crate does inline rendered markdown editing with cursor navigation. Budget 3x the time estimate and be prepared to cut it.

## Implications for Roadmap

Based on research, suggested phase structure:

1. **Config + Theme** - Foundation phase, cross-cutting
   - Addresses: Configurable color themes, keybinding mode selection
   - Avoids: Hardcoded colors accumulating across new features
   - New files: src/config.rs, src/theme.rs (~220 LOC)
   - Modified: every file with hardcoded Color:: references

2. **Vim Keybindings** - Core interaction overhaul
   - Addresses: Vim-style keybindings as default (user's top priority)
   - Avoids: Reworking key routing after other features are built on top
   - New files: src/vim.rs, src/command_line.rs (~380 LOC)
   - Modified: app.rs (AppMode expansion), editor.rs (remove nano bindings), status_bar.rs

3. **Clipboard Integration** - Small, depends on vim yank model
   - Addresses: Copy/paste via OSC 52
   - Avoids: Building clipboard before yank/paste semantics are defined
   - New files: src/clipboard.rs (~60 LOC)
   - Modified: Cargo.toml (osc52 feature), app.rs (yank/paste routing)

4. **Browser Companion** - Independent feature, behind feature flag
   - Addresses: GitHub-accurate rendering in browser
   - Avoids: Adding async/tokio -- uses sync background thread
   - New files: src/browser.rs, assets/ (~250 LOC + embedded HTML/CSS)
   - Modified: main.rs (--browser flag), app.rs (channel sender)

5. **WYSIWYG Terminal Editing** - Highest risk, most novel
   - Addresses: --wysiwyg inline editing mode
   - Avoids: Blocking other features if this proves too complex
   - New files: src/wysiwyg.rs (~400 LOC, highest uncertainty)
   - Modified: main.rs (--wysiwyg flag), app.rs (editor mode switching)

**Phase ordering rationale:**
- Config/theme is a dependency for all visual features (vim mode indicator, themed search highlights)
- Vim changes the fundamental key routing in app.rs -- must happen before layering more modes
- Clipboard is small but semantically coupled to vim (yank/paste)
- Browser companion is fully independent (background thread, channel communication)
- WYSIWYG depends on stable vim + theme + clipboard infrastructure

**Research flags for phases:**
- Phase 2 (Vim): Standard pattern, well-understood -- ratatui-textarea's vim example is a working reference
- Phase 4 (Browser): May need deeper research on comrak vs pulldown-cmark HTML output quality
- Phase 5 (WYSIWYG): Likely needs significant research -- no precedent in Rust, prototype recommended before committing

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All crates verified on crates.io, crossterm osc52 confirmed in 0.29 changelog |
| Vim Integration | HIGH | Pattern well-established: state machine + existing textarea operations |
| Config/Theme | HIGH | serde + toml is standard Rust pattern, straightforward |
| Clipboard | HIGH | crossterm 0.29 has osc52 feature, bracketed paste already works |
| Browser Companion | MEDIUM | Pattern clear (tiny-http on background thread), untested in this codebase |
| WYSIWYG | LOW | No existing Rust implementation, bidirectional source mapping is novel |
| Architecture | HIGH | Integration points clearly identified, no existing code needs to be discarded |

## Gaps to Address

- WYSIWYG cursor-in-rendered-markdown needs a prototype before committing to full implementation
- Browser companion: verify tiny-http works correctly when spawned from a raw-mode terminal process
- Mouse support (scrolling + clicking) mentioned in PROJECT.md but orthogonal to the five main features -- can slot into any phase
- Adjustable split ratio is trivial once config exists -- not a separate phase
- edtui ratatui 0.30 compatibility not confirmed (irrelevant since we're not using it)

---
*Research completed: 2026-03-22*
*Supersedes: v1.0 research summary (2026-03-21)*
