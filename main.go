package main

import (
	"context"
	"flag"
	"fmt"
	"os"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/osteele/mutagui/internal/app"
	"github.com/osteele/mutagui/internal/config"
	"github.com/osteele/mutagui/internal/mutagen"
	"github.com/osteele/mutagui/internal/ui"
)

var (
	projectDir = flag.String("d", "", "Directory to search for mutagen project files (default: current directory)")
	showHelp   = flag.Bool("h", false, "Show help")
)

func main() {
	flag.StringVar(projectDir, "project-dir", "", "Directory to search for mutagen project files (default: current directory)")
	flag.BoolVar(showHelp, "help", false, "Show help")
	flag.Parse()

	if *showHelp {
		fmt.Println("mutagui - Terminal UI for managing Mutagen sync sessions")
		fmt.Println()
		fmt.Println("Usage: mutagui [options]")
		fmt.Println()
		fmt.Println("Options:")
		flag.PrintDefaults()
		os.Exit(0)
	}

	if err := run(); err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
}

func run() error {
	// Load configuration
	cfg, err := config.Load()
	if err != nil {
		return fmt.Errorf("failed to load config: %w", err)
	}

	// Create app
	mainApp := app.NewApp(cfg)

	// Check if mutagen is installed
	if !mainApp.Client.IsInstalled() {
		return fmt.Errorf("mutagen is not installed or not in PATH")
	}

	// Get theme
	theme := ui.GetTheme(string(cfg.UI.Theme))

	// Create model
	model := ui.NewModel(theme)

	// Load projects
	ctx := context.Background()
	if err := mainApp.LoadProjects(ctx, *projectDir); err != nil {
		model.StatusMessage = &ui.StatusMessage{Type: ui.StatusWarning, Text: "Failed to load some projects: " + err.Error()}
	}

	// Rebuild selection from projects
	mainApp.State.Selection.RebuildFromProjects(mainApp.State.Projects)

	// Share state between app and model
	model.Projects = mainApp.State.Projects
	model.Selection = mainApp.State.Selection
	model.ShowPaths = mainApp.State.ShowPaths

	// Initial session refresh
	if err := mainApp.RefreshSessions(ctx); err != nil {
		model.StatusMessage = &ui.StatusMessage{Type: ui.StatusWarning, Text: "Failed to refresh sessions: " + err.Error()}
	}
	model.LastRefresh = mainApp.State.LastRefresh

	// Set up callbacks
	model.OnRefresh = func(ctx context.Context) error {
		err := mainApp.RefreshSessions(ctx)
		model.LastRefresh = mainApp.State.LastRefresh
		if mainApp.State.StatusMessage != nil {
			model.StatusMessage = &ui.StatusMessage{
				Type: ui.StatusMessageType(mainApp.State.StatusMessage.Type),
				Text: mainApp.State.StatusMessage.Text,
			}
		}
		return err
	}

	model.OnStart = func(ctx context.Context) *ui.StatusMessage {
		if model.Selection.IsSpecSelected() {
			mainApp.StartSelectedSpec(ctx)
		} else {
			mainApp.StartSelectedProject(ctx)
		}
		return getStatus(mainApp)
	}

	model.OnTerminate = func(ctx context.Context) *ui.StatusMessage {
		mainApp.TerminateSelected(ctx)
		return getStatus(mainApp)
	}

	model.OnFlush = func(ctx context.Context) *ui.StatusMessage {
		mainApp.FlushSelected(ctx)
		return getStatus(mainApp)
	}

	model.OnPause = func(ctx context.Context) *ui.StatusMessage {
		mainApp.TogglePauseSelected(ctx)
		return getStatus(mainApp)
	}

	model.OnResume = func(ctx context.Context) *ui.StatusMessage {
		mainApp.ResumeSelected(ctx)
		return getStatus(mainApp)
	}

	model.OnPush = func(ctx context.Context) *ui.StatusMessage {
		if model.Selection.IsSpecSelected() {
			mainApp.PushSelectedSpec(ctx)
		} else {
			mainApp.PushSelectedProject(ctx)
		}
		return getStatus(mainApp)
	}

	model.OnPushConflicts = func(ctx context.Context) *ui.StatusMessage {
		mainApp.PushConflictsToBeta(ctx)
		return getStatus(mainApp)
	}

	model.OnPullConflicts = func(ctx context.Context) *ui.StatusMessage {
		mainApp.PullConflictsToAlpha(ctx)
		return getStatus(mainApp)
	}

	// Set confirmation preferences from config
	model.ConfirmPushToBeta = cfg.Confirmations.PushToBeta
	model.ConfirmPullToAlpha = cfg.Confirmations.PullToAlpha

	model.OnToggleFold = func(projIdx int) {
		mainApp.ToggleProjectFold(projIdx)
	}

	model.OnOpenEditor = func(projIdx int) error {
		return mainApp.OpenEditor(projIdx)
	}

	model.GetConflicts = func() []ui.SessionConflicts {
		return mainApp.GetConflictsForSelection()
	}

	model.GetSelectedSession = func() *mutagen.SyncSession {
		return mainApp.GetSelectedSession()
	}

	// Create program
	p := tea.NewProgram(model, tea.WithAltScreen(), tea.WithMouseCellMotion())

	// Set up auto-refresh
	if cfg.Refresh.Enabled {
		go func() {
			ticker := time.NewTicker(time.Duration(cfg.Refresh.IntervalSecs) * time.Second)
			defer ticker.Stop()

			for range ticker.C {
				if mainApp.ShouldQuit() {
					return
				}
				p.Send(ui.TickMsg(time.Now()))
			}
		}()
	}

	// Run the program
	if _, err := p.Run(); err != nil {
		return fmt.Errorf("application error: %w", err)
	}

	return nil
}

// getStatus returns the current status message from the app as a UI status message
func getStatus(mainApp *app.App) *ui.StatusMessage {
	if mainApp.State.StatusMessage != nil {
		return &ui.StatusMessage{
			Type: ui.StatusMessageType(mainApp.State.StatusMessage.Type),
			Text: mainApp.State.StatusMessage.Text,
		}
	}
	return nil
}
