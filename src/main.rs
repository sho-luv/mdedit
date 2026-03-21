use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

mod app;
mod editor;
mod file_io;
mod status_bar;

#[derive(Parser)]
#[command(name = "mdedit", about = "A terminal markdown editor")]
struct Cli {
    /// File to edit
    file: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load file content if path provided (D-01)
    let content = if let Some(ref path) = cli.file {
        file_io::load_file(path)?
    } else {
        None
    };

    ratatui::run(|terminal| {
        let mut app = app::App::new(content, cli.file);
        app.run(terminal)
    })?;
    Ok(())
}
