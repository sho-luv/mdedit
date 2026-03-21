use std::path::PathBuf;

/// Stub App — will be replaced in Task 2 with full event loop and rendering.
pub struct App {}

impl App {
    pub fn new(_content: Option<String>, _filepath: Option<PathBuf>) -> Self {
        App {}
    }

    pub fn run(&mut self, _terminal: &mut ratatui::DefaultTerminal) -> anyhow::Result<()> {
        Ok(())
    }
}
