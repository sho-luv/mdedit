use ratatui::style::Color;
use serde::Deserialize;

/// Terminal color capability detected from environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorCapability {
    TrueColor,
    Color256,
}

/// Detect terminal color capability from $COLORTERM environment variable.
pub fn detect_color_capability() -> ColorCapability {
    match std::env::var("COLORTERM").as_deref() {
        Ok("truecolor") | Ok("24bit") => ColorCapability::TrueColor,
        _ => ColorCapability::Color256,
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
    pub syntect_theme: String,
    // Vim mode indicator colors
    pub mode_normal_bg: Color,
    pub mode_insert_bg: Color,
    pub mode_visual_bg: Color,
    pub mode_command_bg: Color,
}

impl Theme {
    /// Ocean theme -- the default. Matches the original hardcoded values.
    pub fn ocean() -> Self {
        Theme {
            editor_bg: None,
            editor_fg: None,
            line_number_fg: Color::DarkGray,
            selection_bg: Color::Rgb(68, 68, 102),
            search_active_bg: Color::Cyan,
            search_active_fg: Color::Black,
            search_match_bg: Color::Yellow,
            search_match_fg: Color::Black,
            divider_fg: Color::DarkGray,
            status_bar_bg: Color::DarkGray,
            status_bar_fg: Color::White,
            confirm_bg: Color::Red,
            confirm_fg: Color::White,
            prompt_bg: Color::Blue,
            prompt_fg: Color::White,
            syntect_theme: "base16-ocean.dark".to_string(),
            mode_normal_bg: Color::DarkGray,
            mode_insert_bg: Color::Rgb(0, 100, 0),
            mode_visual_bg: Color::Rgb(0, 0, 139),
            mode_command_bg: Color::Blue,
        }
    }

    /// Dracula theme.
    pub fn dracula() -> Self {
        Theme {
            editor_bg: None,
            editor_fg: Some(Color::Rgb(248, 248, 242)),
            line_number_fg: Color::Rgb(98, 114, 164),
            selection_bg: Color::Rgb(68, 71, 90),
            search_active_bg: Color::Rgb(80, 250, 123),
            search_active_fg: Color::Rgb(40, 42, 54),
            search_match_bg: Color::Rgb(241, 250, 140),
            search_match_fg: Color::Rgb(40, 42, 54),
            divider_fg: Color::Rgb(68, 71, 90),
            status_bar_bg: Color::Rgb(68, 71, 90),
            status_bar_fg: Color::Rgb(248, 248, 242),
            confirm_bg: Color::Rgb(255, 85, 85),
            confirm_fg: Color::Rgb(248, 248, 242),
            prompt_bg: Color::Rgb(98, 114, 164),
            prompt_fg: Color::Rgb(248, 248, 242),
            syntect_theme: "Solarized (dark)".to_string(),
            mode_normal_bg: Color::Rgb(68, 71, 90),
            mode_insert_bg: Color::Rgb(80, 250, 123),
            mode_visual_bg: Color::Rgb(189, 147, 249),
            mode_command_bg: Color::Rgb(98, 114, 164),
        }
    }

    /// Solarized Light theme.
    pub fn solarized_light() -> Self {
        Theme {
            editor_bg: None,
            editor_fg: Some(Color::Rgb(101, 123, 131)),
            line_number_fg: Color::Rgb(147, 161, 161),
            selection_bg: Color::Rgb(238, 232, 213),
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
            mode_normal_bg: Color::Rgb(238, 232, 213),
            mode_insert_bg: Color::Rgb(133, 153, 0),
            mode_visual_bg: Color::Rgb(38, 139, 210),
            mode_command_bg: Color::Rgb(38, 139, 210),
        }
    }

    /// Gruvbox Dark theme.
    pub fn gruvbox_dark() -> Self {
        Theme {
            editor_bg: None,
            editor_fg: Some(Color::Rgb(235, 219, 178)),
            line_number_fg: Color::Rgb(146, 131, 116),
            selection_bg: Color::Rgb(80, 73, 69),
            search_active_bg: Color::Rgb(184, 187, 38),
            search_active_fg: Color::Rgb(40, 40, 40),
            search_match_bg: Color::Rgb(250, 189, 47),
            search_match_fg: Color::Rgb(40, 40, 40),
            divider_fg: Color::Rgb(80, 73, 69),
            status_bar_bg: Color::Rgb(80, 73, 69),
            status_bar_fg: Color::Rgb(235, 219, 178),
            confirm_bg: Color::Rgb(204, 36, 29),
            confirm_fg: Color::Rgb(235, 219, 178),
            prompt_bg: Color::Rgb(69, 133, 136),
            prompt_fg: Color::Rgb(235, 219, 178),
            syntect_theme: "base16-ocean.dark".to_string(),
            mode_normal_bg: Color::Rgb(80, 73, 69),
            mode_insert_bg: Color::Rgb(152, 151, 26),
            mode_visual_bg: Color::Rgb(69, 133, 136),
            mode_command_bg: Color::Rgb(69, 133, 136),
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
        t.mode_normal_bg = rgb_to_256(t.mode_normal_bg);
        t.mode_insert_bg = rgb_to_256(t.mode_insert_bg);
        t.mode_visual_bg = rgb_to_256(t.mode_visual_bg);
        t.mode_command_bg = rgb_to_256(t.mode_command_bg);
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
    match color {
        Color::Rgb(r, g, b) => {
            // Map each channel to the 6-level cube (0-5)
            let ri = nearest_cube_index(r);
            let gi = nearest_cube_index(g);
            let bi = nearest_cube_index(b);
            Color::Indexed(16 + 36 * ri + 6 * gi + bi)
        }
        other => other,
    }
}

/// Map a 0-255 channel value to the nearest 6x6x6 cube index (0-5).
fn nearest_cube_index(val: u8) -> u8 {
    // The 6 cube levels correspond to values: 0, 95, 135, 175, 215, 255
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let mut best = 0u8;
    let mut best_dist = 255u16;
    for (i, &level) in LEVELS.iter().enumerate() {
        let dist = (val as i16 - level as i16).unsigned_abs();
        if dist < best_dist {
            best_dist = dist;
            best = i as u8;
        }
    }
    best
}
