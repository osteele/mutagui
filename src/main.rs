mod app;
mod command;
mod config;
mod mutagen;
mod project;
mod selection;
mod theme;
mod ui;
mod widgets;

use anyhow::Result;
use app::{App, StatusMessage};
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "mutagui")]
#[command(about = "Terminal UI for managing Mutagen sync sessions", long_about = None)]
struct Cli {
    /// Directory to search for mutagen project files (default: current directory)
    #[arg(short = 'd', long, value_name = "DIR")]
    project_dir: Option<PathBuf>,
}

fn get_editor() -> String {
    std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vim".to_string())
}

fn is_gui_editor(editor_path: &str) -> bool {
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(cli.project_dir);

    let res = run_app(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    app.refresh_sessions().await?;

    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    // Handle Ctrl-C for graceful quit (before other key handlers)
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        app.quit();
                    }

                    match key.code {
                        KeyCode::Char('q') => {
                            app.quit();
                        }
                        KeyCode::Char('r') => {
                            app.refresh_sessions().await?;
                        }
                        KeyCode::Tab => {
                            app.toggle_focus_area();
                        }
                        KeyCode::Char('m') => {
                            app.toggle_session_display();
                        }
                        KeyCode::Enter => {
                            // Edit selected project file
                            if let Some(project_idx) = app.get_selected_project_index() {
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
                                        execute!(
                                            io::stdout(),
                                            LeaveAlternateScreen,
                                            DisableMouseCapture
                                        )?;
                                        terminal.show_cursor()?;

                                        let status = Command::new(&editor).arg(file_path).status();

                                        // Restore TUI
                                        enable_raw_mode()?;
                                        execute!(
                                            io::stdout(),
                                            EnterAlternateScreen,
                                            EnableMouseCapture
                                        )?;
                                        terminal.hide_cursor()?;

                                        status
                                    };

                                    // Handle editor result
                                    match status {
                                        Ok(exit_status) if exit_status.success() => {
                                            app.status_message = Some(StatusMessage::info(
                                                format!("Edited: {}", project.file.display_name()),
                                            ));
                                            app.refresh_sessions().await?;
                                        }
                                        Ok(exit_status) => {
                                            app.status_message =
                                                Some(StatusMessage::warning(format!(
                                                    "Editor exited with code: {}",
                                                    exit_status.code().unwrap_or(-1)
                                                )));
                                        }
                                        Err(e) => {
                                            app.status_message = Some(StatusMessage::error(
                                                format!("Failed to launch editor: {}", e),
                                            ));
                                        }
                                    }
                                }
                            } else {
                                app.status_message = Some(StatusMessage::info(
                                    "Select a project to edit its configuration file",
                                ));
                            }
                        }
                        KeyCode::Char('s') => {
                            // Show blocking modal for start/stop project operations (10s timeout)
                            let operation_name = if app.selected_project_has_sessions() {
                                "Stopping project..."
                            } else {
                                "Starting project..."
                            };

                            app.blocking_op = Some(app::BlockingOperation {
                                message: operation_name.to_string(),
                            });
                            terminal.draw(|f| ui::draw(f, app))?;

                            app.toggle_selected_project();
                            app.blocking_op = None;
                            app.refresh_sessions().await?;
                        }
                        KeyCode::Char('p') => {
                            // Check if a session is selected (Sessions view) or project is selected (Projects view)
                            if app.get_selected_session_index().is_some() {
                                // Session selected: pause it
                                app.pause_selected();
                                app.refresh_sessions().await?;
                            } else {
                                // Project selected: create push session
                                if !app.selected_project_has_sessions() {
                                    // Count sessions to create for proper plural message
                                    let session_count = if let Some(project_idx) =
                                        app.get_selected_project_index()
                                    {
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
                                    app.blocking_op = Some(app::BlockingOperation { message });
                                    terminal.draw(|f| ui::draw(f, app))?;

                                    app.push_selected_project();
                                    app.blocking_op = None;
                                    app.refresh_sessions().await?;
                                } else {
                                    app.status_message = Some(StatusMessage::warning("Cannot push: project has active sessions. Stop the project first."));
                                }
                            }
                        }
                        KeyCode::Char('u') => {
                            // Resume selected session
                            app.resume_selected();
                            app.refresh_sessions().await?;
                        }
                        KeyCode::Char(' ') => {
                            // Check if operating on project (multiple sessions) or single session
                            if app.get_selected_project_index().is_some()
                                && app.get_selected_session_index().is_none()
                            {
                                // Project selected: show blocking modal for pause/resume all
                                let has_running =
                                    if let Some(project_idx) = app.get_selected_project_index() {
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

                                app.blocking_op = Some(app::BlockingOperation {
                                    message: operation_name.to_string(),
                                });
                                terminal.draw(|f| ui::draw(f, app))?;

                                app.toggle_pause_selected();
                                app.blocking_op = None;
                            } else {
                                // Single session: no modal needed (quick operation)
                                app.toggle_pause_selected();
                            }
                            app.refresh_sessions().await?;
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.select_previous();
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.select_next();
                        }
                        KeyCode::Char('t') => {
                            app.terminate_selected();
                        }
                        KeyCode::Char('f') => {
                            app.flush_selected();
                            app.refresh_sessions().await?;
                        }
                        KeyCode::Char('c') => {
                            app.toggle_conflict_view();
                        }
                        _ => {}
                    }
                }
                Event::Resize(_, _) => {
                    // Terminal was resized, just redraw on next iteration
                }
                _ => {
                    // Ignore other events (mouse, etc.)
                }
            }
        } else if app.should_auto_refresh() {
            let _ = app.refresh_sessions().await;
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
