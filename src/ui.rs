use crate::app::App;
use crate::project::SyncSpecState;
use crate::selection::SelectableItem;
use crate::widgets::{HelpBar, StyledText};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

/// Safely truncate a digest string to 8 characters, or return the whole string if shorter.
/// Prevents panics when Mutagen returns unexpectedly short digest values.
fn truncate_digest(digest: &str) -> &str {
    if digest.len() >= 8 {
        &digest[..8]
    } else {
        digest
    }
}

/// Format a byte size as a human-readable string (e.g., "1.5 MB")
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1}G", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}M", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}K", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

/// Format a FileState for display, handling None (deleted/new files) and Some cases.
fn format_file_state(state: &Option<crate::mutagen::FileState>) -> String {
    match state {
        Some(fs) => match &fs.digest {
            Some(digest) => format!("{} ({})", truncate_digest(digest), fs.kind),
            None => fs.kind.clone(),
        },
        None => "-".to_string(),
    }
}

/// Calculate the height needed for the status area based on message length.
/// Returns a value between 3 and 7 (min 1 line of text, max 5 lines of text, plus 2 for borders).
fn calculate_status_height(status_text: &str, available_width: u16) -> u16 {
    // Account for borders and padding (2 for left/right borders, 2 for internal padding)
    let content_width = if available_width > 4 {
        (available_width - 4) as usize
    } else {
        1
    };

    // Use textwrap to calculate how many lines the text will wrap to
    let wrapped_lines = textwrap::wrap(status_text, content_width);
    let line_count = wrapped_lines.len() as u16;

    // Add 2 for borders, clamp between 3 (min) and 7 (max: 5 lines of content + 2 borders)
    (line_count + 2).clamp(3, 7)
}

pub fn draw(f: &mut Frame, app: &App) {
    // Build status text to calculate required height
    let mut status_text = app
        .status_message
        .as_ref()
        .map(|msg| msg.text().to_string())
        .unwrap_or_else(|| "Ready".to_string());

    if let Some(last_refresh) = app.last_refresh {
        let refresh_info = format!(" | Last refresh: {}", last_refresh.format("%H:%M:%S"));
        status_text.push_str(&refresh_info);
    }

    // Check if text will be clipped (more than 5 lines of content)
    let content_width = if f.area().width > 4 {
        (f.area().width - 4) as usize
    } else {
        1
    };
    let wrapped_lines = textwrap::wrap(&status_text, content_width);
    let will_be_clipped = wrapped_lines.len() > 5;

    if will_be_clipped {
        // Add ellipsis indicator to the status text
        status_text.push_str(" ...");
    }

    // Calculate dynamic status height based on message length (clamped to 3-7 lines)
    let status_height = calculate_status_height(&status_text, f.area().width);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(status_height),
            Constraint::Length(3),
        ])
        .split(f.area());

    draw_header(f, app, chunks[0]);

    if app.projects.is_empty() {
        draw_empty_state(f, app, chunks[1]);
    } else {
        draw_unified_panel(f, app, chunks[1]);
    }

    draw_status(f, app, chunks[2]);
    draw_help(f, app, chunks[3]);

    // Draw conflict detail overlay if viewing conflicts
    if app.viewing_conflicts {
        draw_conflict_detail(f, app);
    }

    // Draw help screen overlay if viewing help
    if app.viewing_help {
        draw_help_screen(f, app);
    }

    // Draw blocking operation modal if one is active
    if let Some(blocking_op) = &app.blocking_op {
        draw_blocking_modal(f, app, blocking_op);
    }
}

fn draw_empty_state(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.color_scheme;
    let message = Paragraph::new(vec![
        Line::from(""),
        StyledText::new(theme)
            .styled(
                "No Mutagen projects found",
                Style::default()
                    .fg(theme.session_status_fg)
                    .add_modifier(Modifier::BOLD),
            )
            .build(),
        Line::from(""),
        StyledText::new(theme)
            .help_text("• Create a mutagen.yml file in your project directory")
            .build(),
        StyledText::new(theme)
            .help_text("• Press 'r' to refresh")
            .build(),
    ])
    .block(Block::default().borders(Borders::ALL).title("Welcome"))
    .style(Style::default());

    f.render_widget(message, area);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let title = Paragraph::new(
        StyledText::new(&app.color_scheme)
            .header("Mutagen TUI")
            .build(),
    )
    .style(Style::default().add_modifier(Modifier::BOLD))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, area);
}

/// Draw the unified panel showing projects and their sync specs
fn draw_unified_panel(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.color_scheme;
    let mut items: Vec<ListItem> = Vec::new();

    // Count total specs across all projects
    let total_specs: usize = app.projects.iter().map(|p| p.specs.len()).sum();

    // Build list items from the selection manager's flattened view
    for (item_idx, item) in app.selection.items().enumerate() {
        let is_selected = item_idx == app.selection.raw_index();

        match item {
            SelectableItem::Project { index: proj_idx } => {
                // Render project header
                if let Some(project) = app.projects.get(*proj_idx) {
                    let spans = render_project_header(app, project);

                    let style = if is_selected {
                        Style::default()
                            .bg(theme.selection_bg)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };

                    items.push(ListItem::new(Line::from(spans)).style(style));
                }
            }
            SelectableItem::Spec {
                project_index: proj_idx,
                spec_index: spec_idx,
            } => {
                // Render spec row
                if let Some(project) = app.projects.get(*proj_idx) {
                    if let Some(spec) = project.specs.get(*spec_idx) {
                        let spans = render_spec_row(app, project, spec);

                        let style = if is_selected {
                            Style::default()
                                .bg(theme.selection_bg)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        };

                        items.push(ListItem::new(Line::from(spans)).style(style));
                    }
                }
            }
        }
    }

    let title = format!(
        " Sync Projects ({} projects, {} specs) ",
        app.projects.len(),
        total_specs
    );
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(title));

    f.render_widget(list, area);
}

/// Render a project header row with fold indicator, status, and stats
fn render_project_header(app: &App, project: &crate::project::Project) -> Vec<Span<'static>> {
    let theme = &app.color_scheme;

    // Fold icon
    let fold_icon = if project.folded { "▶" } else { "▼" };

    // Status icon (active if any spec is running)
    let is_active = project.specs.iter().any(|s| s.is_running());
    let status_icon = if is_active { "✓" } else { "○" };
    let status_color = if is_active {
        theme.status_running_fg
    } else {
        theme.status_paused_fg
    };

    // Count running specs
    let running_count = project.specs.iter().filter(|s| s.is_running()).count();
    let total_count = project.specs.len();

    // Count paused specs (running but paused)
    let paused_count = project.specs.iter().filter(|s| s.is_paused()).count();

    // Count push mode specs
    let push_count = project
        .specs
        .iter()
        .filter(|s| s.state == crate::project::SyncSpecState::RunningPush)
        .count();

    // Count conflicts across all running specs
    let conflict_count: usize = project
        .specs
        .iter()
        .filter_map(|s| s.running_session.as_ref())
        .map(|s| s.conflict_count())
        .sum();

    let mut spans = vec![
        Span::styled(
            format!("{} ", fold_icon),
            Style::default().fg(theme.session_name_fg),
        ),
        Span::styled(
            format!("{} ", status_icon),
            Style::default().fg(status_color),
        ),
        Span::styled(
            format!("{:<30}", project.file.display_name()),
            Style::default()
                .fg(theme.session_name_fg)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    // Collect status icons from all running sessions - bubble up if all agree
    let unified_status_icon: Option<&str> = {
        let status_icons: Vec<&str> = project
            .specs
            .iter()
            .filter_map(|s| s.running_session.as_ref())
            .filter(|s| !s.paused)
            .map(|s| s.status_icon())
            .collect();

        if !status_icons.is_empty() && status_icons.iter().all(|&icon| icon == status_icons[0]) {
            Some(status_icons[0])
        } else {
            None
        }
    };

    // Add unified status icon if all running sessions agree
    if let Some(icon) = unified_status_icon {
        spans.push(Span::styled(
            format!(" {}", icon),
            Style::default().fg(theme.session_status_fg),
        ));
    }

    // Add running status
    // Check if all running specs are paused
    let all_paused = running_count > 0 && paused_count == running_count;

    if running_count == 0 {
        spans.push(Span::styled(
            "  Not running".to_string(),
            Style::default().fg(theme.session_status_fg),
        ));
    } else if all_paused {
        // All running specs are paused
        let status_text = if running_count == total_count {
            "  Paused".to_string()
        } else {
            format!("  {}/{} paused", running_count, total_count)
        };
        spans.push(Span::styled(
            status_text,
            Style::default().fg(theme.status_paused_fg),
        ));
    } else if running_count == total_count {
        let status_text = if push_count > 0 {
            if push_count == running_count {
                "  Running (all push)".to_string()
            } else {
                format!("  Running ({} push)", push_count)
            }
        } else {
            "  Running".to_string()
        };
        spans.push(Span::styled(
            status_text,
            Style::default().fg(theme.session_status_fg),
        ));
    } else {
        let status_text = if push_count > 0 {
            format!(
                "  {}/{} running ({} push)",
                running_count, total_count, push_count
            )
        } else {
            format!("  {}/{} running", running_count, total_count)
        };
        spans.push(Span::styled(
            status_text,
            Style::default().fg(theme.session_status_fg),
        ));
    }

    // Add conflict indicator if there are conflicts
    if conflict_count > 0 {
        spans.push(Span::raw("  ".to_string()));
        spans.push(Span::styled(
            format!(
                "⚠ {} conflict{}",
                conflict_count,
                if conflict_count == 1 { "" } else { "s" }
            ),
            Style::default()
                .fg(theme.status_paused_fg)
                .add_modifier(Modifier::BOLD),
        ));
    }

    spans
}

/// Render a spec row with state indicator and details
fn render_spec_row(
    app: &App,
    project: &crate::project::Project,
    spec: &crate::project::SyncSpec,
) -> Vec<Span<'static>> {
    let theme = &app.color_scheme;

    let mut spans = vec![Span::raw("    ".to_string())]; // Indent for spec under project

    match &spec.state {
        SyncSpecState::NotRunning => {
            // Not running: show ○ icon and session definition endpoints
            spans.push(Span::styled(
                "○ ".to_string(),
                Style::default().fg(theme.status_paused_fg),
            ));
            spans.push(Span::styled(
                format!("{:<32}", spec.name),
                Style::default().fg(theme.session_name_fg),
            ));
            spans.push(Span::raw(" ".to_string()));

            // Look up session definition from project file to show endpoints
            if let Some(session_def) = project.file.sessions.get(&spec.name) {
                // Alpha endpoint (no status icon since not running)
                spans.push(Span::styled(
                    format!("{} ", apply_tilde(&session_def.alpha)),
                    Style::default().fg(theme.session_alpha_fg),
                ));

                // Arrow
                spans.push(Span::raw("⇄ ".to_string()));

                // Beta endpoint
                spans.push(Span::styled(
                    apply_tilde(&session_def.beta),
                    Style::default().fg(theme.session_beta_fg),
                ));
            } else {
                // Fallback if session definition not found
                spans.push(Span::styled(
                    "Not running".to_string(),
                    Style::default().fg(theme.session_status_fg),
                ));
            }
        }
        SyncSpecState::RunningTwoWay | SyncSpecState::RunningPush => {
            // Running: show session details
            if let Some(session) = &spec.running_session {
                let status_icon = if session.paused { "⏸" } else { "▶" };
                let status_color = if session.paused {
                    theme.status_paused_fg
                } else {
                    theme.status_running_fg
                };

                spans.push(Span::styled(
                    format!("{} ", status_icon),
                    Style::default().fg(status_color),
                ));

                // Session name with push mode indicator
                let name_with_mode = if spec.state == SyncSpecState::RunningPush {
                    format!("{} (push)", spec.name)
                } else {
                    spec.name.clone()
                };
                spans.push(Span::styled(
                    format!("{:<32}", name_with_mode),
                    Style::default()
                        .fg(theme.session_name_fg)
                        .add_modifier(Modifier::BOLD),
                ));

                spans.push(Span::raw(" ".to_string()));

                // Session status icon
                spans.push(Span::styled(
                    format!("{}  ", session.status_icon()),
                    Style::default().fg(theme.session_status_fg),
                ));

                // Alpha endpoint
                spans.push(Span::styled(
                    session.alpha.status_icon().to_string(),
                    Style::default().fg(if session.alpha.connected {
                        theme.status_running_fg
                    } else {
                        theme.status_paused_fg
                    }),
                ));
                spans.push(Span::styled(
                    format!("{} ", session.alpha_display()),
                    Style::default().fg(theme.session_alpha_fg),
                ));

                // Arrow and mode indicator (⇄ for two-way, ⬆ for push)
                if spec.state == SyncSpecState::RunningPush {
                    spans.push(Span::styled(
                        "⬆ ".to_string(),
                        Style::default()
                            .fg(theme.status_paused_fg)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else {
                    spans.push(Span::raw("⇄ ".to_string()));
                };

                // Beta endpoint
                spans.push(Span::styled(
                    session.beta.status_icon().to_string(),
                    Style::default().fg(if session.beta.connected {
                        theme.status_running_fg
                    } else {
                        theme.status_paused_fg
                    }),
                ));
                spans.push(Span::styled(
                    session.beta_display(),
                    Style::default().fg(theme.session_beta_fg),
                ));

                // Conflict indicator
                if session.has_conflicts() {
                    spans.push(Span::raw(" ".to_string()));
                    spans.push(Span::styled(
                        format!(
                            "⚠ {} conflict{}",
                            session.conflict_count(),
                            if session.conflict_count() == 1 {
                                ""
                            } else {
                                "s"
                            }
                        ),
                        Style::default()
                            .fg(theme.status_paused_fg)
                            .add_modifier(Modifier::BOLD),
                    ));
                }
            }
        }
    }

    spans
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    // Build status text: show selected spec status if available, otherwise show status message
    let (mut status_text, fg_color) = if let Some((proj_idx, spec_idx)) = app.get_selected_spec() {
        // Spec is selected - show its status
        if let Some(project) = app.projects.get(proj_idx) {
            if let Some(spec) = project.specs.get(spec_idx) {
                if let Some(session) = &spec.running_session {
                    // Build detailed status: "Name: Status"
                    let mut parts = vec![
                        session.name.clone(),
                        ": ".to_string(),
                        session.status_text().to_string(),
                    ];

                    // Add staging progress details if available
                    let staging = session
                        .beta
                        .staging_progress
                        .as_ref()
                        .or(session.alpha.staging_progress.as_ref());

                    if let Some(progress) = staging {
                        // Show percentage - prefer byte-based for granular progress on large files
                        let pct = if let (Some(received), Some(expected)) =
                            (progress.received_size, progress.expected_size)
                        {
                            if expected > 0 {
                                Some(((received * 100) / expected).min(100))
                            } else {
                                None
                            }
                        } else if let (Some(received), Some(expected)) =
                            (progress.received_files, progress.expected_files)
                        {
                            if expected > 0 {
                                Some(((received * 100) / expected).min(100))
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        if let Some(pct) = pct {
                            parts.push(format!(" ({}%)", pct));
                        }

                        // Show current file being copied
                        if let Some(path) = &progress.path {
                            // Extract just the filename for brevity
                            let filename = std::path::Path::new(path)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(path);
                            parts.push(format!(" | {}", filename));
                        }

                        // Show file size if available
                        if let (Some(received), Some(expected)) =
                            (progress.received_size, progress.expected_size)
                        {
                            if expected > 0 {
                                parts.push(format!(
                                    " [{}/{}]",
                                    format_size(received),
                                    format_size(expected)
                                ));
                            }
                        }
                    }

                    // Add conflict count if any
                    let conflict_count = session.conflict_count();
                    if conflict_count > 0 {
                        parts.push(format!(
                            " | {} conflict{}",
                            conflict_count,
                            if conflict_count == 1 { "" } else { "s" }
                        ));
                    }

                    (parts.join(""), app.color_scheme.status_message_fg)
                } else {
                    // Spec not running - show session definition endpoints
                    let mut parts = vec![format!("{}: Not running", spec.name)];

                    // Look up session definition to show what would be synced
                    if let Some(session_def) = project.file.sessions.get(&spec.name) {
                        parts.push(format!(
                            "\nAlpha: {}\nBeta:  {}",
                            apply_tilde(&session_def.alpha),
                            apply_tilde(&session_def.beta)
                        ));
                    }

                    (parts.join(""), app.color_scheme.status_message_fg)
                }
            } else {
                (
                    app.status_message
                        .as_ref()
                        .map(|msg| msg.text().to_string())
                        .unwrap_or_else(|| "Ready".to_string()),
                    app.color_scheme.status_message_fg,
                )
            }
        } else {
            (
                app.status_message
                    .as_ref()
                    .map(|msg| msg.text().to_string())
                    .unwrap_or_else(|| "Ready".to_string()),
                app.color_scheme.status_message_fg,
            )
        }
    } else if let Some(project_idx) = app.get_selected_project_index() {
        // Project selected - show status message if important, otherwise project info
        // Important messages: errors, warnings, or operation results (not "Sessions refreshed")
        let has_important_status = app.status_message.as_ref().is_some_and(|msg| {
            matches!(
                msg,
                crate::app::StatusMessage::Error(_) | crate::app::StatusMessage::Warning(_)
            ) || (matches!(msg, crate::app::StatusMessage::Info(text) if text != "Sessions refreshed"))
        });

        if has_important_status {
            let msg = app.status_message.as_ref().unwrap();
            let color = match msg {
                crate::app::StatusMessage::Error(_) => app.color_scheme.status_error_fg,
                crate::app::StatusMessage::Warning(_) => app.color_scheme.status_paused_fg,
                crate::app::StatusMessage::Info(_) => app.color_scheme.status_message_fg,
            };
            (msg.text().to_string(), color)
        } else if let Some(project) = app.projects.get(project_idx) {
            let total_specs = project.specs.len();
            let running_specs = project.specs.iter().filter(|s| s.is_running()).count();
            let text = format!(
                "{}: {} spec{}, {} running",
                project.file.display_name(),
                total_specs,
                if total_specs == 1 { "" } else { "s" },
                running_specs
            );
            (text, app.color_scheme.status_message_fg)
        } else {
            (
                app.status_message
                    .as_ref()
                    .map(|msg| msg.text().to_string())
                    .unwrap_or_else(|| "Ready".to_string()),
                app.color_scheme.status_message_fg,
            )
        }
    } else {
        // Nothing selected - show status message
        let text = app
            .status_message
            .as_ref()
            .map(|msg| msg.text().to_string())
            .unwrap_or_else(|| "Ready".to_string());

        let color = app
            .status_message
            .as_ref()
            .map(|msg| match msg {
                crate::app::StatusMessage::Error(_) => app.color_scheme.status_error_fg,
                crate::app::StatusMessage::Warning(_) => app.color_scheme.status_paused_fg,
                crate::app::StatusMessage::Info(_) => app.color_scheme.status_message_fg,
            })
            .unwrap_or(app.color_scheme.status_message_fg);

        (text, color)
    };

    if let Some(last_refresh) = app.last_refresh {
        let refresh_info = format!(" | Last refresh: {}", last_refresh.format("%H:%M:%S"));
        status_text.push_str(&refresh_info);
    }

    let status = Paragraph::new(status_text)
        .style(Style::default().fg(fg_color))
        .block(Block::default().borders(Borders::ALL).title("Status"))
        .wrap(Wrap { trim: true });

    f.render_widget(status, area);
}

fn draw_help(f: &mut Frame, app: &App, area: Rect) {
    // Check if project is selected
    let is_project_selected = app.selection.is_project_selected();

    // Check if a spec is selected
    let is_spec_selected = app.selection.is_spec_selected();

    let mut help_bar = HelpBar::new(&app.color_scheme)
        .item("↑/↓/j/k", "Nav")
        .item("h/l/↵", "Fold")
        .item("r", "Refresh")
        .item("?", "Help");

    if is_project_selected {
        // Project-specific commands
        help_bar = help_bar
            .item("e", "Edit")
            .item("s", "Start")
            .item("t", "Terminate")
            .item("f", "Flush")
            .item("p", "Push")
            .item("Space", "Pause/Resume");
    } else if is_spec_selected {
        // Spec-specific commands
        help_bar = help_bar
            .item("s", "Start")
            .item("t", "Terminate")
            .item("f", "Flush")
            .item("p", "Push")
            .item("Space", "Pause/Resume")
            .item("c", "Conflicts");
    }

    // Common commands at end
    help_bar = help_bar.item("q", "Quit");

    let help = Paragraph::new(help_bar.build())
        .block(Block::default().borders(Borders::ALL).title("Help"));

    f.render_widget(help, area);
}

fn draw_blocking_modal(f: &mut Frame, app: &App, blocking_op: &crate::app::BlockingOperation) {
    use ratatui::layout::{Alignment, Margin};
    use ratatui::widgets::Clear;

    // Create a centered overlay area (50% width, 7 lines height)
    let area = f.area();
    let overlay_width = (area.width as f32 * 0.5) as u16;
    let overlay_height = 7;
    let overlay_x = (area.width - overlay_width) / 2;
    let overlay_y = (area.height - overlay_height) / 2;

    let overlay_area = Rect {
        x: overlay_x,
        y: overlay_y,
        width: overlay_width,
        height: overlay_height,
    };

    // Clear the background (prevents visual artifacts)
    f.render_widget(Clear, overlay_area);

    // Render the modal block
    let modal_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.color_scheme.help_key_fg))
        .style(Style::default().bg(Color::White));

    f.render_widget(modal_block, overlay_area);

    // Inner area for content
    let inner_area = overlay_area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });

    // Build message with optional progress indicator
    let message = if let (Some(current), Some(total)) = (blocking_op.current, blocking_op.total) {
        format!(
            "⏳ {}\n\nProgress: {} / {}\n\nPlease wait...",
            blocking_op.message, current, total
        )
    } else {
        format!("⏳ {}\n\nPlease wait...", blocking_op.message)
    };

    let paragraph = Paragraph::new(message)
        .alignment(Alignment::Center)
        .style(Style::default().fg(app.color_scheme.status_message_fg));

    f.render_widget(paragraph, inner_area);
}

fn draw_conflict_detail(f: &mut Frame, app: &App) {
    use ratatui::layout::{Alignment, Margin};
    use ratatui::widgets::Clear;

    // Create a centered overlay area (80% width, 80% height)
    let area = f.area();
    let overlay_width = (area.width as f32 * 0.8) as u16;
    let overlay_height = (area.height as f32 * 0.8) as u16;
    let overlay_x = (area.width - overlay_width) / 2;
    let overlay_y = (area.height - overlay_height) / 2;

    let overlay_area = Rect {
        x: overlay_x,
        y: overlay_y,
        width: overlay_width,
        height: overlay_height,
    };

    f.render_widget(Clear, overlay_area);

    // Clear the overlay area with a white background
    let overlay_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.color_scheme.help_key_fg))
        .title(" Conflict Details (Esc or 'c' to close) ")
        .title_alignment(Alignment::Center)
        .style(Style::default().bg(Color::White));

    f.render_widget(overlay_block, overlay_area);

    // Draw conflict list inside the overlay
    let inner_area = overlay_area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(inner_area);

    let info_lines = vec![
        Line::from(vec![Span::styled(
            "Press 'b' to push all conflicts to beta (alpha → beta copy)",
            Style::default()
                .fg(app.color_scheme.help_key_fg)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            "Press Esc or 'c' to close this view",
            Style::default().fg(app.color_scheme.help_text_fg),
        )]),
    ];

    let info = Paragraph::new(info_lines)
        .alignment(Alignment::Center)
        .style(Style::default().bg(Color::White));
    f.render_widget(info, sections[0]);

    if let Some(conflicts) = app.get_selected_spec_conflicts() {
        if conflicts.is_empty() {
            let no_conflicts = Paragraph::new("No conflicts found")
                .style(Style::default().fg(app.color_scheme.session_status_fg))
                .alignment(Alignment::Center);
            f.render_widget(no_conflicts, sections[1]);
        } else {
            let conflict_items: Vec<ListItem> = conflicts
                .iter()
                .map(|conflict| {
                    let mut lines = vec![Line::from(vec![
                        Span::styled(
                            "Root: ",
                            Style::default()
                                .fg(app.color_scheme.session_name_fg)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            &conflict.root,
                            Style::default().fg(app.color_scheme.session_alpha_fg),
                        ),
                        Span::raw("  "),
                        Span::styled(
                            format!(
                                "α {} / β {} change{}",
                                conflict.alpha_changes.len(),
                                conflict.beta_changes.len(),
                                if conflict.alpha_changes.len() + conflict.beta_changes.len() == 1 {
                                    ""
                                } else {
                                    "s"
                                }
                            ),
                            Style::default().fg(app.color_scheme.session_status_fg),
                        ),
                    ])];

                    if !conflict.alpha_changes.is_empty() {
                        lines.push(Line::from(vec![
                            Span::styled(
                                "  α ",
                                Style::default()
                                    .fg(app.color_scheme.session_alpha_fg)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                summarize_conflict_changes(&conflict.alpha_changes),
                                Style::default().fg(app.color_scheme.session_status_fg),
                            ),
                        ]));
                    }

                    if !conflict.beta_changes.is_empty() {
                        lines.push(Line::from(vec![
                            Span::styled(
                                "  β ",
                                Style::default()
                                    .fg(app.color_scheme.session_beta_fg)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                summarize_conflict_changes(&conflict.beta_changes),
                                Style::default().fg(app.color_scheme.session_status_fg),
                            ),
                        ]));
                    }

                    ListItem::new(lines)
                })
                .collect();

            let conflict_list = List::new(conflict_items)
                .block(Block::default())
                .highlight_symbol("→ ");
            f.render_widget(conflict_list, sections[1]);
        }
    } else {
        let error = Paragraph::new("No session selected")
            .style(Style::default().fg(app.color_scheme.session_status_fg))
            .alignment(Alignment::Center);
        f.render_widget(error, sections[1]);
    }
}

fn summarize_conflict_changes(changes: &[crate::mutagen::Change]) -> String {
    const MAX_DISPLAY: usize = 3;
    if changes.is_empty() {
        return "No changes".to_string();
    }

    let mut parts: Vec<String> = changes
        .iter()
        .take(MAX_DISPLAY)
        .map(|change| {
            let old_state = format_file_state(&change.old);
            let new_state = format_file_state(&change.new);
            if old_state == new_state || (old_state == "-" && new_state == "-") {
                change.path.clone()
            } else {
                format!("{} ({} → {})", change.path, old_state, new_state)
            }
        })
        .collect();

    if changes.len() > MAX_DISPLAY {
        parts.push(format!("+{} more", changes.len() - MAX_DISPLAY));
    }

    parts.join(", ")
}

fn draw_help_screen(f: &mut Frame, app: &App) {
    use ratatui::layout::{Alignment, Margin};
    use ratatui::widgets::Clear;

    // Create a centered overlay area (85% width, 90% height)
    let area = f.area();
    let overlay_width = (area.width as f32 * 0.85) as u16;
    let overlay_height = (area.height as f32 * 0.90) as u16;
    let overlay_x = (area.width - overlay_width) / 2;
    let overlay_y = (area.height - overlay_height) / 2;

    let overlay_area = Rect {
        x: overlay_x,
        y: overlay_y,
        width: overlay_width,
        height: overlay_height,
    };

    // Clear the background (prevents transparency)
    f.render_widget(Clear, overlay_area);

    // Draw the overlay block with background
    let overlay_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.color_scheme.help_key_fg))
        .title(" Mutagen TUI - Keyboard Commands (press '?' to close) ")
        .title_alignment(Alignment::Center)
        .style(Style::default().bg(Color::White));

    f.render_widget(overlay_block, overlay_area);

    // Draw help content inside the overlay
    let inner_area = overlay_area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });

    let theme = &app.color_scheme;

    let help_lines = vec![
        Line::from(vec![Span::styled(
            "NAVIGATION",
            Style::default()
                .fg(theme.header_fg)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("  ↑/k, ↓/j", Style::default().fg(theme.help_key_fg)),
            Span::raw("        "),
            Span::styled(
                "Move selection up/down",
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
        Line::from(vec![
            Span::styled("  h/←, l/→/↵", Style::default().fg(theme.help_key_fg)),
            Span::raw("    "),
            Span::styled(
                "Fold/unfold project",
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
        Line::from(""),
        Line::from(""),
        Line::from(vec![Span::styled(
            "GLOBAL ACTIONS",
            Style::default()
                .fg(theme.header_fg)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("  r", Style::default().fg(theme.help_key_fg)),
            Span::raw("             "),
            Span::styled(
                "Refresh session list and projects",
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
        Line::from(vec![
            Span::styled("  m", Style::default().fg(theme.help_key_fg)),
            Span::raw("             "),
            Span::styled(
                "Toggle display mode (paths vs. last sync time)",
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
        Line::from(vec![
            Span::styled("  q, Ctrl-C", Style::default().fg(theme.help_key_fg)),
            Span::raw("     "),
            Span::styled("Quit application", Style::default().fg(theme.help_text_fg)),
        ]),
        Line::from(vec![
            Span::styled("  ?", Style::default().fg(theme.help_key_fg)),
            Span::raw("             "),
            Span::styled(
                "Toggle this help screen",
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "PROJECT ACTIONS ",
                Style::default()
                    .fg(theme.header_fg)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "(when project selected)",
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
        Line::from(vec![
            Span::styled("  e", Style::default().fg(theme.help_key_fg)),
            Span::raw("             "),
            Span::styled(
                "Edit project configuration file",
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
        Line::from(vec![
            Span::styled("  s", Style::default().fg(theme.help_key_fg)),
            Span::raw("             "),
            Span::styled(
                "Start all specs in project",
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
        Line::from(vec![
            Span::styled("  t", Style::default().fg(theme.help_key_fg)),
            Span::raw("             "),
            Span::styled(
                "Terminate all specs in project",
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
        Line::from(vec![
            Span::styled("  f", Style::default().fg(theme.help_key_fg)),
            Span::raw("             "),
            Span::styled(
                "Flush all specs in project",
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
        Line::from(vec![
            Span::styled("  p", Style::default().fg(theme.help_key_fg)),
            Span::raw("             "),
            Span::styled(
                "Create push sessions for all specs",
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Space", Style::default().fg(theme.help_key_fg)),
            Span::raw("         "),
            Span::styled(
                "Pause/resume all running specs",
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
        Line::from(vec![
            Span::styled("  u", Style::default().fg(theme.help_key_fg)),
            Span::raw("             "),
            Span::styled(
                "Resume all paused specs",
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "SPEC ACTIONS ",
                Style::default()
                    .fg(theme.header_fg)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "(when individual spec selected)",
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
        Line::from(vec![
            Span::styled("  s", Style::default().fg(theme.help_key_fg)),
            Span::raw("             "),
            Span::styled("Start this spec", Style::default().fg(theme.help_text_fg)),
        ]),
        Line::from(vec![
            Span::styled("  t", Style::default().fg(theme.help_key_fg)),
            Span::raw("             "),
            Span::styled(
                "Terminate this spec",
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
        Line::from(vec![
            Span::styled("  f", Style::default().fg(theme.help_key_fg)),
            Span::raw("             "),
            Span::styled("Flush this spec", Style::default().fg(theme.help_text_fg)),
        ]),
        Line::from(vec![
            Span::styled("  p", Style::default().fg(theme.help_key_fg)),
            Span::raw("             "),
            Span::styled(
                "Create push session (replaces two-way if running)",
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Space", Style::default().fg(theme.help_key_fg)),
            Span::raw("         "),
            Span::styled("Pause/resume spec", Style::default().fg(theme.help_text_fg)),
        ]),
        Line::from(vec![
            Span::styled("  u", Style::default().fg(theme.help_key_fg)),
            Span::raw("             "),
            Span::styled(
                "Resume paused spec",
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
        Line::from(vec![
            Span::styled("  c", Style::default().fg(theme.help_key_fg)),
            Span::raw("             "),
            Span::styled(
                "View conflicts (shows overlay with conflict details)",
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
        Line::from(vec![
            Span::styled("  b", Style::default().fg(theme.help_key_fg)),
            Span::raw("             "),
            Span::styled(
                "While viewing conflicts: push alpha changes to beta",
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
        Line::from(""),
        Line::from(""),
        Line::from(vec![Span::styled(
            "VISUAL INDICATORS",
            Style::default()
                .fg(theme.header_fg)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("  ▼/▶", Style::default().fg(theme.help_key_fg)),
            Span::raw("           "),
            Span::styled(
                "Project expanded/collapsed",
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
        Line::from(vec![
            Span::styled("  ✓/○", Style::default().fg(theme.help_key_fg)),
            Span::raw("           "),
            Span::styled(
                "Project active/inactive",
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
        Line::from(vec![
            Span::styled("  ▶/⏸/○", Style::default().fg(theme.help_key_fg)),
            Span::raw("        "),
            Span::styled(
                "Spec running/paused/not running",
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
        Line::from(vec![
            Span::styled("  ⇄/⬆", Style::default().fg(theme.help_key_fg)),
            Span::raw("          "),
            Span::styled(
                "Two-way sync / Push mode (one-way)",
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
        Line::from(vec![
            Span::styled("  ✓⟳⊗", Style::default().fg(theme.help_key_fg)),
            Span::raw("          "),
            Span::styled(
                "Endpoint connected/scanning/disconnected",
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
        Line::from(vec![
            Span::styled("  ⚠ X conflicts", Style::default().fg(theme.help_key_fg)),
            Span::raw(" "),
            Span::styled(
                "Number of conflicts detected",
                Style::default().fg(theme.help_text_fg),
            ),
        ]),
    ];

    let help_text = Paragraph::new(help_lines)
        .style(Style::default().bg(Color::White))
        .wrap(Wrap { trim: false })
        .scroll((0, 0));

    f.render_widget(help_text, inner_area);
}

/// Apply tilde abbreviation to a path string
/// Replaces /Users/username with ~ or ~username depending on current user
fn apply_tilde(endpoint: &str) -> String {
    // Handle SSH-style endpoints (user@host:path or host:path)
    if let Some(colon_pos) = endpoint.rfind(':') {
        let (prefix, path) = endpoint.split_at(colon_pos + 1);
        return format!("{}{}", prefix, apply_tilde_to_path(path));
    }

    // Local path
    apply_tilde_to_path(endpoint)
}

/// Apply tilde abbreviation to just the path component
fn apply_tilde_to_path(path: &str) -> String {
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() && path.starts_with(&home) {
            return path.replacen(&home, "~", 1);
        }
    }

    // Handle other users' home directories (/Users/otheruser -> ~otheruser)
    #[cfg(target_os = "macos")]
    {
        if let Some(stripped) = path.strip_prefix("/Users/") {
            if let Some(slash_pos) = stripped.find('/') {
                let username = &stripped[..slash_pos];
                let rest = &stripped[slash_pos..];
                return format!("~{}{}", username, rest);
            } else {
                // Just /Users/username with no trailing path
                return format!("~{}", stripped);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(stripped) = path.strip_prefix("/home/") {
            if let Some(slash_pos) = stripped.find('/') {
                let username = &stripped[..slash_pos];
                let rest = &stripped[slash_pos..];
                return format!("~{}{}", username, rest);
            } else {
                // Just /home/username with no trailing path
                return format!("~{}", stripped);
            }
        }
    }

    path.to_string()
}
