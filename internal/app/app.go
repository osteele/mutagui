// Package app provides the main application logic for mutagui.
package app

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"

	"github.com/osteele/mutagui/internal/config"
	"github.com/osteele/mutagui/internal/mutagen"
	"github.com/osteele/mutagui/internal/project"
	"github.com/osteele/mutagui/internal/ui"
)

// App represents the application state.
type App struct {
	Config *config.Config
	Client *mutagen.Client
	State  *ui.AppState

	shouldQuit bool
}

// NewApp creates a new App with the given configuration.
func NewApp(cfg *config.Config) *App {
	// Determine theme
	var colorScheme ui.ColorScheme
	switch cfg.UI.Theme {
	case config.ThemeModeLight:
		colorScheme = ui.LightTheme()
	case config.ThemeModeDark:
		colorScheme = ui.DarkTheme()
	default:
		colorScheme = ui.DetectTheme()
	}

	return &App{
		Config: cfg,
		Client: mutagen.NewClient(30 * time.Second),
		State: &ui.AppState{
			ColorScheme: colorScheme,
			Projects:    []*project.Project{},
			Selection:   ui.NewSelectionManager(),
			ShowPaths:   cfg.UI.DefaultDisplayMode == config.DisplayModePaths,
		},
	}
}

// LoadProjects loads projects from the configured search paths.
// If projectDir is specified, it takes priority over config search paths.
func (a *App) LoadProjects(ctx context.Context, projectDir string) error {
	searchPaths := a.Config.Projects.SearchPaths

	// If projectDir is specified, use it as the base directory
	// Otherwise use current directory as base, plus config search paths
	baseDir := projectDir
	if baseDir == "" {
		// Default to current directory
		if cwd, err := os.Getwd(); err == nil {
			baseDir = cwd
		}
	}

	projects, err := project.FindProjects(baseDir, searchPaths, a.Config.Projects.ExcludePatterns)
	if err != nil {
		return err
	}

	a.State.Projects = projects
	a.State.Selection.RebuildFromProjects(projects)
	return nil
}

// RefreshSessions fetches the latest session data and updates project states.
func (a *App) RefreshSessions(ctx context.Context) error {
	sessions, err := a.Client.ListSessions(ctx)
	if err != nil {
		a.SetStatus(ui.StatusError, "Failed to refresh sessions: "+err.Error())
		return err
	}

	// Update each project with session data
	for _, proj := range a.State.Projects {
		proj.UpdateFromSessions(sessions)
	}

	now := time.Now()
	a.State.LastRefresh = &now
	// Only update status to "refreshed" if there's no existing error/warning
	if a.State.StatusMessage == nil || a.State.StatusMessage.Type == ui.StatusInfo {
		a.SetStatus(ui.StatusInfo, "Sessions refreshed")
	}
	return nil
}

// SetStatus sets a status message.
func (a *App) SetStatus(msgType ui.StatusMessageType, text string) {
	a.State.StatusMessage = &ui.StatusMessage{Type: msgType, Text: text}
}

// ClearStatus clears the status message.
func (a *App) ClearStatus() {
	a.State.StatusMessage = nil
}

// Quit marks the application for quitting.
func (a *App) Quit() {
	a.shouldQuit = true
}

// ShouldQuit returns true if the application should quit.
func (a *App) ShouldQuit() bool {
	return a.shouldQuit
}

// ToggleHelp toggles the help view.
func (a *App) ToggleHelp() {
	a.State.ViewingHelp = !a.State.ViewingHelp
}

// ToggleConflictView toggles the conflict view.
func (a *App) ToggleConflictView() {
	a.State.ViewingConflicts = !a.State.ViewingConflicts
}

// ToggleSyncStatusView toggles the sync status view.
func (a *App) ToggleSyncStatusView() {
	a.State.ViewingSyncStatus = !a.State.ViewingSyncStatus
}

// ToggleDisplayMode toggles between showing paths and last refresh time.
func (a *App) ToggleDisplayMode() {
	a.State.ShowPaths = !a.State.ShowPaths
}

// SelectNext moves selection to the next item.
func (a *App) SelectNext() {
	a.State.Selection.SelectNext()
}

// SelectPrevious moves selection to the previous item.
func (a *App) SelectPrevious() {
	a.State.Selection.SelectPrevious()
}

// ToggleProjectFold toggles the fold state of the project at the given index.
func (a *App) ToggleProjectFold(projIdx int) {
	if projIdx >= 0 && projIdx < len(a.State.Projects) {
		a.State.Projects[projIdx].Folded = !a.State.Projects[projIdx].Folded
		a.State.Selection.RebuildFromProjects(a.State.Projects)
	}
}

// GetSelectedProjectIndex returns the index of the selected project.
func (a *App) GetSelectedProjectIndex() int {
	return a.State.Selection.SelectedProjectIndex()
}

// GetSelectedSpec returns the project and spec indices if a spec is selected.
func (a *App) GetSelectedSpec() (int, int) {
	return a.State.Selection.SelectedSpec()
}

// GetConflictsForSelection returns conflict data for the currently selected
// project/spec. When a project is selected, it aggregates conflicts for all
// running specs within that project.
func (a *App) GetConflictsForSelection() []ui.SessionConflicts {
	item := a.State.Selection.SelectedItem()
	if item == nil {
		return nil
	}

	switch item.Type {
	case ui.SelectableProject:
		return a.getProjectConflicts(item.ProjectIndex)
	case ui.SelectableSpec:
		conflict := a.getSpecConflicts(item.ProjectIndex, item.SpecIndex)
		if conflict == nil {
			return nil
		}
		return []ui.SessionConflicts{*conflict}
	default:
		return nil
	}
}

// getProjectConflicts gathers conflicts for all specs within a project.
func (a *App) getProjectConflicts(projIdx int) []ui.SessionConflicts {
	if projIdx < 0 || projIdx >= len(a.State.Projects) {
		return nil
	}

	proj := a.State.Projects[projIdx]
	conflicts := make([]ui.SessionConflicts, 0, len(proj.Specs))
	for specIdx := range proj.Specs {
		if sc := a.getSpecConflicts(projIdx, specIdx); sc != nil {
			conflicts = append(conflicts, *sc)
		}
	}
	return conflicts
}

// getSpecConflicts builds the conflict bundle for a single spec.
func (a *App) getSpecConflicts(projIdx, specIdx int) *ui.SessionConflicts {
	if projIdx < 0 || projIdx >= len(a.State.Projects) {
		return nil
	}
	proj := a.State.Projects[projIdx]
	if specIdx < 0 || specIdx >= len(proj.Specs) {
		return nil
	}

	spec := &proj.Specs[specIdx]
	if spec.RunningSession == nil {
		return nil
	}
	if len(spec.RunningSession.Conflicts) == 0 {
		return nil
	}

	return &ui.SessionConflicts{
		SpecName:  spec.Name,
		Session:   spec.RunningSession,
		Conflicts: spec.RunningSession.Conflicts,
	}
}

// GetSelectedSession returns the running session for the selected spec.
func (a *App) GetSelectedSession() *mutagen.SyncSession {
	projIdx, specIdx := a.GetSelectedSpec()
	if projIdx < 0 || specIdx < 0 {
		return nil
	}
	if projIdx >= len(a.State.Projects) {
		return nil
	}
	proj := a.State.Projects[projIdx]
	if specIdx >= len(proj.Specs) {
		return nil
	}
	spec := &proj.Specs[specIdx]
	return spec.RunningSession
}

// StartSelectedSpec starts the selected spec.
func (a *App) StartSelectedSpec(ctx context.Context) {
	projIdx, specIdx := a.GetSelectedSpec()
	if projIdx < 0 || specIdx < 0 {
		a.SetStatus(ui.StatusWarning, "No spec selected")
		return
	}

	proj := a.State.Projects[projIdx]
	spec := &proj.Specs[specIdx]

	// Skip if already running
	if spec.IsRunning() {
		a.SetStatus(ui.StatusInfo, spec.Name+" is already running")
		return
	}

	a.SetStatus(ui.StatusInfo, "Starting "+spec.Name+"...")

	sessionDef, exists := proj.File.Sessions[spec.Name]
	if !exists {
		a.SetStatus(ui.StatusError, "Session definition not found")
		return
	}

	// Terminate any existing sessions with this name to avoid duplicates
	// (may exist from previous runs or other sources)
	_ = a.Client.TerminateSession(ctx, spec.Name)

	// Prepare endpoint directories before creating session
	if err := prepareEndpoints(ctx, sessionDef.Alpha, sessionDef.Beta); err != nil {
		a.SetStatus(ui.StatusError, "Failed to prepare endpoints: "+err.Error())
		return
	}

	opts := buildSessionOptions(&sessionDef, proj.File.Defaults)
	err := a.Client.CreateSession(ctx, spec.Name, sessionDef.Alpha, sessionDef.Beta, opts)
	if err != nil {
		a.SetStatus(ui.StatusError, "Failed to start session: "+err.Error())
		return
	}
	a.SetStatus(ui.StatusInfo, "Started session: "+spec.Name)
}

// StartSelectedProject starts all non-running specs in the selected project.
func (a *App) StartSelectedProject(ctx context.Context) {
	projIdx := a.GetSelectedProjectIndex()
	if projIdx < 0 || projIdx >= len(a.State.Projects) {
		a.SetStatus(ui.StatusWarning, "No project selected")
		return
	}

	proj := a.State.Projects[projIdx]
	a.SetStatus(ui.StatusInfo, "Starting "+proj.File.DisplayName()+"...")

	// Start each non-running session individually
	// (mutagen project start fails if any session is already running)
	started := 0
	for i := range proj.Specs {
		spec := &proj.Specs[i]
		if spec.IsRunning() {
			continue // Skip already running sessions
		}

		sessionDef, exists := proj.File.Sessions[spec.Name]
		if !exists {
			continue
		}

		// Terminate any existing sessions with this name to avoid duplicates
		// (may exist from previous runs or other sources)
		_ = a.Client.TerminateSession(ctx, spec.Name)

		// Prepare endpoint directories before creating session
		if err := prepareEndpoints(ctx, sessionDef.Alpha, sessionDef.Beta); err != nil {
			a.SetStatus(ui.StatusError, "Failed to prepare endpoints for "+spec.Name+": "+err.Error())
			return
		}

		opts := buildSessionOptions(&sessionDef, proj.File.Defaults)
		if err := a.Client.CreateSession(ctx, spec.Name, sessionDef.Alpha, sessionDef.Beta, opts); err != nil {
			a.SetStatus(ui.StatusError, "Failed to start "+spec.Name+": "+err.Error())
			return
		}
		started++
	}

	if started == 0 {
		a.SetStatus(ui.StatusWarning, "All sessions already running")
	} else {
		a.SetStatus(ui.StatusInfo, fmt.Sprintf("Started %d session(s)", started))
	}
}

// TerminateSelected terminates the selected spec or all specs in the project.
func (a *App) TerminateSelected(ctx context.Context) {
	if a.State.Selection.IsSpecSelected() {
		projIdx, specIdx := a.GetSelectedSpec()
		if projIdx >= 0 && specIdx >= 0 {
			spec := &a.State.Projects[projIdx].Specs[specIdx]
			if spec.RunningSession == nil {
				a.SetStatus(ui.StatusWarning, "Session not running")
				return
			}
			sessionName := spec.RunningSession.Name
			a.SetStatus(ui.StatusInfo, "Terminating "+spec.Name+"...")
			if err := a.Client.TerminateSession(ctx, sessionName); err != nil {
				a.SetStatus(ui.StatusError, "Failed to terminate: "+err.Error())
				return
			}
			a.SetStatus(ui.StatusInfo, "Terminated session: "+spec.Name)
		}
	} else if a.State.Selection.IsProjectSelected() {
		projIdx := a.GetSelectedProjectIndex()
		if projIdx >= 0 && projIdx < len(a.State.Projects) {
			proj := a.State.Projects[projIdx]
			a.SetStatus(ui.StatusInfo, "Terminating "+proj.File.DisplayName()+"...")

			// Terminate each running session individually
			// This handles both regular sessions and push sessions correctly
			terminated := 0
			for i := range proj.Specs {
				spec := &proj.Specs[i]
				if spec.RunningSession != nil {
					if err := a.Client.TerminateSession(ctx, spec.RunningSession.Name); err != nil {
						a.SetStatus(ui.StatusError, "Failed to terminate "+spec.Name+": "+err.Error())
						return
					}
					terminated++
				}
			}

			if terminated == 0 {
				a.SetStatus(ui.StatusWarning, "No sessions running")
			} else {
				a.SetStatus(ui.StatusInfo, fmt.Sprintf("Terminated %d session(s)", terminated))
			}
		}
	}
}

// FlushSelected flushes the selected spec or all specs in the project.
func (a *App) FlushSelected(ctx context.Context) {
	if a.State.Selection.IsSpecSelected() {
		projIdx, specIdx := a.GetSelectedSpec()
		if projIdx >= 0 && specIdx >= 0 {
			spec := &a.State.Projects[projIdx].Specs[specIdx]
			if !spec.IsRunning() {
				a.SetStatus(ui.StatusWarning, "Session not running")
				return
			}
			sessionName := spec.RunningSession.Name
			a.SetStatus(ui.StatusInfo, "Flushing "+spec.Name+"...")
			if err := a.Client.FlushSession(ctx, sessionName); err != nil {
				a.SetStatus(ui.StatusError, "Failed to flush: "+err.Error())
				return
			}
			a.SetStatus(ui.StatusInfo, "Flushed session: "+spec.Name)
		}
	} else if a.State.Selection.IsProjectSelected() {
		projIdx := a.GetSelectedProjectIndex()
		if projIdx >= 0 && projIdx < len(a.State.Projects) {
			proj := a.State.Projects[projIdx]
			a.SetStatus(ui.StatusInfo, "Flushing "+proj.File.DisplayName()+"...")

			// Flush each running session individually
			flushed := 0
			for i := range proj.Specs {
				spec := &proj.Specs[i]
				if spec.RunningSession != nil {
					if err := a.Client.FlushSession(ctx, spec.RunningSession.Name); err != nil {
						a.SetStatus(ui.StatusError, "Failed to flush "+spec.Name+": "+err.Error())
						return
					}
					flushed++
				}
			}

			if flushed == 0 {
				a.SetStatus(ui.StatusWarning, "No sessions running")
			} else {
				a.SetStatus(ui.StatusInfo, fmt.Sprintf("Flushed %d session(s)", flushed))
			}
		}
	}
}

// TogglePauseSelected pauses or resumes the selected spec or all specs in the project.
func (a *App) TogglePauseSelected(ctx context.Context) {
	if a.State.Selection.IsSpecSelected() {
		projIdx, specIdx := a.GetSelectedSpec()
		if projIdx >= 0 && specIdx >= 0 {
			spec := &a.State.Projects[projIdx].Specs[specIdx]
			if spec.RunningSession == nil {
				a.SetStatus(ui.StatusWarning, "Session not running")
				return
			}
			sessionName := spec.RunningSession.Name
			if spec.RunningSession.Paused {
				if err := a.Client.ResumeSession(ctx, sessionName); err != nil {
					a.SetStatus(ui.StatusError, "Failed to resume: "+err.Error())
					return
				}
				a.SetStatus(ui.StatusInfo, "Resumed session: "+spec.Name)
			} else {
				if err := a.Client.PauseSession(ctx, sessionName); err != nil {
					a.SetStatus(ui.StatusError, "Failed to pause: "+err.Error())
					return
				}
				a.SetStatus(ui.StatusInfo, "Paused session: "+spec.Name)
			}
		}
	} else if a.State.Selection.IsProjectSelected() {
		projIdx := a.GetSelectedProjectIndex()
		if projIdx >= 0 && projIdx < len(a.State.Projects) {
			proj := a.State.Projects[projIdx]
			// Check if any are running and not paused
			hasRunning := false
			for _, spec := range proj.Specs {
				if spec.RunningSession != nil && !spec.RunningSession.Paused {
					hasRunning = true
					break
				}
			}

			if hasRunning {
				// Pause all running sessions individually
				paused := 0
				for i := range proj.Specs {
					spec := &proj.Specs[i]
					if spec.RunningSession != nil && !spec.RunningSession.Paused {
						if err := a.Client.PauseSession(ctx, spec.RunningSession.Name); err != nil {
							a.SetStatus(ui.StatusError, "Failed to pause "+spec.Name+": "+err.Error())
							return
						}
						paused++
					}
				}
				a.SetStatus(ui.StatusInfo, fmt.Sprintf("Paused %d session(s)", paused))
			} else {
				// Resume all paused sessions individually
				resumed := 0
				for i := range proj.Specs {
					spec := &proj.Specs[i]
					if spec.RunningSession != nil && spec.RunningSession.Paused {
						if err := a.Client.ResumeSession(ctx, spec.RunningSession.Name); err != nil {
							a.SetStatus(ui.StatusError, "Failed to resume "+spec.Name+": "+err.Error())
							return
						}
						resumed++
					}
				}
				if resumed == 0 {
					a.SetStatus(ui.StatusWarning, "No sessions to resume")
				} else {
					a.SetStatus(ui.StatusInfo, fmt.Sprintf("Resumed %d session(s)", resumed))
				}
			}
		}
	}
}

// ResumeSelected resumes the selected spec or all specs in the project.
func (a *App) ResumeSelected(ctx context.Context) {
	if a.State.Selection.IsSpecSelected() {
		projIdx, specIdx := a.GetSelectedSpec()
		if projIdx >= 0 && specIdx >= 0 {
			spec := &a.State.Projects[projIdx].Specs[specIdx]
			if spec.RunningSession == nil {
				a.SetStatus(ui.StatusWarning, "Session not running")
				return
			}
			sessionName := spec.RunningSession.Name
			if err := a.Client.ResumeSession(ctx, sessionName); err != nil {
				a.SetStatus(ui.StatusError, "Failed to resume: "+err.Error())
				return
			}
			a.SetStatus(ui.StatusInfo, "Resumed session: "+spec.Name)
		}
	} else if a.State.Selection.IsProjectSelected() {
		projIdx := a.GetSelectedProjectIndex()
		if projIdx >= 0 && projIdx < len(a.State.Projects) {
			proj := a.State.Projects[projIdx]
			resumed := 0
			for i := range proj.Specs {
				spec := &proj.Specs[i]
				if spec.RunningSession != nil {
					if err := a.Client.ResumeSession(ctx, spec.RunningSession.Name); err != nil {
						a.SetStatus(ui.StatusError, "Failed to resume "+spec.Name+": "+err.Error())
						return
					}
					resumed++
				}
			}

			if resumed == 0 {
				a.SetStatus(ui.StatusWarning, "No sessions to resume")
			} else {
				a.SetStatus(ui.StatusInfo, fmt.Sprintf("Resumed %d session(s)", resumed))
			}
		}
	}
}

// PushSelectedSpec creates a push session for the selected spec.
func (a *App) PushSelectedSpec(ctx context.Context) {
	projIdx, specIdx := a.GetSelectedSpec()
	if projIdx < 0 || specIdx < 0 {
		a.SetStatus(ui.StatusWarning, "No spec selected")
		return
	}

	proj := a.State.Projects[projIdx]
	spec := &proj.Specs[specIdx]
	a.SetStatus(ui.StatusInfo, "Creating push session for "+spec.Name+"...")

	sessionDef, exists := proj.File.Sessions[spec.Name]
	if !exists {
		a.SetStatus(ui.StatusError, "Session definition not found")
		return
	}

	// Terminate any existing sessions with this name to avoid duplicates
	// (handles both running sessions and stray duplicates)
	_ = a.Client.TerminateSession(ctx, spec.Name)

	// Prepare endpoint directories before creating session
	if err := prepareEndpoints(ctx, sessionDef.Alpha, sessionDef.Beta); err != nil {
		a.SetStatus(ui.StatusError, "Failed to prepare endpoints: "+err.Error())
		return
	}

	// Build session options from session definition and project defaults
	opts := buildSessionOptions(&sessionDef, proj.File.Defaults)

	err := a.Client.CreatePushSession(ctx, spec.Name, sessionDef.Alpha, sessionDef.Beta, opts)
	if err != nil {
		a.SetStatus(ui.StatusError, "Failed to create push session: "+err.Error())
		return
	}
	a.SetStatus(ui.StatusInfo, "Created push session: "+spec.Name)
}

// PushSelectedProject creates push sessions for all specs in the selected project.
func (a *App) PushSelectedProject(ctx context.Context) {
	projIdx := a.GetSelectedProjectIndex()
	if projIdx < 0 || projIdx >= len(a.State.Projects) {
		a.SetStatus(ui.StatusWarning, "No project selected")
		return
	}

	proj := a.State.Projects[projIdx]
	a.SetStatus(ui.StatusInfo, "Creating push sessions for "+proj.File.DisplayName()+"...")

	// Terminate all existing sessions first (project-level and by name to catch strays)
	_ = a.Client.ProjectTerminate(ctx, proj.File.Path)

	for _, spec := range proj.Specs {
		sessionDef, exists := proj.File.Sessions[spec.Name]
		if !exists {
			continue
		}

		// Terminate any stray sessions with this name
		_ = a.Client.TerminateSession(ctx, spec.Name)

		// Prepare endpoint directories before creating session
		if err := prepareEndpoints(ctx, sessionDef.Alpha, sessionDef.Beta); err != nil {
			a.SetStatus(ui.StatusError, "Failed to prepare endpoints for "+spec.Name+": "+err.Error())
			return
		}

		// Build session options from session definition and project defaults
		opts := buildSessionOptions(&sessionDef, proj.File.Defaults)

		if err := a.Client.CreatePushSession(ctx, spec.Name, sessionDef.Alpha, sessionDef.Beta, opts); err != nil {
			a.SetStatus(ui.StatusError, "Failed to create push session for "+spec.Name+": "+err.Error())
			return
		}
	}
	a.SetStatus(ui.StatusInfo, "Created push sessions for all specs in project")
}

// PushConflictsToBeta resolves conflicts by pushing alpha changes to beta.
// This terminates the existing session and creates a one-way push session.
func (a *App) PushConflictsToBeta(ctx context.Context) {
	projIdx, specIdx := a.GetSelectedSpec()
	if projIdx < 0 || specIdx < 0 {
		a.SetStatus(ui.StatusWarning, "No spec selected")
		return
	}

	proj := a.State.Projects[projIdx]
	spec := &proj.Specs[specIdx]
	sessionDef, exists := proj.File.Sessions[spec.Name]
	if !exists {
		a.SetStatus(ui.StatusError, "Session definition not found")
		return
	}

	// Terminate any existing sessions with this name to avoid duplicates
	_ = a.Client.TerminateSession(ctx, spec.Name)

	// Prepare endpoint directories
	if err := prepareEndpoints(ctx, sessionDef.Alpha, sessionDef.Beta); err != nil {
		a.SetStatus(ui.StatusError, "Failed to prepare endpoints: "+err.Error())
		return
	}

	// Build session options from session definition and project defaults
	opts := buildSessionOptions(&sessionDef, proj.File.Defaults)

	// Create a one-way push session to overwrite beta with alpha
	if err := a.Client.CreatePushSession(ctx, spec.Name, sessionDef.Alpha, sessionDef.Beta, opts); err != nil {
		a.SetStatus(ui.StatusError, "Failed to create push session: "+err.Error())
		return
	}
	a.SetStatus(ui.StatusInfo, "Created push session to resolve conflicts: "+spec.Name)
}

// GetEditor returns the configured editor from environment variables.
func GetEditor() string {
	if editor := os.Getenv("VISUAL"); editor != "" {
		return editor
	}
	if editor := os.Getenv("EDITOR"); editor != "" {
		return editor
	}
	return "vim"
}

// IsGUIEditor determines if an editor is a GUI editor (doesn't need terminal).
func IsGUIEditor(editorPath string) bool {
	// Check user override
	if val := os.Getenv("MUTAGUI_EDITOR_IS_GUI"); val == "1" || val == "true" {
		return true
	}

	// SSH detection (GUI won't work over SSH)
	if os.Getenv("SSH_CLIENT") != "" || os.Getenv("SSH_TTY") != "" {
		return false
	}

	// Known GUI editors
	guiEditors := []string{
		"code", "code-insiders", "zed", "subl", "sublime", "sublime_text",
		"atom", "gedit", "gnome-text-editor", "kwrite", "kate", "mousepad",
		"xed", "pluma", "bbedit", "textmate", "textedit", "xcode", "macvim", "gvim",
	}

	// Known terminal editors
	terminalEditors := []string{
		"vim", "vi", "nvim", "nano", "emacs", "emacsclient", "ed", "ex",
		"joe", "jed", "pico", "micro", "helix", "hx", "kakoune", "kak",
	}

	// Get just the basename
	editorName := editorPath
	for i := len(editorPath) - 1; i >= 0; i-- {
		if editorPath[i] == '/' {
			editorName = editorPath[i+1:]
			break
		}
	}
	editorName = toLower(editorName)

	for _, gui := range guiEditors {
		if contains(editorName, gui) {
			return true
		}
	}

	for _, term := range terminalEditors {
		if contains(editorName, term) {
			return false
		}
	}

	// Default to terminal editor
	return false
}

// OpenEditor opens the project file in an editor.
func (a *App) OpenEditor(projIdx int) error {
	if projIdx < 0 || projIdx >= len(a.State.Projects) {
		return nil
	}

	proj := a.State.Projects[projIdx]
	editor := GetEditor()
	filePath := proj.File.Path

	// Parse editor command into program and arguments
	editorParts := parseEditorCommand(editor)
	if len(editorParts) == 0 {
		a.SetStatus(ui.StatusError, "Invalid editor command")
		return nil
	}

	editorProgram := editorParts[0]
	editorArgs := append(editorParts[1:], filePath)

	if IsGUIEditor(editorProgram) {
		// GUI editor - spawn detached
		cmd := exec.Command(editorProgram, editorArgs...)
		if err := cmd.Start(); err != nil {
			a.SetStatus(ui.StatusError, "Failed to launch editor: "+err.Error())
			return err
		}
		a.SetStatus(ui.StatusInfo, "Opened in "+editorProgram+": "+proj.File.DisplayName())
		return nil
	}

	// Terminal editor needs special handling (suspend TUI)
	// This will be handled by the UI layer
	return errTerminalEditor
}

// GetEditorCommand returns the parsed editor command (program and args).
func GetEditorCommand() []string {
	return parseEditorCommand(GetEditor())
}

// errTerminalEditor is returned when a terminal editor needs to be launched.
var errTerminalEditor = &terminalEditorError{}

type terminalEditorError struct{}

func (e *terminalEditorError) Error() string {
	return "terminal editor requested"
}

// IsTerminalEditorError checks if an error indicates a terminal editor is needed.
func IsTerminalEditorError(err error) bool {
	_, ok := err.(*terminalEditorError)
	return ok
}

// Helper functions
func toLower(s string) string {
	result := make([]byte, len(s))
	for i := 0; i < len(s); i++ {
		c := s[i]
		if c >= 'A' && c <= 'Z' {
			c += 'a' - 'A'
		}
		result[i] = c
	}
	return string(result)
}

func contains(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}

// parseEditorCommand parses an editor command string into program and arguments.
// Handles simple shell-like quoting (single and double quotes).
func parseEditorCommand(cmd string) []string {
	var parts []string
	var current []byte
	var inSingleQuote, inDoubleQuote bool

	for i := 0; i < len(cmd); i++ {
		c := cmd[i]
		switch {
		case c == '\'' && !inDoubleQuote:
			inSingleQuote = !inSingleQuote
		case c == '"' && !inSingleQuote:
			inDoubleQuote = !inDoubleQuote
		case c == ' ' && !inSingleQuote && !inDoubleQuote:
			if len(current) > 0 {
				parts = append(parts, string(current))
				current = nil
			}
		default:
			current = append(current, c)
		}
	}
	if len(current) > 0 {
		parts = append(parts, string(current))
	}
	return parts
}

// buildSessionOptions creates SessionOptions from a SessionDefinition and project defaults.
func buildSessionOptions(def *project.SessionDefinition, defaults *project.DefaultConfig) *mutagen.SessionOptions {
	opts := &mutagen.SessionOptions{}

	// Apply mode from definition
	if def.Mode != nil {
		opts.Mode = *def.Mode
	}

	// Apply ignore patterns - merge defaults and definition
	if defaults != nil && defaults.Ignore != nil && defaults.Ignore.Paths != nil {
		opts.Ignore = append(opts.Ignore, defaults.Ignore.Paths...)
	}
	if def.Ignore != nil && def.Ignore.Paths != nil {
		opts.Ignore = append(opts.Ignore, def.Ignore.Paths...)
	}

	// Apply ignore VCS setting - definition overrides defaults
	if def.Ignore != nil && def.Ignore.VCS != nil {
		opts.IgnoreVCS = def.Ignore.VCS
	} else if defaults != nil && defaults.Ignore != nil && defaults.Ignore.VCS != nil {
		opts.IgnoreVCS = defaults.Ignore.VCS
	}

	// Apply symlink mode from Extra field
	if def.Extra != nil {
		if symlink, ok := def.Extra["symlink"].(map[string]interface{}); ok {
			if mode, ok := symlink["mode"].(string); ok {
				opts.SymlinkMode = mode
			}
		}
	}

	return opts
}

// endpointType represents the type of a mutagen endpoint.
type endpointType int

const (
	endpointLocal  endpointType = iota // Local filesystem path
	endpointSSH                        // SSH remote (host:path or user@host:path)
	endpointScheme                     // URL-style scheme (docker://, kubernetes://, etc.)
)

// parseEndpoint parses a mutagen endpoint string and returns its type, host, and path.
// URL-style schemes (docker://, kubernetes://) return endpointScheme.
// SSH endpoints (host:path) return endpointSSH with host and path.
// Local paths return endpointLocal with empty host.
func parseEndpoint(endpoint string) (epType endpointType, host, path string) {
	// Check for URL-style scheme (e.g., docker://container/path, kubernetes://namespace/pod:path)
	if strings.Contains(endpoint, "://") {
		return endpointScheme, "", endpoint
	}

	// Check for SSH-style remote endpoint (contains : but not Windows drive letter like C:)
	colonIdx := strings.Index(endpoint, ":")
	if colonIdx > 1 { // More than one char before colon (not a Windows drive)
		return endpointSSH, endpoint[:colonIdx], endpoint[colonIdx+1:]
	}

	// Local path
	return endpointLocal, "", endpoint
}

// isSSHEndpoint returns true if the endpoint is an SSH remote (host:path).
func isSSHEndpoint(endpoint string) bool {
	epType, _, _ := parseEndpoint(endpoint)
	return epType == endpointSSH
}

// isLocalEndpoint returns true if the endpoint is a local filesystem path.
func isLocalEndpoint(endpoint string) bool {
	epType, _, _ := parseEndpoint(endpoint)
	return epType == endpointLocal
}

// ensureLocalDirectory creates the local directory if it doesn't exist.
func ensureLocalDirectory(path string) error {
	// Expand ~ in path
	if strings.HasPrefix(path, "~/") {
		home, err := os.UserHomeDir()
		if err != nil {
			return fmt.Errorf("failed to get home directory: %w", err)
		}
		path = filepath.Join(home, path[2:])
	}
	return os.MkdirAll(path, 0755)
}

// prepareRemoteDirectory creates the directory on the remote host via SSH.
func prepareRemoteDirectory(ctx context.Context, host, path string) error {
	ctx, cancel := context.WithTimeout(ctx, 30*time.Second)
	defer cancel()

	cmd := exec.CommandContext(ctx, "ssh", host, "mkdir", "-p", path)
	if output, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("failed to create remote directory: %s", strings.TrimSpace(string(output)))
	}
	return nil
}

// prepareEndpoint prepares a single endpoint directory if applicable.
// Returns nil for URL-style schemes (docker://, kubernetes://) which are handled by Mutagen.
func prepareEndpoint(ctx context.Context, endpoint, label string) error {
	epType, host, path := parseEndpoint(endpoint)

	switch epType {
	case endpointLocal:
		if err := ensureLocalDirectory(path); err != nil {
			return fmt.Errorf("failed to prepare %s endpoint: %w", label, err)
		}
	case endpointSSH:
		if err := prepareRemoteDirectory(ctx, host, path); err != nil {
			return fmt.Errorf("failed to prepare %s endpoint: %w", label, err)
		}
	case endpointScheme:
		// URL-style schemes (docker://, kubernetes://, etc.) are handled by Mutagen
		// Skip directory preparation for these endpoints
	}

	return nil
}

// prepareEndpoints ensures both alpha and beta directories exist before creating a session.
// Skips preparation for URL-style endpoints (docker://, kubernetes://) which are handled by Mutagen.
func prepareEndpoints(ctx context.Context, alpha, beta string) error {
	if err := prepareEndpoint(ctx, alpha, "alpha"); err != nil {
		return err
	}
	if err := prepareEndpoint(ctx, beta, "beta"); err != nil {
		return err
	}
	return nil
}
