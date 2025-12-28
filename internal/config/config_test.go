package config

import (
	"os"
	"path/filepath"
	"testing"
)

// withConfigPath temporarily overrides configPathFunc for a test.
func withConfigPath(t *testing.T, path string) {
	t.Helper()
	original := configPathFunc
	configPathFunc = func() string { return path }
	t.Cleanup(func() { configPathFunc = original })
}

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
	// Point to a non-existent path in a temp directory
	tmpDir := t.TempDir()
	withConfigPath(t, filepath.Join(tmpDir, "nonexistent", "config.toml"))

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
	// Create a temporary config file
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, "config.toml")

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

	// Point Load() to our temp config file
	withConfigPath(t, configPath)

	// Actually call Load() to test the full path
	cfg, err := Load()
	if err != nil {
		t.Fatalf("Load() error = %v", err)
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

func TestLoad_PartialOverride(t *testing.T) {
	// Test that partial config overrides only specified values
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, "config.toml")

	content := `
[ui]
theme = "light"
`
	if err := os.WriteFile(configPath, []byte(content), 0644); err != nil {
		t.Fatalf("Failed to write config file: %v", err)
	}

	withConfigPath(t, configPath)

	cfg, err := Load()
	if err != nil {
		t.Fatalf("Load() error = %v", err)
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

func TestLoad_InvalidTOML(t *testing.T) {
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, "config.toml")

	content := `invalid toml [[[`
	if err := os.WriteFile(configPath, []byte(content), 0644); err != nil {
		t.Fatalf("Failed to write config file: %v", err)
	}

	withConfigPath(t, configPath)

	_, err := Load()
	if err == nil {
		t.Error("Load() should return error for invalid TOML")
	}
}

func TestLoad_EmptyPath(t *testing.T) {
	// When configPathFunc returns empty string, Load should return defaults
	withConfigPath(t, "")

	cfg, err := Load()
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}

	defaultCfg := DefaultConfig()
	if cfg.UI.Theme != defaultCfg.UI.Theme {
		t.Errorf("UI.Theme = %v, want %v", cfg.UI.Theme, defaultCfg.UI.Theme)
	}
}

func TestLoad_Confirmations(t *testing.T) {
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, "config.toml")

	content := `
[confirmations]
push_to_beta = false
pull_to_alpha = true
`
	if err := os.WriteFile(configPath, []byte(content), 0644); err != nil {
		t.Fatalf("Failed to write config file: %v", err)
	}

	withConfigPath(t, configPath)

	cfg, err := Load()
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}

	if cfg.Confirmations.PushToBeta != false {
		t.Errorf("Confirmations.PushToBeta = %v, want false", cfg.Confirmations.PushToBeta)
	}
	if cfg.Confirmations.PullToAlpha != true {
		t.Errorf("Confirmations.PullToAlpha = %v, want true", cfg.Confirmations.PullToAlpha)
	}
}
