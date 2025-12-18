// Package config provides configuration management for mutagui.
package config

import (
	"os"
	"path/filepath"

	"github.com/pelletier/go-toml/v2"
)

// ThemeMode represents the theme selection mode.
type ThemeMode string

const (
	ThemeModeAuto  ThemeMode = "auto"
	ThemeModeLight ThemeMode = "light"
	ThemeModeDark  ThemeMode = "dark"
)

// DisplayMode represents the default display mode for sessions.
type DisplayMode string

const (
	DisplayModePaths       DisplayMode = "paths"
	DisplayModeLastRefresh DisplayMode = "lastrefresh"
)

// UIConfig contains UI-related settings.
type UIConfig struct {
	Theme              ThemeMode   `toml:"theme"`
	DefaultDisplayMode DisplayMode `toml:"default_display_mode"`
}

// RefreshConfig contains auto-refresh settings.
type RefreshConfig struct {
	Enabled      bool  `toml:"enabled"`
	IntervalSecs int64 `toml:"interval_secs"`
}

// ProjectConfig contains project discovery settings.
type ProjectConfig struct {
	SearchPaths     []string `toml:"search_paths"`
	ExcludePatterns []string `toml:"exclude_patterns"`
}

// Config represents the application configuration.
type Config struct {
	UI       UIConfig      `toml:"ui"`
	Refresh  RefreshConfig `toml:"refresh"`
	Projects ProjectConfig `toml:"projects"`
}

// DefaultConfig returns the default configuration.
func DefaultConfig() *Config {
	return &Config{
		UI: UIConfig{
			Theme:              ThemeModeAuto,
			DefaultDisplayMode: DisplayModePaths,
		},
		Refresh: RefreshConfig{
			Enabled:      true,
			IntervalSecs: 3,
		},
		Projects: ProjectConfig{
			SearchPaths:     []string{},
			ExcludePatterns: []string{"node_modules", ".git", "target"},
		},
	}
}

// Load loads the configuration from the standard config file location.
// Returns the default config if no config file exists.
func Load() (*Config, error) {
	path := configPath()
	if path == "" {
		return DefaultConfig(), nil
	}

	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return DefaultConfig(), nil
		}
		return nil, err
	}

	config := DefaultConfig()
	if err := toml.Unmarshal(data, config); err != nil {
		return nil, err
	}

	return config, nil
}

// configPath returns the standard config file path for the current platform.
func configPath() string {
	configDir, err := os.UserConfigDir()
	if err != nil {
		return ""
	}
	return filepath.Join(configDir, "mutagui", "config.toml")
}
