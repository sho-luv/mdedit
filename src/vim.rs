use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui_textarea::CursorMove;

/// Vim modal states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VimMode {
    Normal,
    Insert,
    Visual { line_wise: bool },
    Command,
}

/// Actions the VimHandler returns for the app/editor to execute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VimCommand {
    // Motion commands
    Move(CursorMoveCmd),
    MoveN(CursorMoveCmd, usize),

    // Editing commands
    Delete { motion: Motion },
    Change { motion: Motion },
    Yank { motion: Motion },
    PasteAfter,
    PasteBefore,
    DeleteChar,

    // Insert mode entry variants
    EnterInsert(InsertPosition),
    ExitInsert,

    // Visual mode
    EnterVisual { line_wise: bool },
    ExitVisual,
    VisualDelete,
    VisualChange,
    VisualYank,
    VisualIndent,
    VisualOutdent,

    // Command mode
    EnterCommand,
    ExitCommand,
    CommandExecute(String),
    CommandAppend(char),
    CommandBackspace,

    // Other
    Undo,
    Redo,
    EnterSearch,
    Save,
    Quit { force: bool },
    SaveAndQuit,
    ContentChanged,
    None,
}

/// Wrapper around CursorMove to derive common traits.
/// ratatui_textarea::CursorMove doesn't implement Eq/PartialEq,
/// so we use our own enum that maps to CursorMove at the call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorMoveCmd {
    Up,
    Down,
    Back,
    Forward,
    Head,
    End,
    Top,
    Bottom,
    WordForward,
    WordBack,
    ParagraphForward,
    ParagraphBack,
}

impl CursorMoveCmd {
    /// Convert to ratatui_textarea CursorMove.
    pub fn to_cursor_move(self) -> CursorMove {
        match self {
            Self::Up => CursorMove::Up,
            Self::Down => CursorMove::Down,
            Self::Back => CursorMove::Back,
            Self::Forward => CursorMove::Forward,
            Self::Head => CursorMove::Head,
            Self::End => CursorMove::End,
            Self::Top => CursorMove::Top,
            Self::Bottom => CursorMove::Bottom,
            Self::WordForward => CursorMove::WordForward,
            Self::WordBack => CursorMove::WordBack,
            Self::ParagraphForward => CursorMove::ParagraphForward,
            Self::ParagraphBack => CursorMove::ParagraphBack,
        }
    }
}

/// Represents a vim motion target for operators (d, c, y).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    Left,
    Right,
    Up,
    Down,
    WordForward,
    WordBack,
    WordEnd,
    LineStart,
    LineEnd,
    FileStart,
    FileEnd,
    ParagraphUp,
    ParagraphDown,
    Line,
    ToEnd,
    ToStart,
}

/// How insert mode was entered -- determines cursor positioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertPosition {
    BeforeCursor,
    AfterCursor,
    LineStart,
    LineEnd,
    NewLineBelow,
    NewLineAbove,
}

/// Pending operator waiting for a motion (e.g., d waiting for w).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operator {
    Delete,
    Change,
    Yank,
}

/// The vim key handling state machine.
pub struct VimHandler {
    mode: VimMode,
    pending_operator: Option<Operator>,
    count_prefix: Option<usize>,
    partial_key: Option<char>,
    pub yank_register: String,
    command_buffer: String,
}

impl VimHandler {
    pub fn new() -> Self {
        VimHandler {
            mode: VimMode::Normal,
            pending_operator: None,
            count_prefix: None,
            partial_key: None,
            yank_register: String::new(),
            command_buffer: String::new(),
        }
    }

    /// Return a reference to the current mode.
    pub fn mode(&self) -> &VimMode {
        &self.mode
    }

    /// Return the current command buffer contents (for : prompt display).
    pub fn command_buffer(&self) -> &str {
        &self.command_buffer
    }

    /// Return the yank register contents.
    pub fn yank_register(&self) -> &str {
        &self.yank_register
    }

    /// Set the yank register contents.
    pub fn set_yank_register(&mut self, text: String) {
        self.yank_register = text;
    }

    /// Set vim mode to Visual (for mouse drag selection).
    pub fn set_mode_visual(&mut self, line_wise: bool) {
        self.mode = VimMode::Visual { line_wise };
    }

    /// Clear count prefix after use.
    pub fn reset_count(&mut self) {
        self.count_prefix = None;
    }

    /// Take the accumulated count, returning 1 if none was set, and clear it.
    pub fn take_count(&mut self) -> usize {
        let count = self.count_prefix.unwrap_or(1);
        self.count_prefix = None;
        count
    }

    /// Process a key event and return the resulting command.
    pub fn handle_key(&mut self, key: KeyEvent) -> VimCommand {
        match self.mode {
            VimMode::Normal => self.handle_normal_key(key),
            VimMode::Insert => self.handle_insert_key(key),
            VimMode::Visual { .. } => self.handle_visual_key(key),
            VimMode::Command => self.handle_command_key(key),
        }
    }

    /// Map a key to a Motion enum variant (for operator+motion combining).
    fn key_to_motion(&self, code: KeyCode) -> Option<Motion> {
        match code {
            KeyCode::Char('h') => Some(Motion::Left),
            KeyCode::Char('l') => Some(Motion::Right),
            KeyCode::Char('j') => Some(Motion::Down),
            KeyCode::Char('k') => Some(Motion::Up),
            KeyCode::Char('w') | KeyCode::Char('e') => Some(Motion::WordForward),
            KeyCode::Char('b') => Some(Motion::WordBack),
            KeyCode::Char('0') => Some(Motion::LineStart),
            KeyCode::Char('$') => Some(Motion::LineEnd),
            KeyCode::Char('G') => Some(Motion::FileEnd),
            KeyCode::Char('{') => Some(Motion::ParagraphUp),
            KeyCode::Char('}') => Some(Motion::ParagraphDown),
            _ => None,
        }
    }

    /// Map a Motion to the corresponding CursorMoveCmd.
    fn motion_to_cursor_cmd(motion: &Motion) -> Option<CursorMoveCmd> {
        match motion {
            Motion::Left => Some(CursorMoveCmd::Back),
            Motion::Right => Some(CursorMoveCmd::Forward),
            Motion::Up => Some(CursorMoveCmd::Up),
            Motion::Down => Some(CursorMoveCmd::Down),
            Motion::WordForward | Motion::WordEnd => Some(CursorMoveCmd::WordForward),
            Motion::WordBack => Some(CursorMoveCmd::WordBack),
            Motion::LineStart => Some(CursorMoveCmd::Head),
            Motion::LineEnd => Some(CursorMoveCmd::End),
            Motion::FileStart => Some(CursorMoveCmd::Top),
            Motion::FileEnd => Some(CursorMoveCmd::Bottom),
            Motion::ParagraphUp => Some(CursorMoveCmd::ParagraphBack),
            Motion::ParagraphDown => Some(CursorMoveCmd::ParagraphForward),
            Motion::Line | Motion::ToEnd | Motion::ToStart => None,
        }
    }

    /// Handle key events in Normal mode.
    fn handle_normal_key(&mut self, key: KeyEvent) -> VimCommand {
        // Handle Ctrl+R for redo
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return match key.code {
                KeyCode::Char('r') => {
                    self.reset_count();
                    VimCommand::Redo
                }
                _ => VimCommand::None,
            };
        }

        // Ignore ALT combinations
        if key.modifiers.contains(KeyModifiers::ALT) {
            return VimCommand::None;
        }

        // Handle partial key sequences (gg)
        if let Some(partial) = self.partial_key.take() {
            if partial == 'g' {
                if key.code == KeyCode::Char('g') {
                    self.pending_operator = None;
                    let count = self.take_count();
                    // gg with no pending operator = move to top
                    return VimCommand::Move(CursorMoveCmd::Top);
                }
                // Any other key after 'g' -- clear partial and fall through to process normally
            }
        }

        match key.code {
            // Count prefix accumulation
            KeyCode::Char(c @ '1'..='9') => {
                let digit = c as usize - '0' as usize;
                self.count_prefix = Some(self.count_prefix.unwrap_or(0) * 10 + digit);
                VimCommand::None
            }
            KeyCode::Char('0') if self.count_prefix.is_some() => {
                self.count_prefix = Some(self.count_prefix.unwrap() * 10);
                VimCommand::None
            }

            // Motions -- if a pending operator is set, combine operator+motion
            KeyCode::Char('h') | KeyCode::Char('j') | KeyCode::Char('k') | KeyCode::Char('l')
            | KeyCode::Char('w') | KeyCode::Char('b') | KeyCode::Char('e')
            | KeyCode::Char('$') | KeyCode::Char('{') | KeyCode::Char('}') => {
                let count = self.take_count();
                let motion = self.key_to_motion(key.code).unwrap();

                if let Some(op) = self.pending_operator.take() {
                    // Operator + motion combination
                    match op {
                        Operator::Delete => VimCommand::Delete { motion },
                        Operator::Change => VimCommand::Change { motion },
                        Operator::Yank => VimCommand::Yank { motion },
                    }
                } else {
                    // Pure motion
                    let cmd = Self::motion_to_cursor_cmd(&motion).unwrap();
                    if count > 1 {
                        VimCommand::MoveN(cmd, count)
                    } else {
                        VimCommand::Move(cmd)
                    }
                }
            }

            // 0 with no count pending = line start
            KeyCode::Char('0') => {
                let motion = Motion::LineStart;
                if let Some(op) = self.pending_operator.take() {
                    match op {
                        Operator::Delete => VimCommand::Delete { motion },
                        Operator::Change => VimCommand::Change { motion },
                        Operator::Yank => VimCommand::Yank { motion },
                    }
                } else {
                    VimCommand::Move(CursorMoveCmd::Head)
                }
            }

            // G = go to bottom (or operator + FileEnd)
            KeyCode::Char('G') => {
                let _count = self.take_count();
                let motion = Motion::FileEnd;
                if let Some(op) = self.pending_operator.take() {
                    match op {
                        Operator::Delete => VimCommand::Delete { motion },
                        Operator::Change => VimCommand::Change { motion },
                        Operator::Yank => VimCommand::Yank { motion },
                    }
                } else {
                    VimCommand::Move(CursorMoveCmd::Bottom)
                }
            }

            // g -- start of gg sequence
            KeyCode::Char('g') => {
                if let Some(op) = &self.pending_operator {
                    // operator pending + g = wait for gg
                    self.partial_key = Some('g');
                    VimCommand::None
                } else {
                    self.partial_key = Some('g');
                    VimCommand::None
                }
            }

            // Operators: d, c, y
            KeyCode::Char('d') => {
                let count = self.take_count();
                if let Some(Operator::Delete) = self.pending_operator {
                    // dd = delete line(s)
                    self.pending_operator = None;
                    VimCommand::Delete { motion: Motion::Line }
                } else {
                    self.pending_operator = Some(Operator::Delete);
                    VimCommand::None
                }
            }
            KeyCode::Char('c') => {
                let count = self.take_count();
                if let Some(Operator::Change) = self.pending_operator {
                    // cc = change line(s)
                    self.pending_operator = None;
                    VimCommand::Change { motion: Motion::Line }
                } else {
                    self.pending_operator = Some(Operator::Change);
                    VimCommand::None
                }
            }
            KeyCode::Char('y') => {
                let count = self.take_count();
                if let Some(Operator::Yank) = self.pending_operator {
                    // yy = yank line(s)
                    self.pending_operator = None;
                    VimCommand::Yank { motion: Motion::Line }
                } else {
                    self.pending_operator = Some(Operator::Yank);
                    VimCommand::None
                }
            }

            // D = shortcut for d$ (delete to end of line)
            KeyCode::Char('D') => {
                self.reset_count();
                self.pending_operator = None;
                VimCommand::Delete { motion: Motion::ToEnd }
            }

            // C = shortcut for c$ (change to end of line)
            KeyCode::Char('C') => {
                self.reset_count();
                self.pending_operator = None;
                VimCommand::Change { motion: Motion::ToEnd }
            }

            // x = delete char under cursor
            KeyCode::Char('x') => {
                self.reset_count();
                self.pending_operator = None;
                VimCommand::DeleteChar
            }

            // u = undo
            KeyCode::Char('u') => {
                self.reset_count();
                self.pending_operator = None;
                VimCommand::Undo
            }

            // p/P = paste after/before
            KeyCode::Char('p') => {
                self.reset_count();
                self.pending_operator = None;
                VimCommand::PasteAfter
            }
            KeyCode::Char('P') => {
                self.reset_count();
                self.pending_operator = None;
                VimCommand::PasteBefore
            }

            // Insert mode entries
            KeyCode::Char('i') => {
                self.mode = VimMode::Insert;
                self.reset_count();
                self.pending_operator = None;
                VimCommand::EnterInsert(InsertPosition::BeforeCursor)
            }
            KeyCode::Char('a') => {
                self.mode = VimMode::Insert;
                self.reset_count();
                self.pending_operator = None;
                VimCommand::EnterInsert(InsertPosition::AfterCursor)
            }
            KeyCode::Char('I') => {
                self.mode = VimMode::Insert;
                self.reset_count();
                self.pending_operator = None;
                VimCommand::EnterInsert(InsertPosition::LineStart)
            }
            KeyCode::Char('A') => {
                self.mode = VimMode::Insert;
                self.reset_count();
                self.pending_operator = None;
                VimCommand::EnterInsert(InsertPosition::LineEnd)
            }
            KeyCode::Char('o') => {
                self.mode = VimMode::Insert;
                self.reset_count();
                self.pending_operator = None;
                VimCommand::EnterInsert(InsertPosition::NewLineBelow)
            }
            KeyCode::Char('O') => {
                self.mode = VimMode::Insert;
                self.reset_count();
                self.pending_operator = None;
                VimCommand::EnterInsert(InsertPosition::NewLineAbove)
            }

            // Command mode
            KeyCode::Char(':') => {
                self.mode = VimMode::Command;
                self.command_buffer.clear();
                self.reset_count();
                self.pending_operator = None;
                VimCommand::EnterCommand
            }

            // Visual mode
            KeyCode::Char('v') => {
                self.mode = VimMode::Visual { line_wise: false };
                self.reset_count();
                self.pending_operator = None;
                VimCommand::EnterVisual { line_wise: false }
            }
            KeyCode::Char('V') => {
                self.mode = VimMode::Visual { line_wise: true };
                self.reset_count();
                self.pending_operator = None;
                VimCommand::EnterVisual { line_wise: true }
            }

            // Search
            KeyCode::Char('/') => {
                self.reset_count();
                self.pending_operator = None;
                VimCommand::EnterSearch
            }

            // Esc in normal mode cancels pending state
            KeyCode::Esc => {
                self.reset_count();
                self.pending_operator = None;
                self.partial_key = None;
                VimCommand::None
            }

            // Unknown key with pending operator -- cancel operator (like real vim)
            _ => {
                self.reset_count();
                self.pending_operator = None;
                VimCommand::None
            }
        }
    }

    /// Handle key events in Insert mode.
    fn handle_insert_key(&mut self, key: KeyEvent) -> VimCommand {
        match key.code {
            KeyCode::Esc => {
                self.mode = VimMode::Normal;
                VimCommand::ExitInsert
            }
            // All other keys: return None so app.rs forwards to textarea
            _ => VimCommand::None,
        }
    }

    /// Handle key events in Visual mode.
    fn handle_visual_key(&mut self, key: KeyEvent) -> VimCommand {
        match key.code {
            KeyCode::Esc => {
                self.mode = VimMode::Normal;
                VimCommand::ExitVisual
            }
            // All other keys return None for now
            _ => VimCommand::None,
        }
    }

    /// Handle key events in Command mode.
    fn handle_command_key(&mut self, key: KeyEvent) -> VimCommand {
        match key.code {
            KeyCode::Esc => {
                self.mode = VimMode::Normal;
                self.command_buffer.clear();
                VimCommand::ExitCommand
            }
            KeyCode::Enter => {
                self.mode = VimMode::Normal;
                let cmd = self.command_buffer.clone();
                self.command_buffer.clear();
                self.parse_command(&cmd)
            }
            KeyCode::Backspace => {
                self.command_buffer.pop();
                if self.command_buffer.is_empty() {
                    // If buffer is empty after backspace, exit command mode
                    self.mode = VimMode::Normal;
                    VimCommand::ExitCommand
                } else {
                    VimCommand::CommandBackspace
                }
            }
            KeyCode::Char(c) => {
                self.command_buffer.push(c);
                VimCommand::CommandAppend(c)
            }
            _ => VimCommand::None,
        }
    }

    /// Parse a command string (typed after ':') into a VimCommand.
    fn parse_command(&self, cmd: &str) -> VimCommand {
        let cmd = cmd.trim();
        match cmd {
            "w" => VimCommand::Save,
            "q" => VimCommand::Quit { force: false },
            "q!" => VimCommand::Quit { force: true },
            "wq" | "x" => VimCommand::SaveAndQuit,
            _ => VimCommand::None,
        }
    }
}
