use ratatui::style::Color;
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};

/// Set once at startup when the terminal can't display 24-bit color, so
/// colors produced outside the `Theme` struct (syntax highlighting) can be
/// downgraded too.
static DOWNGRADE_TO_256: AtomicBool = AtomicBool::new(false);

/// Record the terminal's color capability for [`adapt_color`].
pub fn set_color_capability(cap: ColorCapability) {
    DOWNGRADE_TO_256.store(cap == ColorCapability::Color256, Ordering::Relaxed);
}

/// Map a color to what the terminal can display (identity on truecolor).
pub fn adapt_color(color: Color) -> Color {
    if DOWNGRADE_TO_256.load(Ordering::Relaxed) {
        rgb_to_256(color)
    } else {
        color
    }
}

/// RGB components of a color, when it has a fixed value (truecolor or the
/// xterm 256-color cube / gray ramp). Named ANSI colors return `None`
/// because their appearance is defined by the terminal.
pub fn to_rgb(color: Color) -> Option<(u8, u8, u8)> {
    match color {
        Color::Rgb(r, g, b) => Some((r, g, b)),
        Color::Indexed(i @ 16..=231) => {
            let i = i - 16;
            Some((
                CUBE_LEVELS[usize::from(i / 36)],
                CUBE_LEVELS[usize::from(i / 6 % 6)],
                CUBE_LEVELS[usize::from(i % 6)],
            ))
        }
        Color::Indexed(i @ 232..=255) => {
            let v = 8 + 10 * (i - 232);
            Some((v, v, v))
        }
        _ => None,
    }
}

/// Channel values of the xterm 6x6x6 color cube.
const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

/// Terminal color capability detected from environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorCapability {
    TrueColor,
    Color256,
}

/// Detect terminal color capability from the environment.
///
/// `$COLORTERM` is the standard signal, but it is often lost over SSH or
/// inside tmux, so well-known truecolor terminals are also recognized by
/// `$TERM_PROGRAM` / `$TERM`.
pub fn detect_color_capability() -> ColorCapability {
    let var = |name: &str| std::env::var(name).unwrap_or_default().to_lowercase();
    capability_from_env(&var("COLORTERM"), &var("TERM_PROGRAM"), &var("TERM"))
}

fn capability_from_env(colorterm: &str, term_program: &str, term: &str) -> ColorCapability {
    const TRUECOLOR_PROGRAMS: [&str; 9] = [
        "iterm.app",
        "warpterminal",
        "ghostty",
        "wezterm",
        "vscode",
        "hyper",
        "tabby",
        "rio",
        "zed",
    ];
    const TRUECOLOR_TERMS: [&str; 6] =
        ["kitty", "alacritty", "ghostty", "wezterm", "foot", "direct"];

    if matches!(colorterm, "truecolor" | "24bit")
        || TRUECOLOR_PROGRAMS.contains(&term_program)
        || TRUECOLOR_TERMS.iter().any(|t| term.contains(t))
    {
        ColorCapability::TrueColor
    } else {
        ColorCapability::Color256
    }
}

/// Complete theme definition with all color fields used across the application.
///
/// Every hardcoded color in the codebase maps to a field here. `Option<Color>`
/// fields use `None` to mean "use terminal default".
#[derive(Debug, Clone)]
pub struct Theme {
    pub editor_bg: Option<Color>,
    pub editor_fg: Option<Color>,
    pub line_number_fg: Color,
    pub selection_bg: Color,
    pub search_active_bg: Color,
    pub search_active_fg: Color,
    pub search_match_bg: Color,
    pub search_match_fg: Color,
    pub divider_fg: Color,
    pub status_bar_bg: Color,
    pub status_bar_fg: Color,
    pub confirm_bg: Color,
    pub confirm_fg: Color,
    pub prompt_bg: Color,
    pub prompt_fg: Color,
    /// Legacy: name of a bundled syntect theme. Syntax colors are now derived
    /// from this theme's own palette; kept so existing configs still parse.
    #[allow(dead_code)]
    pub syntect_theme: String,
    pub preview_fg: Color,
    pub preview_muted_fg: Color,
    pub preview_link_fg: Color,
    pub preview_inline_code_fg: Color,
    pub preview_inline_code_bg: Color,
    pub preview_code_bg: Color,
    pub preview_indicator_fg: Color,
    pub preview_indicator_bg: Color,
    pub preview_accent_primary: Color,
    pub preview_accent_secondary: Color,
    pub preview_accent_tertiary: Color,
    pub preview_accent_warning: Color,
    pub preview_accent_danger: Color,
    // Vim mode indicator colors
    pub mode_normal_bg: Color,
    pub mode_insert_bg: Color,
    pub mode_visual_bg: Color,
    pub mode_command_bg: Color,
    /// Background tint of the line the editor cursor is on.
    pub cursor_line_bg: Color,
    /// Text color on filled chips (mode pill, prompt labels).
    pub chip_fg: Color,
}

impl Theme {
    /// Ocean theme -- the default. Matches the original hardcoded values.
    pub fn ocean() -> Self {
        Theme {
            editor_bg: None,
            editor_fg: Some(Color::Rgb(201, 209, 217)),
            line_number_fg: Color::Rgb(92, 99, 112),
            selection_bg: Color::Rgb(38, 79, 120),
            search_active_bg: Color::Rgb(255, 208, 96),
            search_active_fg: Color::Rgb(13, 17, 23),
            search_match_bg: Color::Rgb(77, 64, 22),
            search_match_fg: Color::Rgb(240, 225, 170),
            divider_fg: Color::Rgb(61, 68, 77),
            status_bar_bg: Color::Rgb(28, 33, 40),
            status_bar_fg: Color::Rgb(173, 186, 199),
            confirm_bg: Color::Rgb(248, 81, 73),
            confirm_fg: Color::Rgb(13, 17, 23),
            prompt_bg: Color::Rgb(88, 166, 255),
            prompt_fg: Color::Rgb(13, 17, 23),
            syntect_theme: "base16-ocean.dark".to_string(),
            preview_fg: Color::Rgb(230, 237, 243),
            preview_muted_fg: Color::Rgb(139, 148, 158),
            preview_link_fg: Color::Rgb(88, 166, 255),
            preview_inline_code_fg: Color::Rgb(255, 208, 96),
            preview_inline_code_bg: Color::Rgb(38, 44, 54),
            preview_code_bg: Color::Rgb(26, 31, 38),
            preview_indicator_fg: Color::Rgb(88, 166, 255),
            preview_indicator_bg: Color::Rgb(28, 42, 62),
            preview_accent_primary: Color::Rgb(88, 166, 255),
            preview_accent_secondary: Color::Rgb(126, 231, 135),
            preview_accent_tertiary: Color::Rgb(168, 131, 255),
            preview_accent_warning: Color::Rgb(255, 208, 96),
            preview_accent_danger: Color::Rgb(248, 81, 73),
            mode_normal_bg: Color::Rgb(88, 166, 255),
            mode_insert_bg: Color::Rgb(126, 231, 135),
            mode_visual_bg: Color::Rgb(168, 131, 255),
            mode_command_bg: Color::Rgb(255, 208, 96),
            cursor_line_bg: Color::Rgb(28, 33, 40),
            chip_fg: Color::Rgb(13, 17, 23),
        }
    }

    /// Dracula theme.
    pub fn dracula() -> Self {
        Theme {
            editor_bg: Some(Color::Rgb(40, 42, 54)),
            editor_fg: Some(Color::Rgb(248, 248, 242)),
            line_number_fg: Color::Rgb(98, 114, 164),
            selection_bg: Color::Rgb(68, 71, 90),
            search_active_bg: Color::Rgb(80, 250, 123),
            search_active_fg: Color::Rgb(40, 42, 54),
            search_match_bg: Color::Rgb(241, 250, 140),
            search_match_fg: Color::Rgb(40, 42, 54),
            divider_fg: Color::Rgb(86, 90, 112),
            status_bar_bg: Color::Rgb(52, 55, 70),
            status_bar_fg: Color::Rgb(248, 248, 242),
            confirm_bg: Color::Rgb(255, 85, 85),
            confirm_fg: Color::Rgb(40, 42, 54),
            prompt_bg: Color::Rgb(139, 233, 253),
            prompt_fg: Color::Rgb(40, 42, 54),
            syntect_theme: "Solarized (dark)".to_string(),
            preview_fg: Color::Rgb(248, 248, 242),
            preview_muted_fg: Color::Rgb(98, 114, 164),
            preview_link_fg: Color::Rgb(139, 233, 253),
            preview_inline_code_fg: Color::Rgb(241, 250, 140),
            preview_inline_code_bg: Color::Rgb(68, 71, 90),
            preview_code_bg: Color::Rgb(40, 42, 54),
            preview_indicator_fg: Color::Rgb(139, 233, 253),
            preview_indicator_bg: Color::Rgb(58, 61, 80),
            preview_accent_primary: Color::Rgb(139, 233, 253),
            preview_accent_secondary: Color::Rgb(80, 250, 123),
            preview_accent_tertiary: Color::Rgb(189, 147, 249),
            preview_accent_warning: Color::Rgb(255, 184, 108),
            preview_accent_danger: Color::Rgb(255, 85, 85),
            mode_normal_bg: Color::Rgb(189, 147, 249),
            mode_insert_bg: Color::Rgb(80, 250, 123),
            mode_visual_bg: Color::Rgb(255, 184, 108),
            mode_command_bg: Color::Rgb(139, 233, 253),
            cursor_line_bg: Color::Rgb(52, 55, 70),
            chip_fg: Color::Rgb(40, 42, 54),
        }
    }

    /// Solarized Light theme.
    pub fn solarized_light() -> Self {
        Theme {
            editor_bg: Some(Color::Rgb(253, 246, 227)),
            editor_fg: Some(Color::Rgb(101, 123, 131)),
            line_number_fg: Color::Rgb(147, 161, 161),
            selection_bg: Color::Rgb(220, 214, 194),
            search_active_bg: Color::Rgb(133, 153, 0),
            search_active_fg: Color::Rgb(253, 246, 227),
            search_match_bg: Color::Rgb(181, 137, 0),
            search_match_fg: Color::Rgb(253, 246, 227),
            divider_fg: Color::Rgb(147, 161, 161),
            status_bar_bg: Color::Rgb(238, 232, 213),
            status_bar_fg: Color::Rgb(88, 110, 117),
            confirm_bg: Color::Rgb(220, 50, 47),
            confirm_fg: Color::Rgb(253, 246, 227),
            prompt_bg: Color::Rgb(38, 139, 210),
            prompt_fg: Color::Rgb(253, 246, 227),
            syntect_theme: "Solarized (light)".to_string(),
            preview_fg: Color::Rgb(88, 110, 117),
            preview_muted_fg: Color::Rgb(147, 161, 161),
            preview_link_fg: Color::Rgb(38, 139, 210),
            preview_inline_code_fg: Color::Rgb(181, 137, 0),
            preview_inline_code_bg: Color::Rgb(238, 232, 213),
            preview_code_bg: Color::Rgb(248, 242, 223),
            preview_indicator_fg: Color::Rgb(38, 139, 210),
            preview_indicator_bg: Color::Rgb(233, 226, 203),
            preview_accent_primary: Color::Rgb(38, 139, 210),
            preview_accent_secondary: Color::Rgb(133, 153, 0),
            preview_accent_tertiary: Color::Rgb(108, 113, 196),
            preview_accent_warning: Color::Rgb(181, 137, 0),
            preview_accent_danger: Color::Rgb(220, 50, 47),
            mode_normal_bg: Color::Rgb(38, 139, 210),
            mode_insert_bg: Color::Rgb(133, 153, 0),
            mode_visual_bg: Color::Rgb(108, 113, 196),
            mode_command_bg: Color::Rgb(181, 137, 0),
            cursor_line_bg: Color::Rgb(238, 232, 213),
            chip_fg: Color::Rgb(253, 246, 227),
        }
    }

    /// Gruvbox Dark theme.
    pub fn gruvbox_dark() -> Self {
        Theme {
            editor_bg: Some(Color::Rgb(40, 40, 40)),
            editor_fg: Some(Color::Rgb(235, 219, 178)),
            line_number_fg: Color::Rgb(146, 131, 116),
            selection_bg: Color::Rgb(80, 73, 69),
            search_active_bg: Color::Rgb(184, 187, 38),
            search_active_fg: Color::Rgb(40, 40, 40),
            search_match_bg: Color::Rgb(250, 189, 47),
            search_match_fg: Color::Rgb(40, 40, 40),
            divider_fg: Color::Rgb(102, 92, 84),
            status_bar_bg: Color::Rgb(60, 56, 54),
            status_bar_fg: Color::Rgb(235, 219, 178),
            confirm_bg: Color::Rgb(251, 73, 52),
            confirm_fg: Color::Rgb(40, 40, 40),
            prompt_bg: Color::Rgb(131, 165, 152),
            prompt_fg: Color::Rgb(40, 40, 40),
            syntect_theme: "base16-ocean.dark".to_string(),
            preview_fg: Color::Rgb(235, 219, 178),
            preview_muted_fg: Color::Rgb(146, 131, 116),
            preview_link_fg: Color::Rgb(131, 165, 152),
            preview_inline_code_fg: Color::Rgb(250, 189, 47),
            preview_inline_code_bg: Color::Rgb(60, 56, 54),
            preview_code_bg: Color::Rgb(50, 48, 47),
            preview_indicator_fg: Color::Rgb(131, 165, 152),
            preview_indicator_bg: Color::Rgb(60, 56, 54),
            preview_accent_primary: Color::Rgb(131, 165, 152),
            preview_accent_secondary: Color::Rgb(184, 187, 38),
            preview_accent_tertiary: Color::Rgb(211, 134, 155),
            preview_accent_warning: Color::Rgb(250, 189, 47),
            preview_accent_danger: Color::Rgb(204, 36, 29),
            mode_normal_bg: Color::Rgb(131, 165, 152),
            mode_insert_bg: Color::Rgb(184, 187, 38),
            mode_visual_bg: Color::Rgb(211, 134, 155),
            mode_command_bg: Color::Rgb(250, 189, 47),
            cursor_line_bg: Color::Rgb(60, 56, 54),
            chip_fg: Color::Rgb(40, 40, 40),
        }
    }

    /// Look up a built-in theme by name (case-insensitive).
    pub fn by_name(name: &str) -> Option<Theme> {
        match name.to_lowercase().as_str() {
            "ocean" => Some(Theme::ocean()),
            "dracula" => Some(Theme::dracula()),
            "solarized-light" => Some(Theme::solarized_light()),
            "gruvbox-dark" => Some(Theme::gruvbox_dark()),
            _ => None,
        }
    }

    /// List all available built-in theme names.
    #[allow(dead_code)]
    pub fn available_themes() -> &'static [&'static str] {
        &["ocean", "dracula", "solarized-light", "gruvbox-dark"]
    }

    /// Return a clone of this theme with all Rgb colors mapped to their nearest
    /// 256-color (indexed) equivalents using the 6x6x6 color cube (indices 16-231).
    pub fn with_256_color_fallback(&self) -> Theme {
        let mut t = self.clone();
        t.editor_bg = t.editor_bg.map(rgb_to_256);
        t.editor_fg = t.editor_fg.map(rgb_to_256);
        t.line_number_fg = rgb_to_256(t.line_number_fg);
        t.selection_bg = rgb_to_256(t.selection_bg);
        t.search_active_bg = rgb_to_256(t.search_active_bg);
        t.search_active_fg = rgb_to_256(t.search_active_fg);
        t.search_match_bg = rgb_to_256(t.search_match_bg);
        t.search_match_fg = rgb_to_256(t.search_match_fg);
        t.divider_fg = rgb_to_256(t.divider_fg);
        t.status_bar_bg = rgb_to_256(t.status_bar_bg);
        t.status_bar_fg = rgb_to_256(t.status_bar_fg);
        t.confirm_bg = rgb_to_256(t.confirm_bg);
        t.confirm_fg = rgb_to_256(t.confirm_fg);
        t.prompt_bg = rgb_to_256(t.prompt_bg);
        t.prompt_fg = rgb_to_256(t.prompt_fg);
        t.preview_fg = rgb_to_256(t.preview_fg);
        t.preview_muted_fg = rgb_to_256(t.preview_muted_fg);
        t.preview_link_fg = rgb_to_256(t.preview_link_fg);
        t.preview_inline_code_fg = rgb_to_256(t.preview_inline_code_fg);
        t.preview_inline_code_bg = rgb_to_256(t.preview_inline_code_bg);
        t.preview_code_bg = rgb_to_256(t.preview_code_bg);
        t.preview_indicator_fg = rgb_to_256(t.preview_indicator_fg);
        t.preview_indicator_bg = rgb_to_256(t.preview_indicator_bg);
        t.preview_accent_primary = rgb_to_256(t.preview_accent_primary);
        t.preview_accent_secondary = rgb_to_256(t.preview_accent_secondary);
        t.preview_accent_tertiary = rgb_to_256(t.preview_accent_tertiary);
        t.preview_accent_warning = rgb_to_256(t.preview_accent_warning);
        t.preview_accent_danger = rgb_to_256(t.preview_accent_danger);
        t.mode_normal_bg = rgb_to_256(t.mode_normal_bg);
        t.mode_insert_bg = rgb_to_256(t.mode_insert_bg);
        t.mode_visual_bg = rgb_to_256(t.mode_visual_bg);
        t.mode_command_bg = rgb_to_256(t.mode_command_bg);
        t.cursor_line_bg = rgb_to_256(t.cursor_line_bg);
        t.chip_fg = rgb_to_256(t.chip_fg);
        t
    }

    /// Create a theme by overlaying custom colors on top of a base theme.
    /// Any `None` field in the custom colors leaves the base value unchanged.
    pub fn from_custom(base: &Theme, custom: &ThemeColors) -> Theme {
        let mut t = base.clone();
        if let Some(ref s) = custom.editor_bg {
            if let Some(c) = parse_color(s) {
                t.editor_bg = Some(c);
            }
        }
        if let Some(ref s) = custom.editor_fg {
            if let Some(c) = parse_color(s) {
                t.editor_fg = Some(c);
            }
        }
        if let Some(ref s) = custom.line_number_fg {
            if let Some(c) = parse_color(s) {
                t.line_number_fg = c;
            }
        }
        if let Some(ref s) = custom.selection_bg {
            if let Some(c) = parse_color(s) {
                t.selection_bg = c;
            }
        }
        if let Some(ref s) = custom.search_active_bg {
            if let Some(c) = parse_color(s) {
                t.search_active_bg = c;
            }
        }
        if let Some(ref s) = custom.search_active_fg {
            if let Some(c) = parse_color(s) {
                t.search_active_fg = c;
            }
        }
        if let Some(ref s) = custom.search_match_bg {
            if let Some(c) = parse_color(s) {
                t.search_match_bg = c;
            }
        }
        if let Some(ref s) = custom.search_match_fg {
            if let Some(c) = parse_color(s) {
                t.search_match_fg = c;
            }
        }
        if let Some(ref s) = custom.divider_fg {
            if let Some(c) = parse_color(s) {
                t.divider_fg = c;
            }
        }
        if let Some(ref s) = custom.status_bar_bg {
            if let Some(c) = parse_color(s) {
                t.status_bar_bg = c;
            }
        }
        if let Some(ref s) = custom.status_bar_fg {
            if let Some(c) = parse_color(s) {
                t.status_bar_fg = c;
            }
        }
        if let Some(ref s) = custom.confirm_bg {
            if let Some(c) = parse_color(s) {
                t.confirm_bg = c;
            }
        }
        if let Some(ref s) = custom.confirm_fg {
            if let Some(c) = parse_color(s) {
                t.confirm_fg = c;
            }
        }
        if let Some(ref s) = custom.prompt_bg {
            if let Some(c) = parse_color(s) {
                t.prompt_bg = c;
            }
        }
        if let Some(ref s) = custom.prompt_fg {
            if let Some(c) = parse_color(s) {
                t.prompt_fg = c;
            }
        }
        if let Some(ref s) = custom.syntect_theme {
            t.syntect_theme = s.clone();
        }
        if let Some(ref s) = custom.preview_fg {
            if let Some(c) = parse_color(s) {
                t.preview_fg = c;
            }
        }
        if let Some(ref s) = custom.preview_muted_fg {
            if let Some(c) = parse_color(s) {
                t.preview_muted_fg = c;
            }
        }
        if let Some(ref s) = custom.preview_link_fg {
            if let Some(c) = parse_color(s) {
                t.preview_link_fg = c;
            }
        }
        if let Some(ref s) = custom.preview_inline_code_fg {
            if let Some(c) = parse_color(s) {
                t.preview_inline_code_fg = c;
            }
        }
        if let Some(ref s) = custom.preview_inline_code_bg {
            if let Some(c) = parse_color(s) {
                t.preview_inline_code_bg = c;
            }
        }
        if let Some(ref s) = custom.preview_code_bg {
            if let Some(c) = parse_color(s) {
                t.preview_code_bg = c;
            }
        }
        if let Some(ref s) = custom.preview_indicator_fg {
            if let Some(c) = parse_color(s) {
                t.preview_indicator_fg = c;
            }
        }
        if let Some(ref s) = custom.preview_indicator_bg {
            if let Some(c) = parse_color(s) {
                t.preview_indicator_bg = c;
            }
        }
        if let Some(ref s) = custom.preview_accent_primary {
            if let Some(c) = parse_color(s) {
                t.preview_accent_primary = c;
            }
        }
        if let Some(ref s) = custom.preview_accent_secondary {
            if let Some(c) = parse_color(s) {
                t.preview_accent_secondary = c;
            }
        }
        if let Some(ref s) = custom.preview_accent_tertiary {
            if let Some(c) = parse_color(s) {
                t.preview_accent_tertiary = c;
            }
        }
        if let Some(ref s) = custom.preview_accent_warning {
            if let Some(c) = parse_color(s) {
                t.preview_accent_warning = c;
            }
        }
        if let Some(ref s) = custom.preview_accent_danger {
            if let Some(c) = parse_color(s) {
                t.preview_accent_danger = c;
            }
        }
        if let Some(ref s) = custom.mode_normal_bg {
            if let Some(c) = parse_color(s) {
                t.mode_normal_bg = c;
            }
        }
        if let Some(ref s) = custom.mode_insert_bg {
            if let Some(c) = parse_color(s) {
                t.mode_insert_bg = c;
            }
        }
        if let Some(ref s) = custom.mode_visual_bg {
            if let Some(c) = parse_color(s) {
                t.mode_visual_bg = c;
            }
        }
        if let Some(ref s) = custom.mode_command_bg {
            if let Some(c) = parse_color(s) {
                t.mode_command_bg = c;
            }
        }
        t
    }
}

/// Custom theme color definitions for TOML deserialization.
/// All fields are optional -- only specified colors override the base theme.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ThemeColors {
    pub editor_bg: Option<String>,
    pub editor_fg: Option<String>,
    pub line_number_fg: Option<String>,
    pub selection_bg: Option<String>,
    pub search_active_bg: Option<String>,
    pub search_active_fg: Option<String>,
    pub search_match_bg: Option<String>,
    pub search_match_fg: Option<String>,
    pub divider_fg: Option<String>,
    pub status_bar_bg: Option<String>,
    pub status_bar_fg: Option<String>,
    pub confirm_bg: Option<String>,
    pub confirm_fg: Option<String>,
    pub prompt_bg: Option<String>,
    pub prompt_fg: Option<String>,
    pub syntect_theme: Option<String>,
    pub preview_fg: Option<String>,
    pub preview_muted_fg: Option<String>,
    pub preview_link_fg: Option<String>,
    pub preview_inline_code_fg: Option<String>,
    pub preview_inline_code_bg: Option<String>,
    pub preview_code_bg: Option<String>,
    pub preview_indicator_fg: Option<String>,
    pub preview_indicator_bg: Option<String>,
    pub preview_accent_primary: Option<String>,
    pub preview_accent_secondary: Option<String>,
    pub preview_accent_tertiary: Option<String>,
    pub preview_accent_warning: Option<String>,
    pub preview_accent_danger: Option<String>,
    pub mode_normal_bg: Option<String>,
    pub mode_insert_bg: Option<String>,
    pub mode_visual_bg: Option<String>,
    pub mode_command_bg: Option<String>,
}

/// Parse a color string into a ratatui Color.
/// Supports `#RRGGBB` hex strings and named colors.
pub fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim();
    if s.starts_with('#') && s.len() == 7 {
        let r = u8::from_str_radix(&s[1..3], 16).ok()?;
        let g = u8::from_str_radix(&s[3..5], 16).ok()?;
        let b = u8::from_str_radix(&s[5..7], 16).ok()?;
        return Some(Color::Rgb(r, g, b));
    }
    match s.to_lowercase().as_str() {
        "red" => Some(Color::Red),
        "white" => Some(Color::White),
        "black" => Some(Color::Black),
        "blue" => Some(Color::Blue),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "cyan" => Some(Color::Cyan),
        "magenta" => Some(Color::Magenta),
        "gray" | "grey" => Some(Color::Gray),
        "darkgray" | "darkgrey" => Some(Color::DarkGray),
        _ => None,
    }
}

/// Map an Rgb color to the nearest 256-color indexed value (6x6x6 cube, indices 16-231).
/// Non-Rgb colors are returned unchanged.
fn rgb_to_256(color: Color) -> Color {
    let Color::Rgb(r, g, b) = color else {
        return color;
    };
    let dist = |(cr, cg, cb): (u8, u8, u8)| -> u32 {
        let d = |a: u8, b: u8| (i32::from(a) - i32::from(b)).pow(2) as u32;
        d(r, cr) + d(g, cg) + d(b, cb)
    };

    // Candidate 1: the 6x6x6 color cube.
    let (ri, gi, bi) = (
        nearest_cube_index(r),
        nearest_cube_index(g),
        nearest_cube_index(b),
    );
    let cube = 16 + 36 * ri + 6 * gi + bi;
    let cube_rgb = (
        CUBE_LEVELS[usize::from(ri)],
        CUBE_LEVELS[usize::from(gi)],
        CUBE_LEVELS[usize::from(bi)],
    );

    // Candidate 2: the 24-step gray ramp, which is far finer for the dark,
    // low-saturation surface colors themes use (the cube would crush them
    // all to black).
    let avg = (u16::from(r) + u16::from(g) + u16::from(b)) / 3;
    let step = ((avg.saturating_sub(8) + 5) / 10).min(23) as u8;
    let gray_value = 8 + 10 * step;

    if dist((gray_value, gray_value, gray_value)) < dist(cube_rgb) {
        Color::Indexed(232 + step)
    } else {
        Color::Indexed(cube)
    }
}

/// Map a 0-255 channel value to the nearest 6x6x6 cube index (0-5).
fn nearest_cube_index(val: u8) -> u8 {
    let mut best = 0u8;
    let mut best_dist = u16::MAX;
    for (i, &level) in CUBE_LEVELS.iter().enumerate() {
        let dist = (i16::from(val) - i16::from(level)).unsigned_abs();
        if dist < best_dist {
            best_dist = dist;
            best = i as u8;
        }
    }
    best
}
