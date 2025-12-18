# Development Guide

## Project Overview

**mutagui** is a terminal user interface (TUI) for managing [Mutagen](https://mutagen.io/) sync sessions. It provides real-time monitoring and control of file synchronization sessions with automatic theme detection and auto-refresh capabilities.

## Prerequisites

- **Mutagen CLI** must be installed and available in PATH
- Go 1.21+ for building

## Version Control

This project uses **Jujutsu (jj)** for version control (with Git as a backing store). Use `jj` commands for all version control operations.

## Development Commands

### Building
```bash
go build -o mutagui .   # Build binary
just build              # Same as above
```

### Running
```bash
go run .                # Run directly
just run                # Same as above
```

### Testing
```bash
go test ./...           # Run all tests
just test               # Same as above
```

### Code Quality
```bash
gofmt -w .              # Format code
go vet ./...            # Run linter
just check              # Run format-check, lint, and test
just fix                # Auto-fix formatting
```

### Installation
```bash
go install .            # Install to $GOPATH/bin/mutagui
just install            # Same as above
```

## Architecture

### Package Structure

The application follows a modular architecture with clear separation of concerns:

- **main.go** - Entry point and event loop
  - Sets up terminal with tview
  - Runs event loop with keyboard handling
  - Handles auto-refresh triggers
  - Cleans up terminal on exit
  - Editor integration: Detects editor from `$VISUAL`, `$EDITOR`, or defaults to `vim`
  - Terminal mode switching: Properly suspends TUI mode when launching external editors

- **internal/app/app.go** - Application state management
  - `App` struct holds all application state (sessions, projects, selection)
  - Contains all session operations: start, pause, resume, flush, terminate
  - Auto-refresh logic: refreshes every 3 seconds when idle
  - Editor integration with GUI/terminal detection

- **internal/mutagen/** - Mutagen CLI integration
  - **types.go**: Data models (`SyncSession`, `Endpoint`, `Conflict`)
  - **client.go**: `Client` wraps Mutagen CLI commands via `os/exec`
  - Parses JSON output from `mutagen sync list --template '{{json .}}'`
  - Endpoint status tracking: connected, scanned, file/directory counts

- **internal/project/project.go** - Project file discovery and correlation
  - Discovers `mutagen.yml` and `mutagen-*.yml` files
  - Parses YAML config files to extract session definitions
  - Correlates project files with active sessions by matching names and paths
  - Manages sync spec states (NotRunning, RunningTwoWay, RunningPush)

- **internal/config/config.go** - Configuration management
  - Loads TOML config from standard locations
  - Theme mode, refresh settings, project search paths
  - Default values with user overrides

- **internal/ui/** - TUI components
  - **theme.go**: Color scheme definitions (light/dark themes)
  - **selection.go**: Selection manager for navigating project/spec tree
  - **view.go**: TUI rendering using tview

### Key Design Patterns

**Event-Driven UI**: Uses tview for the terminal UI with event capture for keyboard handling. All UI updates go through `QueueUpdateDraw` for thread safety.

**CLI Integration**: All Mutagen operations shell out to the `mutagen` CLI binary rather than using a library. This means:
- The application requires `mutagen` to be installed and in PATH
- Operations use context with timeout for cancellation
- Errors are captured from stderr output

**Auto-refresh**: A background goroutine ticks every N seconds (configurable) and refreshes sessions automatically.

**Project Discovery**: The application searches configured paths to find `mutagen.yml` files. It correlates these configuration files with active sessions to provide a project-centric view.

**State Management**: The `App` struct is the single source of truth for application state. The `AppState` struct is shared with the view for rendering. All mutations go through `App` methods.

### Data Flow

1. **Startup**: App initializes, detects theme, calls `LoadProjects()` and `RefreshSessions()`
2. **Refresh cycle**:
   - Execute `mutagen sync list --template '{{json .}}'`
   - Parse JSON into `[]SyncSession`
   - Update each project's specs with session data
   - Update `LastRefresh` timestamp
3. **User action** (e.g., pause):
   - App method calls `Client` method
   - CLI command executes with timeout
   - Result updates status message
   - Session list refreshes
4. **Rendering**: View reads from shared `AppState` and renders using tview

## Testing Considerations

- **Mutagen dependency**: Tests that interact with actual Mutagen sessions require a running Mutagen daemon
- **CLI mocking**: Consider creating mock implementations of the Client interface for unit tests
- **Project discovery**: Tests may need to set up temporary file structures

## Common Development Tasks

### Adding a new keyboard command

1. Add key handler in `main.go` `handleInput()` function
2. Add corresponding method to `App` in `internal/app/app.go` (if needed)
3. If it modifies sessions, call `RefreshSessions()` after the operation
4. Update help text in `internal/ui/view.go` (if applicable)

For the complete keyboard bindings list, see README.md.

### Editor Integration

Editor launching uses hybrid GUI detection to determine terminal handling:
- **GUI editors** (VS Code, Zed, etc.): TUI remains active, editor spawns without terminal disruption
- **Terminal editors** (vim, nano, etc.): TUI suspends using `Suspend()`, then resumes after editor exits

Detection logic (`app.IsGUIEditor()`):
1. User override via `MUTAGUI_EDITOR_IS_GUI` env var
2. SSH detection (assumes terminal editor over SSH)
3. Hardcoded list of GUI editors
4. Hardcoded list of terminal editors
5. Default: terminal editor (safe fallback)

### Adding a new Mutagen operation

1. Add method to `Client` in `internal/mutagen/client.go` following the existing pattern:
   - Use `exec.CommandContext` with timeout
   - Capture combined output for errors
   - Return appropriate error type
2. Add wrapper method to `App` in `internal/app/app.go`
3. Add keyboard binding in `main.go`

### Modifying the data model

1. Update structs in `internal/mutagen/types.go` (add json tags)
2. Test with actual `mutagen sync list --template '{{json .}}'` output
3. Update display logic in `internal/ui/view.go` if needed
