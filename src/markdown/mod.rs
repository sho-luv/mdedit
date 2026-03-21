mod renderer;
pub use renderer::TuiMarkdownRenderer;

use ratatui::text::Text;

/// Abstraction over markdown rendering so tui-markdown can be replaced later (D-01).
/// Returns owned Text<'static> so the result can be cached in App state.
pub trait MarkdownRenderer {
    fn render(&self, markdown: &str) -> Text<'static>;
}
