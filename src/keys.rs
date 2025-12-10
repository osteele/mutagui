//! Keyboard event handling for the TUI.
//!
//! This module extracts keyboard handling logic from main.rs to improve
//! code organization and readability.

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::Backend, Terminal};
use std::io;
use std::process::Command;

use crate::app::{App, BlockingOperation, StatusMessage};
use crate::ui;

/// Result of handling a key event.
pub enum KeyAction {
    /// Continue running the event loop
    Continue,
    /// Quit the application
    Quit,
    /// Refresh sessions after the action
    Refresh,
}

/// Get the configured editor from environment variables.
pub fn get_editor() -> String {
    std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vim".to_string())
}

/// Determine if an editor is a GUI editor (doesn't need terminal).
pub fn is_gui_editor(editor_path: &str) -> bool {
    use std::path::PathBuf;

    // Priority 1: User override (always respect this)
    if let Ok(val) = std::env::var("MUTAGUI_EDITOR_IS_GUI") {
        return val == "1" || val.to_lowercase() == "true";
    }

    // Priority 2: SSH detection (GUI won't work over SSH)
    if std::env::var("SSH_CLIENT").is_ok() || std::env::var("SSH_TTY").is_ok() {
        return false;
    }

    // Priority 3: Extract editor binary name
    let editor_name = PathBuf::from(editor_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(editor_path)
        .to_lowercase();

    // Priority 4: Known GUI editors
    let gui_editors = [
        "code",
        "code-insiders", // VS Code
        "zed",           // Zed
        "subl",
        "sublime",
        "sublime_text", // Sublime Text
        "atom",         // Atom
        "gedit",
        "gnome-text-editor", // GNOME
        "kwrite",
        "kate", // KDE
        "mousepad",
        "xed",   // XFCE
        "pluma", // MATE
        "bbedit",
        "textmate", // macOS commercial
        "textedit", // macOS built-in
        "xcode",    // Xcode
        "macvim",
        "gvim", // GUI vim
    ];

    if gui_editors.iter().any(|&e| editor_name.contains(e)) {
        return true;
    }

    // Priority 5: Known terminal editors (explicit negative check)
    let terminal_editors = [
        "vim",
        "vi",
        "nvim",
        "nano",
        "emacs",
        "emacsclient",
        "ed",
        "ex",
        "joe",
        "jed",
        "pico",
        "micro",
        "helix",
        "hx",
        "kakoune",
        "kak",
    ];

    if terminal_editors.iter().any(|&e| editor_name.contains(e)) {
        return false;
    }

    // Priority 6: Platform-specific path checks
    #[cfg(target_os = "macos")]
    {
        if editor_path.contains(".app/Contents/MacOS/") || editor_path.starts_with("/Applications/")
        {
            return true;
        }
    }

    // Priority 7: Default to terminal editor (safe for TUI)
    false
}

/// Handle a key event and return the appropriate action.
pub async fn handle_key_event<B: Backend>(
    key: KeyEvent,
    app: &mut App,
    terminal: &mut Terminal<B>,
) -> Result<KeyAction> {
    // Handle Ctrl-C for graceful quit (before other key handlers)
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.quit();
        return Ok(KeyAction::Quit);
    }

    match key.code {
        KeyCode::Char('q') => {
            app.quit();
            Ok(KeyAction::Quit)
        }
        KeyCode::Char('r') => Ok(KeyAction::Refresh),
        KeyCode::Tab => {
            app.toggle_focus_area();
            Ok(KeyAction::Continue)
        }
        KeyCode::Char('m') => {
            app.toggle_session_display();
            Ok(KeyAction::Continue)
        }
        KeyCode::Enter => {
            handle_enter_key(app, terminal)?;
            Ok(KeyAction::Refresh)
        }
        KeyCode::Char('s') => {
            handle_start_stop_project(app, terminal).await?;
            Ok(KeyAction::Refresh)
        }
        KeyCode::Char('p') => {
            handle_pause_or_push(app, terminal).await?;
            Ok(KeyAction::Refresh)
        }
        KeyCode::Char('u') => {
            app.resume_selected().await;
            Ok(KeyAction::Refresh)
        }
        KeyCode::Char(' ') => {
            handle_toggle_pause(app, terminal).await?;
            Ok(KeyAction::Refresh)
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.select_previous();
            Ok(KeyAction::Continue)
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.select_next();
            Ok(KeyAction::Continue)
        }
        KeyCode::Char('t') => {
            app.terminate_selected().await;
            Ok(KeyAction::Continue)
        }
        KeyCode::Char('f') => {
            app.flush_selected().await;
            Ok(KeyAction::Refresh)
        }
        KeyCode::Char('c') => {
            app.toggle_conflict_view();
            Ok(KeyAction::Continue)
        }
        _ => Ok(KeyAction::Continue),
    }
}

/// Handle Enter key - edit selected project file.
fn handle_enter_key<B: Backend>(app: &mut App, terminal: &mut Terminal<B>) -> Result<()> {
    if let Some(project_idx) = app.get_effective_project_index() {
        if let Some(project) = app.projects.get(project_idx) {
            let editor = get_editor();
            let file_path = &project.file.path;
            let is_gui = is_gui_editor(&editor);

            let status = if is_gui {
                // GUI editor - don't touch terminal, just spawn
                Command::new(&editor).arg(file_path).status()
            } else {
                // Terminal editor - suspend TUI
                disable_raw_mode()?;
                execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
                terminal.show_cursor()?;

                let status = Command::new(&editor).arg(file_path).status();

                // Restore TUI
                enable_raw_mode()?;
                execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
                terminal.hide_cursor()?;

                status
            };

            // Handle editor result
            match status {
                Ok(exit_status) if exit_status.success() => {
                    app.status_message = Some(StatusMessage::info(format!(
                        "Edited: {}",
                        project.file.display_name()
                    )));
                }
                Ok(exit_status) => {
                    app.status_message = Some(StatusMessage::warning(format!(
                        "Editor exited with code: {}",
                        exit_status.code().unwrap_or(-1)
                    )));
                }
                Err(e) => {
                    app.status_message = Some(StatusMessage::error(format!(
                        "Failed to launch editor: {}",
                        e
                    )));
                }
            }
        }
    } else {
        app.status_message = Some(StatusMessage::info(
            "Select a project to edit its configuration file",
        ));
    }
    Ok(())
}

/// Handle 's' key - start/stop project.
async fn handle_start_stop_project<B: Backend>(
    app: &mut App,
    terminal: &mut Terminal<B>,
) -> Result<()> {
    let operation_name = if app.selected_project_has_sessions() {
        "Stopping project..."
    } else {
        "Starting project..."
    };

    app.blocking_op = Some(BlockingOperation {
        message: operation_name.to_string(),
    });
    terminal.draw(|f| ui::draw(f, app))?;

    app.toggle_selected_project().await;
    app.blocking_op = None;
    Ok(())
}

/// Handle 'p' key - pause session or create push session.
async fn handle_pause_or_push<B: Backend>(app: &mut App, terminal: &mut Terminal<B>) -> Result<()> {
    if app.get_selected_session_index().is_some() {
        // Individual session selected: pause it
        app.pause_selected().await;
    } else if app.get_effective_project_index().is_some() {
        // Project selected (from either panel or session panel header): create push session
        if !app.selected_project_has_sessions() {
            // Count sessions to create for proper plural message
            let session_count = if let Some(project_idx) = app.get_effective_project_index() {
                app.projects
                    .get(project_idx)
                    .map(|p| p.file.sessions.len())
                    .unwrap_or(0)
            } else {
                0
            };
            let message = if session_count == 1 {
                "Creating push session...".to_string()
            } else {
                format!("Creating {} push sessions...", session_count)
            };

            // Show blocking modal before operation
            app.blocking_op = Some(BlockingOperation { message });
            terminal.draw(|f| ui::draw(f, app))?;

            app.push_selected_project().await;
            app.blocking_op = None;
        } else {
            app.status_message = Some(StatusMessage::warning(
                "Cannot push: project has active sessions. Stop the project first.",
            ));
        }
    }
    Ok(())
}

/// Handle space key - toggle pause for session or all project sessions.
async fn handle_toggle_pause<B: Backend>(app: &mut App, terminal: &mut Terminal<B>) -> Result<()> {
    // Check if operating on project (from either panel or header) vs single session
    if app.get_effective_project_index().is_some() && app.get_selected_session_index().is_none() {
        // Project selected: show blocking modal for pause/resume all
        let has_running = if let Some(project_idx) = app.get_effective_project_index() {
            if let Some(project) = app.projects.get(project_idx) {
                project.active_sessions.iter().any(|s| !s.paused)
            } else {
                false
            }
        } else {
            false
        };

        let operation_name = if has_running {
            "Pausing all sessions..."
        } else {
            "Resuming all sessions..."
        };

        app.blocking_op = Some(BlockingOperation {
            message: operation_name.to_string(),
        });
        terminal.draw(|f| ui::draw(f, app))?;

        app.toggle_pause_selected().await;
        app.blocking_op = None;
    } else {
        // Single session: no modal needed (quick operation)
        app.toggle_pause_selected().await;
    }
    Ok(())
}
