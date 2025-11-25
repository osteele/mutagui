//! Configuration file support for mutagui.
//!
//! This module provides user configuration management, supporting TOML config files
//! in standard locations (XDG on Linux, ~/Library on macOS, etc.)

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// UI-related settings.
    pub ui: UiConfig,
    /// Auto-refresh settings.
    pub refresh: RefreshConfig,
    /// Project discovery settings.
    pub projects: ProjectConfig,
}

/// UI configuration options.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    /// Force a specific theme (light, dark, or auto).
    pub theme: ThemeMode,
    /// Show session paths or last refresh time by default.
    pub default_display_mode: DisplayMode,
}

/// Theme mode selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    /// Automatically detect terminal background.
    #[default]
    Auto,
    /// Force light theme.
    Light,
    /// Force dark theme.
    Dark,
}

/// Default display mode for sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DisplayMode {
    /// Show endpoint paths.
    #[default]
    Paths,
    /// Show last refresh time.
    LastRefresh,
}

/// Auto-refresh configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RefreshConfig {
    /// Enable auto-refresh when idle.
    pub enabled: bool,
    /// Refresh interval in seconds.
    pub interval_secs: u64,
}

/// Project discovery configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectConfig {
    /// Additional directories to search for mutagen.yml files.
    pub search_paths: Vec<PathBuf>,
    /// Directories to exclude from project discovery.
    pub exclude_patterns: Vec<String>,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: ThemeMode::Auto,
            default_display_mode: DisplayMode::Paths,
        }
    }
}

impl Default for RefreshConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_secs: 3,
        }
    }
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            search_paths: Vec::new(),
            exclude_patterns: vec![
                "node_modules".to_string(),
                ".git".to_string(),
                "target".to_string(),
            ],
        }
    }
}

impl Config {
    /// Load configuration from the standard config file location.
    ///
    /// Returns the default config if no config file exists.
    pub fn load() -> Result<Self> {
        if let Some(path) = Self::config_path() {
            if path.exists() {
                let contents = std::fs::read_to_string(&path)?;
                let config: Config = toml::from_str(&contents)?;
                return Ok(config);
            }
        }
        Ok(Self::default())
    }

    /// Get the standard config file path for the current platform.
    fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|mut path| {
            path.push("mutagui");
            path.push("config.toml");
            path
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.refresh.enabled);
        assert_eq!(config.refresh.interval_secs, 3);
        assert_eq!(config.ui.theme, ThemeMode::Auto);
        assert_eq!(config.ui.default_display_mode, DisplayMode::Paths);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();

        assert_eq!(config.refresh.enabled, parsed.refresh.enabled);
        assert_eq!(config.refresh.interval_secs, parsed.refresh.interval_secs);
    }

    #[test]
    fn test_parse_partial_config() {
        let toml_str = r#"
            [refresh]
            interval_secs = 5
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();

        // Specified value
        assert_eq!(config.refresh.interval_secs, 5);
        // Default values for unspecified fields
        assert!(config.refresh.enabled);
        assert_eq!(config.ui.theme, ThemeMode::Auto);
    }

    #[test]
    fn test_theme_mode_parsing() {
        let toml_str = r#"
            [ui]
            theme = "dark"
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.ui.theme, ThemeMode::Dark);
    }

    #[test]
    fn test_display_mode_parsing() {
        let toml_str = r#"
            [ui]
            default_display_mode = "lastrefresh"
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.ui.default_display_mode, DisplayMode::LastRefresh);
    }

    #[test]
    fn test_project_config_defaults() {
        let config = ProjectConfig::default();
        assert!(config.search_paths.is_empty());
        assert!(config
            .exclude_patterns
            .contains(&"node_modules".to_string()));
    }

    #[test]
    fn test_custom_search_paths() {
        let toml_str = r#"
            [projects]
            search_paths = ["/custom/path", "~/another"]
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.projects.search_paths.len(), 2);
    }
}
