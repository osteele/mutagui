package main

import (
	"context"
	"flag"
	"fmt"
	"os"
	"os/exec"
	"time"

	"github.com/gdamore/tcell/v2"
	"github.com/osteele/mutagui/internal/app"
	"github.com/osteele/mutagui/internal/config"
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

	// Create view with the shared state
	view := ui.NewView(mainApp.State)

	// Load projects
	ctx := context.Background()
	if err := mainApp.LoadProjects(ctx, *projectDir); err != nil {
		mainApp.SetStatus(ui.StatusWarning, "Failed to load some projects: "+err.Error())
	}

	// Initial session refresh
	if err := mainApp.RefreshSessions(ctx); err != nil {
		mainApp.SetStatus(ui.StatusWarning, "Failed to refresh sessions: "+err.Error())
	}

	// Update UI
	view.RefreshList()
	view.UpdateStatus()

	// Set up input handler
	view.App.SetInputCapture(func(event *tcell.EventKey) *tcell.EventKey {
		return handleInput(view, mainApp, event)
	})

	// Set up auto-refresh
	if cfg.Refresh.Enabled {
		go autoRefresh(view, mainApp, time.Duration(cfg.Refresh.IntervalSecs)*time.Second)
	}

	// Run the application
	if err := view.App.Run(); err != nil {
		return fmt.Errorf("application error: %w", err)
	}

	return nil
}

func handleInput(view *ui.View, mainApp *app.App, event *tcell.EventKey) *tcell.EventKey {
	ctx := context.Background()

	// Handle Escape to close modals
	if event.Key() == tcell.KeyEscape {
		if mainApp.State.ViewingHelp {
			mainApp.ToggleHelp()
			view.HideHelpModal()
			return nil
		}
		if mainApp.State.ViewingConflicts {
			mainApp.ToggleConflictView()
			view.HideConflictModal()
			return nil
		}
		if mainApp.State.ViewingSyncStatus {
			mainApp.ToggleSyncStatusView()
			view.HideSyncStatusModal()
			return nil
		}
	}

	// Handle Ctrl-C
	if event.Key() == tcell.KeyCtrlC {
		mainApp.Quit()
		view.App.Stop()
		return nil
	}

	switch event.Key() {
	case tcell.KeyUp:
		mainApp.SelectPrevious()
		view.RefreshList()
		view.UpdateStatus()
		view.UpdateHelpText()
		return nil

	case tcell.KeyDown:
		mainApp.SelectNext()
		view.RefreshList()
		view.UpdateStatus()
		view.UpdateHelpText()
		return nil

	case tcell.KeyLeft:
		if projIdx := mainApp.GetSelectedProjectIndex(); projIdx >= 0 {
			mainApp.ToggleProjectFold(projIdx)
			view.RefreshList()
		}
		return nil

	case tcell.KeyRight, tcell.KeyEnter:
		if projIdx := mainApp.GetSelectedProjectIndex(); projIdx >= 0 {
			mainApp.ToggleProjectFold(projIdx)
			view.RefreshList()
		}
		return nil

	case tcell.KeyRune:
		switch event.Rune() {
		case 'q':
			mainApp.Quit()
			view.App.Stop()
			return nil

		case 'k':
			mainApp.SelectPrevious()
			view.RefreshList()
			view.UpdateStatus()
			view.UpdateHelpText()
			return nil

		case 'j':
			mainApp.SelectNext()
			view.RefreshList()
			view.UpdateStatus()
			view.UpdateHelpText()
			return nil

		case 'h':
			if projIdx := mainApp.GetSelectedProjectIndex(); projIdx >= 0 {
				mainApp.ToggleProjectFold(projIdx)
				view.RefreshList()
			}
			return nil

		case 'l':
			if projIdx := mainApp.GetSelectedProjectIndex(); projIdx >= 0 {
				mainApp.ToggleProjectFold(projIdx)
				view.RefreshList()
			}
			return nil

		case 'r':
			go func() {
				mainApp.RefreshSessions(ctx)
				view.App.QueueUpdateDraw(func() {
					view.RefreshList()
					view.UpdateStatus()
				})
			}()
			return nil

		case 'm':
			mainApp.ToggleDisplayMode()
			view.RefreshList()
			return nil

		case '?':
			if mainApp.State.ViewingHelp {
				mainApp.ToggleHelp()
				view.HideHelpModal()
			} else {
				mainApp.ToggleHelp()
				view.ShowHelpModal()
			}
			return nil

		case 'c':
			if mainApp.State.ViewingConflicts {
				mainApp.ToggleConflictView()
				view.HideConflictModal()
			} else {
				mainApp.ToggleConflictView()
				conflicts := mainApp.GetSelectedSpecConflicts()
				session := mainApp.GetSelectedSession()
				view.ShowConflictModal(conflicts, session)
			}
			return nil

		case 'e':
			if projIdx := mainApp.GetSelectedProjectIndex(); projIdx >= 0 {
				err := mainApp.OpenEditor(projIdx)
				if app.IsTerminalEditorError(err) {
					// Need to suspend TUI for terminal editor
					view.App.Suspend(func() {
						proj := mainApp.State.Projects[projIdx]
						editorParts := app.GetEditorCommand()
						if len(editorParts) == 0 {
							mainApp.SetStatus(ui.StatusError, "Invalid editor command")
							return
						}
						args := append(editorParts[1:], proj.File.Path)
						cmd := exec.Command(editorParts[0], args...)
						cmd.Stdin = os.Stdin
						cmd.Stdout = os.Stdout
						cmd.Stderr = os.Stderr
						if err := cmd.Run(); err != nil {
							mainApp.SetStatus(ui.StatusError, "Editor error: "+err.Error())
						} else {
							mainApp.SetStatus(ui.StatusInfo, "Edited: "+proj.File.DisplayName())
						}
					})
					// Refresh after editing
					go func() {
						mainApp.RefreshSessions(ctx)
						view.App.QueueUpdateDraw(func() {
							view.RefreshList()
							view.UpdateStatus()
						})
					}()
				}
			}
			return nil

		case 's':
			// Set immediate feedback
			if mainApp.State.Selection.IsSpecSelected() {
				if projIdx, specIdx := mainApp.GetSelectedSpec(); projIdx >= 0 && specIdx >= 0 {
					mainApp.SetStatus(ui.StatusInfo, "Starting "+mainApp.State.Projects[projIdx].Specs[specIdx].Name+"...")
				}
			} else if projIdx := mainApp.GetSelectedProjectIndex(); projIdx >= 0 {
				mainApp.SetStatus(ui.StatusInfo, "Starting "+mainApp.State.Projects[projIdx].File.DisplayName()+"...")
			}
			view.UpdateStatus()
			go func() {
				if mainApp.State.Selection.IsSpecSelected() {
					mainApp.StartSelectedSpec(ctx)
				} else {
					mainApp.StartSelectedProject(ctx)
				}
				mainApp.RefreshSessions(ctx)
				view.App.QueueUpdateDraw(func() {
					view.RefreshList()
					view.UpdateStatus()
				})
			}()
			return nil

		case 't':
			// Set immediate feedback
			if mainApp.State.Selection.IsSpecSelected() {
				if projIdx, specIdx := mainApp.GetSelectedSpec(); projIdx >= 0 && specIdx >= 0 {
					mainApp.SetStatus(ui.StatusInfo, "Terminating "+mainApp.State.Projects[projIdx].Specs[specIdx].Name+"...")
				}
			} else if projIdx := mainApp.GetSelectedProjectIndex(); projIdx >= 0 {
				mainApp.SetStatus(ui.StatusInfo, "Terminating "+mainApp.State.Projects[projIdx].File.DisplayName()+"...")
			}
			view.UpdateStatus()
			go func() {
				mainApp.TerminateSelected(ctx)
				mainApp.RefreshSessions(ctx)
				view.App.QueueUpdateDraw(func() {
					view.RefreshList()
					view.UpdateStatus()
				})
			}()
			return nil

		case 'f':
			// Set immediate feedback
			if mainApp.State.Selection.IsSpecSelected() {
				if projIdx, specIdx := mainApp.GetSelectedSpec(); projIdx >= 0 && specIdx >= 0 {
					mainApp.SetStatus(ui.StatusInfo, "Flushing "+mainApp.State.Projects[projIdx].Specs[specIdx].Name+"...")
				}
			} else if projIdx := mainApp.GetSelectedProjectIndex(); projIdx >= 0 {
				mainApp.SetStatus(ui.StatusInfo, "Flushing "+mainApp.State.Projects[projIdx].File.DisplayName()+"...")
			}
			view.UpdateStatus()
			go func() {
				mainApp.FlushSelected(ctx)
				mainApp.RefreshSessions(ctx)
				view.App.QueueUpdateDraw(func() {
					view.RefreshList()
					view.UpdateStatus()
				})
			}()
			return nil

		case 'p', ' ':
			go func() {
				mainApp.TogglePauseSelected(ctx)
				mainApp.RefreshSessions(ctx)
				view.App.QueueUpdateDraw(func() {
					view.RefreshList()
					view.UpdateStatus()
				})
			}()
			return nil

		case 'u':
			go func() {
				mainApp.ResumeSelected(ctx)
				mainApp.RefreshSessions(ctx)
				view.App.QueueUpdateDraw(func() {
					view.RefreshList()
					view.UpdateStatus()
				})
			}()
			return nil

		case 'P':
			// Set immediate feedback
			if mainApp.State.Selection.IsSpecSelected() {
				if projIdx, specIdx := mainApp.GetSelectedSpec(); projIdx >= 0 && specIdx >= 0 {
					mainApp.SetStatus(ui.StatusInfo, "Creating push session for "+mainApp.State.Projects[projIdx].Specs[specIdx].Name+"...")
				}
			} else if projIdx := mainApp.GetSelectedProjectIndex(); projIdx >= 0 {
				mainApp.SetStatus(ui.StatusInfo, "Creating push sessions for "+mainApp.State.Projects[projIdx].File.DisplayName()+"...")
			}
			view.UpdateStatus()
			go func() {
				if mainApp.State.Selection.IsSpecSelected() {
					mainApp.PushSelectedSpec(ctx)
				} else {
					mainApp.PushSelectedProject(ctx)
				}
				mainApp.RefreshSessions(ctx)
				view.App.QueueUpdateDraw(func() {
					view.RefreshList()
					view.UpdateStatus()
				})
			}()
			return nil

		case 'b':
			if mainApp.State.ViewingConflicts {
				go func() {
					mainApp.PushConflictsToBeta(ctx)
					mainApp.RefreshSessions(ctx)
					view.App.QueueUpdateDraw(func() {
						view.RefreshList()
						view.UpdateStatus()
					})
				}()
			}
			return nil

		case 'i':
			if mainApp.State.ViewingSyncStatus {
				mainApp.ToggleSyncStatusView()
				view.HideSyncStatusModal()
			} else {
				mainApp.ToggleSyncStatusView()
				session := mainApp.GetSelectedSession()
				view.ShowSyncStatusModal(session)
			}
			return nil
		}
	}

	return event
}

func autoRefresh(view *ui.View, mainApp *app.App, interval time.Duration) {
	ticker := time.NewTicker(interval)
	defer ticker.Stop()

	for range ticker.C {
		if mainApp.ShouldQuit() {
			return
		}

		ctx := context.Background()
		if err := mainApp.RefreshSessions(ctx); err == nil {
			view.App.QueueUpdateDraw(func() {
				view.RefreshList()
				view.UpdateStatus()
			})
		}
	}
}
