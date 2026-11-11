use anyhow::Result;
use crossterm::cursor::SetCursorStyle;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use ratatui_textarea::CursorMove;
use regex::Regex;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::editor::{Editor, EditorAction};
use crate::file_io;
use crate::markdown::{MarkdownRenderer, PreviewRenderer, RenderResult};
use crate::preview::Preview;
use crate::status_bar::{Prompt, StatusBar, StatusBarContent};
use crate::vim::{InsertPosition, Motion, VimCommand, VimHandler};

/// Layout mode — controls how the main area is split (D-11, D-12, D-13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    /// Edit inside the rendered document (see `crate::live`).
    Live,
    Split,
    EditorOnly,
    PreviewOnly,
}

impl LayoutMode {
    pub fn next(self) -> Self {
        match self {
            Self::Live => Self::Split,
            Self::Split => Self::EditorOnly,
            Self::EditorOnly => Self::PreviewOnly,
            Self::PreviewOnly => Self::Live,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Live => "Live View",
            Self::Split => "Split View",
            Self::EditorOnly => "Editor Only",
            Self::PreviewOnly => "Preview Only",
        }
    }
}

/// Which pane currently has input focus (btop-style focus-follows-click).
/// The focused pane gets an accent border; while the preview is focused,
/// scroll keys drive it and cursor scroll-sync is paused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusedPane {
    Editor,
    Preview,
}

/// Clickable buttons drawn over the top border of the rightmost panel
/// (btop-style `┤ key label ├` controls).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UiButton {
    Help,
    Settings,
    View,
    Quit,
}

/// Application mode — determines how key events are routed and what the
/// status bar displays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppMode {
    /// Nano-style editing mode.
    Editing,
    /// Vim Normal mode.
    Normal,
    /// Vim Insert mode.
    Insert,
    /// Vim Visual mode.
    Visual,
    /// Vim Command mode (: prompt).
    Command,
    /// Prompt: "Unsaved changes. Save? (y/n/Esc)" (D-13)
    ConfirmQuit,
    /// Prompt: "Save as: ___" for untitled buffers (D-02)
    PromptFilename,
    /// Incremental search mode (D-05). Captures keystrokes for search query.
    Search,
    /// Settings overlay panel.
    Settings,
}

/// Which row is selected in the settings panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsRow {
    Theme,
    Mode,
    Profile,
    SyncIndicator,
    WordWrap,
    Save,
    Reset,
}

/// Everything needed to render the preview off the main thread.
#[derive(Clone)]
struct PreviewJob {
    content: String,
    profile: crate::config::RenderProfile,
    theme: crate::theme::Theme,
    width: u16,
}

/// A finished render: the document (or the panic message), the width it was
/// laid out for, and how long it took.
type PreviewOutcome = (Result<RenderResult, String>, u16, Duration);

impl PreviewJob {
    /// Render, containing any renderer panic so a bad document can never
    /// take the editor down.
    fn run(self) -> PreviewOutcome {
        let started = Instant::now();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            PreviewRenderer.render(&self.content, self.profile, &self.theme, self.width)
        }))
        .map_err(panic_message);
        (outcome, self.width, started.elapsed())
    }
}

/// Handle to the background thread that renders preview jobs.
struct PreviewWorker {
    jobs: std::sync::mpsc::Sender<PreviewJob>,
    results: std::sync::mpsc::Receiver<PreviewOutcome>,
    in_flight: bool,
}

impl PreviewWorker {
    /// Start the render thread. It exits when the `App` (the job sender) is
    /// dropped. Returns `None` if the thread can't be spawned.
    fn spawn() -> Option<Self> {
        let (jobs, job_rx) = std::sync::mpsc::channel::<PreviewJob>();
        let (result_tx, results) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("mdedit-preview".into())
            .spawn(move || {
                while let Ok(job) = job_rx.recv() {
                    if result_tx.send(job.run()).is_err() {
                        break;
                    }
                }
            })
            .ok()?;
        Some(PreviewWorker {
            jobs,
            results,
            in_flight: false,
        })
    }
}

/// Top-level application struct owning the editor and managing the event loop.
pub struct App<'a> {
    editor: Editor<'a>,
    mode: AppMode,
    should_quit: bool,
    /// Text buffer for the filename prompt.
    filename_input: String,
    /// Status bar widget with timed messages.
    status_bar: StatusBar,
    /// Flag indicating we should quit after saving (from ConfirmQuit -> 'y' flow).
    quit_after_save: bool,
    /// Current layout mode (split, editor-only, preview-only).
    layout_mode: LayoutMode,
    /// Preview component managing scroll state.
    preview: Preview,
    /// Live view (rendered document with the cursor's block shown raw).
    live: crate::live::LiveView,
    /// Cached preview, laid out for the current preview pane width. Carries
    /// the exact source-line <-> row maps used for scroll sync.
    preview_doc: RenderResult,
    /// How long the last preview render took; small documents skip the
    /// debounce and update on every keystroke.
    last_render_cost: Duration,
    /// Cached word count for the status bar (`None` = recount needed).
    word_count: Option<usize>,
    /// Background preview renderer (`None` = render synchronously).
    preview_worker: Option<PreviewWorker>,
    /// Pane width of the most recently requested preview render.
    preview_requested_width: Option<u16>,
    /// Flag: content changed since last preview render.
    content_dirty: bool,
    /// Timestamp of last edit (for debounce).
    last_edit_time: Option<Instant>,
    /// Current search query text (D-05).
    search_query: String,
    /// Cursor position when search started, for Esc restore (D-07).
    search_cursor_before: (usize, usize),
    /// Current match index (0-based) for [3/17] display (D-10).
    search_match_index: usize,
    /// Total match count for [3/17] display (D-10).
    search_match_count: usize,
    /// Resolved color theme (wired to rendering in Plan 02).
    pub theme: crate::theme::Theme,
    /// Editing mode -- vim or nano (used in Phase 5).
    #[allow(dead_code)]
    pub editing_mode: crate::config::EditingMode,
    /// Vim key handler state machine (Some when vim mode, None when nano).
    vim_handler: Option<VimHandler>,
    /// Split ratio as percentage (0-100) for editor pane width. Default 50.
    split_ratio: u16,
    /// Track layout areas for mouse hit testing.
    editor_area: Option<Rect>,
    /// Track preview area for mouse hit testing.
    preview_area: Option<Rect>,
    /// Track divider area for mouse hit testing.
    divider_area: Option<Rect>,
    /// Full body area of the split layout (for divider drag ratio math).
    split_body_area: Option<Rect>,
    /// Which pane has input focus (btop-style focus-follows-click).
    focused_pane: FocusedPane,
    /// Hit areas of the title-bar buttons, rebuilt every frame.
    button_areas: Vec<(Rect, UiButton)>,
    /// Whether the keybindings help overlay is visible.
    show_help: bool,
    /// Whether user is currently dragging the divider.
    dragging_divider: bool,
    /// Whether user is currently drag-selecting text.
    drag_selecting: bool,
    /// System clipboard provider (OSC 52 + platform-native).
    clipboard: Box<dyn crate::clipboard::ClipboardProvider>,
    /// Whether we've already shown the "clipboard unavailable" warning.
    clipboard_warned: bool,
    /// Live config state for :set commands and saving.
    config: crate::config::Config,
    /// Selected row in settings panel.
    settings_row: SettingsRow,
    /// Index into the theme list for the settings panel.
    settings_theme_index: usize,
    /// Index into the mode list for the settings panel.
    settings_mode_index: usize,
    /// Index into the render profile list for the settings panel.
    settings_profile_index: usize,
}

impl<'a> App<'a> {
    pub fn new(
        content: Option<String>,
        filepath: Option<PathBuf>,
        theme: crate::theme::Theme,
        editing_mode: crate::config::EditingMode,
        clipboard: Box<dyn crate::clipboard::ClipboardProvider>,
        config: crate::config::Config,
    ) -> Self {
        let is_vim = editing_mode == crate::config::EditingMode::Vim;
        let initial_mode = if is_vim {
            AppMode::Normal
        } else {
            AppMode::Editing
        };
        let vim_handler = if is_vim {
            Some(VimHandler::new())
        } else {
            None
        };

        let mut editor = Editor::new(content, filepath, theme.clone());
        editor.word_wrap = config.word_wrap;

        App {
            editor,
            mode: initial_mode,
            should_quit: false,
            filename_input: String::new(),
            status_bar: StatusBar::new(),
            quit_after_save: false,
            layout_mode: match config.view {
                crate::config::StartView::Live => LayoutMode::Live,
                crate::config::StartView::Split => LayoutMode::Split,
                crate::config::StartView::Editor => LayoutMode::EditorOnly,
                crate::config::StartView::Preview => LayoutMode::PreviewOnly,
            },
            live: crate::live::LiveView::new(),
            preview: Preview::new(),
            preview_doc: RenderResult::empty(),
            last_render_cost: Duration::ZERO,
            word_count: None,
            preview_worker: None,
            preview_requested_width: None,
            content_dirty: true, // Render initial content on first frame
            last_edit_time: Some(Instant::now()),
            search_query: String::new(),
            search_cursor_before: (0, 0),
            search_match_index: 0,
            search_match_count: 0,
            theme,
            editing_mode,
            vim_handler,
            split_ratio: 50,
            editor_area: None,
            preview_area: None,
            divider_area: None,
            split_body_area: None,
            focused_pane: FocusedPane::Editor,
            button_areas: Vec::new(),
            show_help: false,
            dragging_divider: false,
            drag_selecting: false,
            clipboard,
            clipboard_warned: false,
            settings_theme_index: {
                let themes = crate::config::available_theme_names(&config);
                themes.iter().position(|t| *t == config.theme).unwrap_or(0)
            },
            settings_mode_index: match config.mode {
                crate::config::EditingMode::Vim => 0,
                crate::config::EditingMode::Nano => 1,
            },
            settings_profile_index: crate::config::RenderProfile::all()
                .iter()
                .position(|p| *p == config.render_profile)
                .unwrap_or(0),
            config,
            settings_row: SettingsRow::Theme,
        }
    }

    /// Synchronize preview scroll to the editor cursor (Split mode only).
    ///
    /// The first preview row of the cursor's source line is placed on the same
    /// screen row as the start of that line in the editor, so a line and its
    /// rendered counterpart sit side by side. The editor renders first each
    /// frame, so its screen-row information is fresh.
    fn sync_preview_scroll(&mut self, preview_area_height: u16) {
        if self.layout_mode != LayoutMode::Split {
            return;
        }
        let (cursor_row, _) = self.editor.cursor_position();
        let total_rows = self.preview_doc.rows.len();
        let target_row = match self.preview_doc.row_for_source(cursor_row) {
            Some(row) => row,
            None if total_rows == 0 => {
                self.preview.set_scroll(0);
                return;
            }
            // Unmapped (e.g. preview is showing an error): proportional.
            None => {
                let total_source = self.editor.line_count().saturating_sub(1).max(1);
                cursor_row * total_rows.saturating_sub(1) / total_source
            }
        };

        let line_screen_row = self.editor.cursor_line_screen_row();
        let offset = (target_row as i64 - i64::from(line_screen_row)).max(0) as usize;
        let max_scroll = total_rows.saturating_sub(usize::from(preview_area_height));
        self.preview.set_scroll(offset.min(max_scroll));
    }

    /// Render the preview on a background thread from now on, so neither
    /// startup nor typing ever waits on markdown rendering. Without this
    /// (tests), rendering is synchronous.
    pub fn enable_background_preview(&mut self) {
        self.preview_worker = PreviewWorker::spawn();
    }

    /// How long to wait after the last edit before re-rendering the preview.
    /// With the background worker there is no wait: an in-flight render
    /// already coalesces bursts of edits. Synchronous rendering debounces
    /// documents that are expensive to render (D-04).
    fn preview_debounce(&self) -> Duration {
        if self.preview_worker.is_some() || self.last_render_cost < Duration::from_millis(8) {
            Duration::ZERO
        } else {
            Duration::from_millis(80)
        }
    }

    /// Time until a pending debounced preview render is due, if any.
    fn preview_due_in(&self) -> Option<Duration> {
        if !self.content_dirty {
            return None;
        }
        let debounce = self.preview_debounce();
        Some(match self.last_edit_time {
            Some(t) => debounce.saturating_sub(t.elapsed()),
            None => Duration::ZERO,
        })
    }

    /// Whether a background preview render is running.
    fn preview_in_flight(&self) -> bool {
        self.preview_worker.as_ref().is_some_and(|w| w.in_flight)
    }

    /// Install a finished preview render (or report its failure).
    fn apply_preview(&mut self, outcome: Result<RenderResult, String>, width: u16) {
        match outcome {
            Ok(doc) => self.preview_doc = doc,
            Err(message) => {
                let mut doc = RenderResult::message(&["Preview render failed", &message]);
                doc.width = width;
                self.preview_doc = doc;
                self.preview.set_scroll(0);
                self.status_bar
                    .set_message("Preview render failed; editor still available");
            }
        }
    }

    /// Keep the preview up to date: collect a finished background render,
    /// and start a new one when content changed (after the debounce) or the
    /// pane width changed. Called during render with the actual pane width.
    fn refresh_preview(&mut self, width: u16) {
        if let Some(worker) = self.preview_worker.as_mut() {
            if let Ok((outcome, rendered_width, cost)) = worker.results.try_recv() {
                worker.in_flight = false;
                self.last_render_cost = cost;
                self.apply_preview(outcome, rendered_width);
            }
        }

        let content_due = self.preview_due_in() == Some(Duration::ZERO);
        let width_changed = self.preview_requested_width != Some(width);
        self.preview.show_empty_hint = !self.preview_in_flight();
        if (!content_due && !width_changed) || self.preview_in_flight() {
            // Nothing to do, or a render is running: whatever is still
            // dirty is picked up on a later frame.
            return;
        }

        let job = PreviewJob {
            content: self.editor.content(),
            profile: self.config.render_profile,
            theme: self.theme.clone(),
            width,
        };
        let first_render = self.preview_requested_width.is_none();
        self.preview_requested_width = Some(width);
        self.content_dirty = false;
        self.last_edit_time = None;

        match self.preview_worker.as_mut() {
            Some(worker) if worker.jobs.send(job.clone()).is_ok() => {
                worker.in_flight = true;
                // The live view is the rendered document, so the very first
                // frame waits (briefly) for it instead of flashing raw text.
                if first_render && self.layout_mode == LayoutMode::Live {
                    if let Ok((outcome, rendered_width, cost)) =
                        worker.results.recv_timeout(Duration::from_millis(150))
                    {
                        worker.in_flight = false;
                        self.last_render_cost = cost;
                        self.apply_preview(outcome, rendered_width);
                    }
                }
            }
            _ => {
                // No worker (tests), or it died: render right here.
                self.preview_worker = None;
                let (outcome, width, cost) = job.run();
                self.last_render_cost = cost;
                self.apply_preview(outcome, width);
            }
        }
    }

    /// Main event loop. Draws the UI and processes events until quit.
    pub fn run(
        &mut self,
        terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
        loop {
            terminal.draw(|frame| self.render(frame))?;

            // Sleep until input arrives, a preview render is due or finishing,
            // or a periodic tick (lets timed status messages expire).
            let timeout = if self.preview_in_flight() {
                // Check back soon for the finished render.
                Duration::from_millis(4)
            } else {
                self.preview_due_in()
                    .unwrap_or(Duration::from_millis(250))
                    .max(Duration::from_millis(1))
            };
            if !event::poll(timeout)? {
                continue;
            }

            // Drain every queued event before drawing again, so key repeat and
            // bursts of input never queue up behind redraws.
            loop {
                let event = event::read()?;
                self.handle_event(event);
                if self.should_quit || !event::poll(Duration::ZERO)? {
                    break;
                }
            }

            if self.should_quit {
                break;
            }
        }
        Ok(())
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Key(key) => {
                // IMPORTANT: filter for Press only to avoid double-handling
                // on Windows and some terminals
                if key.kind != KeyEventKind::Press || self.handle_global_key(key) {
                    return;
                }
                let mode_before = self.mode.clone();
                match self.mode {
                    AppMode::Editing => self.handle_editing_key(key),
                    AppMode::Normal => self.handle_vim_key(key),
                    AppMode::Insert => self.handle_vim_insert_key(key),
                    AppMode::Visual => self.handle_vim_visual_key(key),
                    AppMode::Command => self.handle_vim_command_key(key),
                    AppMode::ConfirmQuit => self.handle_confirm_quit_key(key),
                    AppMode::PromptFilename => self.handle_prompt_filename_key(key),
                    AppMode::Search => self.handle_search_key(key),
                    AppMode::Settings => self.handle_settings_key(key),
                }

                // Cursor shape follows the mode; only touch the terminal when
                // the mode actually changed.
                if self.mode != mode_before {
                    let style = match self.mode {
                        AppMode::Normal | AppMode::Visual | AppMode::Command => {
                            Some(SetCursorStyle::SteadyBlock)
                        }
                        AppMode::Insert | AppMode::Editing => Some(SetCursorStyle::SteadyBar),
                        _ => None, // ConfirmQuit, PromptFilename, Search keep current
                    };
                    // (Skipped under test, where stdout isn't a terminal.)
                    if let (Some(style), false) = (style, cfg!(test)) {
                        let _ = crossterm::execute!(std::io::stdout(), style);
                    }
                }
            }
            Event::Mouse(mouse) => self.handle_mouse_event(mouse),
            Event::Paste(text) => {
                // Bracketed paste: insert text at cursor regardless of mode
                self.editor.textarea_mut().insert_str(&text);
                self.mark_content_changed();
            }
            _ => {} // Resize: the next draw re-lays out (and re-renders the preview at the new width)
        }
    }

    /// Perform the save operation. Returns true if save succeeded.
    fn do_save(&mut self) -> bool {
        if let Some(path) = self.editor.filepath().cloned() {
            let content = self.editor.content();
            match file_io::save_file(&path, &content) {
                Ok(()) => {
                    self.editor.mark_saved();
                    self.status_bar.set_message("Saved");
                    true
                }
                Err(e) => {
                    self.status_bar.set_message(&format!("Save failed: {}", e));
                    false
                }
            }
        } else {
            // No filepath — need to prompt for filename
            false
        }
    }

    /// Handle key events in normal editing mode.
    fn handle_editing_key(&mut self, key: crossterm::event::KeyEvent) {
        // Ctrl+F enters search mode (D-05) — intercept before editor
        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('f') {
            self.search_query.clear();
            self.search_cursor_before = self.editor.cursor_position();
            self.search_match_index = 0;
            self.search_match_count = 0;
            self.mode = AppMode::Search;
            return;
        }

        // Ctrl+P toggles layout mode (D-12) — intercept before editor to avoid conflict (Pitfall 2)
        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('p') {
            self.cycle_layout(); // D-16
            return;
        }

        // Ctrl+C copies selection to system clipboard (nano mode)
        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('c') {
            let text = self.editor.get_selection_text();
            if !text.is_empty() {
                if self.clipboard.write(&text).is_ok() {
                    self.status_bar.set_message("Copied to clipboard");
                } else {
                    self.status_bar.set_message("Clipboard unavailable");
                }
                self.editor.textarea_mut().cancel_selection();
            }
            return;
        }

        // Ctrl+V pastes from system clipboard (nano mode)
        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('v') {
            if let Ok(text) = self.clipboard.read() {
                if !text.is_empty() {
                    self.editor.textarea_mut().insert_str(&text);
                    self.mark_content_changed();
                }
            }
            return;
        }

        // Preview-focused or preview-only: scroll keys drive the preview (D-13, D-14)
        if self.preview_focus_intercept(key) {
            return;
        }

        if let Some(action) = self.editor.handle_key(key) {
            match action {
                EditorAction::Save => {
                    if self.editor.filepath().is_some() {
                        self.do_save();
                    } else {
                        // No filepath — prompt for filename
                        self.filename_input = String::new();
                        self.quit_after_save = false;
                        self.mode = AppMode::PromptFilename;
                    }
                }
                EditorAction::RequestQuit => {
                    if self.editor.is_modified() {
                        self.mode = AppMode::ConfirmQuit;
                    } else {
                        self.should_quit = true;
                    }
                }
                EditorAction::ContentChanged => self.mark_content_changed(),
            }
        }
    }

    /// Return to the appropriate editing mode (Normal for vim, Editing for nano).
    fn return_to_editing_mode(&mut self) {
        if self.vim_handler.is_some() {
            self.mode = AppMode::Normal;
        } else {
            self.mode = AppMode::Editing;
        }
    }

    /// Global key intercepts that work in any editing surface: the help
    /// overlay (modal while open), F1 help, and F2 settings.
    /// Returns true when the key was consumed.
    fn handle_global_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        if self.show_help {
            match key.code {
                KeyCode::F(1) | KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                    self.show_help = false;
                }
                _ => {}
            }
            return true; // modal: swallow everything else while help is open
        }
        match key.code {
            KeyCode::F(1)
                if matches!(
                    self.mode,
                    AppMode::Editing | AppMode::Normal | AppMode::Insert | AppMode::Visual
                ) =>
            {
                self.show_help = true;
                true
            }
            KeyCode::F(2) if matches!(self.mode, AppMode::Editing | AppMode::Normal) => {
                self.settings_row = SettingsRow::Theme;
                self.mode = AppMode::Settings;
                true
            }
            _ => false,
        }
    }

    /// Cycle the layout mode and move focus to the pane that remains visible.
    fn cycle_layout(&mut self) {
        self.layout_mode = self.layout_mode.next();
        self.focused_pane = match self.layout_mode {
            LayoutMode::PreviewOnly => FocusedPane::Preview,
            _ => FocusedPane::Editor,
        };
        self.status_bar.set_message(self.layout_mode.label());
    }

    /// Ask to quit: prompt about unsaved changes, otherwise quit immediately.
    fn request_quit(&mut self) {
        if self.editor.is_modified() {
            self.mode = AppMode::ConfirmQuit;
        } else {
            self.should_quit = true;
        }
    }

    /// When the preview pane is focused (or in preview-only layout), scroll
    /// keys drive the preview instead of the editor. Returns true when the
    /// key was consumed. Any other key hands focus back to the editor and
    /// falls through to normal handling (restoring split view from
    /// preview-only, matching the pre-existing behavior).
    fn preview_focus_intercept(&mut self, key: crossterm::event::KeyEvent) -> bool {
        let preview_active = self.layout_mode == LayoutMode::PreviewOnly
            || (self.layout_mode == LayoutMode::Split && self.focused_pane == FocusedPane::Preview);
        if !preview_active {
            return false;
        }

        let refocus_editor = |app: &mut Self| {
            app.focused_pane = FocusedPane::Editor;
            if app.layout_mode == LayoutMode::PreviewOnly {
                app.layout_mode = LayoutMode::Split;
            }
        };

        // Ctrl/Alt chords are app-level (layout toggle, etc.) — hand focus
        // back and let the mode handler deal with them.
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            refocus_editor(self);
            return false;
        }

        // Letters are text in Insert/nano modes — only vim Normal/Visual
        // treat j/k/g/G as preview scroll keys.
        let letters = matches!(self.mode, AppMode::Normal | AppMode::Visual);
        let page = self.preview_area.map(|a| a.height).unwrap_or(20);
        match key.code {
            KeyCode::Up => {
                self.preview.scroll_up(1);
                true
            }
            KeyCode::Down => {
                self.preview.scroll_down(1);
                true
            }
            KeyCode::Char('k') if letters => {
                self.preview.scroll_up(1);
                true
            }
            KeyCode::Char('j') if letters => {
                self.preview.scroll_down(1);
                true
            }
            KeyCode::PageUp => {
                self.preview.scroll_up(page);
                true
            }
            KeyCode::PageDown => {
                self.preview.scroll_down(page);
                true
            }
            KeyCode::Home => {
                self.preview.set_scroll(0);
                true
            }
            KeyCode::Char('g') if letters => {
                self.preview.set_scroll(0);
                true
            }
            KeyCode::End => {
                // Render clamps and persists this to the true bottom.
                self.preview.set_scroll(usize::MAX);
                true
            }
            KeyCode::Char('G') if letters => {
                self.preview.set_scroll(usize::MAX);
                true
            }
            KeyCode::Esc => {
                refocus_editor(self);
                true
            }
            _ => {
                refocus_editor(self);
                false
            }
        }
    }

    /// Handle key events in the "Unsaved changes" confirmation prompt (D-13).
    fn handle_confirm_quit_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if self.editor.filepath().is_some() {
                    // Save and quit
                    if self.do_save() {
                        self.should_quit = true;
                    } else {
                        // Save failed — return to editing
                        self.return_to_editing_mode();
                    }
                } else {
                    // No filepath — prompt for filename, then quit after save
                    self.filename_input = String::new();
                    self.quit_after_save = true;
                    self.mode = AppMode::PromptFilename;
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                // Quit without saving
                self.should_quit = true;
            }
            KeyCode::Esc => {
                // Cancel — return to editing
                self.return_to_editing_mode();
            }
            _ => {
                // Ignore other keys in this mode
            }
        }
    }

    /// Handle key events in the filename prompt (for untitled buffers).
    fn handle_prompt_filename_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                if !self.filename_input.is_empty() {
                    let path = PathBuf::from(&self.filename_input);
                    self.editor.set_filepath(path);
                    self.filename_input.clear();

                    // Now save with the new filepath
                    if self.do_save() {
                        if self.quit_after_save {
                            self.should_quit = true;
                        } else {
                            self.return_to_editing_mode();
                        }
                    } else {
                        // Save failed — return to editing
                        self.return_to_editing_mode();
                    }
                    self.quit_after_save = false;
                }
            }
            KeyCode::Esc => {
                self.filename_input.clear();
                self.quit_after_save = false;
                self.return_to_editing_mode();
            }
            KeyCode::Backspace => {
                self.filename_input.pop();
            }
            KeyCode::Char(c) => {
                self.filename_input.push(c);
            }
            _ => {}
        }
    }

    /// Handle key events in search mode (D-05 through D-11).
    fn handle_search_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                // Exit search, restore cursor to pre-search position (D-07)
                let (row, col) = self.search_cursor_before;
                self.editor
                    .textarea_mut()
                    .move_cursor(CursorMove::Jump(row as u16, col as u16));
                // Clear search pattern to prevent ghost highlights (Pitfall 4)
                let _ = self.editor.textarea_mut().set_search_pattern("");
                self.search_query.clear();
                self.return_to_editing_mode();
            }
            // Vim: Enter accepts the search and returns to Normal mode, where
            // n / N step through matches (highlights stay until Esc).
            KeyCode::Enter if self.vim_handler.is_some() => {
                if self.search_match_count == 0 {
                    if !self.search_query.is_empty() {
                        self.status_bar
                            .set_message(&format!("Pattern not found: {}", self.search_query));
                    }
                    let _ = self.editor.textarea_mut().set_search_pattern("");
                    self.search_query.clear();
                }
                self.mode = AppMode::Normal;
            }
            KeyCode::Enter => {
                if self.search_match_count > 0 {
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        // Shift+Enter: previous match (D-07)
                        self.editor.textarea_mut().search_back(false);
                        self.search_match_index = if self.search_match_index == 0 {
                            self.search_match_count - 1
                        } else {
                            self.search_match_index - 1
                        };
                    } else {
                        // Enter: next match (D-07)
                        self.editor.textarea_mut().search_forward(false);
                        self.search_match_index =
                            (self.search_match_index + 1) % self.search_match_count;
                    }
                    // Update cursor_before so Esc now stays at this match (D-07)
                    self.search_cursor_before = self.editor.cursor_position();
                }
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.update_search_pattern();
            }
            KeyCode::Char(c) => {
                self.search_query.push(c);
                self.update_search_pattern();
            }
            _ => {}
        }
    }

    /// Update the search pattern on the textarea and recount matches (D-06, D-09).
    fn update_search_pattern(&mut self) {
        if self.search_query.is_empty() {
            let _ = self.editor.textarea_mut().set_search_pattern("");
            self.search_match_count = 0;
            self.search_match_index = 0;
            return;
        }
        // Case-insensitive plain text search (D-09)
        let pattern = format!("(?i){}", regex::escape(&self.search_query));
        let _ = self.editor.textarea_mut().set_search_pattern(&pattern);

        // Count matches across all lines (D-10)
        if let Ok(re) = Regex::new(&pattern) {
            let content = self.editor.content();
            self.search_match_count = re.find_iter(&content).count();
        } else {
            self.search_match_count = 0;
        }
        self.search_match_index = 0;

        // Jump to first match from current position (D-06)
        if self.search_match_count > 0 {
            self.editor.textarea_mut().search_forward(true);
        }
    }

    /// Jump to the next search match (for n in normal mode).
    fn find_next_match(&mut self) {
        if self.search_match_count > 0 {
            self.editor.textarea_mut().search_forward(false);
            self.search_match_index = (self.search_match_index + 1) % self.search_match_count;
            self.status_bar.set_message(&format!(
                "/{} [{}/{}]",
                self.search_query,
                self.search_match_index + 1,
                self.search_match_count
            ));
        }
    }

    /// Jump to the previous search match (for N in normal mode).
    fn find_prev_match(&mut self) {
        if self.search_match_count > 0 {
            self.editor.textarea_mut().search_back(false);
            self.search_match_index = if self.search_match_index == 0 {
                self.search_match_count - 1
            } else {
                self.search_match_index - 1
            };
            self.status_bar.set_message(&format!(
                "?{} [{}/{}]",
                self.search_query,
                self.search_match_index + 1,
                self.search_match_count
            ));
        }
    }

    /// Return the current search query if in search mode, empty string otherwise.
    pub fn current_search_query(&self) -> &str {
        &self.search_query
    }

    /// Write text to both the internal vim yank register and the system clipboard.
    /// Shows a one-time warning if clipboard is unavailable.
    fn yank_to_clipboard(&mut self, text: &str) {
        if let Some(ref mut handler) = self.vim_handler {
            handler.set_yank_register(text.to_string());
        }
        if let Err(_e) = self.clipboard.write(text) {
            if !self.clipboard_warned {
                self.status_bar
                    .set_message("System clipboard unavailable - using internal register");
                self.clipboard_warned = true;
            }
        }
    }

    /// Mark content as changed (sets dirty flag and edit timestamp).
    fn mark_content_changed(&mut self) {
        self.editor.mark_modified();
        self.word_count = None;
        self.content_dirty = true;
        self.last_edit_time = Some(Instant::now());
    }

    /// Convert a Motion to a CursorMove for textarea operations.
    fn motion_to_cursor_move(motion: &Motion) -> CursorMove {
        match motion {
            Motion::Left => CursorMove::Back,
            Motion::Right => CursorMove::Forward,
            Motion::Up => CursorMove::Up,
            Motion::Down => CursorMove::Down,
            Motion::WordForward | Motion::WordEnd => CursorMove::WordForward,
            Motion::WordBack => CursorMove::WordBack,
            Motion::LineStart => CursorMove::Head,
            Motion::LineEnd => CursorMove::End,
            Motion::FileStart => CursorMove::Top,
            Motion::FileEnd => CursorMove::Bottom,
            Motion::ParagraphUp => CursorMove::ParagraphBack,
            Motion::ParagraphDown => CursorMove::ParagraphForward,
            Motion::Line | Motion::ToEnd | Motion::ToStart => CursorMove::Head, // handled specially
        }
    }

    /// Execute a VimCommand by translating it to textarea/editor operations.
    fn execute_vim_command(&mut self, cmd: VimCommand) {
        match cmd {
            VimCommand::Move(cursor_cmd) => {
                self.editor
                    .textarea_mut()
                    .move_cursor(cursor_cmd.to_cursor_move());
            }
            VimCommand::MoveN(cursor_cmd, n) => {
                for _ in 0..n {
                    self.editor
                        .textarea_mut()
                        .move_cursor(cursor_cmd.to_cursor_move());
                }
            }
            VimCommand::Delete { motion } => {
                self.execute_vim_operator_delete(motion);
            }
            VimCommand::Change { motion } => {
                self.execute_vim_operator_change(motion);
            }
            VimCommand::Yank { motion } => {
                self.execute_vim_operator_yank(motion);
            }
            VimCommand::DeleteChar => {
                self.editor.textarea_mut().delete_next_char();
                self.mark_content_changed();
            }
            VimCommand::PasteAfter => {
                let text = match self.clipboard.read() {
                    Ok(clip_text) if !clip_text.is_empty() => clip_text,
                    _ => self
                        .vim_handler
                        .as_ref()
                        .map(|h| h.yank_register().to_string())
                        .unwrap_or_default(),
                };
                if !text.is_empty() {
                    if text.ends_with('\n') {
                        // Line-wise paste: paste on next line
                        self.editor.textarea_mut().move_cursor(CursorMove::End);
                        self.editor.textarea_mut().insert_newline();
                        let trimmed = text.trim_end_matches('\n');
                        self.editor.textarea_mut().insert_str(trimmed);
                    } else {
                        self.editor.textarea_mut().move_cursor(CursorMove::Forward);
                        self.editor.textarea_mut().insert_str(&text);
                    }
                    self.mark_content_changed();
                }
            }
            VimCommand::PasteBefore => {
                let text = match self.clipboard.read() {
                    Ok(clip_text) if !clip_text.is_empty() => clip_text,
                    _ => self
                        .vim_handler
                        .as_ref()
                        .map(|h| h.yank_register().to_string())
                        .unwrap_or_default(),
                };
                if !text.is_empty() {
                    if text.ends_with('\n') {
                        // Line-wise paste: paste on previous line
                        self.editor.textarea_mut().move_cursor(CursorMove::Head);
                        self.editor.textarea_mut().insert_newline();
                        self.editor.textarea_mut().move_cursor(CursorMove::Up);
                        let trimmed = text.trim_end_matches('\n');
                        self.editor.textarea_mut().insert_str(trimmed);
                    } else {
                        self.editor.textarea_mut().insert_str(&text);
                    }
                    self.mark_content_changed();
                }
            }
            VimCommand::EnterInsert(pos) => {
                match pos {
                    InsertPosition::BeforeCursor => {}
                    InsertPosition::AfterCursor => {
                        self.editor.textarea_mut().move_cursor(CursorMove::Forward);
                    }
                    InsertPosition::LineStart => {
                        self.editor.textarea_mut().move_cursor(CursorMove::Head);
                    }
                    InsertPosition::LineEnd => {
                        self.editor.textarea_mut().move_cursor(CursorMove::End);
                    }
                    InsertPosition::NewLineBelow => {
                        self.editor.textarea_mut().move_cursor(CursorMove::End);
                        self.editor.textarea_mut().insert_newline();
                        self.mark_content_changed();
                    }
                    InsertPosition::NewLineAbove => {
                        self.editor.textarea_mut().move_cursor(CursorMove::Head);
                        self.editor.textarea_mut().insert_newline();
                        self.editor.textarea_mut().move_cursor(CursorMove::Up);
                        self.mark_content_changed();
                    }
                }
                self.mode = AppMode::Insert;
            }
            VimCommand::ExitInsert => {
                self.mode = AppMode::Normal;
            }
            VimCommand::EnterVisual { line_wise } => {
                if line_wise {
                    // Line-wise: select from start of current line
                    self.editor.textarea_mut().move_cursor(CursorMove::Head);
                    self.editor.textarea_mut().start_selection();
                    self.editor.textarea_mut().move_cursor(CursorMove::End);
                } else {
                    self.editor.textarea_mut().start_selection();
                }
                self.mode = AppMode::Visual;
            }
            VimCommand::ExitVisual => {
                self.editor.textarea_mut().cancel_selection();
                self.mode = AppMode::Normal;
            }
            VimCommand::EnterCommand => {
                self.mode = AppMode::Command;
            }
            VimCommand::ExitCommand => {
                self.mode = AppMode::Normal;
            }
            VimCommand::CommandExecute(ref cmd_str) => {
                let cmd_str = cmd_str.clone();
                self.execute_ex_command(&cmd_str);
                // Only reset to Normal if the command didn't switch to another mode
                if self.mode == AppMode::Command {
                    self.mode = AppMode::Normal;
                }
            }
            VimCommand::Save => {
                if self.editor.filepath().is_some() {
                    self.do_save();
                    self.mode = AppMode::Normal;
                } else {
                    self.filename_input = String::new();
                    self.quit_after_save = false;
                    self.mode = AppMode::PromptFilename;
                }
            }
            VimCommand::Quit { force } => {
                if force || !self.editor.is_modified() {
                    self.should_quit = true;
                } else {
                    self.mode = AppMode::ConfirmQuit;
                }
            }
            VimCommand::SaveAndQuit => {
                if self.editor.filepath().is_some() {
                    if self.do_save() {
                        self.should_quit = true;
                    }
                } else {
                    self.filename_input = String::new();
                    self.quit_after_save = true;
                    self.mode = AppMode::PromptFilename;
                }
            }
            VimCommand::EnterSearch => {
                self.search_query.clear();
                self.search_cursor_before = self.editor.cursor_position();
                self.search_match_index = 0;
                self.search_match_count = 0;
                self.mode = AppMode::Search;
            }
            VimCommand::Undo => {
                self.editor.textarea_mut().undo();
                self.mark_content_changed();
            }
            VimCommand::Redo => {
                self.editor.textarea_mut().redo();
                self.mark_content_changed();
            }
            VimCommand::ContentChanged => {
                self.mark_content_changed();
            }
            VimCommand::VisualDelete => {
                let line_wise = self
                    .vim_handler
                    .as_ref()
                    .map(|h| h.was_visual_line_wise())
                    .unwrap_or(false);
                if line_wise {
                    // Line-wise: expand selection to full lines, then cut
                    if let Some(((sr, _), (er, _))) = self.editor.textarea_mut().selection_range() {
                        self.editor.textarea_mut().cancel_selection();
                        // Select full lines sr..=er
                        let mut yanked = String::new();
                        let lines = self.editor.textarea_mut().lines().to_vec();
                        for r in sr..=er {
                            if r < lines.len() {
                                yanked.push_str(&lines[r]);
                                yanked.push('\n');
                            }
                        }
                        self.yank_to_clipboard(&yanked);
                        // Delete lines from bottom to top to avoid index shift
                        self.editor
                            .textarea_mut()
                            .move_cursor(CursorMove::Jump(sr as u16, 0));
                        for _ in sr..=er {
                            self.editor.delete_current_line();
                        }
                    }
                } else {
                    // Char-wise: yank selected text then cut
                    let text = self.editor.get_selection_text();
                    self.yank_to_clipboard(&text);
                    self.editor.textarea_mut().cut();
                }
                self.editor.textarea_mut().cancel_selection();
                self.mark_content_changed();
                self.mode = AppMode::Normal;
            }
            VimCommand::VisualChange => {
                let line_wise = self
                    .vim_handler
                    .as_ref()
                    .map(|h| h.was_visual_line_wise())
                    .unwrap_or(false);
                if line_wise {
                    if let Some(((sr, _), (er, _))) = self.editor.textarea_mut().selection_range() {
                        self.editor.textarea_mut().cancel_selection();
                        let mut yanked = String::new();
                        let lines = self.editor.textarea_mut().lines().to_vec();
                        for r in sr..=er {
                            if r < lines.len() {
                                yanked.push_str(&lines[r]);
                                yanked.push('\n');
                            }
                        }
                        self.yank_to_clipboard(&yanked);
                        // Delete all selected lines, then leave an empty line for insert
                        self.editor
                            .textarea_mut()
                            .move_cursor(CursorMove::Jump(sr as u16, 0));
                        // Delete content of first line, then remaining lines
                        self.editor.delete_current_line_content();
                        for _ in (sr + 1)..=er {
                            // Delete the line below (now at position sr+1, but after deletion it shifts)
                            self.editor.textarea_mut().move_cursor(CursorMove::End);
                            self.editor.textarea_mut().delete_next_char(); // delete newline joining next line
                            self.editor.textarea_mut().start_selection();
                            self.editor.textarea_mut().move_cursor(CursorMove::End);
                            self.editor.textarea_mut().cut();
                        }
                    }
                } else {
                    let text = self.editor.get_selection_text();
                    self.yank_to_clipboard(&text);
                    self.editor.textarea_mut().cut();
                }
                self.editor.textarea_mut().cancel_selection();
                self.mark_content_changed();
                self.mode = AppMode::Insert;
                if let Some(handler) = self.vim_handler.as_mut() {
                    handler.set_mode_insert();
                }
            }
            VimCommand::VisualYank => {
                let text = self.editor.get_selection_text();
                self.yank_to_clipboard(&text);
                self.editor.textarea_mut().cancel_selection();
                self.mode = AppMode::Normal;
            }
            VimCommand::VisualIndent => {
                if let Some(((sr, _), (er, _))) = self.editor.textarea_mut().selection_range() {
                    self.editor.textarea_mut().cancel_selection();
                    for row in sr..=er {
                        self.editor
                            .textarea_mut()
                            .move_cursor(CursorMove::Jump(row as u16, 0));
                        self.editor.textarea_mut().insert_str("  ");
                    }
                    // Re-select the range for continued visual mode operations
                    self.editor
                        .textarea_mut()
                        .move_cursor(CursorMove::Jump(sr as u16, 0));
                    self.editor.textarea_mut().start_selection();
                    self.editor
                        .textarea_mut()
                        .move_cursor(CursorMove::Jump(er as u16, 0));
                    self.editor.textarea_mut().move_cursor(CursorMove::End);
                    self.mark_content_changed();
                }
            }
            VimCommand::VisualOutdent => {
                if let Some(((sr, _), (er, _))) = self.editor.textarea_mut().selection_range() {
                    self.editor.textarea_mut().cancel_selection();
                    for row in sr..=er {
                        let line = self.editor.textarea_mut().lines()[row].clone();
                        let spaces = line.chars().take(2).take_while(|c| *c == ' ').count();
                        if spaces > 0 {
                            self.editor
                                .textarea_mut()
                                .move_cursor(CursorMove::Jump(row as u16, 0));
                            for _ in 0..spaces {
                                self.editor.textarea_mut().delete_next_char();
                            }
                        }
                    }
                    // Re-select the range for continued visual mode operations
                    self.editor
                        .textarea_mut()
                        .move_cursor(CursorMove::Jump(sr as u16, 0));
                    self.editor.textarea_mut().start_selection();
                    self.editor
                        .textarea_mut()
                        .move_cursor(CursorMove::Jump(er as u16, 0));
                    self.editor.textarea_mut().move_cursor(CursorMove::End);
                    self.mark_content_changed();
                }
            }
            VimCommand::PageDown => {
                let page_size = self.editor_area.map(|a| a.height as usize).unwrap_or(20);
                for _ in 0..page_size {
                    self.editor.textarea_mut().move_cursor(CursorMove::Down);
                }
            }
            VimCommand::PageUp => {
                let page_size = self.editor_area.map(|a| a.height as usize).unwrap_or(20);
                for _ in 0..page_size {
                    self.editor.textarea_mut().move_cursor(CursorMove::Up);
                }
            }
            VimCommand::HalfPageDown => {
                let half = self
                    .editor_area
                    .map(|a| a.height as usize / 2)
                    .unwrap_or(10);
                for _ in 0..half {
                    self.editor.textarea_mut().move_cursor(CursorMove::Down);
                }
            }
            VimCommand::HalfPageUp => {
                let half = self
                    .editor_area
                    .map(|a| a.height as usize / 2)
                    .unwrap_or(10);
                for _ in 0..half {
                    self.editor.textarea_mut().move_cursor(CursorMove::Up);
                }
            }
            VimCommand::ReplaceChar(ch) => {
                self.editor.textarea_mut().delete_next_char();
                self.editor.textarea_mut().insert_char(ch);
                self.editor.textarea_mut().move_cursor(CursorMove::Back);
                self.mark_content_changed();
            }
            VimCommand::JoinLines => {
                let (row, _) = self.editor.textarea_mut().cursor();
                let total = self.editor.textarea_mut().lines().len();
                if row + 1 < total {
                    // Move to end of current line, delete newline, insert space
                    self.editor.textarea_mut().move_cursor(CursorMove::End);
                    self.editor.textarea_mut().delete_next_char(); // delete the newline
                                                                   // Remove leading whitespace from the joined line
                    loop {
                        let col = self.editor.textarea_mut().cursor().1;
                        let ch = self
                            .editor
                            .textarea_mut()
                            .lines()
                            .get(row)
                            .and_then(|l| l.chars().nth(col));
                        match ch {
                            Some(' ') | Some('\t') => {
                                self.editor.textarea_mut().delete_next_char();
                            }
                            _ => break,
                        }
                    }
                    self.editor.textarea_mut().insert_char(' ');
                    self.mark_content_changed();
                }
            }
            VimCommand::SearchNext => {
                if !self.search_query.is_empty() {
                    self.find_next_match();
                }
            }
            VimCommand::SearchPrev => {
                if !self.search_query.is_empty() {
                    self.find_prev_match();
                }
            }
            VimCommand::None | VimCommand::CommandAppend(_) | VimCommand::CommandBackspace => {}
        }
    }

    /// Execute a delete operator with a motion.
    fn execute_vim_operator_delete(&mut self, motion: Motion) {
        match motion {
            Motion::Line => {
                // dd: delete current line
                let text = self.editor.delete_current_line();
                self.yank_to_clipboard(&text);
                self.mark_content_changed();
            }
            Motion::ToEnd => {
                // D / d$: select to end of line, delete
                self.editor.textarea_mut().start_selection();
                self.editor.textarea_mut().move_cursor(CursorMove::End);
                let text = self.editor.cut_selection();
                self.yank_to_clipboard(&text);
                self.mark_content_changed();
            }
            Motion::ToStart => {
                // d0: select to start of line, delete
                self.editor.textarea_mut().start_selection();
                self.editor.textarea_mut().move_cursor(CursorMove::Head);
                let text = self.editor.cut_selection();
                self.yank_to_clipboard(&text);
                self.mark_content_changed();
            }
            other => {
                let cursor_move = Self::motion_to_cursor_move(&other);
                self.editor.textarea_mut().start_selection();
                self.editor.textarea_mut().move_cursor(cursor_move);
                let text = self.editor.cut_selection();
                self.yank_to_clipboard(&text);
                self.mark_content_changed();
            }
        }
    }

    /// Execute a change operator with a motion (delete + enter insert mode).
    fn execute_vim_operator_change(&mut self, motion: Motion) {
        match motion {
            Motion::Line => {
                // cc: delete line content but keep the line, enter insert
                let text = self.editor.delete_current_line_content();
                self.yank_to_clipboard(&text);
                self.mark_content_changed();
                self.mode = AppMode::Insert;
                if let Some(handler) = self.vim_handler.as_mut() {
                    handler.set_mode_insert();
                }
            }
            Motion::ToEnd => {
                self.editor.textarea_mut().start_selection();
                self.editor.textarea_mut().move_cursor(CursorMove::End);
                let text = self.editor.cut_selection();
                self.yank_to_clipboard(&text);
                self.mark_content_changed();
                self.mode = AppMode::Insert;
                if let Some(handler) = self.vim_handler.as_mut() {
                    handler.set_mode_insert();
                }
            }
            other => {
                let cursor_move = Self::motion_to_cursor_move(&other);
                self.editor.textarea_mut().start_selection();
                self.editor.textarea_mut().move_cursor(cursor_move);
                let text = self.editor.cut_selection();
                self.yank_to_clipboard(&text);
                self.mark_content_changed();
                self.mode = AppMode::Insert;
                if let Some(handler) = self.vim_handler.as_mut() {
                    handler.set_mode_insert();
                }
            }
        }
    }

    /// Execute a yank operator with a motion (copy without deleting).
    fn execute_vim_operator_yank(&mut self, motion: Motion) {
        match motion {
            Motion::Line => {
                // yy: yank current line
                let text = self.editor.yank_current_line();
                self.yank_to_clipboard(&text);
            }
            Motion::ToEnd => {
                self.editor.textarea_mut().start_selection();
                self.editor.textarea_mut().move_cursor(CursorMove::End);
                let text = self.editor.get_selection_text();
                self.editor.textarea_mut().cancel_selection();
                self.yank_to_clipboard(&text);
            }
            other => {
                let cursor_move = Self::motion_to_cursor_move(&other);
                self.editor.textarea_mut().start_selection();
                self.editor.textarea_mut().move_cursor(cursor_move);
                let text = self.editor.get_selection_text();
                self.editor.textarea_mut().cancel_selection();
                self.yank_to_clipboard(&text);
            }
        }
    }

    /// Execute an ex command (typed after ':').
    fn execute_ex_command(&mut self, cmd: &str) {
        match cmd.trim() {
            "w" => {
                if self.editor.filepath().is_some() {
                    self.do_save();
                } else {
                    self.filename_input = String::new();
                    self.quit_after_save = false;
                    self.mode = AppMode::PromptFilename;
                }
            }
            "q" => {
                if self.editor.is_modified() {
                    self.status_bar
                        .set_message("No write since last change (add ! to override)");
                } else {
                    self.should_quit = true;
                }
            }
            "q!" => {
                self.should_quit = true;
            }
            "wq" | "x" => {
                if self.editor.filepath().is_some() {
                    if self.do_save() {
                        self.should_quit = true;
                    }
                } else {
                    self.filename_input = String::new();
                    self.quit_after_save = true;
                    self.mode = AppMode::PromptFilename;
                }
            }
            "help" | "h" => {
                self.show_help = true;
            }
            "noh" | "nohlsearch" => {
                let _ = self.editor.textarea_mut().set_search_pattern("");
                self.search_query.clear();
            }
            other if other.starts_with("set ") || other == "set" => {
                self.execute_set_command(other);
            }
            other => {
                // Try parsing as a line number (e.g. :42)
                if let Ok(line_num) = other.trim().parse::<usize>() {
                    let target = if line_num == 0 { 0 } else { line_num - 1 };
                    let max_line = self.editor.textarea_mut().lines().len().saturating_sub(1);
                    let target = target.min(max_line);
                    self.editor
                        .textarea_mut()
                        .move_cursor(ratatui_textarea::CursorMove::Jump(target as u16, 0));
                } else {
                    self.status_bar
                        .set_message(&format!("Not a command: {}", other));
                }
            }
        }
    }

    /// Handle :set commands for live configuration.
    ///
    /// Supported:
    ///   :set                    -- show current settings
    ///   :set theme <name>       -- switch theme (ocean, dracula, solarized-light, gruvbox-dark)
    ///   :set mode <vim|nano>    -- switch editing mode (takes effect on next launch)
    ///   :set save               -- persist current settings to config file
    fn execute_set_command(&mut self, cmd: &str) {
        let args: Vec<&str> = cmd.split_whitespace().collect();
        match args.as_slice() {
            // :set -- open settings panel
            ["set"] => {
                self.settings_row = SettingsRow::Theme;
                self.mode = AppMode::Settings;
            }
            // :set theme <name>
            ["set", "theme", name] => {
                let name = name.to_lowercase();
                if let Some(new_theme) = crate::config::resolve_theme_name(&self.config, &name) {
                    let cap = crate::theme::detect_color_capability();
                    let applied = if cap == crate::theme::ColorCapability::Color256 {
                        new_theme.with_256_color_fallback()
                    } else {
                        new_theme
                    };
                    self.editor.apply_theme(applied.clone());
                    self.theme = applied;
                    self.config.theme = name.clone();
                    self.content_dirty = true;
                    self.status_bar
                        .set_message(&format!("Theme set to {}", name));
                } else {
                    let available = crate::config::available_theme_names(&self.config).join(", ");
                    self.status_bar.set_message(&format!(
                        "Unknown theme '{}'. Available: {}",
                        name, available
                    ));
                }
            }
            // :set mode <vim|nano>
            ["set", "mode", mode] => match mode.to_lowercase().as_str() {
                "vim" => {
                    self.config.mode = crate::config::EditingMode::Vim;
                    self.status_bar
                        .set_message("Mode set to vim (restart to apply)");
                }
                "nano" => {
                    self.config.mode = crate::config::EditingMode::Nano;
                    self.status_bar
                        .set_message("Mode set to nano (restart to apply)");
                }
                other => {
                    self.status_bar
                        .set_message(&format!("Unknown mode '{}'. Available: vim, nano", other));
                }
            },
            // :set profile <name>
            ["set", "profile", name] => {
                if let Some(profile) = crate::config::RenderProfile::from_name(name) {
                    self.config.render_profile = profile;
                    self.settings_profile_index = crate::config::RenderProfile::all()
                        .iter()
                        .position(|p| *p == profile)
                        .unwrap_or(0);
                    self.content_dirty = true;
                    self.status_bar.set_message(&format!(
                        "Preview profile set to {}",
                        profile.display_name()
                    ));
                } else {
                    let available: Vec<&str> = crate::config::RenderProfile::all()
                        .iter()
                        .map(|p| p.name())
                        .collect();
                    self.status_bar.set_message(&format!(
                        "Unknown profile '{}'. Available: {}",
                        name,
                        available.join(", ")
                    ));
                }
            }
            // :set wrap -- toggle word wrap
            ["set", "wrap"] => {
                self.config.word_wrap = !self.config.word_wrap;
                self.editor.word_wrap = self.config.word_wrap;
                let state = if self.config.word_wrap { "on" } else { "off" };
                self.status_bar.set_message(&format!("Word wrap {}", state));
            }
            // :set save -- persist to config file
            ["set", "save"] => match crate::config::save_config(&self.config) {
                Ok(path) => {
                    self.status_bar
                        .set_message(&format!("Config saved to {}", path));
                }
                Err(e) => {
                    self.status_bar
                        .set_message(&format!("Failed to save: {}", e));
                }
            },
            _ => {
                self.status_bar.set_message(
                    "Usage: :set [theme <name>|mode <vim|nano>|profile <..>|wrap|save]",
                );
            }
        }
    }

    /// Handle key events in Vim Normal mode.
    fn handle_vim_key(&mut self, key: crossterm::event::KeyEvent) {
        // Ctrl+P toggles layout mode -- intercept before vim handler
        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('p') {
            self.cycle_layout();
            return;
        }

        // Ctrl+Q requests quit (matches the title-bar button hint)
        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('q') {
            self.request_quit();
            return;
        }

        // Preview-focused or preview-only: scroll keys drive the preview
        if self.preview_focus_intercept(key) {
            return;
        }

        // Esc in Normal mode clears lingering search highlights (like :noh)
        if key.code == KeyCode::Esc && !self.search_query.is_empty() {
            let _ = self.editor.textarea_mut().set_search_pattern("");
            self.search_query.clear();
        }

        let cmd = self
            .vim_handler
            .as_mut()
            .map(|h| h.handle_key(key))
            .unwrap_or(VimCommand::None);
        self.execute_vim_command(cmd);
    }

    /// Handle key events in Vim Insert mode.
    fn handle_vim_insert_key(&mut self, key: crossterm::event::KeyEvent) {
        // If the preview was focused by mouse, scroll keys drive it
        if self.preview_focus_intercept(key) {
            return;
        }

        let cmd = self
            .vim_handler
            .as_mut()
            .map(|h| h.handle_key(key))
            .unwrap_or(VimCommand::None);

        match cmd {
            VimCommand::ExitInsert => {
                self.execute_vim_command(cmd);
            }
            VimCommand::None => {
                // Forward all non-Esc keys to textarea for text input
                let changed = self.editor.textarea_mut().input_without_shortcuts(key);
                if changed {
                    self.mark_content_changed();
                }
            }
            _ => {
                self.execute_vim_command(cmd);
            }
        }
    }

    /// Handle key events in Vim Visual mode.
    fn handle_vim_visual_key(&mut self, key: crossterm::event::KeyEvent) {
        // Ctrl+P toggles layout mode
        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('p') {
            self.cycle_layout();
            return;
        }

        // Preview-focused: scroll keys drive the preview
        if self.preview_focus_intercept(key) {
            return;
        }

        let cmd = self
            .vim_handler
            .as_mut()
            .map(|h| h.handle_key(key))
            .unwrap_or(VimCommand::None);

        match cmd {
            // Motions extend selection (selection is active, just move cursor)
            VimCommand::Move(cursor_cmd) => {
                self.editor
                    .textarea_mut()
                    .move_cursor(cursor_cmd.to_cursor_move());
            }
            VimCommand::MoveN(cursor_cmd, n) => {
                for _ in 0..n {
                    self.editor
                        .textarea_mut()
                        .move_cursor(cursor_cmd.to_cursor_move());
                }
            }
            // All other commands (ExitVisual, VisualDelete, etc.) go through execute
            VimCommand::None => {}
            other => {
                self.execute_vim_command(other);
            }
        }
    }

    /// Handle key events in Vim Command mode.
    fn handle_vim_command_key(&mut self, key: crossterm::event::KeyEvent) {
        let cmd = self
            .vim_handler
            .as_mut()
            .map(|h| h.handle_key(key))
            .unwrap_or(VimCommand::None);
        self.execute_vim_command(cmd);
    }

    /// Handle mouse events: click, scroll, drag-select, divider drag (D-19 through D-23).
    fn handle_mouse_event(&mut self, mouse: crossterm::event::MouseEvent) {
        use crossterm::event::{MouseButton, MouseEventKind};

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let col = mouse.column;
                let row = mouse.row;

                // Any click dismisses the help overlay (btop menu behavior)
                if self.show_help {
                    self.show_help = false;
                    return;
                }

                // Title-bar buttons (only from editing surfaces, so a click
                // can't abandon an active prompt)
                if matches!(
                    self.mode,
                    AppMode::Editing | AppMode::Normal | AppMode::Insert | AppMode::Visual
                ) {
                    for (rect, button) in self.button_areas.clone() {
                        if row == rect.y && col >= rect.x && col < rect.x + rect.width {
                            match button {
                                UiButton::Help => self.show_help = true,
                                UiButton::Settings => {
                                    self.settings_row = SettingsRow::Theme;
                                    self.mode = AppMode::Settings;
                                }
                                UiButton::View => self.cycle_layout(),
                                UiButton::Quit => self.request_quit(),
                            }
                            return;
                        }
                    }
                }

                // Live view: click places the cursor (opening that block)
                if self.layout_mode == LayoutMode::Live {
                    if let Some((line, char_col)) =
                        self.live.click(col, row, &self.preview_doc, &self.editor)
                    {
                        self.editor.textarea_mut().cancel_selection();
                        self.editor
                            .textarea_mut()
                            .move_cursor(CursorMove::Jump(line as u16, char_col as u16));
                    }
                    return;
                }

                // Check if click is on divider (D-22)
                if let Some(div) = self.divider_area {
                    if col >= div.x
                        && col < div.x + div.width
                        && row >= div.y
                        && row < div.y + div.height
                    {
                        self.dragging_divider = true;
                        return;
                    }
                }

                // Check if click is in editor area (D-19)
                if let Some(editor) = self.editor_area {
                    if col >= editor.x
                        && col < editor.x + editor.width
                        && row >= editor.y
                        && row < editor.y + editor.height
                    {
                        self.focused_pane = FocusedPane::Editor;
                        self.click_to_editor_cursor(col, row, &editor);
                        return;
                    }
                }

                // Click in preview area focuses it (btop focus-follows-click)
                if let Some(preview) = self.preview_area {
                    if col >= preview.x
                        && col < preview.x + preview.width
                        && row >= preview.y
                        && row < preview.y + preview.height
                    {
                        self.focused_pane = FocusedPane::Preview;
                    }
                }
            }

            MouseEventKind::Drag(MouseButton::Left) => {
                let col = mouse.column;
                let row = mouse.row;

                // Divider drag (D-22)
                if self.dragging_divider {
                    if let Some(body) = self.split_body_area {
                        if body.width > 0 {
                            let relative_x = col.saturating_sub(body.x);
                            let new_ratio =
                                ((relative_x as f32 / body.width as f32) * 100.0) as u16;
                            // Clamp to reasonable bounds (min 20%, max 80%)
                            self.split_ratio = new_ratio.clamp(20, 80);
                        }
                    }
                    return;
                }

                // Text drag selection in editor (D-21)
                // Stay in current mode — don't enter Visual. Just track selection
                // internally and copy to clipboard on mouse-up.
                if let Some(editor) = self.editor_area {
                    if editor.width == 0 || editor.height == 0 {
                        return;
                    }
                    // Clamp to editor area to prevent runaway scrolling
                    let clamped_col =
                        col.clamp(editor.x, editor.x + editor.width.saturating_sub(1));
                    let clamped_row =
                        row.clamp(editor.y, editor.y + editor.height.saturating_sub(1));
                    if !self.drag_selecting {
                        self.editor.textarea_mut().start_selection();
                        self.drag_selecting = true;
                    }
                    self.click_to_editor_cursor(clamped_col, clamped_row, &editor);
                }
            }

            MouseEventKind::Up(MouseButton::Left) => {
                if self.drag_selecting {
                    // Copy selection to clipboard
                    let text = self.editor.get_selection_text();
                    if !text.is_empty() {
                        if self.vim_handler.is_some() {
                            self.yank_to_clipboard(&text);
                        } else {
                            let _ = self.clipboard.write(&text);
                        }
                        self.status_bar.set_message("Copied to clipboard");
                    }
                    self.editor.textarea_mut().cancel_selection();
                }
                self.dragging_divider = false;
                self.drag_selecting = false;
            }

            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
                if self.layout_mode == LayoutMode::Live =>
            {
                let step = if mouse.kind == MouseEventKind::ScrollUp {
                    CursorMove::Up
                } else {
                    CursorMove::Down
                };
                for _ in 0..3 {
                    self.editor.textarea_mut().move_cursor(step);
                }
            }

            MouseEventKind::ScrollUp => {
                let col = mouse.column;
                // Determine which pane to scroll (D-20)
                if let Some(editor) = self.editor_area {
                    if col >= editor.x && col < editor.x + editor.width {
                        self.focused_pane = FocusedPane::Editor;
                        // Scroll editor up by 3 lines
                        for _ in 0..3 {
                            self.editor.textarea_mut().move_cursor(CursorMove::Up);
                        }
                        return;
                    }
                }
                if let Some(preview) = self.preview_area {
                    if col >= preview.x && col < preview.x + preview.width {
                        // Focus the preview so scroll-sync doesn't undo this
                        self.focused_pane = FocusedPane::Preview;
                        self.preview.scroll_up(3);
                    }
                }
            }

            MouseEventKind::ScrollDown => {
                let col = mouse.column;
                if let Some(editor) = self.editor_area {
                    if col >= editor.x && col < editor.x + editor.width {
                        self.focused_pane = FocusedPane::Editor;
                        for _ in 0..3 {
                            self.editor.textarea_mut().move_cursor(CursorMove::Down);
                        }
                        return;
                    }
                }
                if let Some(preview) = self.preview_area {
                    if col >= preview.x && col < preview.x + preview.width {
                        // Focus the preview so scroll-sync doesn't undo this
                        self.focused_pane = FocusedPane::Preview;
                        self.preview.scroll_down(3);
                    }
                }
            }

            _ => {} // Other mouse events ignored
        }
    }

    /// Translate a screen click position to an editor cursor position (D-19).
    fn click_to_editor_cursor(&mut self, screen_col: u16, screen_row: u16, editor_area: &Rect) {
        if editor_area.width == 0 || editor_area.height == 0 {
            return;
        }

        // Calculate editor-relative position
        let relative_row = (screen_row - editor_area.y) as usize;
        let relative_col = (screen_col - editor_area.x) as usize;

        // Account for line number gutter width
        let total_lines = self.editor.line_count();
        let lnum_width = crate::highlighter::line_number_width(total_lines);

        // Calculate actual text column (subtract line number width)
        let text_col = relative_col.saturating_sub(lnum_width);

        // Convert visual click to logical position (handles word wrap)
        let content_width = (editor_area.width as usize).saturating_sub(lnum_width);
        let (clamped_row, clamped_col) =
            self.editor
                .visual_click_to_logical(relative_row, text_col, content_width);

        // Cancel any existing selection if not drag-selecting
        if !self.drag_selecting {
            self.editor.textarea_mut().cancel_selection();
        }

        self.editor
            .textarea_mut()
            .move_cursor(CursorMove::Jump(clamped_row as u16, clamped_col as u16));
    }

    /// Render the editor, preview, and status bar based on current layout mode.
    fn render(&mut self, frame: &mut Frame) {
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Fill(1), Constraint::Length(1)])
            .split(frame.area());

        let body_area = outer[0];
        let status_area = outer[1];

        let search_query = self.current_search_query().to_string();
        self.button_areas.clear();

        // Themes with their own background paint the whole screen; others
        // (ocean) sit on the terminal's background.
        if let Some(bg) = self.theme.editor_bg {
            let area = frame.area();
            frame
                .buffer_mut()
                .set_style(area, Style::default().bg(bg).fg(self.theme.preview_fg));
        }

        match self.layout_mode {
            LayoutMode::Split => {
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(self.split_ratio),
                        Constraint::Percentage(100 - self.split_ratio),
                    ])
                    .split(body_area);
                let editor_outer = chunks[0];
                let preview_outer = chunks[1];

                self.split_body_area = Some(body_area);
                // The two adjacent panel border columns double as the resize
                // handle (D-22).
                self.divider_area = Some(Rect::new(
                    preview_outer.x.saturating_sub(1),
                    body_area.y,
                    2.min(body_area.width),
                    body_area.height,
                ));

                // Editor panel (left) — with syntax highlighting (D-09)
                let editor_focused = self.focused_pane == FocusedPane::Editor;
                let editor_title = self.editor_title(editor_focused);
                let editor_inner =
                    self.render_panel(frame, editor_outer, editor_title, editor_focused, false);
                self.editor_area = Some(editor_inner);
                self.editor
                    .render_highlighted(frame, editor_inner, &search_query);
                let (total, top) = self.editor.scroll_metrics();
                self.render_scrollbar(frame, editor_outer, total, top, editor_focused);

                // Preview panel (right) — carries the title-bar buttons
                let preview_title = self.preview_title(!editor_focused);
                let preview_inner =
                    self.render_panel(frame, preview_outer, preview_title, !editor_focused, true);
                self.preview_area = Some(preview_inner);

                self.refresh_preview(preview_inner.width);

                // Scroll sync: preview follows editor cursor (D-03) — paused
                // while the preview itself has focus so manual scrolling sticks
                if editor_focused {
                    self.sync_preview_scroll(preview_inner.height);
                }

                let highlight = self.config.sync_indicator.then(|| {
                    let (cursor_row, _) = self.editor.cursor_position();
                    self.preview_doc.rows_for_source(cursor_row)
                });

                self.preview.render(
                    frame,
                    preview_inner,
                    &self.preview_doc,
                    &self.theme,
                    highlight,
                );
                self.render_scrollbar(
                    frame,
                    preview_outer,
                    self.preview_doc.rows.len(),
                    self.preview.scroll_offset(),
                    !editor_focused,
                );
            }
            LayoutMode::Live => {
                self.split_body_area = None;
                self.divider_area = None;
                self.editor_area = None;
                self.preview_area = None;
                let title = self.editor_title(true);
                let inner = self.render_panel(frame, body_area, title, true, true);
                self.refresh_preview(crate::live::LiveView::content_width(inner));
                self.live.render(
                    frame,
                    inner,
                    &self.preview_doc,
                    &self.editor,
                    &self.theme,
                    &search_query,
                );
                let (total, top) = self.live.scroll_metrics();
                self.render_scrollbar(frame, body_area, total, top, true);
            }
            LayoutMode::EditorOnly => {
                self.split_body_area = None;
                self.divider_area = None;
                self.preview_area = None;
                let title = self.editor_title(true);
                let inner = self.render_panel(frame, body_area, title, true, true);
                self.editor_area = Some(inner);
                self.editor.render_highlighted(frame, inner, &search_query);
                let (total, top) = self.editor.scroll_metrics();
                self.render_scrollbar(frame, body_area, total, top, true);
            }
            LayoutMode::PreviewOnly => {
                self.split_body_area = None;
                self.editor_area = None;
                self.divider_area = None;
                let title = self.preview_title(true);
                let inner = self.render_panel(frame, body_area, title, true, true);
                self.preview_area = Some(inner);
                self.refresh_preview(inner.width);
                self.preview
                    .render(frame, inner, &self.preview_doc, &self.theme, None);
                self.render_scrollbar(
                    frame,
                    body_area,
                    self.preview_doc.rows.len(),
                    self.preview.scroll_offset(),
                    true,
                );
            }
        }

        self.render_status(frame, status_area);

        // Settings overlay (rendered on top of everything)
        if self.mode == AppMode::Settings {
            self.render_settings_overlay(frame, body_area);
        }

        // Help overlay (topmost)
        if self.show_help {
            self.render_help_overlay(frame, body_area);
        }
    }

    /// Render the bottom row: the status line, or the active input prompt.
    fn render_status(&mut self, frame: &mut Frame, area: Rect) {
        type Hints = &'static [(&'static str, &'static str)];
        let theme = &self.theme;
        let prompt = match self.mode {
            AppMode::Editing | AppMode::Normal | AppMode::Insert | AppMode::Visual => {
                let preview_focused = self.focused_pane == FocusedPane::Preview
                    && self.layout_mode != LayoutMode::EditorOnly;
                let (mode, hints): (Option<(&str, ratatui::style::Color)>, Hints) = match self.mode
                {
                    AppMode::Editing if preview_focused => {
                        (None, &[("↑↓", "scroll"), ("Esc", "editor"), ("F1", "help")])
                    }
                    AppMode::Editing => (
                        None,
                        &[
                            ("^S", "save"),
                            ("^F", "find"),
                            ("^P", "view"),
                            ("F1", "help"),
                            ("^Q", "quit"),
                        ],
                    ),
                    AppMode::Normal if preview_focused => (
                        Some(("NORMAL", theme.mode_normal_bg)),
                        &[("j/k", "scroll"), ("g/G", "top/end"), ("Esc", "editor")],
                    ),
                    AppMode::Normal => (
                        Some(("NORMAL", theme.mode_normal_bg)),
                        &[
                            ("i", "insert"),
                            ("v", "visual"),
                            ("/", "find"),
                            (":w", "save"),
                            ("F1", "help"),
                        ],
                    ),
                    AppMode::Insert => (
                        Some(("INSERT", theme.mode_insert_bg)),
                        &[("Esc", "normal"), ("F1", "help")],
                    ),
                    _ => (
                        Some(("VISUAL", theme.mode_visual_bg)),
                        &[
                            ("d", "delete"),
                            ("y", "yank"),
                            (">", "indent"),
                            ("Esc", "normal"),
                        ],
                    ),
                };
                let words = *self.word_count.get_or_insert_with(|| {
                    self.editor
                        .widget()
                        .lines()
                        .iter()
                        .map(|l| l.split_whitespace().count())
                        .sum()
                });
                self.status_bar.render(
                    frame,
                    area,
                    &self.theme,
                    StatusBarContent {
                        filename: self.editor.display_name(),
                        cursor: self.editor.cursor_position(),
                        modified: self.editor.is_modified(),
                        mode,
                        hints,
                        words,
                    },
                );
                return;
            }
            AppMode::Command => {
                let cmd = self
                    .vim_handler
                    .as_ref()
                    .map(|h| h.command_buffer())
                    .unwrap_or("");
                (
                    "COMMAND",
                    theme.mode_command_bg,
                    format!(":{}", cmd),
                    "Enter run \u{00B7} Esc cancel".to_string(),
                    true,
                )
            }
            AppMode::ConfirmQuit => (
                "UNSAVED",
                theme.confirm_bg,
                "Save changes before quitting?".to_string(),
                "y save \u{00B7} n discard \u{00B7} Esc cancel".to_string(),
                false,
            ),
            AppMode::PromptFilename => (
                "SAVE AS",
                theme.prompt_bg,
                self.filename_input.clone(),
                "Enter save \u{00B7} Esc cancel".to_string(),
                true,
            ),
            AppMode::Search => {
                let enter = if self.vim_handler.is_some() {
                    "Enter accept"
                } else {
                    "Enter next"
                };
                let hint = if self.search_match_count > 0 {
                    format!(
                        "{}/{} \u{00B7} {} \u{00B7} Esc cancel",
                        self.search_match_index + 1,
                        self.search_match_count,
                        enter
                    )
                } else if self.search_query.is_empty() {
                    "type to search \u{00B7} Esc cancel".to_string()
                } else {
                    "no matches \u{00B7} Esc cancel".to_string()
                };
                (
                    "SEARCH",
                    theme.prompt_bg,
                    self.search_query.clone(),
                    hint,
                    true,
                )
            }
            AppMode::Settings => (
                "SETTINGS",
                theme.prompt_bg,
                String::new(),
                "\u{2191}\u{2193} select \u{00B7} \u{2190}\u{2192} change \u{00B7} Esc close"
                    .to_string(),
                false,
            ),
        };
        let (label, color, text, hint, cursor) = prompt;
        self.status_bar.render_prompt(
            frame,
            area,
            &self.theme,
            Prompt {
                label,
                color,
                text: &text,
                hint: &hint,
                cursor,
            },
        );
    }

    /// Draw a scrollbar thumb over a panel's right border. Nothing is drawn
    /// when the content fits.
    fn render_scrollbar(
        &self,
        frame: &mut Frame,
        panel: Rect,
        total: usize,
        offset: usize,
        focused: bool,
    ) {
        let track = usize::from(panel.height.saturating_sub(2));
        if track < 3 || total <= track || panel.width < 2 {
            return;
        }
        let thumb = (track * track / total).clamp(1, track - 1);
        let max_offset = total - track;
        let top = (offset.min(max_offset) * (track - thumb) + max_offset / 2) / max_offset;
        let color = if focused {
            self.theme.status_bar_fg
        } else {
            self.theme.preview_muted_fg
        };
        let x = panel.x + panel.width - 1;
        let buf = frame.buffer_mut();
        for i in top..top + thumb {
            if let Some(cell) = buf.cell_mut((x, panel.y + 1 + i as u16)) {
                // Half-width block: reads as a thumb, not a second border.
                cell.set_symbol("\u{2590}").set_fg(color);
            }
        }
    }

    /// Render a rounded btop-style panel border with an embedded title and
    /// return the inner content area. When `with_buttons` is set, the
    /// clickable title-bar buttons are drawn over the top border.
    fn render_panel(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        title: ratatui::text::Line<'static>,
        focused: bool,
        with_buttons: bool,
    ) -> Rect {
        use ratatui::widgets::{Block, BorderType};

        let border_color = if focused {
            self.theme.preview_accent_primary
        } else {
            self.theme.divider_fg
        };
        let title_width = title.width() as u16;
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color))
            .title_top(title);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        if with_buttons {
            // Buttons stop short of the title so they never chop it mid-word
            let min_x = area.x + 1 + title_width + 1;
            self.render_title_buttons(frame, area, min_x);
        }
        inner
    }

    /// Editor panel title: ` filename + ` with the modified marker.
    fn editor_title(&self, focused: bool) -> ratatui::text::Line<'static> {
        let color = if focused {
            self.theme.preview_accent_primary
        } else {
            self.theme.divider_fg
        };
        let modified = if self.editor.is_modified() {
            " \u{25CF}"
        } else {
            ""
        };
        ratatui::text::Line::from(Span::styled(
            format!(" {}{} ", self.editor.display_name(), modified),
            Style::default()
                .fg(color)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ))
    }

    /// Preview panel title: ` preview · profile `.
    fn preview_title(&self, focused: bool) -> ratatui::text::Line<'static> {
        let color = if focused {
            self.theme.preview_accent_primary
        } else {
            self.theme.divider_fg
        };
        ratatui::text::Line::from(Span::styled(
            format!(" preview \u{00B7} {} ", self.config.render_profile.name()),
            Style::default()
                .fg(color)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ))
    }

    /// Draw btop-style `┤ key label ├` buttons over a panel's top border,
    /// right-aligned, and record their hit areas for mouse handling.
    /// Buttons that don't fit to the right of `min_x` are dropped
    /// (rightmost kept).
    fn render_title_buttons(&mut self, frame: &mut Frame, panel: Rect, min_x: u16) {
        if panel.height == 0 || panel.width < 4 {
            return;
        }
        // Laid out right-to-left; later entries are dropped first when space
        // is tight, so help and quit survive the longest.
        let buttons: [(UiButton, &str, &str); 4] = [
            (UiButton::Quit, "^Q", "quit"),
            (UiButton::Help, "F1", "help"),
            (UiButton::Settings, "F2", "set"),
            (UiButton::View, "^P", "view"),
        ];
        let key_style = Style::default()
            .fg(self.theme.preview_accent_primary)
            .add_modifier(ratatui::style::Modifier::BOLD);
        let label_style = Style::default().fg(self.theme.status_bar_fg);
        let tee_style = Style::default().fg(self.theme.divider_fg);

        // Lay out right-to-left, leaving the corner glyph intact.
        let mut right_edge = panel.x + panel.width - 2;
        for (button, key, label) in buttons {
            // "┤" + key + " " + label + "├"
            let width = (1 + key.len() + 1 + label.len() + 1) as u16;
            if right_edge < min_x.max(panel.x + 2) + width {
                break;
            }
            let x = right_edge - width;
            let rect = Rect::new(x, panel.y, width, 1);
            let line = ratatui::text::Line::from(vec![
                Span::styled("\u{2524}", tee_style),
                Span::styled(key.to_string(), key_style),
                Span::styled(format!(" {}", label), label_style),
                Span::styled("\u{251C}", tee_style),
            ]);
            frame.render_widget(Paragraph::new(line), rect);
            self.button_areas.push((rect, button));
            right_edge = x.saturating_sub(1);
        }
    }

    /// Render the keybindings help overlay (btop-style menu box).
    fn render_help_overlay(&self, frame: &mut Frame, area: Rect) {
        use ratatui::widgets::{Block, BorderType, Clear};

        let is_vim = self.vim_handler.is_some();
        let key_rows: &[(&str, &str)] = if is_vim {
            &[
                ("h j k l", "move cursor"),
                ("w b e", "word motions"),
                ("0 $  gg G", "line / file start and end"),
                ("i a o O", "enter insert mode"),
                ("v V", "visual / visual line"),
                ("d c y +motion", "delete / change / yank"),
                ("dd yy cc", "line delete / yank / change"),
                ("p P", "paste after / before"),
                ("x r J", "delete char / replace / join"),
                ("u ^R", "undo / redo"),
                ("/ n N  Esc", "search / next / previous / clear"),
                (":w :q :wq", "save / quit / save+quit"),
                (":set  :help", "settings panel / this help"),
                ("^P  F2  ^Q", "cycle view / settings / quit"),
            ]
        } else {
            &[
                ("^S  ^Q", "save / quit"),
                ("^F", "find"),
                ("^C  ^V", "copy / paste"),
                ("^Z  ^Y", "undo / redo"),
                ("Tab  S-Tab", "indent / outdent"),
                ("Shift+arrows", "select text"),
                ("^P  F2", "cycle view / settings"),
            ]
        };
        let mouse_rows: &[(&str, &str)] = &[
            ("click", "focus pane / place cursor"),
            ("wheel", "scroll pane under cursor"),
            ("drag", "select text / resize split"),
            ("j k g G ↑↓", "scroll preview when focused"),
        ];

        let popup_w = 56u16.min(area.width);
        let popup_h = ((key_rows.len() + mouse_rows.len()) as u16 + 5).min(area.height);
        if popup_w < 10 || popup_h < 3 {
            return;
        }
        let x = area.x + area.width.saturating_sub(popup_w) / 2;
        let y = area.y + area.height.saturating_sub(popup_h) / 2;
        let popup_area = Rect::new(x, y, popup_w, popup_h);

        frame.render_widget(Clear, popup_area);

        let accent = self.theme.preview_accent_primary;
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(accent))
            .title_top(ratatui::text::Line::from(Span::styled(
                " help ",
                Style::default()
                    .fg(accent)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            )))
            .title_bottom(
                ratatui::text::Line::from(Span::styled(
                    " Esc close ",
                    Style::default().fg(self.theme.divider_fg),
                ))
                .right_aligned(),
            )
            .style(Style::default().bg(self.theme.status_bar_bg));
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let key_style = Style::default()
            .fg(accent)
            .add_modifier(ratatui::style::Modifier::BOLD);
        let desc_style = Style::default().fg(self.theme.status_bar_fg);
        let section_style = Style::default()
            .fg(self.theme.preview_muted_fg)
            .add_modifier(ratatui::style::Modifier::BOLD);

        let mut lines: Vec<ratatui::text::Line> = Vec::new();
        lines.push(ratatui::text::Line::from(Span::styled(
            if is_vim {
                " keys (vim)"
            } else {
                " keys (nano)"
            },
            section_style,
        )));
        for (key, desc) in key_rows {
            lines.push(ratatui::text::Line::from(vec![
                Span::styled(format!("  {:>14}  ", key), key_style),
                Span::styled(*desc, desc_style),
            ]));
        }
        lines.push(ratatui::text::Line::default());
        lines.push(ratatui::text::Line::from(Span::styled(
            " mouse & panes",
            section_style,
        )));
        for (key, desc) in mouse_rows {
            lines.push(ratatui::text::Line::from(vec![
                Span::styled(format!("  {:>14}  ", key), key_style),
                Span::styled(*desc, desc_style),
            ]));
        }
        lines.truncate(inner.height as usize);
        frame.render_widget(Paragraph::new(lines), inner);
    }

    /// Render the settings overlay panel centered on screen.
    fn render_settings_overlay(&self, frame: &mut Frame, area: Rect) {
        use ratatui::style::Modifier;
        use ratatui::text::Line;
        use ratatui::widgets::{Block, BorderType, Clear};

        let themes = self.theme_names();
        let modes = ["vim", "nano"];
        let profiles = crate::config::RenderProfile::all();
        let on_off = |on: bool| if on { "on" } else { "off" };

        // (row, label, value, is the default value)
        let options: [(SettingsRow, &str, String, bool); 5] = [
            (
                SettingsRow::Theme,
                "Theme",
                themes[self.settings_theme_index].clone(),
                themes[self.settings_theme_index] == "ocean",
            ),
            (
                SettingsRow::Mode,
                "Keys",
                modes[self.settings_mode_index].to_string(),
                self.settings_mode_index == 0,
            ),
            (
                SettingsRow::Profile,
                "Preview style",
                profiles[self.settings_profile_index].name().to_string(),
                self.settings_profile_index == 0,
            ),
            (
                SettingsRow::SyncIndicator,
                "Sync highlight",
                on_off(self.config.sync_indicator).to_string(),
                self.config.sync_indicator,
            ),
            (
                SettingsRow::WordWrap,
                "Word wrap",
                on_off(self.config.word_wrap).to_string(),
                self.config.word_wrap,
            ),
        ];
        let actions = [
            (SettingsRow::Save, "Save as default"),
            (SettingsRow::Reset, "Reset to defaults"),
        ];

        let popup_w = 48u16.min(area.width);
        let popup_h = (options.len() + actions.len() + 5) as u16;
        let popup_h = popup_h.min(area.height);
        if popup_w < 24 || popup_h < 5 {
            return;
        }
        let popup_area = Rect::new(
            area.x + (area.width - popup_w) / 2,
            area.y + (area.height - popup_h) / 2,
            popup_w,
            popup_h,
        );
        frame.render_widget(Clear, popup_area);

        let accent = self.theme.preview_accent_primary;
        let surface = self.theme.status_bar_bg;
        let dim = Style::default().fg(self.theme.preview_muted_fg);
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(accent))
            .title_top(Line::from(Span::styled(
                " settings ",
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            )))
            .style(Style::default().bg(surface).fg(self.theme.status_bar_fg));
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let row_style = |selected: bool| {
            if selected {
                Style::default()
                    .bg(self.theme.selection_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            }
        };
        let marker = |selected: bool| {
            if selected {
                Span::styled(" \u{258C} ", Style::default().fg(accent))
            } else {
                Span::raw("   ")
            }
        };

        let mut lines: Vec<Line> = vec![Line::default()];
        for (row, label, value, is_default) in &options {
            let selected = *row == self.settings_row;
            let arrow = if selected {
                Style::default().fg(accent)
            } else {
                dim
            };
            let mut spans = vec![
                marker(selected),
                Span::raw(format!("{:<16}", label)),
                Span::styled("\u{2039} ", arrow),
                Span::styled(value.clone(), Style::default().fg(accent)),
                Span::styled(" \u{203A}", arrow),
            ];
            if *is_default {
                spans.push(Span::styled("  default", dim));
            }
            lines.push(Line::from(spans).style(row_style(selected)));
        }
        lines.push(Line::default());
        for (row, label) in &actions {
            let selected = *row == self.settings_row;
            lines.push(
                Line::from(vec![marker(selected), Span::raw(*label)]).style(row_style(selected)),
            );
        }
        lines.truncate(usize::from(inner.height));

        // Line styles only cover their text, so pad rows to the full width
        // to make the selection bar span the panel.
        let width = usize::from(inner.width);
        for line in &mut lines {
            let used = line.width();
            if used < width {
                line.spans.push(Span::raw(" ".repeat(width - used)));
            }
        }
        frame.render_widget(Paragraph::new(lines), inner);
    }

    /// Handle key events in the settings overlay.
    fn handle_settings_key(&mut self, key: crossterm::event::KeyEvent) {
        let themes = self.theme_names();
        let n_themes = themes.len();
        let profiles = crate::config::RenderProfile::all();
        let n_profiles = profiles.len();

        match key.code {
            KeyCode::Esc => {
                // Cancel — restore to what config had before opening
                self.mode = if self.editing_mode == crate::config::EditingMode::Vim {
                    AppMode::Normal
                } else {
                    AppMode::Editing
                };
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.settings_row = match self.settings_row {
                    SettingsRow::Theme => SettingsRow::Reset,
                    SettingsRow::Mode => SettingsRow::Theme,
                    SettingsRow::Profile => SettingsRow::Mode,
                    SettingsRow::SyncIndicator => SettingsRow::Profile,
                    SettingsRow::WordWrap => SettingsRow::SyncIndicator,
                    SettingsRow::Save => SettingsRow::WordWrap,
                    SettingsRow::Reset => SettingsRow::Save,
                };
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.settings_row = match self.settings_row {
                    SettingsRow::Theme => SettingsRow::Mode,
                    SettingsRow::Mode => SettingsRow::Profile,
                    SettingsRow::Profile => SettingsRow::SyncIndicator,
                    SettingsRow::SyncIndicator => SettingsRow::WordWrap,
                    SettingsRow::WordWrap => SettingsRow::Save,
                    SettingsRow::Save => SettingsRow::Reset,
                    SettingsRow::Reset => SettingsRow::Theme,
                };
            }
            KeyCode::Left | KeyCode::Char('h') => match self.settings_row {
                SettingsRow::Theme => {
                    self.settings_theme_index = if self.settings_theme_index == 0 {
                        n_themes - 1
                    } else {
                        self.settings_theme_index - 1
                    };
                    self.apply_settings_theme(&themes[self.settings_theme_index]);
                }
                SettingsRow::Mode => {
                    self.settings_mode_index = 1 - self.settings_mode_index;
                }
                SettingsRow::Profile => {
                    self.settings_profile_index = if self.settings_profile_index == 0 {
                        n_profiles - 1
                    } else {
                        self.settings_profile_index - 1
                    };
                    self.config.render_profile = profiles[self.settings_profile_index];
                    self.content_dirty = true;
                }
                SettingsRow::SyncIndicator => {
                    self.config.sync_indicator = !self.config.sync_indicator;
                }
                SettingsRow::WordWrap => {
                    self.config.word_wrap = !self.config.word_wrap;
                    self.editor.word_wrap = self.config.word_wrap;
                }
                SettingsRow::Save | SettingsRow::Reset => {}
            },
            KeyCode::Right | KeyCode::Char('l') => match self.settings_row {
                SettingsRow::Theme => {
                    self.settings_theme_index = (self.settings_theme_index + 1) % n_themes;
                    self.apply_settings_theme(&themes[self.settings_theme_index]);
                }
                SettingsRow::Mode => {
                    self.settings_mode_index = 1 - self.settings_mode_index;
                }
                SettingsRow::Profile => {
                    self.settings_profile_index = (self.settings_profile_index + 1) % n_profiles;
                    self.config.render_profile = profiles[self.settings_profile_index];
                    self.content_dirty = true;
                }
                SettingsRow::SyncIndicator => {
                    self.config.sync_indicator = !self.config.sync_indicator;
                }
                SettingsRow::WordWrap => {
                    self.config.word_wrap = !self.config.word_wrap;
                    self.editor.word_wrap = self.config.word_wrap;
                }
                SettingsRow::Save | SettingsRow::Reset => {}
            },
            KeyCode::Enter => {
                match self.settings_row {
                    SettingsRow::Save => {
                        // Persist to config file
                        self.config.mode = if self.settings_mode_index == 0 {
                            crate::config::EditingMode::Vim
                        } else {
                            crate::config::EditingMode::Nano
                        };
                        self.config.render_profile = profiles[self.settings_profile_index];
                        match crate::config::save_config(&self.config) {
                            Ok(path) => self.status_bar.set_message(&format!("Saved to {}", path)),
                            Err(e) => self.status_bar.set_message(&format!("Error: {}", e)),
                        }
                        self.mode = if self.editing_mode == crate::config::EditingMode::Vim {
                            AppMode::Normal
                        } else {
                            AppMode::Editing
                        };
                    }
                    SettingsRow::Reset => {
                        // Reset to defaults
                        self.settings_theme_index = 0; // ocean
                        self.settings_mode_index = 0; // vim
                        self.settings_profile_index = 0; // clean
                        self.apply_settings_theme("ocean");
                        self.config.mode = crate::config::EditingMode::Vim;
                        self.config.theme = "ocean".to_string();
                        self.config.render_profile = crate::config::RenderProfile::default();
                        self.config.sync_indicator = true;
                        self.config.word_wrap = true;
                        self.editor.word_wrap = true;
                        self.content_dirty = true;
                        self.status_bar.set_message("Reset to defaults");
                    }
                    _ => {
                        // Enter on Theme/Mode/Profile row — close settings
                        self.config.mode = if self.settings_mode_index == 0 {
                            crate::config::EditingMode::Vim
                        } else {
                            crate::config::EditingMode::Nano
                        };
                        self.config.render_profile = profiles[self.settings_profile_index];
                        self.mode = if self.editing_mode == crate::config::EditingMode::Vim {
                            AppMode::Normal
                        } else {
                            AppMode::Editing
                        };
                    }
                }
            }
            _ => {}
        }
    }

    /// Apply a theme by name from the settings panel (live preview).
    fn apply_settings_theme(&mut self, name: &str) {
        if let Some(new_theme) = crate::config::resolve_theme_name(&self.config, name) {
            let cap = crate::theme::detect_color_capability();
            let applied = if cap == crate::theme::ColorCapability::Color256 {
                new_theme.with_256_color_fallback()
            } else {
                new_theme
            };
            self.editor.apply_theme(applied.clone());
            self.theme = applied;
            self.config.theme = name.to_string();
            self.content_dirty = true;
        }
    }

    fn theme_names(&self) -> Vec<String> {
        crate::config::available_theme_names(&self.config)
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "panic payload was not a string".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{App, FocusedPane, LayoutMode, UiButton};
    use crate::clipboard::ClipboardProvider;
    use crate::config::{Config, EditingMode};
    use crate::markdown::MarkdownRenderer;
    use crate::theme::Theme;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::path::PathBuf;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    struct TestClipboard;

    impl ClipboardProvider for TestClipboard {
        fn write(&self, _text: &str) -> Result<(), String> {
            Ok(())
        }

        fn read(&self) -> Result<String, String> {
            Ok(String::new())
        }

        fn name(&self) -> &str {
            "test"
        }
    }

    fn test_app(content: &str) -> App<'static> {
        let config = Config {
            mode: EditingMode::Nano,
            ..Config::default()
        };

        App::new(
            Some(content.to_string()),
            None,
            Theme::ocean(),
            EditingMode::Nano,
            Box::new(TestClipboard),
            config,
        )
    }

    #[test]
    fn do_save_preserves_crlf_and_trailing_newline() {
        let filename = format!(
            "mdedit-save-test-{}-{}.md",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock before unix epoch")
                .as_nanos()
        );
        let path = std::env::temp_dir().join(filename);

        let config = Config {
            mode: EditingMode::Nano,
            ..Config::default()
        };

        let mut app = App::new(
            Some("alpha\r\nbeta\r\n".to_string()),
            Some(PathBuf::from(&path)),
            Theme::ocean(),
            EditingMode::Nano,
            Box::new(TestClipboard),
            config,
        );

        app.editor.textarea_mut().insert_str("!");
        app.mark_content_changed();

        assert!(app.do_save());
        let saved = std::fs::read(&path).expect("saved file should exist");
        assert_eq!(saved, b"!alpha\r\nbeta\r\n");

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn split_render_handles_zero_width_panes() {
        let mut app = test_app("# mdedit\n\nA tiny render test.");
        app.preview_doc = PreviewRenderer.render(
            &app.editor.content(),
            app.config.render_profile,
            &app.theme,
            1,
        );
        app.content_dirty = false;
        app.layout_mode = LayoutMode::Split;

        let backend = TestBackend::new(1, 2);
        let mut terminal = Terminal::new(backend).expect("test terminal should initialize");

        terminal
            .draw(|frame| app.render(frame))
            .expect("render should not panic on zero-width panes");
    }

    fn click(app: &mut App<'static>, column: u16, row: u16) {
        use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        app.handle_mouse_event(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        });
    }

    #[test]
    fn title_buttons_are_registered_and_help_button_opens_overlay() {
        let mut app = test_app("# hi");
        let backend = TestBackend::new(160, 30);
        let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
        terminal.draw(|frame| app.render(frame)).expect("render");

        assert_eq!(
            app.button_areas.len(),
            4,
            "all four buttons fit at 160 cols"
        );

        let (rect, _) = *app
            .button_areas
            .iter()
            .find(|(_, b)| *b == UiButton::Help)
            .expect("help button registered");
        click(&mut app, rect.x, rect.y);
        assert!(app.show_help, "clicking the help button opens the overlay");

        // Any further click dismisses it
        click(&mut app, 5, 5);
        assert!(!app.show_help, "clicking anywhere closes the overlay");

        // Buttons render in every layout mode; help/quit survive narrowing
        for _ in 0..3 {
            app.cycle_layout();
            terminal.draw(|frame| app.render(frame)).expect("render");
            assert!(
                app.button_areas
                    .iter()
                    .any(|(_, b)| *b == UiButton::Help || *b == UiButton::Quit),
                "help/quit buttons survive in layout {:?}",
                app.layout_mode
            );
        }
    }

    #[test]
    fn help_overlay_renders_across_terminal_sizes() {
        let mut app = test_app("# hi");
        app.show_help = true;
        for (w, h) in [(5u16, 3u16), (40, 12), (120, 40)] {
            let backend = TestBackend::new(w, h);
            let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
            terminal
                .draw(|frame| app.render(frame))
                .expect("help overlay render should not panic");
        }
    }

    #[test]
    fn help_overlay_shows_keybindings_for_active_mode() {
        let mut app = test_app("# hi"); // nano-mode test app
        app.show_help = true;
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
        terminal.draw(|frame| app.render(frame)).expect("render");

        let buffer = terminal.backend().buffer().clone();
        let mut screen = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                screen.push_str(buffer[(x, y)].symbol());
            }
            screen.push('\n');
        }
        println!("{screen}");

        assert!(screen.contains(" help "), "overlay title present");
        assert!(screen.contains("keys (nano)"), "nano key section present");
        assert!(screen.contains("indent / outdent"), "nano rows present");
        assert!(screen.contains("mouse & panes"), "mouse section present");
        assert!(screen.contains("Esc close"), "close hint present");
    }

    #[test]
    fn clicking_preview_focuses_it_and_manual_scroll_persists() {
        // Blank-line separated paragraphs so the preview is taller than the pane
        let content = "paragraph\n\n".repeat(200);
        let mut app = test_app(&content);
        app.layout_mode = LayoutMode::Split;

        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
        terminal.draw(|frame| app.render(frame)).expect("render");

        let preview = app.preview_area.expect("split layout has a preview pane");
        click(&mut app, preview.x + 1, preview.y + 1);
        assert_eq!(app.focused_pane, FocusedPane::Preview);

        // With the preview focused, cursor scroll-sync is paused, so a manual
        // scroll survives the next frame.
        app.preview.scroll_down(5);
        terminal.draw(|frame| app.render(frame)).expect("render");
        assert_eq!(app.preview.scroll_offset(), 5);

        // Clicking back into the editor refocuses it
        let editor = app.editor_area.expect("split layout has an editor pane");
        click(&mut app, editor.x + 1, editor.y + 1);
        assert_eq!(app.focused_pane, FocusedPane::Editor);
    }

    #[test]
    fn readme_preview_update_and_split_render_survive_terminal_size_matrix() {
        let mut app = test_app(include_str!("../README.md"));
        app.last_edit_time = Some(Instant::now() - Duration::from_millis(100));
        app.refresh_preview(80);
        app.layout_mode = LayoutMode::Split;

        for width in 1..=140 {
            for height in [2u16, 3, 4, 5, 8, 12, 20, 30, 40] {
                let backend = TestBackend::new(width, height);
                let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
                terminal
                    .draw(|frame| app.render(frame))
                    .expect("README render matrix should not panic");
            }
        }
    }

    use super::AppMode;
    use crate::markdown::PreviewRenderer;
    use crossterm::event::{Event, KeyCode, KeyModifiers};

    fn vim_app(content: &str) -> App<'static> {
        App::new(
            Some(content.to_string()),
            None,
            Theme::ocean(),
            EditingMode::Vim,
            Box::new(TestClipboard),
            Config::default(),
        )
    }

    fn keys(app: &mut App<'static>, input: &str) {
        for c in input.chars() {
            let code = match c {
                '\n' => KeyCode::Enter,
                '\x1b' => KeyCode::Esc,
                c => KeyCode::Char(c),
            };
            app.handle_event(Event::Key(crossterm::event::KeyEvent::new(
                code,
                KeyModifiers::NONE,
            )));
        }
    }

    /// Regression: edits made in vim mode never set the modified flag, so
    /// `:q` silently discarded unsaved work.
    #[test]
    fn vim_edits_mark_the_buffer_modified_and_q_refuses_to_quit() {
        for edit in ["ihello\x1b", "x", "dd", "ofoo\x1b", "yyp", "J", "rz"] {
            let mut app = vim_app("first line\nsecond line\n");
            assert!(!app.editor.is_modified());
            keys(&mut app, edit);
            assert!(app.editor.is_modified(), "{edit:?} should mark modified");
            keys(&mut app, ":q\n");
            assert!(!app.should_quit, "{edit:?}: :q must not discard changes");
        }

        // Motions alone leave the buffer clean, and :q quits.
        let mut app = vim_app("first line\nsecond line\n");
        keys(&mut app, "jwk$");
        assert!(!app.editor.is_modified());
        keys(&mut app, ":q\n");
        assert!(app.should_quit);
    }

    /// Regression: Enter in a vim search stayed in search mode, so the next
    /// commands were typed into the search box.
    #[test]
    fn vim_search_enter_returns_to_normal_and_n_steps_through_matches() {
        let mut app = vim_app("alpha\nbeta alpha\ngamma\nalpha\n");
        keys(&mut app, "/alpha\n");
        assert_eq!(app.mode, AppMode::Normal);
        assert_eq!(app.current_search_query(), "alpha", "highlights persist");
        let first = app.editor.cursor_position();
        keys(&mut app, "n");
        assert_ne!(
            app.editor.cursor_position(),
            first,
            "n moves to the next match"
        );
        assert_eq!(app.mode, AppMode::Normal);

        keys(&mut app, "\x1b");
        assert_eq!(app.current_search_query(), "", "Esc clears highlights");

        // A search with no matches reports it and leaves no stale query.
        keys(&mut app, "/zzz\n");
        assert_eq!(app.mode, AppMode::Normal);
        assert_eq!(app.current_search_query(), "");
    }

    #[test]
    fn help_and_version_style_ex_commands_work() {
        let mut app = vim_app("text\n");
        keys(&mut app, ":help\n");
        assert!(app.show_help);
    }

    #[test]
    fn every_theme_renders_every_mode_without_panicking() {
        for name in Theme::available_themes() {
            let theme = Theme::by_name(name).unwrap();
            for theme in [theme.clone(), theme.with_256_color_fallback()] {
                let mut app = App::new(
                    Some(include_str!("../README.md").to_string()),
                    None,
                    theme,
                    EditingMode::Vim,
                    Box::new(TestClipboard),
                    Config::default(),
                );
                let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
                for input in ["", "i", "\x1bV", "\x1b/the", "\x1b:set", "\x1b:help\n"] {
                    keys(&mut app, input);
                    terminal.draw(|frame| app.render(frame)).expect("render");
                }
            }
        }
    }

    fn screen_text(terminal: &Terminal<TestBackend>) -> String {
        let buf = terminal.backend().buffer();
        let area = buf.area;
        (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn live_view_renders_the_document_and_shows_only_the_cursor_block_raw() {
        let mut app = vim_app("# Title\n\nSome **bold** text.\n\n- one\n- two\n");
        assert_eq!(
            app.layout_mode,
            LayoutMode::Live,
            "live is the default view"
        );
        let mut terminal = Terminal::new(TestBackend::new(60, 14)).unwrap();

        // Cursor on the heading: it is raw, the rest is rendered.
        terminal.draw(|f| app.render(f)).unwrap();
        let screen = screen_text(&terminal);
        assert!(screen.contains("# Title"), "{screen}");
        assert!(screen.contains("Some bold text."), "paragraph is rendered");
        assert!(!screen.contains("**bold**"));
        assert!(screen.contains("\u{2022} one"), "list is rendered");

        // Move into the paragraph: heading renders, paragraph turns raw.
        keys(&mut app, "jj");
        terminal.draw(|f| app.render(f)).unwrap();
        let screen = screen_text(&terminal);
        assert!(screen.contains("Some **bold** text."), "{screen}");
        assert!(!screen.contains("# Title"), "heading re-rendered");
        assert!(screen.contains("Title"));

        // Only the list item under the cursor is raw, not the whole list.
        keys(&mut app, "jjj");
        terminal.draw(|f| app.render(f)).unwrap();
        let screen = screen_text(&terminal);
        assert!(
            screen.contains("\u{2022} one") && screen.contains("- two"),
            "{screen}"
        );

        // Editing in place updates the buffer and, once rendered, the view.
        keys(&mut app, "A!\x1bgg");
        terminal.draw(|f| app.render(f)).unwrap();
        let screen = screen_text(&terminal);
        assert!(screen.contains("\u{2022} two!"), "{screen}");
        assert!(app.editor.content().contains("- two!"));
    }

    #[test]
    fn live_view_keeps_the_cursor_on_screen_everywhere() {
        let readme = include_str!("../README.md");
        let line_count = readme.lines().count();
        for (width, height) in [(110u16, 32u16), (60, 16), (30, 8), (12, 5), (4, 3)] {
            let mut app = vim_app(readme);
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            // Down through every line, then back up, then edits that add and
            // remove lines (the rendered doc lags the buffer by design).
            for step in ["j"; 400].iter().chain(["k"; 400].iter()).chain(
                [
                    "o", "new", "\n", "\n", "text", "\x1b", "dd", "dd", "u", "G", "gg",
                ]
                .iter(),
            ) {
                keys(&mut app, step);
                terminal.draw(|f| app.render(f)).expect("render");
                if width > 6 && height > 3 {
                    let pos = terminal.get_cursor_position().unwrap();
                    assert!(
                        pos.x < width && pos.y < height - 1,
                        "cursor {pos:?} off screen at {width}x{height}"
                    );
                }
            }
            assert!(app.editor.line_count() >= line_count);
        }
    }

    #[test]
    fn live_view_click_on_rendered_text_opens_that_block() {
        let mut app = vim_app("# Title\n\nFirst paragraph.\n\nSecond paragraph.\n");
        let mut terminal = Terminal::new(TestBackend::new(60, 14)).unwrap();
        terminal.draw(|f| app.render(f)).unwrap();
        let screen = screen_text(&terminal);
        let row = screen
            .lines()
            .position(|l| l.contains("Second paragraph."))
            .unwrap() as u16;
        click(&mut app, 10, row);
        assert_eq!(app.editor.cursor_position().0, 4);
    }

    /// A document is untrusted input: control characters in it (ESC, BEL,
    /// CSI/OSC introducers) must never reach the terminal, or opening a file
    /// could rewrite the clipboard, the window title, or the screen.
    #[test]
    fn control_characters_in_a_document_never_reach_the_terminal() {
        let hostile = "# Title \x1b]52;c;ZXZpbA==\x07 end\n\n\
                       text \x1b[2J\x1b[31m red \u{9b}31m \u{9d}0;pwned\u{9c} \x07\x08\x7f\n\n\
                       - item \x1b]0;title\x07\n\n\
                       ```sh\necho \x1b[5m blink\n```\n\n\
                       | a \x1b[1m | b |\n|---|---|\n| c \x1b]8;;http://x\x07 | d |\n";
        for view in [
            LayoutMode::Live,
            LayoutMode::Split,
            LayoutMode::EditorOnly,
            LayoutMode::PreviewOnly,
        ] {
            let mut app = vim_app(hostile);
            app.layout_mode = view;
            let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
            // Visit every line so each block is shown both raw and rendered,
            // and put the hostile text in the search prompt too.
            for step in ["", "j", "j", "j", "j", "j", "j", "j", "j", "/\x1b[2J", "\n"] {
                keys(&mut app, step);
                terminal.draw(|f| app.render(f)).unwrap();
                let buf = terminal.backend().buffer();
                for cell in buf.content() {
                    assert!(
                        !cell.symbol().chars().any(char::is_control),
                        "control char {:?} reached the screen in {view:?}",
                        cell.symbol()
                    );
                }
            }
        }
    }
}
