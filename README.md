# Mutagen TUI

A terminal user interface for managing [Mutagen](https://mutagen.io/) sync sessions.

For more development tools, see: https://osteele.com/software/development-tools

## Features

- **Unified hierarchical view**: Projects and their sync specs in a single tree view
  - Fold/unfold projects to show or hide individual sync specs
  - Auto-unfold when conflicts are detected
- **Project discovery**: Automatically finds and displays `mutagen.yml` files
  - Searches common project directories (`~/code`, `~/projects`, `~/src`, etc.)
  - Supports multiple config files per directory (`mutagen-<target>.yml` pattern)
  - Correlates project files with running sessions
- **Push mode support**: Create one-way sync sessions (alpha → beta)
  - Push individual specs or entire projects
  - Automatically replaces two-way sessions when creating push
  - Clear visual indicators for push mode (⬆ arrow, "(push)" label)
- **Automatic theme detection**: Adapts colors for light and dark terminal backgrounds
- **Auto-refresh**: Session list and projects update every 3 seconds automatically
- **Real-time activity indicators**:
  - Connection status icons (✓ connected, ⊗ disconnected, ⟳ scanning)
  - Session status icons (👁 watching, 📦 staging, ⚖ reconciling, etc.)
  - File and directory counts for each endpoint
  - Sync status display with progress percentages
- **Interactive keyboard controls** for managing syncs:
  - Start/stop projects and individual specs
  - Create push sessions
  - Pause/resume sessions
  - Terminate and flush sessions
  - View and resolve conflicts
  - Edit project configuration files
  - Manual refresh
- Last refresh timestamp display

## Prerequisites

- [Mutagen](https://mutagen.io/) must be installed and in your PATH
- Rust toolchain (for building from source)

## Installation

### From crates.io

```bash
cargo install mutagui
```

### From Source

```bash
# Clone or navigate to the repository
cd mutagui

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

## Interface Overview

The TUI displays a hierarchical tree view of projects and their sync specs:

```
┌─ Sync Projects ─────────────────────────────────────────────────────────────┐
│ ▼ ✓ apollo-research               2/3 running (1 push)                      │
│   ▶ apollo-research (push)           👁  ✓~/code/research ⬆ ✓apollo:/data/. │
│   ▶ apollo-research-tools             👁  ✓~/code/tools ⇄ ✓apollo:/data/... │
│   ○ apollo-datasets                   Not running                            │
│ ▶ ○ mercury-ml                     0/2 running                               │
│ ▼ ✓ starship-dev                  1/1 running  ⚠ 3 conflicts               │
│   ▶ sync-to-orbit                     📦  ✓~/code/starship ⇄ ✓orbit:/home/. │
└──────────────────────────────────────────────────────────────────────────────┘
┌─ Help ───────────────────────────────────────────────────────────────────────┐
│ ↑/↓/j/k Nav │ h/l/↵ Fold │ r Refresh │ e Edit │ s Start/Stop │ p Push │...  │
└──────────────────────────────────────────────────────────────────────────────┘
┌─ Status ─────────────────────────────────────────────────────────────────────┐
│ Created 1 push session(s) | Last refresh: 12:34:56                           │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Visual Indicators:**
- **Fold state**: `▼` (expanded) / `▶` (collapsed)
- **Project status**: `✓` (active) / `○` (inactive)
- **Spec status**: `▶` (running) / `⏸` (paused) / `○` (not running)
- **Direction**: `⇄` (two-way) / `⬆` (push mode, bold/colored)
- **Push mode label**: Specs show `(push)` suffix when in push mode
- **Endpoint status**: `✓` (connected) / `⟳` (scanning) / `⊗` (disconnected)
- **Session activity**: `👁` (watching) / `📦` (staging) / `⚖` (reconciling) / etc.
- **Conflicts**: `⚠ 3 conflicts` shown on project header

### Keyboard Controls

#### Navigation
| Key | Action |
|-----|--------|
| `↑` / `k` | Move selection up |
| `↓` / `j` | Move selection down |
| `h` / `←` / `l` / `→` / `Enter` | Toggle fold/unfold project |

#### Global Actions
| Key | Action |
|-----|--------|
| `r` | Refresh session list and projects |
| `m` | Toggle display mode (show paths vs. last sync time) |
| `q` / `Ctrl-C` | Quit application |

#### Project Actions (when project selected)
| Key | Action |
|-----|--------|
| `e` | Edit project configuration file |
| `s` | Start/stop all specs in project |
| `p` | Create push sessions for all specs |
| `Space` | Pause/resume all running specs |

#### Spec Actions (when individual spec selected)
| Key | Action |
|-----|--------|
| `p` | Create push session (replaces two-way if running) |
| `Space` | Pause/resume spec |
| `u` | Resume paused spec |
| `f` | Flush spec |
| `t` | Terminate spec |
| `c` | View conflicts |

### Editor Integration

When pressing `e` to edit a project file:

**Editor Selection:**
1. `$VISUAL` environment variable (if set)
2. `$EDITOR` environment variable (if set)
3. `vim` (default fallback)

**Automatic GUI Detection:**

The application automatically detects whether your editor is a GUI application or terminal-based and adjusts its behavior accordingly:

- **GUI editors** (VS Code, Zed, Sublime Text, etc.): The TUI remains active in the background while your editor opens in a separate window. No terminal disruption.
- **Terminal editors** (vim, nano, emacs, etc.): The TUI suspends and restores your terminal to normal mode, then resumes after you exit the editor.

**Supported GUI Editors** (automatically detected):
- VS Code (`code`, `code-insiders`)
- Zed (`zed`)
- Sublime Text (`subl`, `sublime`, `sublime_text`)
- Atom (`atom`)
- GNOME editors (`gedit`, `gnome-text-editor`)
- KDE editors (`kate`, `kwrite`)
- XFCE editors (`mousepad`, `xed`)
- MATE editor (`pluma`)
- macOS editors (`bbedit`, `textmate`, `textedit`, `xcode`)
- GUI Vim variants (`macvim`, `gvim`)

**Supported Terminal Editors** (automatically detected):
- vim, vi, nvim
- nano, pico
- emacs, emacsclient
- helix, hx
- kakoune, kak
- micro, joe, jed
- ed, ex

**SSH Behavior**: When connected via SSH, the application assumes terminal editors only (GUI editors won't work).

**Manual Override**: If detection is incorrect for your editor, set:
```bash
export MUTAGUI_EDITOR_IS_GUI=true   # Force GUI behavior
export MUTAGUI_EDITOR_IS_GUI=false  # Force terminal behavior
```

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
- `mutagen-<target>.yml` - Target-specific configurations (e.g., `mutagen-apollo.yml`, `mutagen-mercury.yml`)
- `.mutagen.yml` and `.mutagen-<target>.yml` - Hidden variants of the above

This naming scheme allows you to maintain multiple Mutagen configurations in the same directory for different sync targets.

### Performance Note

The file discovery uses non-recursive glob patterns for fast startup. Deep directory traversal with `**/` patterns is avoided to prevent scanning thousands of files unnecessarily.

## Display

The TUI shows a unified hierarchical view with:

### Unified Projects and Specs View

Shows all projects and their sync specs in a tree structure:
- **Project headers**:
  - Fold indicator: ▼ (expanded) or ▶ (collapsed)
  - Status icon: ✓ (active) or ○ (inactive)
  - Project name (e.g., `mutagen-apollo`, `starship-dev`)
  - Running status: "Running", "Not running", or "X/Y running"
  - Push mode count when applicable: "(2 push)"
  - Conflict indicator when present: "⚠ 3 conflicts"
- **Sync specs** (shown when project is expanded):
  - Status icon: ▶ (running), ⏸ (paused), or ○ (not running)
  - Spec name with push mode label: `sync-name (push)`
  - Session status icon: 👁 (watching), 📦 (staging), ⚖ (reconciling), etc.
  - Alpha endpoint with connection status and path
  - Direction arrow: ⇄ (two-way) or ⬆ (push mode, in bold color)
  - Beta endpoint with connection status and path

#### Session Status Icons

| Icon | Status | Description |
|------|--------|-------------|
| 👁 | Watching | Connected and idle, waiting for file changes |
| 🔌 | Connecting | Establishing connection to remote endpoint |
| 🔍 | Scanning | Scanning files for changes |
| 📦 | Staging | Transferring file content that needs to be synced |
| ⚖ | Reconciling | Computing what changes to make on each side |
| ⏳ | Transitioning | Applying changes (writes/deletes/modifications) to the filesystem |
| 💾 | Saving | Saving synchronized changes |
| ⛔ | Halted | Session halted due to error |
| • | Unknown | Unknown or other status |

#### Sync Cycle

When syncing, Mutagen progresses through these phases:

1. **Scanning** → Examines both endpoints for changes
2. **Staging** → Transfers file content that needs to be synced
3. **Reconciling** → Computes what changes to make on each side
4. **Transitioning** → Applies the changes to the filesystem
5. **Watching** → Monitors for new file changes

The Status area shows progress percentage during staging (e.g., "Staging (45%)").

#### Endpoint Connection Icons

| Icon | Status |
|------|--------|
| ✓ | Connected and scanned |
| ⟳ | Connected, scanning |
| ⊗ | Disconnected |

### Status Bar

- Current status message
- Last refresh timestamp

## Push Sessions

The push feature allows you to create one-way sync sessions (alpha → beta) from a project definition. This is useful for quickly pushing local changes to a remote without starting a full bidirectional sync.

**To create push sessions:**

**For a single spec:**
1. Select an individual sync spec (navigate to it with arrow keys)
2. Press `p` to create a push session
   - If a two-way session is running, it will be automatically terminated and replaced
   - A new session named `<spec-name>-push` will be created

**For all specs in a project:**
1. Select a project header
2. Press `p` to create push sessions for all specs
   - All running two-way sessions will be terminated
   - Push sessions will be created for each spec defined in the project file

The application creates sessions with:
- Mode: `one-way-replica` (alpha → beta)
- Endpoints from the project file
- Ignore patterns from the project configuration

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

## Contributing

Interested in contributing? See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, architecture details, and guidelines.

## License

MIT

## Author

Oliver Steele ([@osteele](https://github.com/osteele))
