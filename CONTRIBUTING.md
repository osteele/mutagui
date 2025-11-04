# Contributing to Mutagen TUI

Thank you for your interest in contributing to Mutagen TUI! This guide will help you get started with development.

## Prerequisites

- [Mutagen](https://mutagen.io/) must be installed and in your PATH
- Rust toolchain (install via [rustup](https://rustup.rs/))

## Getting Started

### Clone and Build

```bash
# Clone or navigate to the repository
cd mutagui

# Build and install locally
just install

# Or manually with cargo
cargo install --path .
```

## Development Commands

### Building

```bash
just build        # Release build
just build-debug  # Debug build
```

Or directly with cargo:

```bash
cargo build              # Debug build
cargo build --release    # Optimized release build
```

### Running

```bash
just run          # Run in debug mode
just run-release  # Run optimized version
```

Or directly with cargo:

```bash
cargo run                # Run in development mode
cargo run --release      # Run optimized version
```

### Testing

```bash
just test         # Run tests
just check        # Run format check, lint, and tests
```

Or directly with cargo:

```bash
cargo test               # Run all tests
```

### Code Quality

```bash
just format       # Format code
just lint         # Run clippy
just fix          # Auto-fix formatting and linting issues
```

Or directly with cargo:

```bash
cargo fmt                                                      # Format code
cargo clippy                                                   # Run linter
cargo fmt && cargo clippy --fix --allow-staged --allow-dirty  # Auto-fix issues
```

## Architecture

The application is structured into several modules with clear separation of concerns:

### Module Structure

- **main.rs** - Entry point and event loop
  - Sets up terminal (crossterm + ratatui)
  - Runs async event loop with 100ms polling interval
  - Handles keyboard events and auto-refresh triggers
  - Cleans up terminal on exit

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

- **ui.rs** - TUI rendering
  - Renders the interface using ratatui
  - Handles both Sessions and Projects view modes
  - Displays session status, endpoints, and project information

### Key Dependencies

- **ratatui**: Terminal UI framework
- **crossterm**: Cross-platform terminal manipulation
- **tokio**: Async runtime
- **serde**: Serialization framework (JSON)
- **serde_yaml**: YAML parsing for mutagen.yml files
- **glob**: Pattern matching for file discovery
- **clap**: Command-line argument parsing
- **anyhow**: Error handling
- **chrono**: Date and time handling
- **terminal-light**: Terminal background detection for theme adaptation

### Design Patterns

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

## Implementation Notes

### Auto-refresh Behavior

- The application automatically refreshes every 3 seconds to show live updates
- Terminal background is detected once at startup to choose appropriate color scheme (light/dark)
- The application polls Mutagen CLI commands to get session information
- All operations that modify sessions automatically refresh the session list
- The application requires that `mutagen` command is available in PATH

### Project File Discovery

The application searches for `mutagen.yml` files starting from the current directory (or the directory specified with `--project-dir`). See the main README for user-facing documentation on search locations.

**Performance:** The search uses non-recursive patterns for fast startup. No `**/` glob patterns are used to avoid scanning thousands of files.

## Testing Considerations

- **Mutagen dependency**: Tests that interact with actual Mutagen sessions require a running Mutagen daemon
- **CLI mocking**: Consider mocking `Command` execution for unit tests
- **Project discovery**: Tests may need to set up temporary file structures or mock glob results

## Common Development Tasks

### Adding a new keyboard command

1. Add key handler in `main.rs::run_app()` match statement
2. Add corresponding method to `App` in `app.rs`
3. If it modifies sessions, call `app.refresh_sessions().await?` after the operation
4. Update help text in `ui.rs` (if applicable)

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

## Version Control

This project uses **Jujutsu (jj)** for version control (with Git as a backing store). Use `jj` commands for version control operations:

```bash
jj status        # Check working copy status
jj diff          # View changes
jj describe      # Set commit description
jj log           # View commit history
```

## Additional Resources

For detailed architectural information and design decisions, see [CLAUDE.md](CLAUDE.md).

## Questions or Issues?

Feel free to open an issue on GitHub if you have questions or encounter problems during development.
