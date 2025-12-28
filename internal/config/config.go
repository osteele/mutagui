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

// ConfirmationsConfig contains settings for confirmation dialogs.
type ConfirmationsConfig struct {
	// PushToBeta controls whether to show confirmation before pushing to beta
	PushToBeta bool `toml:"push_to_beta"`
	// PullToAlpha controls whether to show confirmation before pulling to alpha
	PullToAlpha bool `toml:"pull_to_alpha"`
}

// Config represents the application configuration.
type Config struct {
	UI            UIConfig            `toml:"ui"`
	Refresh       RefreshConfig       `toml:"refresh"`
	Projects      ProjectConfig       `toml:"projects"`
	Confirmations ConfirmationsConfig `toml:"confirmations"`
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
		Confirmations: ConfirmationsConfig{
			PushToBeta:  true, // Confirm before pushing alpha → beta
			PullToAlpha: true, // Confirm before pulling beta → alpha
		},
	}
}

// configPathFunc is the function used to determine the config file path.
// It can be overridden in tests to control the config location.
var configPathFunc = defaultConfigPath

// Load loads the configuration from the standard config file location.
// Returns the default config if no config file exists.
func Load() (*Config, error) {
	path := configPathFunc()
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

// defaultConfigPath returns the standard config file path.
// Uses ~/.config/mutagui/config.toml following XDG conventions.
func defaultConfigPath() string {
	home, err := os.UserHomeDir()
	if err != nil {
		return ""
	}
	return filepath.Join(home, ".config", "mutagui", "config.toml")
}
