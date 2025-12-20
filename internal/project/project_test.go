package project

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/osteele/mutagui/internal/mutagen"
)

func TestSyncSpecState_IsRunning(t *testing.T) {
	tests := []struct {
		name  string
		state SyncSpecState
		want  bool
	}{
		{"not running", NotRunning, false},
		{"running two-way", RunningTwoWay, true},
		{"running push", RunningPush, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			spec := SyncSpec{State: tt.state}
			if got := spec.IsRunning(); got != tt.want {
				t.Errorf("IsRunning() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestSyncSpec_IsPaused(t *testing.T) {
	tests := []struct {
		name string
		spec SyncSpec
		want bool
	}{
		{
			name: "no session",
			spec: SyncSpec{},
			want: false,
		},
		{
			name: "session not paused",
			spec: SyncSpec{
				RunningSession: &mutagen.SyncSession{Paused: false},
			},
			want: false,
		},
		{
			name: "session paused",
			spec: SyncSpec{
				RunningSession: &mutagen.SyncSession{Paused: true},
			},
			want: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.spec.IsPaused(); got != tt.want {
				t.Errorf("IsPaused() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestProjectFile_DisplayName(t *testing.T) {
	tests := []struct {
		name string
		pf   ProjectFile
		want string
	}{
		{
			name: "with target name",
			pf: ProjectFile{
				Path:       "/home/user/myproject/mutagen.yml",
				TargetName: strPtr("cool30"),
			},
			want: "mutagen-cool30",
		},
		{
			name: "empty target name uses filename",
			pf: ProjectFile{
				Path:       "/home/user/myproject/mutagen.yml",
				TargetName: strPtr(""),
			},
			want: "mutagen",
		},
		{
			name: "no target name uses filename",
			pf: ProjectFile{
				Path: "/home/user/myproject/mutagen.yml",
			},
			want: "mutagen",
		},
		{
			name: "named project file",
			pf: ProjectFile{
				Path: "/home/user/.config/mutagen/projects/cool30-research.yml",
			},
			want: "cool30-research",
		},
		{
			name: "studio project file",
			pf: ProjectFile{
				Path: "/Users/osteele/.config/mutagen/projects/studio-research.yml",
			},
			want: "studio-research",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.pf.DisplayName(); got != tt.want {
				t.Errorf("DisplayName() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestNewProject(t *testing.T) {
	pf := ProjectFile{
		Path: "/test/mutagen.yml",
		Sessions: map[string]SessionDefinition{
			"session1": {Alpha: "/local", Beta: "server:/remote"},
			"session2": {Alpha: "/local2", Beta: "server:/remote2"},
		},
	}

	proj := NewProject(pf)

	if len(proj.Specs) != 2 {
		t.Errorf("NewProject() created %d specs, want 2", len(proj.Specs))
	}

	if !proj.Folded {
		t.Error("NewProject() should create folded project by default")
	}

	// Check that all specs are not running initially
	for _, spec := range proj.Specs {
		if spec.State != NotRunning {
			t.Errorf("NewProject() spec %q has state %v, want NotRunning", spec.Name, spec.State)
		}
	}
}

func TestProject_UpdateFromSessions(t *testing.T) {
	projectPath := "/test/mutagen.yml"
	proj := &Project{
		File: ProjectFile{Path: projectPath},
		Specs: []SyncSpec{
			{Name: "session1", State: NotRunning},
			{Name: "session2", State: NotRunning},
		},
	}

	// Simulate running sessions with project labels
	sessions := []mutagen.SyncSession{
		{Name: "session1", Status: "Watching", Labels: map[string]string{"project": projectPath}},
		{Name: "session2", Status: "Watching", Mode: strPtr("one-way-replica"), Labels: map[string]string{"project": projectPath}},
	}

	proj.UpdateFromSessions(sessions)

	// Find and check each spec
	for _, spec := range proj.Specs {
		if spec.Name == "session1" {
			if spec.State != RunningTwoWay {
				t.Errorf("session1 should be RunningTwoWay, got %v", spec.State)
			}
			if spec.RunningSession == nil {
				t.Error("session1 should have RunningSession set")
			}
		}
		if spec.Name == "session2" {
			if spec.State != RunningPush {
				t.Errorf("session2 should be RunningPush, got %v", spec.State)
			}
		}
	}
}

func TestProject_UpdateFromSessions_MatchesByName(t *testing.T) {
	proj := &Project{
		File: ProjectFile{Path: "/test/project1/mutagen.yml"},
		Specs: []SyncSpec{
			{Name: "session1", State: NotRunning},
		},
	}

	// Session matches by name (labels are ignored for matching)
	sessions := []mutagen.SyncSession{
		{Name: "session1", Status: "Watching", Labels: map[string]string{"project": "/different/project/mutagen.yml"}},
	}

	proj.UpdateFromSessions(sessions)

	// Session should be matched by name regardless of project label
	for _, spec := range proj.Specs {
		if spec.State != RunningTwoWay {
			t.Errorf("spec should be RunningTwoWay when name matches, got %v", spec.State)
		}
		if spec.RunningSession == nil {
			t.Error("RunningSession should be set when name matches")
		}
	}
}

func TestProject_UpdateFromSessions_NoMatch(t *testing.T) {
	proj := &Project{
		File: ProjectFile{Path: "/test/mutagen.yml"},
		Specs: []SyncSpec{
			{Name: "session1", State: RunningTwoWay, RunningSession: &mutagen.SyncSession{}},
		},
	}

	// No matching sessions - should reset to NotRunning
	proj.UpdateFromSessions([]mutagen.SyncSession{})

	for _, spec := range proj.Specs {
		if spec.State != NotRunning {
			t.Errorf("spec %q should be NotRunning after no matching sessions", spec.Name)
		}
		if spec.RunningSession != nil {
			t.Errorf("spec %q should have nil RunningSession", spec.Name)
		}
	}
}

func TestLoadProjectFile(t *testing.T) {
	// Create a temporary file
	tmpDir := t.TempDir()
	yamlPath := filepath.Join(tmpDir, "mutagen.yml")

	content := `targetName: "Test Project"
sync:
  web:
    alpha: "/local/path"
    beta: "server:/remote/path"
  api:
    alpha: "/local/api"
    beta: "server:/remote/api"
`
	if err := os.WriteFile(yamlPath, []byte(content), 0644); err != nil {
		t.Fatalf("Failed to write test file: %v", err)
	}

	pf, err := LoadProjectFile(yamlPath)
	if err != nil {
		t.Fatalf("LoadProjectFile() error = %v", err)
	}

	if pf.Path != yamlPath {
		t.Errorf("Path = %q, want %q", pf.Path, yamlPath)
	}

	if pf.TargetName == nil || *pf.TargetName != "Test Project" {
		t.Errorf("TargetName = %v, want 'Test Project'", pf.TargetName)
	}

	if len(pf.Sessions) != 2 {
		t.Errorf("Sessions count = %d, want 2", len(pf.Sessions))
	}

	if web, ok := pf.Sessions["web"]; !ok {
		t.Error("Missing 'web' session")
	} else {
		if web.Alpha != "/local/path" {
			t.Errorf("web.Alpha = %q, want '/local/path'", web.Alpha)
		}
	}
}

func TestLoadProjectFile_WithDefaults(t *testing.T) {
	tmpDir := t.TempDir()
	yamlPath := filepath.Join(tmpDir, "mutagen.yml")

	content := `targetName: "Test Project"
sync:
  defaults:
    ignore:
      paths:
        - ".git"
        - "node_modules"
      vcs: true
  web:
    alpha: "/local/path"
    beta: "server:/remote/path"
`
	if err := os.WriteFile(yamlPath, []byte(content), 0644); err != nil {
		t.Fatalf("Failed to write test file: %v", err)
	}

	pf, err := LoadProjectFile(yamlPath)
	if err != nil {
		t.Fatalf("LoadProjectFile() error = %v", err)
	}

	// Defaults should be extracted and not appear as a session
	if _, exists := pf.Sessions["defaults"]; exists {
		t.Error("'defaults' should not appear in Sessions map")
	}

	if len(pf.Sessions) != 1 {
		t.Errorf("Sessions count = %d, want 1 (only 'web')", len(pf.Sessions))
	}

	if pf.Defaults == nil {
		t.Fatal("Defaults should be populated")
	}

	if pf.Defaults.Ignore == nil {
		t.Fatal("Defaults.Ignore should be populated")
	}

	if len(pf.Defaults.Ignore.Paths) != 2 {
		t.Errorf("Defaults.Ignore.Paths count = %d, want 2", len(pf.Defaults.Ignore.Paths))
	}

	if pf.Defaults.Ignore.VCS == nil || !*pf.Defaults.Ignore.VCS {
		t.Error("Defaults.Ignore.VCS should be true")
	}
}

func TestLoadProjectFile_NotFound(t *testing.T) {
	_, err := LoadProjectFile("/nonexistent/path/mutagen.yml")
	if err == nil {
		t.Error("LoadProjectFile() should return error for nonexistent file")
	}
}

func TestLoadProjectFile_InvalidYAML(t *testing.T) {
	tmpDir := t.TempDir()
	yamlPath := filepath.Join(tmpDir, "mutagen.yml")

	content := `invalid: yaml: content: [[[`
	if err := os.WriteFile(yamlPath, []byte(content), 0644); err != nil {
		t.Fatalf("Failed to write test file: %v", err)
	}

	_, err := LoadProjectFile(yamlPath)
	if err == nil {
		t.Error("LoadProjectFile() should return error for invalid YAML")
	}
}

func TestFindProjects(t *testing.T) {
	// Isolate from real user config by setting HOME to temp dir
	origHome := os.Getenv("HOME")
	t.Cleanup(func() { os.Setenv("HOME", origHome) })
	os.Setenv("HOME", t.TempDir())

	// Create temporary directory structure
	tmpDir := t.TempDir()

	// Create project files at different depths
	proj1Dir := filepath.Join(tmpDir, "project1")
	proj2Dir := filepath.Join(tmpDir, "subdir", "project2")

	if err := os.MkdirAll(proj1Dir, 0755); err != nil {
		t.Fatalf("Failed to create dir: %v", err)
	}
	if err := os.MkdirAll(proj2Dir, 0755); err != nil {
		t.Fatalf("Failed to create dir: %v", err)
	}

	yaml1 := `sync:
  session1:
    alpha: "/local"
    beta: "server:/remote"
`
	if err := os.WriteFile(filepath.Join(proj1Dir, "mutagen.yml"), []byte(yaml1), 0644); err != nil {
		t.Fatalf("Failed to write file: %v", err)
	}
	if err := os.WriteFile(filepath.Join(proj2Dir, "mutagen.yaml"), []byte(yaml1), 0644); err != nil {
		t.Fatalf("Failed to write file: %v", err)
	}

	projects, err := FindProjects(tmpDir, nil, nil)
	if err != nil {
		t.Fatalf("FindProjects() error = %v", err)
	}

	if len(projects) != 2 {
		t.Errorf("FindProjects() found %d projects, want 2", len(projects))
	}
}

func TestFindProjects_ExcludePatterns(t *testing.T) {
	// Isolate from real user config by setting HOME to temp dir
	origHome := os.Getenv("HOME")
	t.Cleanup(func() { os.Setenv("HOME", origHome) })
	os.Setenv("HOME", t.TempDir())

	tmpDir := t.TempDir()

	// Create two project directories
	includedDir := filepath.Join(tmpDir, "included")
	excludedDir := filepath.Join(tmpDir, "node_modules", "excluded")

	if err := os.MkdirAll(includedDir, 0755); err != nil {
		t.Fatalf("Failed to create dir: %v", err)
	}
	if err := os.MkdirAll(excludedDir, 0755); err != nil {
		t.Fatalf("Failed to create dir: %v", err)
	}

	yaml1 := `sync:
  s:
    alpha: "/local"
    beta: "server:/remote"
`
	if err := os.WriteFile(filepath.Join(includedDir, "mutagen.yml"), []byte(yaml1), 0644); err != nil {
		t.Fatalf("Failed to write file: %v", err)
	}
	if err := os.WriteFile(filepath.Join(excludedDir, "mutagen.yml"), []byte(yaml1), 0644); err != nil {
		t.Fatalf("Failed to write file: %v", err)
	}

	projects, err := FindProjects(tmpDir, nil, []string{"node_modules"})
	if err != nil {
		t.Fatalf("FindProjects() error = %v", err)
	}

	if len(projects) != 1 {
		t.Errorf("FindProjects() found %d projects, want 1 (excluding node_modules)", len(projects))
	}
}

func TestFindProjects_DepthLimit(t *testing.T) {
	// Isolate from real user config by setting HOME to temp dir
	origHome := os.Getenv("HOME")
	t.Cleanup(func() { os.Setenv("HOME", origHome) })
	os.Setenv("HOME", t.TempDir())

	tmpDir := t.TempDir()

	// Create a deeply nested project (beyond max depth of 4)
	deepDir := filepath.Join(tmpDir, "a", "b", "c", "d", "e", "f")
	if err := os.MkdirAll(deepDir, 0755); err != nil {
		t.Fatalf("Failed to create dir: %v", err)
	}

	yaml1 := `sync:
  s:
    alpha: "/local"
    beta: "server:/remote"
`
	if err := os.WriteFile(filepath.Join(deepDir, "mutagen.yml"), []byte(yaml1), 0644); err != nil {
		t.Fatalf("Failed to write file: %v", err)
	}

	projects, err := FindProjects(tmpDir, nil, nil)
	if err != nil {
		t.Fatalf("FindProjects() error = %v", err)
	}

	// Should not find the deeply nested project
	if len(projects) != 0 {
		t.Errorf("FindProjects() found %d projects, want 0 (depth limit should exclude deep files)", len(projects))
	}
}

func TestUserConfigPaths(t *testing.T) {
	paths := UserConfigPaths()

	if len(paths) == 0 {
		t.Error("UserConfigPaths() returned empty slice")
	}

	// Should contain user config directories
	home, err := os.UserHomeDir()
	if err != nil {
		t.Skip("Cannot get home directory")
	}

	expectedPaths := []string{
		filepath.Join(home, ".config", "mutagen", "projects"),
		filepath.Join(home, ".mutagen", "projects"),
	}

	for _, expected := range expectedPaths {
		found := false
		for _, p := range paths {
			if p == expected {
				found = true
				break
			}
		}
		if !found {
			t.Errorf("UserConfigPaths() missing expected path %q", expected)
		}
	}
}

// Helper function
func strPtr(s string) *string {
	return &s
}
