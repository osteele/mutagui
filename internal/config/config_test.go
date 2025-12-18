package config

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/pelletier/go-toml/v2"
)

func TestDefaultConfig(t *testing.T) {
	cfg := DefaultConfig()

	// UI defaults
	if cfg.UI.Theme != ThemeModeAuto {
		t.Errorf("UI.Theme = %v, want %v", cfg.UI.Theme, ThemeModeAuto)
	}
	if cfg.UI.DefaultDisplayMode != DisplayModePaths {
		t.Errorf("UI.DefaultDisplayMode = %v, want %v", cfg.UI.DefaultDisplayMode, DisplayModePaths)
	}

	// Refresh defaults
	if !cfg.Refresh.Enabled {
		t.Error("Refresh.Enabled = false, want true")
	}
	if cfg.Refresh.IntervalSecs != 3 {
		t.Errorf("Refresh.IntervalSecs = %d, want 3", cfg.Refresh.IntervalSecs)
	}

	// Projects defaults
	if len(cfg.Projects.SearchPaths) != 0 {
		t.Errorf("Projects.SearchPaths = %v, want empty", cfg.Projects.SearchPaths)
	}
	expectedExclude := []string{"node_modules", ".git", "target"}
	if len(cfg.Projects.ExcludePatterns) != len(expectedExclude) {
		t.Errorf("Projects.ExcludePatterns length = %d, want %d",
			len(cfg.Projects.ExcludePatterns), len(expectedExclude))
	}
}

func TestLoad_NoConfigFile(t *testing.T) {
	// Load should return default config when no file exists
	cfg, err := Load()
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}

	defaultCfg := DefaultConfig()
	if cfg.UI.Theme != defaultCfg.UI.Theme {
		t.Errorf("UI.Theme = %v, want %v", cfg.UI.Theme, defaultCfg.UI.Theme)
	}
	if cfg.Refresh.IntervalSecs != defaultCfg.Refresh.IntervalSecs {
		t.Errorf("Refresh.IntervalSecs = %d, want %d",
			cfg.Refresh.IntervalSecs, defaultCfg.Refresh.IntervalSecs)
	}
}

func TestLoad_WithConfigFile(t *testing.T) {
	// Create a temporary config directory
	tmpDir := t.TempDir()

	// Override the config path by creating a config file in the test
	configDir := filepath.Join(tmpDir, "mutagui")
	if err := os.MkdirAll(configDir, 0755); err != nil {
		t.Fatalf("Failed to create config dir: %v", err)
	}

	configPath := filepath.Join(configDir, "config.toml")
	content := `
[ui]
theme = "dark"
default_display_mode = "lastrefresh"

[refresh]
enabled = false
interval_secs = 10

[projects]
search_paths = ["/home/user/projects", "/opt/code"]
exclude_patterns = ["vendor", "dist"]
`
	if err := os.WriteFile(configPath, []byte(content), 0644); err != nil {
		t.Fatalf("Failed to write config file: %v", err)
	}

	// Read the file directly since we can't override the config path function
	data, err := os.ReadFile(configPath)
	if err != nil {
		t.Fatalf("Failed to read config: %v", err)
	}

	cfg := DefaultConfig()
	if err := parseConfig(data, cfg); err != nil {
		t.Fatalf("parseConfig() error = %v", err)
	}

	// Check parsed values
	if cfg.UI.Theme != ThemeModeDark {
		t.Errorf("UI.Theme = %v, want %v", cfg.UI.Theme, ThemeModeDark)
	}
	if cfg.UI.DefaultDisplayMode != DisplayModeLastRefresh {
		t.Errorf("UI.DefaultDisplayMode = %v, want %v", cfg.UI.DefaultDisplayMode, DisplayModeLastRefresh)
	}
	if cfg.Refresh.Enabled {
		t.Error("Refresh.Enabled = true, want false")
	}
	if cfg.Refresh.IntervalSecs != 10 {
		t.Errorf("Refresh.IntervalSecs = %d, want 10", cfg.Refresh.IntervalSecs)
	}
	if len(cfg.Projects.SearchPaths) != 2 {
		t.Errorf("Projects.SearchPaths length = %d, want 2", len(cfg.Projects.SearchPaths))
	}
}

func TestThemeMode_Values(t *testing.T) {
	if ThemeModeAuto != "auto" {
		t.Errorf("ThemeModeAuto = %q, want 'auto'", ThemeModeAuto)
	}
	if ThemeModeLight != "light" {
		t.Errorf("ThemeModeLight = %q, want 'light'", ThemeModeLight)
	}
	if ThemeModeDark != "dark" {
		t.Errorf("ThemeModeDark = %q, want 'dark'", ThemeModeDark)
	}
}

func TestDisplayMode_Values(t *testing.T) {
	if DisplayModePaths != "paths" {
		t.Errorf("DisplayModePaths = %q, want 'paths'", DisplayModePaths)
	}
	if DisplayModeLastRefresh != "lastrefresh" {
		t.Errorf("DisplayModeLastRefresh = %q, want 'lastrefresh'", DisplayModeLastRefresh)
	}
}

func TestParseConfig_PartialOverride(t *testing.T) {
	// Test that partial config overrides only specified values
	content := `
[ui]
theme = "light"
`
	cfg := DefaultConfig()
	if err := parseConfig([]byte(content), cfg); err != nil {
		t.Fatalf("parseConfig() error = %v", err)
	}

	// Theme should be overridden
	if cfg.UI.Theme != ThemeModeLight {
		t.Errorf("UI.Theme = %v, want %v", cfg.UI.Theme, ThemeModeLight)
	}

	// Other values should remain defaults
	if cfg.UI.DefaultDisplayMode != DisplayModePaths {
		t.Errorf("UI.DefaultDisplayMode = %v, want default %v",
			cfg.UI.DefaultDisplayMode, DisplayModePaths)
	}
	if !cfg.Refresh.Enabled {
		t.Error("Refresh.Enabled should remain default true")
	}
}

func TestParseConfig_InvalidTOML(t *testing.T) {
	content := `invalid toml [[[`
	cfg := DefaultConfig()
	err := parseConfig([]byte(content), cfg)
	if err == nil {
		t.Error("parseConfig() should return error for invalid TOML")
	}
}

// parseConfig is a helper that mirrors the parsing logic in Load
func parseConfig(data []byte, cfg *Config) error {
	return toml.Unmarshal(data, cfg)
}
