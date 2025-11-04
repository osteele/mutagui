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

## Configuration Files

The application automatically discovers `mutagen.yml` project files to help you manage your sync sessions. Understanding where these files are searched can help you organize your projects effectively.

### Search Locations

Starting from the base directory (current directory by default, or specified with `--project-dir`), the application searches in the following order:

1. **Base directory patterns:**
   - `mutagen.yml`, `mutagen-*.yml`
   - `.mutagen.yml`, `.mutagen-*.yml` (hidden variants)

2. **Subdirectories:**
   - `mutagen/*.yml`, `.mutagen/*.yml`
   - `config/*.yml`, `conf/*.yml`

3. **Parent directories:**
   - Walks up the directory tree to filesystem root or home directory
   - Checks for `mutagen/`, `.mutagen/`, `config/`, `conf/` subdirectories

4. **User configuration directories:**
   - `~/.config/mutagen/projects/*.yml`
   - `~/.mutagen/projects/*.yml`

### Supported File Naming Patterns

- `mutagen.yml` - Standard project configuration file
- `mutagen-<target>.yml` - Target-specific configurations (e.g., `mutagen-studio.yml`, `mutagen-cool30.yml`)
- `.mutagen.yml` and `.mutagen-<target>.yml` - Hidden variants of the above

This naming scheme allows you to maintain multiple Mutagen configurations in the same directory for different sync targets.

### Performance Note

The file discovery uses non-recursive glob patterns for fast startup. Deep directory traversal with `**/` patterns is avoided to prevent scanning thousands of files unnecessarily.

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

## Contributing

Interested in contributing? See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, architecture details, and guidelines.

## License

MIT

## Author

Oliver Steele ([@osteele](https://github.com/osteele))
