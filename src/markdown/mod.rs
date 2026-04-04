mod renderer;
pub use renderer::TuiMarkdownRenderer;

use ratatui::text::Text;
use crate::config::RenderProfile;

/// Result of rendering markdown, including the styled text and a source-to-preview line map.
pub struct RenderResult {
    pub text: Text<'static>,
    /// Maps source line number (0-based) to the first preview line (0-based) it produces.
    /// Used for scroll synchronization between editor and preview.
    pub source_to_preview: Vec<usize>,
}

/// Abstraction over markdown rendering so tui-markdown can be replaced later (D-01).
/// Returns owned Text<'static> so the result can be cached in App state.
pub trait MarkdownRenderer {
    fn render(&self, markdown: &str, profile: RenderProfile) -> RenderResult;
}
