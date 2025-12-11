# Development Guide

## Project Overview

**mutagui** is a terminal user interface (TUI) for managing [Mutagen](https://mutagen.io/) sync sessions. It provides real-time monitoring and control of file synchronization sessions with automatic theme detection and auto-refresh capabilities.

## Prerequisites

- **Mutagen CLI** must be installed and available in PATH
- Rust toolchain for building

## Version Control

This project uses **Jujutsu (jj)** for version control (with Git as a backing store). Use `jj` commands for all version control operations.

## Development Commands

### Building
```bash
cargo build              # Debug build
cargo build --release    # Optimized release build
```

### Running
```bash
cargo run                # Run in development mode
cargo run --release      # Run optimized version
```

### Testing
```bash
cargo test               # Run all tests
```

### Code Quality
```bash
cargo fmt                                                      # Format code
cargo clippy                                                   # Run linter
cargo fmt && cargo clippy --fix --allow-staged --allow-dirty  # Auto-fix issues
```

### Installation
```bash
cargo install --path .   # Install to ~/.cargo/bin/mutagui
```

## Architecture

### Module Structure

The application follows a modular architecture with clear separation of concerns:

- **main.rs** - Entry point and event loop
  - Sets up terminal (crossterm + ratatui)
  - Runs async event loop with 100ms polling interval
  - Handles keyboard events and auto-refresh triggers
  - Cleans up terminal on exit
  - Editor integration: Detects editor from `$VISUAL`, `$EDITOR`, or defaults to `vim`
  - Terminal mode switching: Properly suspends TUI mode when launching external editors

- **app.rs** - Application state management
  - `App` struct holds all application state (sessions, projects, selection, theme)
  - Two view modes: `Sessions` (default) and `Projects`
  - Auto-refresh logic: refreshes every 3 seconds when idle
  - Session operations: pause, resume, flush, terminate

- **mutagen.rs** - Mutagen CLI integration
  - `MutagenClient` wraps Mutagen CLI commands via `std::process::Command`
  - Data models: `SyncSession`, `Endpoint`
  - Parses JSON output from `mutagen sync list --template '{{json .}}'`
  - Endpoint status tracking: connected, scanned, file/directory counts

- **project.rs** - Project file discovery and correlation
  - Discovers `mutagen.yml` and `mutagen-*.yml` files across common project directories
  - Parses YAML config files to extract session definitions
  - Correlates project files with active sessions by matching names and paths
  - Search paths include: current directory, `~/code/**`, `~/projects/**`, `~/src/**`, `~/.config/mutagen/projects/`

- **theme.rs** - Color scheme detection
  - Uses `terminal-light` crate to detect terminal background (light/dark)
  - Provides two color schemes with appropriate contrast
  - Theme is detected once at startup

- **ui.rs** - TUI rendering using ratatui

### Key Design Patterns

**Async Runtime**: Uses Tokio for async operations, though most operations are synchronous wrappers around CLI commands.

**CLI Integration**: All Mutagen operations shell out to the `mutagen` CLI binary rather than using a library. This means:
- The application requires `mutagen` to be installed and in PATH
- Operations are synchronous (wrapped in async functions)
- Errors are captured from stderr output

**Auto-refresh**: The event loop checks `app.should_auto_refresh()` every 100ms. If 3+ seconds have elapsed since the last refresh, it automatically calls `refresh_sessions()`.

**Project Discovery**: The application searches multiple common project directory patterns to find `mutagen.yml` files. It attempts to correlate these configuration files with active sessions to provide a project-centric view.

**State Management**: The `App` struct is the single source of truth for application state. All mutations go through `App` methods, which handle both the state change and any necessary CLI calls.

### Data Flow

1. **Startup**: App initializes, detects theme, calls `refresh_sessions()`
2. **Refresh cycle**:
   - Execute `mutagen sync list --template '{{json .}}'`
   - Parse JSON into `Vec<SyncSession>`
   - Discover project files from filesystem
   - Correlate projects with active sessions
   - Update `last_refresh` timestamp
3. **User action** (e.g., pause):
   - App method calls `MutagenClient` method
   - CLI command executes
   - Result updates status message
   - Session list refreshes automatically
4. **Rendering**: UI module reads from `App` state and renders using ratatui

## Testing Considerations

- **Mutagen dependency**: Tests that interact with actual Mutagen sessions require a running Mutagen daemon
- **CLI mocking**: Consider mocking `Command` execution for unit tests
- **Project discovery**: Tests may need to set up temporary file structures or mock glob results

## Common Development Tasks

### Adding a new keyboard command

1. Add key handler in `main.rs::run_app()` match statement
2. Add corresponding method to `App` in `app.rs` (if needed)
3. If it modifies sessions, call `app.refresh_sessions().await?` after the operation
4. Update help text in `ui.rs` (if applicable)

For the complete keyboard bindings list, see README.md.

### Editor Integration

Editor launching uses hybrid GUI detection to determine terminal handling:
- **GUI editors** (VS Code, Zed, etc.): TUI remains active, editor spawns without terminal disruption
- **Terminal editors** (vim, nano, etc.): TUI suspends (disables raw mode, leaves alternate screen), then restores after editor exits

Detection logic (`main.rs::is_gui_editor()`):
1. User override via `MUTAGUI_EDITOR_IS_GUI` env var
2. SSH detection (assumes terminal editor over SSH)
3. Hardcoded list of ~20 GUI editors
4. Hardcoded list of ~15 terminal editors
5. macOS .app path detection
6. Default: terminal editor (safe fallback)

### Adding a new Mutagen operation

1. Add method to `MutagenClient` in `mutagen.rs` following the existing pattern:
   - Use `Command::new("mutagen")` with appropriate args
   - Check `output.status.success()`
   - Parse stderr for errors
   - Return `Result<T>`
2. Add wrapper method to `App` in `app.rs`
3. Add keyboard binding in `main.rs`

### Modifying the data model

1. Update structs in `mutagen.rs` (add serde derive attributes)
2. Test with actual `mutagen sync list --template '{{json .}}'` output
3. Update display logic in `ui.rs` if needed
