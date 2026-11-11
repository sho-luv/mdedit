use anyhow::{Context, Result};
use clap::Parser;
use crossterm::cursor::Show;
use crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::stdout;
#[allow(deprecated)]
use std::panic::PanicInfo;
use std::path::PathBuf;

mod app;
mod clipboard;
mod config;
mod editor;
mod file_io;
mod highlighter;
mod live;
mod markdown;
mod preview;
mod status_bar;
mod theme;
mod vim;

#[derive(Parser)]
#[command(
    name = "mdedit",
    version,
    about = "A terminal markdown editor with live rendered preview"
)]
struct Cli {
    /// File to edit
    file: Option<PathBuf>,

    /// Override theme name (ocean, dracula, solarized-light, gruvbox-dark, or custom)
    #[arg(long)]
    theme: Option<String>,

    /// Override editing mode (vim or nano)
    #[arg(long)]
    mode: Option<String>,

    /// Starting view: live (edit inside the rendered document), split,
    /// editor, or preview
    #[arg(long)]
    view: Option<String>,
}

struct TerminalSession;

impl TerminalSession {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        execute!(
            stdout(),
            EnterAlternateScreen,
            EnableMouseCapture,
            EnableBracketedPaste
        )?;
        Ok(Self)
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        restore_terminal();
    }
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(
        stdout(),
        DisableBracketedPaste,
        DisableMouseCapture,
        LeaveAlternateScreen,
        Show
    );
}

fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        restore_terminal();

        let location = info
            .location()
            .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()))
            .unwrap_or_else(|| "unknown location".to_string());
        let message = panic_payload(info);
        let backtrace = std::backtrace::Backtrace::force_capture();

        let report = format!("mdedit panicked at {location}\n{message}\n\n{backtrace}\n");
        let log_path = std::env::temp_dir().join("mdedit-panic.log");
        let _ = std::fs::write(&log_path, &report);

        eprintln!("{report}");
        eprintln!("Crash log written to {}", log_path.display());
    }));
}

#[allow(deprecated)]
fn panic_payload(info: &PanicInfo<'_>) -> String {
    if let Some(message) = info.payload().downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = info.payload().downcast_ref::<String>() {
        message.clone()
    } else {
        "panic payload was not a string".to_string()
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("mdedit: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    install_panic_hook();

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
            other => anyhow::bail!("unknown --mode '{other}' (expected vim or nano)"),
        }
    }

    if let Some(view) = cli.view {
        cfg.view = config::StartView::from_name(&view).ok_or_else(|| {
            anyhow::anyhow!("unknown --view '{view}' (expected live, split, editor or preview)")
        })?;
    }

    // Resolve theme from config
    let mut resolved_theme = config::resolve_theme(&cfg);

    // Detect terminal color capability and apply fallback if needed
    let cap = theme::detect_color_capability();
    theme::set_color_capability(cap);
    if cap == theme::ColorCapability::Color256 {
        resolved_theme = resolved_theme.with_256_color_fallback();
    }

    // Load file content if path provided
    let content = if let Some(ref path) = cli.file {
        file_io::load_file(path)?
    } else {
        None
    };

    // Manual terminal init with panic-safe cleanup.
    let _session = TerminalSession::enter()
        .context("needs an interactive terminal (could not switch the terminal to raw mode)")?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let clipboard = clipboard::detect_provider();
    let mut app = app::App::new(content, cli.file, resolved_theme, cfg.mode, clipboard, cfg);
    app.enable_background_preview();
    app.run(&mut terminal)
}
