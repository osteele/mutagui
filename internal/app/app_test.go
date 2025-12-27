package app

import (
	"context"
	"errors"
	"testing"

	"github.com/osteele/mutagui/internal/config"
	"github.com/osteele/mutagui/internal/mutagen"
	"github.com/osteele/mutagui/internal/project"
	"github.com/osteele/mutagui/internal/ui"
)

func TestParseEndpoint(t *testing.T) {
	tests := []struct {
		name     string
		endpoint string
		wantType endpointType
		wantHost string
		wantPath string
	}{
		{
			name:     "local absolute path",
			endpoint: "/home/user/project",
			wantType: endpointLocal,
			wantHost: "",
			wantPath: "/home/user/project",
		},
		{
			name:     "local relative path",
			endpoint: "project/src",
			wantType: endpointLocal,
			wantHost: "",
			wantPath: "project/src",
		},
		{
			name:     "local home path",
			endpoint: "~/projects",
			wantType: endpointLocal,
			wantHost: "",
			wantPath: "~/projects",
		},
		{
			name:     "ssh endpoint",
			endpoint: "server:/path/to/dir",
			wantType: endpointSSH,
			wantHost: "server",
			wantPath: "/path/to/dir",
		},
		{
			name:     "ssh with user",
			endpoint: "user@server:/path/to/dir",
			wantType: endpointSSH,
			wantHost: "user@server",
			wantPath: "/path/to/dir",
		},
		{
			name:     "docker scheme",
			endpoint: "docker://container/path",
			wantType: endpointScheme,
			wantHost: "",
			wantPath: "docker://container/path",
		},
		{
			name:     "kubernetes scheme",
			endpoint: "kubernetes://namespace/pod:container/path",
			wantType: endpointScheme,
			wantHost: "",
			wantPath: "kubernetes://namespace/pod:container/path",
		},
		{
			name:     "windows drive letter (local)",
			endpoint: "C:/Users/test",
			wantType: endpointLocal,
			wantHost: "",
			wantPath: "C:/Users/test",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			epType, host, path := parseEndpoint(tt.endpoint)
			if epType != tt.wantType {
				t.Errorf("parseEndpoint() type = %v, want %v", epType, tt.wantType)
			}
			if host != tt.wantHost {
				t.Errorf("parseEndpoint() host = %q, want %q", host, tt.wantHost)
			}
			if path != tt.wantPath {
				t.Errorf("parseEndpoint() path = %q, want %q", path, tt.wantPath)
			}
		})
	}
}

func TestIsSSHEndpoint(t *testing.T) {
	tests := []struct {
		endpoint string
		want     bool
	}{
		{"server:/path", true},
		{"user@server:/path", true},
		{"/local/path", false},
		{"docker://container/path", false},
		{"C:/Windows/path", false},
	}

	for _, tt := range tests {
		t.Run(tt.endpoint, func(t *testing.T) {
			if got := isSSHEndpoint(tt.endpoint); got != tt.want {
				t.Errorf("isSSHEndpoint(%q) = %v, want %v", tt.endpoint, got, tt.want)
			}
		})
	}
}

func TestIsLocalEndpoint(t *testing.T) {
	tests := []struct {
		endpoint string
		want     bool
	}{
		{"/local/path", true},
		{"~/path", true},
		{"relative/path", true},
		{"server:/path", false},
		{"docker://container", false},
	}

	for _, tt := range tests {
		t.Run(tt.endpoint, func(t *testing.T) {
			if got := isLocalEndpoint(tt.endpoint); got != tt.want {
				t.Errorf("isLocalEndpoint(%q) = %v, want %v", tt.endpoint, got, tt.want)
			}
		})
	}
}

func TestParseEditorCommand(t *testing.T) {
	tests := []struct {
		cmd  string
		want []string
	}{
		{"vim", []string{"vim"}},
		{"code --wait", []string{"code", "--wait"}},
		{"subl -w", []string{"subl", "-w"}},
		{`code --user-data-dir "/path with spaces"`, []string{"code", "--user-data-dir", "/path with spaces"}},
		{`'/usr/bin/my editor'`, []string{"/usr/bin/my editor"}},
		{"  vim  -u  NONE  ", []string{"vim", "-u", "NONE"}},
	}

	for _, tt := range tests {
		t.Run(tt.cmd, func(t *testing.T) {
			got := parseEditorCommand(tt.cmd)
			if len(got) != len(tt.want) {
				t.Errorf("parseEditorCommand(%q) = %v, want %v", tt.cmd, got, tt.want)
				return
			}
			for i := range got {
				if got[i] != tt.want[i] {
					t.Errorf("parseEditorCommand(%q)[%d] = %q, want %q", tt.cmd, i, got[i], tt.want[i])
				}
			}
		})
	}
}

func TestIsGUIEditor(t *testing.T) {
	tests := []struct {
		editor string
		want   bool
	}{
		{"vim", false},
		{"nvim", false},
		{"nano", false},
		{"emacs", false},
		{"code", true},
		{"zed", true},
		{"subl", true},
		{"/usr/local/bin/code", true},
		{"/usr/bin/vim", false},
		{"unknown-editor", false}, // defaults to terminal
	}

	for _, tt := range tests {
		t.Run(tt.editor, func(t *testing.T) {
			// Clear env vars that could affect the test
			t.Setenv("MUTAGUI_EDITOR_IS_GUI", "")
			t.Setenv("SSH_CLIENT", "")
			t.Setenv("SSH_TTY", "")

			if got := IsGUIEditor(tt.editor); got != tt.want {
				t.Errorf("IsGUIEditor(%q) = %v, want %v", tt.editor, got, tt.want)
			}
		})
	}
}

func TestIsGUIEditor_SSHOverride(t *testing.T) {
	// Over SSH, GUI editors should be detected as terminal editors
	t.Setenv("SSH_CLIENT", "192.168.1.1 12345 22")
	t.Setenv("MUTAGUI_EDITOR_IS_GUI", "")

	if IsGUIEditor("code") {
		t.Error("IsGUIEditor('code') over SSH should return false")
	}
}

func TestIsGUIEditor_EnvOverride(t *testing.T) {
	// MUTAGUI_EDITOR_IS_GUI=1 should force GUI mode
	t.Setenv("MUTAGUI_EDITOR_IS_GUI", "1")
	t.Setenv("SSH_CLIENT", "")

	if !IsGUIEditor("vim") {
		t.Error("IsGUIEditor('vim') with MUTAGUI_EDITOR_IS_GUI=1 should return true")
	}
}

func TestGetEditor(t *testing.T) {
	// Test precedence: VISUAL > EDITOR > vim
	t.Run("VISUAL takes precedence", func(t *testing.T) {
		t.Setenv("VISUAL", "code")
		t.Setenv("EDITOR", "vim")
		if got := GetEditor(); got != "code" {
			t.Errorf("GetEditor() = %q, want 'code'", got)
		}
	})

	t.Run("EDITOR when no VISUAL", func(t *testing.T) {
		t.Setenv("VISUAL", "")
		t.Setenv("EDITOR", "nano")
		if got := GetEditor(); got != "nano" {
			t.Errorf("GetEditor() = %q, want 'nano'", got)
		}
	})

	t.Run("vim as default", func(t *testing.T) {
		t.Setenv("VISUAL", "")
		t.Setenv("EDITOR", "")
		if got := GetEditor(); got != "vim" {
			t.Errorf("GetEditor() = %q, want 'vim'", got)
		}
	})
}

func TestBuildSessionOptions(t *testing.T) {
	t.Run("empty definition", func(t *testing.T) {
		def := &project.SessionDefinition{}
		opts := buildSessionOptions(def, nil)
		if opts == nil {
			t.Fatal("buildSessionOptions() returned nil")
		}
		if opts.Mode != "" {
			t.Errorf("Mode = %q, want empty", opts.Mode)
		}
	})

	t.Run("with mode", func(t *testing.T) {
		mode := "two-way-safe"
		def := &project.SessionDefinition{Mode: &mode}
		opts := buildSessionOptions(def, nil)
		if opts.Mode != mode {
			t.Errorf("Mode = %q, want %q", opts.Mode, mode)
		}
	})

	t.Run("with ignore patterns", func(t *testing.T) {
		def := &project.SessionDefinition{
			Ignore: &project.IgnoreConfig{
				Paths: []string{"*.log", "tmp/"},
			},
		}
		opts := buildSessionOptions(def, nil)
		if len(opts.Ignore) != 2 {
			t.Errorf("Ignore length = %d, want 2", len(opts.Ignore))
		}
	})

	t.Run("merge defaults and definition ignore", func(t *testing.T) {
		defaults := &project.DefaultConfig{
			Ignore: &project.IgnoreConfig{
				Paths: []string{"vendor/"},
			},
		}
		def := &project.SessionDefinition{
			Ignore: &project.IgnoreConfig{
				Paths: []string{"*.log"},
			},
		}
		opts := buildSessionOptions(def, defaults)
		if len(opts.Ignore) != 2 {
			t.Errorf("Ignore length = %d, want 2", len(opts.Ignore))
		}
	})

	t.Run("ignore VCS from definition overrides defaults", func(t *testing.T) {
		falseVal := false
		trueVal := true
		defaults := &project.DefaultConfig{
			Ignore: &project.IgnoreConfig{
				VCS: &trueVal,
			},
		}
		def := &project.SessionDefinition{
			Ignore: &project.IgnoreConfig{
				VCS: &falseVal,
			},
		}
		opts := buildSessionOptions(def, defaults)
		if opts.IgnoreVCS == nil || *opts.IgnoreVCS != false {
			t.Error("IgnoreVCS should be false from definition")
		}
	})
}

func TestNewApp(t *testing.T) {
	cfg := config.DefaultConfig()
	app := NewApp(cfg)

	if app == nil {
		t.Fatal("NewApp() returned nil")
	}
	if app.Config != cfg {
		t.Error("Config not set correctly")
	}
	if app.Client == nil {
		t.Error("Client is nil")
	}
	if app.State == nil {
		t.Error("State is nil")
	}
	if app.State.Selection == nil {
		t.Error("Selection is nil")
	}
}

func TestSetStatus(t *testing.T) {
	app := NewApp(config.DefaultConfig())

	app.SetStatus(StatusInfo, "test message")
	if app.State.StatusMessage == nil {
		t.Fatal("StatusMessage is nil after SetStatus")
	}
	if app.State.StatusMessage.Text != "test message" {
		t.Errorf("Text = %q, want 'test message'", app.State.StatusMessage.Text)
	}
}

func TestClearStatus(t *testing.T) {
	app := NewApp(config.DefaultConfig())
	app.SetStatus(StatusInfo, "test")
	app.ClearStatus()
	if app.State.StatusMessage != nil {
		t.Error("StatusMessage should be nil after ClearStatus")
	}
}

func TestQuit(t *testing.T) {
	app := NewApp(config.DefaultConfig())
	if app.ShouldQuit() {
		t.Error("ShouldQuit() should be false initially")
	}
	app.Quit()
	if !app.ShouldQuit() {
		t.Error("ShouldQuit() should be true after Quit()")
	}
}

func TestToggleDisplayMode(t *testing.T) {
	app := NewApp(config.DefaultConfig())
	initial := app.State.ShowPaths
	app.ToggleDisplayMode()
	if app.State.ShowPaths == initial {
		t.Error("ShowPaths should be toggled")
	}
	app.ToggleDisplayMode()
	if app.State.ShowPaths != initial {
		t.Error("ShowPaths should be back to initial")
	}
}

func TestIsTerminalEditorError(t *testing.T) {
	if !IsTerminalEditorError(errTerminalEditor) {
		t.Error("IsTerminalEditorError should return true for errTerminalEditor")
	}
	if IsTerminalEditorError(nil) {
		t.Error("IsTerminalEditorError should return false for nil")
	}
	if IsTerminalEditorError(&customError{}) {
		t.Error("IsTerminalEditorError should return false for other errors")
	}
}

type customError struct{}

func (e *customError) Error() string { return "custom" }

// StatusInfo and other constants for tests
const (
	StatusInfo    = 0
	StatusWarning = 1
	StatusError   = 2
)

func TestGetConflictsForSelection_NilSelection(t *testing.T) {
	app := NewApp(config.DefaultConfig())
	// Empty selection should return nil
	conflicts := app.GetConflictsForSelection()
	if conflicts != nil {
		t.Error("GetConflictsForSelection should return nil for empty selection")
	}
}

func TestGetSelectedSession_NoSelection(t *testing.T) {
	app := NewApp(config.DefaultConfig())
	session := app.GetSelectedSession()
	if session != nil {
		t.Error("GetSelectedSession should return nil when nothing selected")
	}
}

func TestToggleProjectFold_OutOfBounds(t *testing.T) {
	app := NewApp(config.DefaultConfig())
	// Should not panic with invalid index
	app.ToggleProjectFold(-1)
	app.ToggleProjectFold(100)
}

// Test helper to create a project with running sessions
func createTestProject(name string, specs []string) *project.Project {
	proj := &project.Project{
		Specs: make([]project.SyncSpec, len(specs)),
	}
	for i, specName := range specs {
		proj.Specs[i] = project.SyncSpec{
			Name:  specName,
			State: project.NotRunning,
		}
	}
	return proj
}

func TestGetProjectConflicts_Empty(t *testing.T) {
	app := NewApp(config.DefaultConfig())
	app.State.Projects = []*project.Project{createTestProject("test", []string{"spec1"})}

	conflicts := app.getProjectConflicts(0)
	if len(conflicts) != 0 {
		t.Errorf("getProjectConflicts should return empty for project with no running sessions, got %d", len(conflicts))
	}
}

func TestGetSpecConflicts_NoSession(t *testing.T) {
	app := NewApp(config.DefaultConfig())
	app.State.Projects = []*project.Project{createTestProject("test", []string{"spec1"})}

	conflict := app.getSpecConflicts(0, 0)
	if conflict != nil {
		t.Error("getSpecConflicts should return nil for spec with no running session")
	}
}

func TestGetSpecConflicts_SessionNoConflicts(t *testing.T) {
	app := NewApp(config.DefaultConfig())
	proj := createTestProject("test", []string{"spec1"})
	proj.Specs[0].RunningSession = &mutagen.SyncSession{
		Name:      "spec1",
		Conflicts: nil,
	}
	app.State.Projects = []*project.Project{proj}

	conflict := app.getSpecConflicts(0, 0)
	if conflict != nil {
		t.Error("getSpecConflicts should return nil for spec with no conflicts")
	}
}

func TestGetSpecConflicts_WithConflicts(t *testing.T) {
	app := NewApp(config.DefaultConfig())
	proj := createTestProject("test", []string{"spec1"})
	proj.Specs[0].RunningSession = &mutagen.SyncSession{
		Name: "spec1",
		Conflicts: []mutagen.Conflict{
			{Root: "/path"},
		},
	}
	app.State.Projects = []*project.Project{proj}

	conflict := app.getSpecConflicts(0, 0)
	if conflict == nil {
		t.Fatal("getSpecConflicts should return conflict data")
	}
	if conflict.SpecName != "spec1" {
		t.Errorf("SpecName = %q, want 'spec1'", conflict.SpecName)
	}
	if len(conflict.Conflicts) != 1 {
		t.Errorf("Conflicts length = %d, want 1", len(conflict.Conflicts))
	}
}

// MockClient implements mutagen.MutagenClient for testing
type MockClient struct {
	// Track calls
	CreateSessionCalls     []CreateSessionCall
	CreatePushSessionCalls []CreateSessionCall
	TerminateCalls         []string
	PauseCalls             []string
	ResumeCalls            []string
	FlushCalls             []string
	ResetCalls             []string
	ListSessionsResult     []mutagen.SyncSession
	ListSessionsError      error

	// Control behavior
	CreateSessionError     error
	CreatePushSessionError error
	TerminateError         error
	PauseError             error
	ResumeError            error
	FlushError             error
}

type CreateSessionCall struct {
	Name, Alpha, Beta string
	Opts              *mutagen.SessionOptions
}

func (m *MockClient) ListSessions(ctx context.Context) ([]mutagen.SyncSession, error) {
	return m.ListSessionsResult, m.ListSessionsError
}

func (m *MockClient) CreateSession(ctx context.Context, name, alpha, beta string, opts *mutagen.SessionOptions) error {
	m.CreateSessionCalls = append(m.CreateSessionCalls, CreateSessionCall{name, alpha, beta, opts})
	return m.CreateSessionError
}

func (m *MockClient) CreatePushSession(ctx context.Context, name, alpha, beta string, opts *mutagen.SessionOptions) error {
	m.CreatePushSessionCalls = append(m.CreatePushSessionCalls, CreateSessionCall{name, alpha, beta, opts})
	return m.CreatePushSessionError
}

func (m *MockClient) TerminateSession(ctx context.Context, name string) error {
	m.TerminateCalls = append(m.TerminateCalls, name)
	return m.TerminateError
}

func (m *MockClient) PauseSession(ctx context.Context, name string) error {
	m.PauseCalls = append(m.PauseCalls, name)
	return m.PauseError
}

func (m *MockClient) ResumeSession(ctx context.Context, name string) error {
	m.ResumeCalls = append(m.ResumeCalls, name)
	return m.ResumeError
}

func (m *MockClient) FlushSession(ctx context.Context, name string) error {
	m.FlushCalls = append(m.FlushCalls, name)
	return m.FlushError
}

func (m *MockClient) ResetSession(ctx context.Context, name string) error {
	m.ResetCalls = append(m.ResetCalls, name)
	return nil
}

func (m *MockClient) ProjectStart(ctx context.Context, path string) error       { return nil }
func (m *MockClient) ProjectTerminate(ctx context.Context, path string) error   { return nil }
func (m *MockClient) ProjectPause(ctx context.Context, path string) error       { return nil }
func (m *MockClient) ProjectResume(ctx context.Context, path string) error      { return nil }
func (m *MockClient) ProjectFlush(ctx context.Context, path string) error       { return nil }
func (m *MockClient) IsInstalled() bool                                         { return true }
func (m *MockClient) GetVersion() (string, error)                               { return "0.0.0", nil }

// newTestApp creates an App with a mock client for testing
func newTestApp(mock *MockClient) *App {
	cfg := config.DefaultConfig()
	return &App{
		Config: cfg,
		Client: mock,
		State: &AppState{
			Projects:  []*project.Project{},
			Selection: ui.NewSelectionManager(),
			ShowPaths: true,
		},
	}
}

// createTestProjectWithFile creates a project with a proper File for testing
func createTestProjectWithFile(name string, specs []string) *project.Project {
	sessions := make(map[string]project.SessionDefinition)
	for _, specName := range specs {
		sessions[specName] = project.SessionDefinition{
			Alpha: "/local/path",
			Beta:  "/remote/path",
		}
	}

	proj := &project.Project{
		File: project.ProjectFile{
			Sessions: sessions,
		},
		Specs: make([]project.SyncSpec, len(specs)),
	}
	for i, specName := range specs {
		proj.Specs[i] = project.SyncSpec{
			Name:  specName,
			State: project.NotRunning,
		}
	}
	return proj
}

// ============================================================================
// Workflow Tests
// ============================================================================

func TestTerminateSelected_Spec(t *testing.T) {
	mock := &MockClient{}
	app := newTestApp(mock)

	// Setup: project with one running spec
	proj := createTestProjectWithFile("test-proj", []string{"spec1"})
	proj.Specs[0].State = project.RunningTwoWay
	proj.Specs[0].RunningSession = &mutagen.SyncSession{Name: "spec1"}
	app.State.Projects = []*project.Project{proj}
	app.State.Selection.RebuildFromProjects(app.State.Projects)

	// Expand project and select spec
	proj.Folded = false
	app.State.Selection.RebuildFromProjects(app.State.Projects)
	app.State.Selection.SelectNext() // Move to spec

	// Execute
	ctx := context.Background()
	app.TerminateSelected(ctx)

	// Verify
	if len(mock.TerminateCalls) != 1 {
		t.Errorf("TerminateCalls = %d, want 1", len(mock.TerminateCalls))
	}
	if mock.TerminateCalls[0] != "spec1" {
		t.Errorf("Terminated session = %q, want 'spec1'", mock.TerminateCalls[0])
	}
}

func TestTerminateSelected_Spec_NotRunning(t *testing.T) {
	mock := &MockClient{}
	app := newTestApp(mock)

	// Setup: project with one non-running spec
	proj := createTestProjectWithFile("test-proj", []string{"spec1"})
	app.State.Projects = []*project.Project{proj}
	app.State.Selection.RebuildFromProjects(app.State.Projects)

	// Expand and select spec
	proj.Folded = false
	app.State.Selection.RebuildFromProjects(app.State.Projects)
	app.State.Selection.SelectNext()

	// Execute
	ctx := context.Background()
	app.TerminateSelected(ctx)

	// Verify: should not call terminate
	if len(mock.TerminateCalls) != 0 {
		t.Errorf("TerminateCalls = %d, want 0 (session not running)", len(mock.TerminateCalls))
	}
	// Should set warning status
	if app.State.StatusMessage == nil || app.State.StatusMessage.Type != ui.StatusWarning {
		t.Error("Should set warning status for non-running session")
	}
}

func TestTerminateSelected_Project(t *testing.T) {
	mock := &MockClient{}
	app := newTestApp(mock)

	// Setup: project with two running specs
	proj := createTestProjectWithFile("test-proj", []string{"spec1", "spec2"})
	proj.Specs[0].State = project.RunningTwoWay
	proj.Specs[0].RunningSession = &mutagen.SyncSession{Name: "spec1"}
	proj.Specs[1].State = project.RunningTwoWay
	proj.Specs[1].RunningSession = &mutagen.SyncSession{Name: "spec2"}
	proj.Folded = true // Keep folded so project is selected
	app.State.Projects = []*project.Project{proj}
	app.State.Selection.RebuildFromProjects(app.State.Projects)

	// Execute
	ctx := context.Background()
	app.TerminateSelected(ctx)

	// Verify: both specs terminated
	if len(mock.TerminateCalls) != 2 {
		t.Errorf("TerminateCalls = %d, want 2", len(mock.TerminateCalls))
	}
}

func TestFlushSelected_Spec(t *testing.T) {
	mock := &MockClient{}
	app := newTestApp(mock)

	// Setup: project with one running spec
	proj := createTestProjectWithFile("test-proj", []string{"spec1"})
	proj.Specs[0].State = project.RunningTwoWay
	proj.Specs[0].RunningSession = &mutagen.SyncSession{Name: "spec1"}
	proj.Folded = false
	app.State.Projects = []*project.Project{proj}
	app.State.Selection.RebuildFromProjects(app.State.Projects)
	app.State.Selection.SelectNext() // Move to spec

	// Execute
	ctx := context.Background()
	app.FlushSelected(ctx)

	// Verify
	if len(mock.FlushCalls) != 1 {
		t.Errorf("FlushCalls = %d, want 1", len(mock.FlushCalls))
	}
	if mock.FlushCalls[0] != "spec1" {
		t.Errorf("Flushed session = %q, want 'spec1'", mock.FlushCalls[0])
	}
}

func TestTogglePauseSelected_Pause(t *testing.T) {
	mock := &MockClient{}
	app := newTestApp(mock)

	// Setup: project with one running (not paused) spec
	proj := createTestProjectWithFile("test-proj", []string{"spec1"})
	proj.Specs[0].State = project.RunningTwoWay
	proj.Specs[0].RunningSession = &mutagen.SyncSession{Name: "spec1", Paused: false}
	proj.Folded = false
	app.State.Projects = []*project.Project{proj}
	app.State.Selection.RebuildFromProjects(app.State.Projects)
	app.State.Selection.SelectNext() // Move to spec

	// Execute
	ctx := context.Background()
	app.TogglePauseSelected(ctx)

	// Verify: should pause
	if len(mock.PauseCalls) != 1 {
		t.Errorf("PauseCalls = %d, want 1", len(mock.PauseCalls))
	}
	if len(mock.ResumeCalls) != 0 {
		t.Errorf("ResumeCalls = %d, want 0", len(mock.ResumeCalls))
	}
}

func TestTogglePauseSelected_Resume(t *testing.T) {
	mock := &MockClient{}
	app := newTestApp(mock)

	// Setup: project with one paused spec
	proj := createTestProjectWithFile("test-proj", []string{"spec1"})
	proj.Specs[0].State = project.RunningTwoWay
	proj.Specs[0].RunningSession = &mutagen.SyncSession{Name: "spec1", Paused: true}
	proj.Folded = false
	app.State.Projects = []*project.Project{proj}
	app.State.Selection.RebuildFromProjects(app.State.Projects)
	app.State.Selection.SelectNext() // Move to spec

	// Execute
	ctx := context.Background()
	app.TogglePauseSelected(ctx)

	// Verify: should resume
	if len(mock.ResumeCalls) != 1 {
		t.Errorf("ResumeCalls = %d, want 1", len(mock.ResumeCalls))
	}
	if len(mock.PauseCalls) != 0 {
		t.Errorf("PauseCalls = %d, want 0", len(mock.PauseCalls))
	}
}

func TestRefreshSessions(t *testing.T) {
	mock := &MockClient{
		ListSessionsResult: []mutagen.SyncSession{
			{Name: "spec1", Status: "Watching"},
		},
	}
	app := newTestApp(mock)

	proj := createTestProjectWithFile("test-proj", []string{"spec1"})
	app.State.Projects = []*project.Project{proj}

	// Execute
	ctx := context.Background()
	err := app.RefreshSessions(ctx)

	// Verify
	if err != nil {
		t.Errorf("RefreshSessions() error = %v", err)
	}
	if app.State.LastRefresh == nil {
		t.Error("LastRefresh should be set")
	}
}

func TestRefreshSessions_Error(t *testing.T) {
	mock := &MockClient{
		ListSessionsError: errors.New("connection failed"),
	}
	app := newTestApp(mock)

	// Execute
	ctx := context.Background()
	err := app.RefreshSessions(ctx)

	// Verify
	if err == nil {
		t.Error("RefreshSessions() should return error")
	}
	if app.State.StatusMessage == nil || app.State.StatusMessage.Type != ui.StatusError {
		t.Error("Should set error status")
	}
}

func TestTerminateSelected_Error(t *testing.T) {
	mock := &MockClient{
		TerminateError: errors.New("terminate failed"),
	}
	app := newTestApp(mock)

	// Setup: project with one running spec
	proj := createTestProjectWithFile("test-proj", []string{"spec1"})
	proj.Specs[0].State = project.RunningTwoWay
	proj.Specs[0].RunningSession = &mutagen.SyncSession{Name: "spec1"}
	proj.Folded = false
	app.State.Projects = []*project.Project{proj}
	app.State.Selection.RebuildFromProjects(app.State.Projects)
	app.State.Selection.SelectNext()

	// Execute
	ctx := context.Background()
	app.TerminateSelected(ctx)

	// Verify: should set error status
	if app.State.StatusMessage == nil || app.State.StatusMessage.Type != ui.StatusError {
		t.Error("Should set error status on terminate failure")
	}
}

func TestFlushSelected_NotRunning(t *testing.T) {
	mock := &MockClient{}
	app := newTestApp(mock)

	// Setup: project with non-running spec
	proj := createTestProjectWithFile("test-proj", []string{"spec1"})
	proj.Folded = false
	app.State.Projects = []*project.Project{proj}
	app.State.Selection.RebuildFromProjects(app.State.Projects)
	app.State.Selection.SelectNext()

	// Execute
	ctx := context.Background()
	app.FlushSelected(ctx)

	// Verify: should not call flush, should set warning
	if len(mock.FlushCalls) != 0 {
		t.Errorf("FlushCalls = %d, want 0", len(mock.FlushCalls))
	}
	if app.State.StatusMessage == nil || app.State.StatusMessage.Type != ui.StatusWarning {
		t.Error("Should set warning status for non-running session")
	}
}

func TestResumeSelected_Spec(t *testing.T) {
	mock := &MockClient{}
	app := newTestApp(mock)

	// Setup: project with one running spec
	proj := createTestProjectWithFile("test-proj", []string{"spec1"})
	proj.Specs[0].State = project.RunningTwoWay
	proj.Specs[0].RunningSession = &mutagen.SyncSession{Name: "spec1"}
	proj.Folded = false
	app.State.Projects = []*project.Project{proj}
	app.State.Selection.RebuildFromProjects(app.State.Projects)
	app.State.Selection.SelectNext()

	// Execute
	ctx := context.Background()
	app.ResumeSelected(ctx)

	// Verify
	if len(mock.ResumeCalls) != 1 {
		t.Errorf("ResumeCalls = %d, want 1", len(mock.ResumeCalls))
	}
}
