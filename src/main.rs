use anyhow::Result;
use clap::Parser;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, EnableBracketedPaste, DisableBracketedPaste};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::stdout;
use std::path::PathBuf;

mod app;
mod clipboard;
mod config;
mod editor;
mod file_io;
mod highlighter;
mod markdown;
mod preview;
mod status_bar;
mod theme;
mod vim;

#[derive(Parser)]
#[command(name = "mdedit", about = "A terminal markdown editor")]
struct Cli {
    /// File to edit
    file: Option<PathBuf>,

    /// Override theme name (ocean, dracula, solarized-light, gruvbox-dark, or custom)
    #[arg(long)]
    theme: Option<String>,

    /// Override editing mode (vim or nano)
    #[arg(long)]
    mode: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load config from ~/.config/mdedit/config.toml (defaults if missing)
    let mut cfg = config::load_config();

    // CLI overrides
    if let Some(theme_name) = cli.theme {
        cfg.theme = theme_name;
    }
    if let Some(mode_str) = cli.mode {
        match mode_str.to_lowercase().as_str() {
            "vim" => cfg.mode = config::EditingMode::Vim,
            "nano" => cfg.mode = config::EditingMode::Nano,
            other => eprintln!("Warning: unknown mode '{}', using vim", other),
        }
    }

    // Resolve theme from config
    let mut resolved_theme = config::resolve_theme(&cfg);

    // Detect terminal color capability and apply fallback if needed
    let cap = theme::detect_color_capability();
    if cap == theme::ColorCapability::Color256 {
        resolved_theme = resolved_theme.with_256_color_fallback();
    }

    // Load file content if path provided
    let content = if let Some(ref path) = cli.file {
        file_io::load_file(path)?
    } else {
        None
    };

    // Manual terminal init with mouse capture (D-18)
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture, EnableBracketedPaste)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let clipboard = clipboard::detect_provider();
    let mut app = app::App::new(content, cli.file, resolved_theme, cfg.mode, clipboard, cfg);
    let result = app.run(&mut terminal);

    // Cleanup: disable mouse, leave alternate screen, disable raw mode
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), DisableBracketedPaste, DisableMouseCapture, LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}
