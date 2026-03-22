# Roadmap: mdedit

## Milestones

- ✅ **v1.0 MVP** — Phases 1-3 (shipped 2026-03-22)
- 🚧 **v2.0 Power User** — Phases 4-8 (in progress)

## Phases

<details>
<summary>v1.0 MVP (Phases 1-3) — SHIPPED 2026-03-22</summary>

- [x] Phase 1: Terminal Editor (2/2 plans) — completed 2026-03-21
- [x] Phase 2: Live Preview (2/2 plans) — completed 2026-03-21
- [x] Phase 3: Polish and Power Features (2/2 plans) — completed 2026-03-22

Full details: `.planning/milestones/v1.0-ROADMAP.md`

</details>

### v2.0 Power User (Phases 4-8)

- [ ] **Phase 4: Configuration and Themes** - User can customize colors and settings via config file
- [ ] **Phase 5: Vim Keybindings and Mouse** - Editor uses vim-style modal editing with mouse support
- [ ] **Phase 6: Clipboard Integration** - User can copy/paste to system clipboard, including over SSH
- [ ] **Phase 7: Browser Companion** - User can preview markdown in a browser with GitHub-accurate styling
- [ ] **Phase 8: WYSIWYG Terminal Mode** - User can edit rendered markdown inline in the terminal

## Phase Details

### Phase 4: Configuration and Themes
**Goal**: Users can personalize their editing environment through a config file and color themes
**Depends on**: Phase 3 (v1.0 codebase)
**Requirements**: CONF-01, CONF-02, CONF-03, CONF-04, CONF-05
**Success Criteria** (what must be TRUE):
  1. User can create `~/.config/mdedit/config.toml` and the editor reads it on startup
  2. User can switch between built-in themes by name (e.g., `theme = "dracula"`) and see colors change
  3. User can define a custom theme in TOML and the editor applies it correctly
  4. User can set `mode = "vim"` or `mode = "nano"` in config and the editor starts in that mode
  5. Editor detects terminal color capability and degrades gracefully (truecolor vs 256-color)
**Plans**: 2 plans

Plans:
- [x] 04-01-PLAN.md — Config/theme types, built-in themes, CLI args, config loading
- [ ] 04-02-PLAN.md — Wire theme through all rendering components, replace hardcoded colors

### Phase 5: Vim Keybindings and Mouse
**Goal**: Users can navigate and edit using vim-style modal keybindings and interact with the mouse
**Depends on**: Phase 4 (config provides mode selection and themed mode indicator)
**Requirements**: VIM-01, VIM-02, VIM-03, VIM-04, VIM-05, VIM-06, VIM-07, VIM-08, VIM-09, VIM-10, MOUSE-01, MOUSE-02, MOUSE-03, MOUSE-04
**Success Criteria** (what must be TRUE):
  1. Editor opens in Normal mode by default; user can switch between Normal, Insert, Visual, and Command modes with standard vim keys
  2. User can navigate with h/j/k/l, w/b/e, 0/$, gg/G and use operators (d, c, y, p) with count prefixes (e.g., 3dd)
  3. User can save with :w, quit with :q, and use :wq and :q! from command mode
  4. Status bar displays the current mode (NORMAL/INSERT/VISUAL/COMMAND)
  5. User can click to place cursor, scroll with mouse wheel, drag to select text, and drag the divider to resize panes
**Plans**: TBD

### Phase 6: Clipboard Integration
**Goal**: Users can copy and paste text to/from the system clipboard, including over SSH
**Depends on**: Phase 5 (vim yank/paste keybindings define clipboard semantics)
**Requirements**: CLIP-01, CLIP-02, CLIP-03, CLIP-04
**Success Criteria** (what must be TRUE):
  1. User can yank text in vim (y) or use Ctrl+C and paste it into another application
  2. User can copy text in another application and paste it into mdedit via vim (p/P) or Ctrl+V
  3. Clipboard works when running mdedit over SSH (via OSC 52)
  4. On local sessions, clipboard uses platform-native tools (pbcopy on macOS, xclip on Linux) when available
**Plans**: TBD

### Phase 7: Browser Companion
**Goal**: Users can view their markdown rendered with GitHub-accurate styling in a browser
**Depends on**: Phase 4 (config for theme awareness; otherwise independent)
**Requirements**: BROW-01, BROW-02, BROW-03, BROW-04, BROW-05
**Success Criteria** (what must be TRUE):
  1. User can run `mdedit --browser file.md` and a browser tab opens with the rendered markdown
  2. The browser preview matches GitHub's markdown rendering (heading styles, code blocks, tables, etc.)
  3. When user saves the file in the terminal editor, the browser preview refreshes automatically
  4. Browser companion serves only on localhost (not accessible from network)
**Plans**: TBD

### Phase 8: WYSIWYG Terminal Mode
**Goal**: Users can edit markdown inline in the terminal, seeing rendered output instead of raw syntax
**Depends on**: Phase 5 (vim keybindings), Phase 4 (themes)
**Requirements**: WYS-01, WYS-02, WYS-03, WYS-04, WYS-05
**Success Criteria** (what must be TRUE):
  1. User can run `mdedit --wysiwyg file.md` and see rendered markdown (styled headings, bold text, etc.) instead of raw syntax
  2. When cursor moves to a line, that line reveals the raw markdown syntax for editing
  3. User can edit in WYSIWYG mode using the same keybindings (vim or nano) as the standard mode
  4. Saving in WYSIWYG mode writes the underlying markdown source, not the rendered form
**Plans**: TBD

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Terminal Editor | v1.0 | 2/2 | Complete | 2026-03-21 |
| 2. Live Preview | v1.0 | 2/2 | Complete | 2026-03-21 |
| 3. Polish and Power Features | v1.0 | 2/2 | Complete | 2026-03-22 |
| 4. Configuration and Themes | v2.0 | 0/2 | Planning complete | - |
| 5. Vim Keybindings and Mouse | v2.0 | 0/? | Not started | - |
| 6. Clipboard Integration | v2.0 | 0/? | Not started | - |
| 7. Browser Companion | v2.0 | 0/? | Not started | - |
| 8. WYSIWYG Terminal Mode | v2.0 | 0/? | Not started | - |
