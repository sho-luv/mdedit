# Phase 4: Configuration and Themes - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning
**Mode:** Auto (recommended defaults selected)

<domain>
## Phase Boundary

Add a configuration system (`~/.config/mdedit/config.toml`) and configurable color themes. Users can select built-in themes, define custom themes, set their default editing mode (vim/nano), and the editor respects terminal color capabilities. No new editing features, no new layout modes.

</domain>

<decisions>
## Implementation Decisions

### Config file format and location
- **D-01:** Config file is TOML format at `~/.config/mdedit/config.toml` (XDG standard). Use the `dirs` crate for platform-appropriate config directory resolution.
- **D-02:** Config is optional — editor works with sensible defaults if no config file exists. Missing fields use defaults (no crash on partial config).
- **D-03:** Config is read once at startup. No hot-reload in v2 (restart to apply changes).
- **D-04:** Add `serde` and `toml` crates for deserialization. Use `#[serde(default)]` on all fields.

### Built-in themes
- **D-05:** Ship 3-4 built-in themes: `ocean` (current base16-ocean.dark, default), `dracula`, `solarized-light`, `gruvbox-dark`.
- **D-06:** Theme selection in config: `theme = "dracula"`. CLI override: `mdedit --theme dracula file.md`.
- **D-07:** Built-in themes are hardcoded Rust structs (no embedded files). This keeps binary lean and avoids file-path resolution issues.

### Theme scope and structure
- **D-08:** A Theme struct covers ALL color values in the app — editor background, line numbers, status bar, search highlights, selection overlay, preview styling, AND the syntect highlighting theme name.
- **D-09:** Extract the ~12 hardcoded `Color::` values across `app.rs`, `editor.rs`, `status_bar.rs`, and `highlighter.rs` into the Theme struct. Every `Color::` literal becomes `theme.something`.
- **D-10:** Custom themes in config follow the same structure as built-in themes. Users define `[theme.custom.mytheme]` sections in TOML with color values as hex strings (`"#282a36"`) or named colors (`"red"`).

### Editing mode config
- **D-11:** Config field `mode = "vim"` (default) or `mode = "nano"`. This field is read in Phase 4 but the actual vim keybinding implementation is Phase 5. In Phase 4, `mode = "vim"` is stored but behaves like nano until Phase 5 ships.
- **D-12:** CLI override: `mdedit --mode nano file.md` (useful for one-off sessions).

### Terminal color detection
- **D-13:** Detect truecolor via `$COLORTERM` env var (`truecolor` or `24bit`). Fall back to 256-color when not detected. Theme struct includes both truecolor and 256-color fallback values.
- **D-14:** No 16-color mode — 256-color is the floor. This is reasonable for any terminal that supports ratatui/crossterm.

### Claude's Discretion
- Exact color values for built-in themes (dracula, solarized-light, gruvbox-dark)
- Internal Theme struct field names and organization
- How to wire Theme through App → Editor → Preview → StatusBar (constructor params vs shared reference)
- Error handling for malformed config files (log warning + use defaults, or hard error)

</decisions>

<specifics>
## Specific Ideas

- User explicitly wants configurable color themes — "I also want color so I can see links and headers etc more clearly maybe these settings are configurable"
- Themes should cover syntax highlighting in the editor pane AND the preview pane colors
- The `mode` field should default to `"vim"` (user preference from prior discussions), even though vim isn't implemented until Phase 5
- Config should feel like a standard Rust CLI tool config (similar to helix, zellij, alacritty TOML configs)

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project context
- `.planning/PROJECT.md` — Core value, constraints, key decisions
- `.planning/REQUIREMENTS.md` — CONF-01 through CONF-05

### Research
- `.planning/research/STACK.md` — serde, toml, dirs crate recommendations with versions
- `.planning/research/ARCHITECTURE.md` — Theme struct design, config integration points
- `.planning/research/PITFALLS.md` — Terminal color detection pitfalls, theme degradation

### Existing code (color extraction targets)
- `src/app.rs` — 4 hardcoded Color:: values (divider DarkGray, confirm Red/White, prompt Blue/White)
- `src/editor.rs` — 3 hardcoded Color:: values (line numbers DarkGray, selection Rgb(68,68,102), search Cyan/Yellow)
- `src/status_bar.rs` — 1 hardcoded Color:: value (bar DarkGray/White)
- `src/highlighter.rs` — 2 hardcoded Color:: values (line numbers DarkGray, syntect Rgb conversion)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `src/highlighter.rs`: Already uses syntect with `base16-ocean.dark` theme — Theme struct will specify the syntect theme name instead of hardcoding it
- `Cargo.toml`: Already has `clap` with `derive` feature — adding `--theme` and `--mode` CLI args is trivial
- `src/main.rs`: Cli struct is the natural place to add `--theme` and `--mode` options

### Established Patterns
- Colors are used via `Style::default().fg(Color::X).bg(Color::Y)` throughout — mechanical extraction to `theme.field_name`
- `App::new()` constructs Editor, Preview, StatusBar — Theme can be passed through constructors
- No global state — Theme should be owned by App and passed by reference to render methods

### Integration Points
- `App::new()` — Load config, construct Theme, pass to all sub-components
- `Editor::render_highlighted()` — Replace hardcoded Color:: with theme references for selection, search, line numbers
- `StatusBar::render()` — Replace hardcoded bar colors with theme values
- `App::render()` — Replace hardcoded divider and prompt colors with theme values
- `MarkdownHighlighter::new()` — Accept theme name instead of hardcoding `base16-ocean.dark`

</code_context>

<deferred>
## Deferred Ideas

- Hot-reload config on file change — v3+ (adds complexity with file watcher)
- Per-file config overrides — v3+ (`.mdedit.toml` in project directory)
- Theme preview/switching during runtime — v3+ (would need a theme picker UI)

</deferred>

---

*Phase: 04-configuration-and-themes*
*Context gathered: 2026-03-22*
