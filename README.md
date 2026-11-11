# mdedit

A terminal markdown editor with live rendered preview. Edit raw markdown on one side, see it rendered on the other — no browser, no bloated IDE, no vault lock-in.

![mdedit demo](demo/mdedit-demo.gif)

### Demo Video

https://github.com/user-attachments/assets/8fdf074f-c237-4759-a589-46ab6648425d

## Install

```bash
# Quick install (macOS / Linux)
curl -fsSL https://raw.githubusercontent.com/sho-luv/mdedit/main/install.sh | sh
```

```bash
# From source (requires Rust 1.75+)
cargo install --git https://github.com/sho-luv/mdedit.git
```

Pre-built binaries are available on the [Releases](https://github.com/sho-luv/mdedit/releases) page.

## Usage

```bash
mdedit file.md          # Edit existing file
mdedit new-file.md      # Creates file if it doesn't exist
mdedit --theme dracula   # Override color theme
mdedit --mode nano       # Use nano-style keybindings
mdedit --view split      # Start in split view (live, split, editor, preview)
```

## Features

- **Live view** (default): edit inside the rendered document. Everything is shown rendered except the block under your cursor, which shows its raw markdown; move away and it renders again
- Side-by-side editor + live rendered preview (`Ctrl+P` cycles live / split / editor only / preview only)
- **btop-style UI**: rounded bordered panels, clickable title-bar buttons, focus-follows-click, F1 help overlay
- **Render profiles**: Clean (default, Warp-inspired), GitHub, Obsidian, and CommonMark preview styles
- **Syntax-highlighted code blocks** in preview (Rust, Python, Bash, JSON, and more)
- **Table rendering** with Unicode box-drawing borders and column alignment
- **Callout/alert support** for GitHub (`> [!NOTE]`) and Obsidian (`> [!tip]`) syntax
- **Sync indicator**: blue highlight line showing your cursor position in the preview
- Vim-style modal editing (Normal/Insert/Visual/Command)
- Markdown syntax highlighting in the editor, with soft word wrap and a current-line highlight
- Status bar with mode, unsaved-changes marker, word count, and cursor position; scrollbars on both panes
- Configurable color themes (ocean, dracula, solarized-light, gruvbox-dark)
- System clipboard integration (OSC 52 + platform-native)
- Mouse support (click, scroll, drag-select, resize panes)
- Text search with highlighting
- Works over SSH
- 3.7MB binary; the editor is on screen in about a millisecond and the preview renders on a background thread, so typing never waits on it

## Keybindings

### Vim Mode (default)

#### Normal Mode
| Key | Action |
|-----|--------|
| `h/j/k/l` | Move cursor |
| `w/b/e` | Word forward/back/end |
| `0/$` | Line start/end |
| `gg/G` | File start/end |
| `i/a/o/O` | Enter insert mode |
| `d{motion}` | Delete |
| `c{motion}` | Change (delete + insert) |
| `y{motion}` | Yank (copy) |
| `p/P` | Paste after/before |
| `dd/yy/cc` | Line operations |
| `x` | Delete character |
| `u` | Undo |
| `Ctrl+R` | Redo |
| `v/V` | Visual mode (char/line) |
| `/` | Search (`Enter` accepts, `n`/`N` next/previous, `Esc` clears highlights) |
| `:w` | Save |
| `:q` | Quit |
| `:wq` | Save and quit |
| `:q!` | Force quit |
| `:help` | Keybindings help (same as `F1`) |

#### Insert Mode
| Key | Action |
|-----|--------|
| `Esc` | Return to Normal mode |
| All keys | Type normally |

### Nano Mode

| Key | Action |
|-----|--------|
| `Ctrl+S` | Save |
| `Ctrl+Q` | Quit |
| `Ctrl+F` | Search |
| `Ctrl+C` | Copy selection to clipboard |
| `Ctrl+V` | Paste from clipboard |
| `Tab/Shift+Tab` | Indent/outdent |

### Global (both modes)

| Key | Action |
|-----|--------|
| `F1` | Help overlay (keybindings reference) |
| `F2` | Settings panel |
| `Ctrl+P` | Toggle view: split / editor only / preview only |
| `Ctrl+Q` | Quit (prompts if unsaved) |
| Mouse wheel | Scroll editor or preview |
| Click | Focus pane / position cursor |
| Click title button | `^P view`, `F2 set`, `F1 help`, `^Q quit` |
| Drag panel border | Resize panes |

The UI follows btop conventions: each pane is a bordered panel with its title
in the border, the focused panel gets an accent border, and the buttons on the
top border are clickable. Click (or scroll) the preview to focus it — arrow
keys (and `j`/`k`/`g`/`G` in vim normal mode) then scroll the preview, and
editor scroll-sync pauses until you click back into the editor.

## Configuration

Settings can be changed live with `:set` in vim mode, which opens an interactive settings panel. Changes can be saved to persist across sessions.

| Command | Action |
|---------|--------|
| `:set` | Open settings panel |
| `:set theme dracula` | Switch theme directly |
| `:set mode nano` | Switch mode (restart to apply) |
| `:set profile obsidian` | Switch render profile |
| `:set save` | Save current settings to config file |

Config file: `~/.config/mdedit/config.toml` (or `$XDG_CONFIG_HOME/mdedit/config.toml`). A config already saved in the OS-native location (`~/Library/Application Support/mdedit/` on macOS) is still read and updated in place.

```toml
# Editing mode: "vim" (default) or "nano"
mode = "vim"

# Color theme
theme = "ocean"

# Render profile: "clean" (default), "github", "obsidian", or "commonmark"
render_profile = "clean"

# Sync indicator line in preview (default: true)
sync_indicator = true

# Soft-wrap long lines in the editor at word boundaries (default: true)
word_wrap = true

# Starting view: "live" (default), "split", "editor", or "preview"
view = "live"
```

### Render Profiles

| Profile | Features |
|---------|----------|
| `clean` | GFM features with a minimal look: plain bold headings, outlined code blocks with language labels, borderless tables (record layout when narrow), reading margins with a 100-column cap |
| `github` | GFM tables, task lists, alerts (`> [!NOTE]`), blue heading accents |
| `obsidian` | Callouts, wikilinks (`[[page]]`), tags (`#tag`), purple/green headings |
| `commonmark` | Strict CommonMark, no extensions, classic colored headings |

### Built-in Themes

- `ocean` — Default dark blue theme
- `dracula` — Purple-tinted dark theme
- `solarized-light` — Light theme
- `gruvbox-dark` — Warm dark theme

## Clipboard

Clipboard works automatically:
- **Local**: Uses `pbcopy`/`pbpaste` (macOS) or `xclip`/`wl-copy` (Linux)
- **SSH**: Uses OSC 52 escape sequences (works in most modern terminals)
- **tmux**: Auto-detects and wraps in DCS passthrough

Every yank and delete syncs to system clipboard. Paste reads from system clipboard.

## Requirements

- Rust 1.75+ (to build)
- A terminal with ANSI color support
- macOS or Linux
