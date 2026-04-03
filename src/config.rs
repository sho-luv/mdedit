use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::theme::{Theme, ThemeColors};

/// Editing mode for keybindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EditingMode {
    Vim,
    Nano,
}

impl Default for EditingMode {
    fn default() -> Self {
        EditingMode::Vim
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
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub custom_themes: HashMap<String, CustomThemeColors>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            theme: "ocean".to_string(),
            mode: EditingMode::Vim,
            custom_themes: HashMap::new(),
        }
    }
}

/// Load configuration from ~/.config/mdedit/config.toml.
/// Returns default config if file doesn't exist or can't be parsed.
pub fn load_config() -> Config {
    let config_dir = match dirs::config_dir() {
        Some(dir) => dir,
        None => return Config::default(),
    };
    let config_path = config_dir.join("mdedit").join("config.toml");

    if !config_path.exists() {
        return Config::default();
    }

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
    // Try built-in theme
    if let Some(theme) = Theme::by_name(&config.theme) {
        return theme;
    }

    // Try custom theme from config
    if let Some(custom) = config.custom_themes.get(&config.theme) {
        return Theme::from_custom(&Theme::ocean(), &custom.to_theme_colors());
    }

    // Unknown theme -- warn and use default
    eprintln!(
        "Warning: unknown theme '{}', using ocean",
        config.theme
    );
    Theme::ocean()
}

/// Save configuration to ~/.config/mdedit/config.toml.
/// Creates the directory if it doesn't exist.
/// Returns Ok(path) on success or Err(message) on failure.
pub fn save_config(config: &Config) -> Result<String, String> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| "Could not determine config directory".to_string())?;
    let mdedit_dir = config_dir.join("mdedit");

    std::fs::create_dir_all(&mdedit_dir)
        .map_err(|e| format!("Failed to create config dir: {}", e))?;

    let config_path = mdedit_dir.join("config.toml");
    let contents = toml::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    std::fs::write(&config_path, contents)
        .map_err(|e| format!("Failed to write config: {}", e))?;

    Ok(config_path.display().to_string())
}
