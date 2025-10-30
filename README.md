# Mutagen TUI

A terminal user interface for managing [Mutagen](https://mutagen.io/) sync sessions.

For more development tools, see: https://osteele.com/software/development-tools

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
cd mutagen-gui

# Build and install
just install

# Or manually with cargo
cargo install --path .
```

## Usage

Simply run the application:

```bash
mutagui
```

Or if building locally:

```bash
just run
```

### Command-Line Options

```bash
mutagui [OPTIONS]

Options:
  -d, --project-dir <DIR>    Directory to search for mutagen project files
                             (default: current directory)
  -h, --help                 Print help
```

**Examples:**

```bash
# Use current directory (default)
mutagui

# Search for projects starting from ~/code
mutagui --project-dir ~/code

# Short form
mutagui -d ~/projects
```

The `--project-dir` option specifies where to start searching for `mutagen.yml` files. The application will:
- Search the specified directory and its subdirectories
- Walk up the directory tree to find project configuration directories
- Still check user config directories (`~/.config/mutagen/projects/`, `~/.mutagen/projects/`)

### Keyboard Controls

| Key | Action |
|-----|--------|
| `Tab` | Toggle between Sessions and Projects views |
| `↑` / `k` | Move selection up |
| `↓` / `j` | Move selection down |
| `r` | Refresh session list and projects |
| `p` | Pause selected session (Sessions view) / Create push session (Projects view) |
| `u` | Resume selected session |
| `f` | Flush selected session |
| `t` | Terminate selected session |
| `s` | Start/stop selected project |
| `Space` | Toggle pause on selected item |
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

## Push Sessions

The push feature allows you to create one-time, one-way sync sessions from a project definition. This is useful for quickly pushing local changes to a remote without starting a full bidirectional sync.

**To create a push session:**
1. Switch to Projects view (press `Tab`)
2. Select a project
3. Press `p` to create a push session

The application will create a temporary session named `<session-name>-push` with:
- Mode: `one-way-replica` (alpha → beta)
- Endpoints from the project file
- Ignore patterns (with limitations - see below)

**Important:** You can only create a push session when the project has no active sessions running. Stop the project first if needed (press `s`).

### Push Session Limitations

**Ignore Pattern Support:**

✅ **Fully Supported:**
- Simple list format in YAML
  ```yaml
  sync:
    myproject:
      alpha: /local/path
      beta: user@host:/remote/path
      ignore:
        - node_modules
        - .git
        - "*.tmp"
  ```

- `sync.defaults` section (merged with session-specific rules)
  ```yaml
  sync:
    defaults:
      ignore:
        - .git
        - node_modules
    myproject:
      alpha: /local/path
      beta: user@host:/remote/path
      ignore:
        - "*.tmp"  # Combined with defaults
  ```

- Object format with `paths` key
  ```yaml
  sync:
    myproject:
      alpha: /local/path
      beta: user@host:/remote/path
      ignore:
        paths:
          - node_modules
          - .git
  ```

- VCS ignore flag (ignores `.git`, `.svn`, `.hg`, `.bzr`, `_darcs`, `.fossil-settings`)
  ```yaml
  sync:
    myproject:
      alpha: /local/path
      beta: user@host:/remote/path
      ignore:
        vcs: true
  ```

- Combined VCS and custom paths
  ```yaml
  sync:
    myproject:
      alpha: /local/path
      beta: user@host:/remote/path
      ignore:
        vcs: true
        paths:
          - node_modules
          - "*.tmp"
  ```

❌ **Not yet supported:**
- Regular expression patterns (`ignore: { regex: "pattern.*" }`)

**Note:** Ignore patterns from `sync.defaults` are merged with session-specific patterns. Session-specific patterns are added to (not replacing) defaults.

**Session Selection:** When a project file contains multiple session definitions:
- If exactly one session is defined → uses that session
- If multiple sessions are defined and one or more are active → uses the first active session (alphabetically)
- If multiple sessions are defined and none are active → uses the first session alphabetically

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
- **clap**: Command-line argument parsing
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

The application searches for `mutagen.yml` files starting from the current directory (or the directory specified with `--project-dir`):

**Search locations:**
- Base directory: `mutagen.yml`, `mutagen-*.yml`, `.mutagen.yml`, `.mutagen-*.yml`
- Subdirectories: `mutagen/*.yml`, `.mutagen/*.yml`, `config/*.yml`, `conf/*.yml`
- Parent directories: Walks up to filesystem root or home, checking for `mutagen/`, `.mutagen/`, `config/`, `conf/` subdirectories
- User config: `~/.config/mutagen/projects/*.yml`, `~/.mutagen/projects/*.yml`

**Supported naming patterns:**
- `mutagen.yml` - Standard project file
- `mutagen-<target>.yml` - Target-specific configurations (e.g., `mutagen-studio.yml`, `mutagen-cool30.yml`)
- `.mutagen.yml` and `.mutagen-<target>.yml` - Hidden variants

This allows you to have multiple Mutagen configurations in the same directory for different sync targets.

**Performance:** The search uses non-recursive patterns for fast startup. No `**/` glob patterns are used to avoid scanning thousands of files.

## License

MIT

## Author

Oliver Steele ([@osteele](https://github.com/osteele))
