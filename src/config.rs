use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::theme::{Theme, ThemeColors};

/// Editing mode for keybindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EditingMode {
    #[default]
    Vim,
    Nano,
}

/// Which view the app opens in (Ctrl+P cycles through them).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StartView {
    /// Edit inside the rendered document: only the block under the cursor
    /// shows raw markdown.
    #[default]
    Live,
    /// Raw editor and rendered preview side by side.
    Split,
    Editor,
    Preview,
}

impl StartView {
    pub fn from_name(name: &str) -> Option<StartView> {
        match name.to_lowercase().as_str() {
            "live" => Some(StartView::Live),
            "split" => Some(StartView::Split),
            "editor" | "edit" => Some(StartView::Editor),
            "preview" => Some(StartView::Preview),
            _ => None,
        }
    }
}

/// Markdown render profile — controls how the preview renders markdown
/// to match the target platform's style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RenderProfile {
    /// GitHub-flavored features with a minimal, Warp-inspired look: plain
    /// bold headings, outlined code blocks, borderless tables, reading margins
    #[default]
    Clean,
    /// GitHub Flavored Markdown: GFM tables, task lists, alerts, no wikilinks
    Github,
    /// Obsidian: callouts, wikilinks, tags, embeds
    Obsidian,
    /// Strict CommonMark: no extensions
    CommonMark,
}

impl RenderProfile {
    pub fn name(&self) -> &'static str {
        match self {
            RenderProfile::Clean => "clean",
            RenderProfile::Github => "github",
            RenderProfile::Obsidian => "obsidian",
            RenderProfile::CommonMark => "commonmark",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            RenderProfile::Clean => "Clean",
            RenderProfile::Github => "GitHub",
            RenderProfile::Obsidian => "Obsidian",
            RenderProfile::CommonMark => "CommonMark",
        }
    }

    pub fn all() -> &'static [RenderProfile] {
        &[
            RenderProfile::Clean,
            RenderProfile::Github,
            RenderProfile::Obsidian,
            RenderProfile::CommonMark,
        ]
    }

    pub fn from_name(name: &str) -> Option<RenderProfile> {
        match name.to_lowercase().as_str() {
            "clean" | "warp" => Some(RenderProfile::Clean),
            "github" | "gfm" => Some(RenderProfile::Github),
            "obsidian" => Some(RenderProfile::Obsidian),
            "commonmark" | "common" => Some(RenderProfile::CommonMark),
            _ => None,
        }
    }
}

/// Custom theme colors for TOML deserialization, mirrors ThemeColors from theme.rs.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct CustomThemeColors {
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

impl CustomThemeColors {
    /// Convert to the theme module's ThemeColors type.
    fn to_theme_colors(&self) -> ThemeColors {
        ThemeColors {
            editor_bg: self.editor_bg.clone(),
            editor_fg: self.editor_fg.clone(),
            line_number_fg: self.line_number_fg.clone(),
            selection_bg: self.selection_bg.clone(),
            search_active_bg: self.search_active_bg.clone(),
            search_active_fg: self.search_active_fg.clone(),
            search_match_bg: self.search_match_bg.clone(),
            search_match_fg: self.search_match_fg.clone(),
            divider_fg: self.divider_fg.clone(),
            status_bar_bg: self.status_bar_bg.clone(),
            status_bar_fg: self.status_bar_fg.clone(),
            confirm_bg: self.confirm_bg.clone(),
            confirm_fg: self.confirm_fg.clone(),
            prompt_bg: self.prompt_bg.clone(),
            prompt_fg: self.prompt_fg.clone(),
            syntect_theme: self.syntect_theme.clone(),
            preview_fg: self.preview_fg.clone(),
            preview_muted_fg: self.preview_muted_fg.clone(),
            preview_link_fg: self.preview_link_fg.clone(),
            preview_inline_code_fg: self.preview_inline_code_fg.clone(),
            preview_inline_code_bg: self.preview_inline_code_bg.clone(),
            preview_code_bg: self.preview_code_bg.clone(),
            preview_indicator_fg: self.preview_indicator_fg.clone(),
            preview_indicator_bg: self.preview_indicator_bg.clone(),
            preview_accent_primary: self.preview_accent_primary.clone(),
            preview_accent_secondary: self.preview_accent_secondary.clone(),
            preview_accent_tertiary: self.preview_accent_tertiary.clone(),
            preview_accent_warning: self.preview_accent_warning.clone(),
            preview_accent_danger: self.preview_accent_danger.clone(),
            mode_normal_bg: self.mode_normal_bg.clone(),
            mode_insert_bg: self.mode_insert_bg.clone(),
            mode_visual_bg: self.mode_visual_bg.clone(),
            mode_command_bg: self.mode_command_bg.clone(),
        }
    }
}

/// Application configuration loaded from ~/.config/mdedit/config.toml.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub theme: String,
    pub mode: EditingMode,
    #[serde(default)]
    pub render_profile: RenderProfile,
    #[serde(default = "default_true")]
    pub sync_indicator: bool,
    #[serde(default = "default_true")]
    pub word_wrap: bool,
    /// View to open in: "live" (default), "split", "editor" or "preview".
    #[serde(default)]
    pub view: StartView,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub custom_themes: HashMap<String, CustomThemeColors>,
}

fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Config {
            theme: "ocean".to_string(),
            mode: EditingMode::Vim,
            render_profile: RenderProfile::default(),
            sync_indicator: true,
            word_wrap: true,
            view: StartView::default(),
            custom_themes: HashMap::new(),
        }
    }
}

/// Config file locations in priority order: `$XDG_CONFIG_HOME/mdedit` or
/// `~/.config/mdedit` (the documented location on every platform), then the
/// OS-native config dir (`~/Library/Application Support` on macOS), which
/// earlier versions used.
fn config_candidates() -> Vec<std::path::PathBuf> {
    let mut dirs_in_order = Vec::new();
    match std::env::var_os("XDG_CONFIG_HOME") {
        Some(xdg) if !xdg.is_empty() => dirs_in_order.push(std::path::PathBuf::from(xdg)),
        _ => {
            if let Some(home) = dirs::home_dir() {
                dirs_in_order.push(home.join(".config"));
            }
        }
    }
    if let Some(native) = dirs::config_dir() {
        if !dirs_in_order.contains(&native) {
            dirs_in_order.push(native);
        }
    }
    dirs_in_order
        .into_iter()
        .map(|d| d.join("mdedit").join("config.toml"))
        .collect()
}

/// Load configuration from ~/.config/mdedit/config.toml.
/// Returns default config if file doesn't exist or can't be parsed.
pub fn load_config() -> Config {
    let Some(config_path) = config_candidates().into_iter().find(|p| p.exists()) else {
        return Config::default();
    };

    match std::fs::read_to_string(&config_path) {
        Ok(contents) => match toml::from_str::<Config>(&contents) {
            Ok(config) => config,
            Err(err) => {
                eprintln!("Warning: failed to parse config: {err}, using defaults");
                Config::default()
            }
        },
        Err(_) => Config::default(),
    }
}

/// Resolve a Theme from the config: try built-in names first, then custom themes,
/// then fall back to ocean with a warning.
pub fn resolve_theme(config: &Config) -> Theme {
    resolve_theme_name(config, &config.theme).unwrap_or_else(|| {
        eprintln!("Warning: unknown theme '{}', using ocean", config.theme);
        Theme::ocean()
    })
}

/// Resolve a theme by name using built-ins first, then custom theme definitions.
pub fn resolve_theme_name(config: &Config, theme_name: &str) -> Option<Theme> {
    // Try built-in theme
    if let Some(theme) = Theme::by_name(theme_name) {
        return Some(theme);
    }

    // Try custom theme from config
    if let Some(custom) = config.custom_themes.get(theme_name) {
        return Some(Theme::from_custom(
            &Theme::ocean(),
            &custom.to_theme_colors(),
        ));
    }

    None
}

/// Return built-in and custom theme names, keeping the built-ins first.
pub fn available_theme_names(config: &Config) -> Vec<String> {
    let mut names: Vec<String> = Theme::available_themes()
        .iter()
        .map(|name| (*name).to_string())
        .collect();

    let mut custom: Vec<String> = config.custom_themes.keys().cloned().collect();
    custom.sort();
    custom.retain(|name| !names.contains(name));
    names.extend(custom);
    names
}

/// Save configuration to ~/.config/mdedit/config.toml.
/// Creates the directory if it doesn't exist.
/// Returns Ok(path) on success or Err(message) on failure.
pub fn save_config(config: &Config) -> Result<String, String> {
    // Keep writing to an existing config wherever it lives; new configs go
    // to the documented ~/.config/mdedit location.
    let candidates = config_candidates();
    let config_path = candidates
        .iter()
        .find(|p| p.exists())
        .or(candidates.first())
        .ok_or_else(|| "Could not determine config directory".to_string())?;
    let mdedit_dir = config_path
        .parent()
        .ok_or_else(|| "Could not determine config directory".to_string())?
        .to_path_buf();

    std::fs::create_dir_all(&mdedit_dir)
        .map_err(|e| format!("Failed to create config dir: {}", e))?;

    let contents =
        toml::to_string_pretty(config).map_err(|e| format!("Failed to serialize config: {}", e))?;

    std::fs::write(config_path, contents).map_err(|e| format!("Failed to write config: {}", e))?;

    Ok(config_path.display().to_string())
}
