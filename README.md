# Mutagen TUI

A terminal user interface for managing [Mutagen](https://mutagen.io/) sync sessions.

## Features

- View all active Mutagen sync sessions in a clean, organized interface
- **Project discovery**: Automatically finds and displays `mutagen.yml` files
  - Searches common project directories (`~/code`, `~/projects`, `~/src`, etc.)
  - Supports multiple config files per directory (`mutagen-<target>.yml` pattern)
  - Correlates project files with running sessions
  - Toggle between Sessions and Projects views with Tab
- **Automatic theme detection**: Adapts colors for light and dark terminal backgrounds
- **Auto-refresh**: Session list and projects update every 3 seconds automatically
- **Real-time activity indicators**:
  - Connection status icons (✓ connected, ⊗ disconnected, ⟳ scanning)
  - File and directory counts for each endpoint
  - Sync status display
- Interactive keyboard controls for managing syncs:
  - Pause/resume sessions
  - Terminate sessions
  - Flush sessions
  - Manual refresh
- Last refresh timestamp display

## Prerequisites

- [Mutagen](https://mutagen.io/) must be installed and in your PATH
- Rust toolchain (for building from source)

## Installation

### From Source

```bash
# Clone or navigate to the repository
cd mutagen-tui

# Build and install
just install

# Or manually with cargo
cargo install --path .
```

## Usage

Simply run the application:

```bash
mutagen-tui
```

Or if building locally:

```bash
just run
```

### Keyboard Controls

| Key | Action |
|-----|--------|
| `Tab` | Toggle between Sessions and Projects views |
| `↑` / `k` | Move selection up |
| `↓` / `j` | Move selection down |
| `r` | Refresh session list and projects |
| `p` | Pause selected session |
| `u` | Resume selected session |
| `f` | Flush selected session |
| `t` | Terminate selected session |
| `q` | Quit application |

## Display

The TUI has two view modes (toggle with Tab):

### Sessions View

Shows all running sync sessions:
- **Status icon**: ▶ (running) or ⏸ (paused)
- **Session name**: Identifier for the sync
- **Alpha endpoint**:
  - Connection status icon (✓/⊗/⟳)
  - Path (local or host:path for remote)
  - File/directory counts when available
- **Beta endpoint**:
  - Connection status icon
  - Path
  - File/directory counts when available
- **Sync status**: Current state (watching, syncing, halted, etc.)

### Projects View

Shows discovered `mutagen.yml` project files:
- **Status icon**: ✓ (has running sessions) or ○ (inactive)
- **File name**: e.g., `mutagen-studio.yml`, `mutagen.yml`
- **File path**: Location of the project file
- **Associated sessions**: Lists running sessions linked to this project
- Helps manage multi-target setups (e.g., different files for different remote hosts)

### Status Bar

- Current status message
- Last refresh timestamp

## Development

### Building

```bash
just build        # Release build
just build-debug  # Debug build
```

### Running

```bash
just run          # Run in debug mode
just run-release  # Run optimized version
```

### Testing

```bash
just test         # Run tests
just check        # Run format check, lint, and tests
```

### Code Quality

```bash
just format       # Format code
just lint         # Run clippy
just fix          # Auto-fix formatting and linting issues
```

## Architecture

The application is structured into several modules:

- `main.rs`: Entry point and event loop with auto-refresh
- `app.rs`: Application state management and view mode handling
- `mutagen.rs`: Mutagen CLI integration and data models
- `project.rs`: Project file discovery, parsing, and session correlation
- `theme.rs`: Color scheme detection and management
- `ui.rs`: TUI rendering with ratatui (sessions and projects views)

### Dependencies

- **ratatui**: Terminal UI framework
- **crossterm**: Cross-platform terminal manipulation
- **tokio**: Async runtime
- **serde**: Serialization framework (JSON)
- **serde_yaml**: YAML parsing for mutagen.yml files
- **glob**: Pattern matching for file discovery
- **anyhow**: Error handling
- **chrono**: Date and time handling
- **terminal-light**: Terminal background detection for theme adaptation

## Notes

- The application automatically refreshes every 3 seconds to show live updates
- Terminal background is detected once at startup to choose appropriate color scheme (light/dark)
- The application polls Mutagen CLI commands to get session information
- All operations that modify sessions automatically refresh the session list
- The application requires that `mutagen` command is available in PATH

### Project File Discovery

The application searches for `mutagen.yml` files in:
- Current directory and subdirectories (`./mutagen.yml`, `./.mutagen/mutagen.yml`, `./config/mutagen.yml`)
- Common workspace directories (`~/code/**`, `~/projects/**`, `~/src/**`, `~/dev/**`)
- User config directories (`~/.config/mutagen/projects/`, `~/.mutagen/projects/`)

It supports multiple naming patterns:
- `mutagen.yml` - Standard project file
- `mutagen-<target>.yml` - Target-specific configurations (e.g., `mutagen-studio.yml`, `mutagen-cool30.yml`)
- `.mutagen.yml` and `.mutagen-<target>.yml` - Hidden variants

This allows you to have multiple Mutagen configurations in the same directory for different sync targets.

## License

MIT

## Author

Oliver Steele ([@osteele](https://github.com/osteele))
