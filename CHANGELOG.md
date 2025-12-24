# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
- Duplicate sessions no longer created when starting specs (now terminates stray sessions first)
- Stale conflict counts no longer shown in project headers after resolving conflicts

## [0.2.0] - 2024-12-24

### Added
- Mouse support: click to select items, click on project headers to fold/unfold
- Enter key to edit project configuration files in your preferred editor
- Tab key navigation to switch between Projects and Sessions areas
- Push session functionality (`p` key on projects)
- Conflict viewing (`c` key)
- VCS ignore flag support
- Project management commands
  - `s` key to start/stop projects
  - `Space` key to pause/resume projects and sessions
  - `u` key to resume sessions
- Display mode toggle (`m` key)
  - Switch between showing paths and last sync time

### Changed
- Conflict Details view now auto-updates when conflicts change and auto-closes when resolved
- Session start/push operations now terminate all existing sessions first (prevents conflicts)
- UI layout dynamically adjusts when projects or sessions are empty
- Projects now displayed before sessions in the list
- Path normalization improved for better session matching
- Home directory paths shortened with `~` in UI

### Fixed
- Project start now works when some sessions are already running (starts remaining sessions individually)
- UI no longer crashes on transient Mutagen CLI errors
- Prevent refresh error loops (manual retry required after errors)
- Cross-platform timeout handling for Mutagen commands
- Handle Windows drive letters correctly in path normalization
- Project-session correlation for remote endpoints (fixes "no running sessions" display when remote sessions are active)
- Improved error message when attempting to start an already-running project

## [0.1.0] - Initial Release

Initial release

[0.2.0]: https://github.com/osteele/mutagui/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/osteele/mutagui/releases/tag/v0.1.0
