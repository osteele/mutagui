use crate::app::{App, SessionDisplayMode};
use crate::widgets::{HelpBar, StyledText};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
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

    if app.sessions.is_empty() && app.projects.is_empty() {
        draw_empty_state(f, app, chunks[1]);
    } else {
        // Build constraints based on what's present
        // Use Percentage(100) when only one section exists to fill the viewport
        let has_sessions = !app.sessions.is_empty();
        let has_projects = !app.projects.is_empty();

        let mut constraints = Vec::new();
        if has_projects {
            if has_sessions {
                constraints.push(Constraint::Percentage(50)); // Both: split 50/50
            } else {
                constraints.push(Constraint::Percentage(100)); // Projects only: fill viewport
            }
        }
        if has_sessions {
            if has_projects {
                constraints.push(Constraint::Percentage(50)); // Both: split 50/50
            } else {
                constraints.push(Constraint::Percentage(100)); // Sessions only: fill viewport
            }
        }

        let content_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(chunks[1]);

        let mut chunk_idx = 0;
        if !app.projects.is_empty() {
            draw_projects(f, app, content_chunks[chunk_idx]);
            chunk_idx += 1;
        }
        if !app.sessions.is_empty() {
            draw_sessions(f, app, content_chunks[chunk_idx]);
        }
    }

    draw_status(f, app, chunks[2]);
    draw_help(f, app, chunks[3]);

    // Draw conflict detail overlay if viewing conflicts
    if app.viewing_conflicts {
        draw_conflict_detail(f, app);
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
                "No Mutagen sessions or projects found",
                Style::default()
                    .fg(theme.session_status_fg)
                    .add_modifier(Modifier::BOLD),
            )
            .build(),
        Line::from(""),
        StyledText::new(theme)
            .help_text("• Start a new session with: mutagen sync create")
            .build(),
        StyledText::new(theme)
            .help_text("• Or create a mutagen.yml file in your project directory")
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

/// Get the display name for a session, with project prefix if applicable
fn get_session_display_name(app: &App, session: &crate::mutagen::SyncSession) -> String {
    // Find which project owns this session
    for project in &app.projects {
        if project
            .active_sessions
            .iter()
            .any(|s| s.name == session.name)
        {
            // Session belongs to this project - format as "project-name > session-name"
            let project_name = project.file.display_name();
            return format!("{} > {}", project_name, session.name);
        }
    }
    // Session doesn't belong to any project - just show the name
    session.name.clone()
}

fn draw_sessions(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .sessions
        .iter()
        .enumerate()
        .map(|(i, session)| {
            let status_icon = if session.paused { "⏸" } else { "▶" };
            let status_color = if session.paused {
                app.color_scheme.status_paused_fg
            } else {
                app.color_scheme.status_running_fg
            };

            let display_name = get_session_display_name(app, session);

            let mut spans = vec![
                Span::styled(
                    format!("{} ", status_icon),
                    Style::default().fg(status_color),
                ),
                Span::styled(
                    format!("{:<30}", display_name), // Increased width to accommodate "project > session"
                    Style::default()
                        .fg(app.color_scheme.session_name_fg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
            ];

            match app.session_display_mode {
                SessionDisplayMode::ShowPaths => {
                    let alpha_stats = session.alpha.stats_display();
                    let beta_stats = session.beta.stats_display();

                    spans.push(Span::styled(
                        session.alpha.status_icon(),
                        Style::default().fg(if session.alpha.connected {
                            app.color_scheme.status_running_fg
                        } else {
                            app.color_scheme.status_paused_fg
                        }),
                    ));
                    spans.push(Span::styled(
                        format!("{:<25}", session.alpha_display()),
                        Style::default().fg(app.color_scheme.session_alpha_fg),
                    ));

                    if !alpha_stats.is_empty() {
                        spans.push(Span::styled(
                            format!("({}) ", alpha_stats),
                            Style::default().fg(app.color_scheme.session_status_fg),
                        ));
                    }

                    spans.extend(vec![
                        Span::raw("⇄ "),
                        Span::styled(
                            session.beta.status_icon(),
                            Style::default().fg(if session.beta.connected {
                                app.color_scheme.status_running_fg
                            } else {
                                app.color_scheme.status_paused_fg
                            }),
                        ),
                        Span::styled(
                            format!("{:<25}", session.beta_display()),
                            Style::default().fg(app.color_scheme.session_beta_fg),
                        ),
                    ]);

                    if !beta_stats.is_empty() {
                        spans.push(Span::styled(
                            format!("({}) ", beta_stats),
                            Style::default().fg(app.color_scheme.session_status_fg),
                        ));
                    }
                }
                SessionDisplayMode::ShowLastRefresh => {
                    spans.push(Span::styled(
                        format!("Last synced: {}", session.time_ago_display()),
                        Style::default().fg(app.color_scheme.session_status_fg),
                    ));
                }
            }

            // Add push indicator for one-way-replica sessions
            if let Some(ref mode) = session.mode {
                if mode == "one-way-replica" {
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        "⬆",
                        Style::default()
                            .fg(app.color_scheme.status_running_fg)
                            .add_modifier(Modifier::BOLD),
                    ));
                }
            }

            spans.extend(vec![
                Span::raw(" • "),
                Span::styled(
                    &session.status,
                    Style::default().fg(app.color_scheme.session_status_fg),
                ),
            ]);

            // Add conflict indicator
            if session.has_conflicts() {
                spans.push(Span::raw(" "));
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
                        .fg(app.color_scheme.help_key_fg) // Using help_key_fg for warning color
                        .add_modifier(Modifier::BOLD),
                ));
            }

            let content = Line::from(spans);

            let is_selected = i + app.projects.len() == app.selected_index();
            let style = if is_selected {
                Style::default()
                    .bg(app.color_scheme.selection_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Sync Sessions ({})", app.sessions.len())),
    );

    f.render_widget(list, area);
}

fn draw_projects(f: &mut Frame, app: &App, area: Rect) {
    let project_index_offset = 0; // Projects are now first
    let theme = &app.color_scheme;

    let items: Vec<ListItem> = app
        .projects
        .iter()
        .enumerate()
        .map(|(i, project)| {
            let status_icon = project.status_icon();
            let is_active = project.is_active();

            let mut lines = Vec::new();

            let file_path = project.file.path.display().to_string();
            let short_path = if let Ok(home) = std::env::var("HOME") {
                if !home.is_empty() && file_path.starts_with(&home) {
                    file_path.replace(&home, "~")
                } else {
                    file_path
                }
            } else {
                file_path
            };

            let header = StyledText::new(theme)
                .status_icon_owned(format!("{} ", status_icon), is_active)
                .session_name_owned(project.file.display_name())
                .text(" ")
                .status_text_owned(format!("({})", short_path))
                .build();
            lines.push(header);

            for session in &project.active_sessions {
                let session_line = StyledText::new(theme)
                    .text("  └─ ")
                    .endpoint_alpha(&session.name)
                    .text(": ")
                    .status_text(&session.status)
                    .build();
                lines.push(session_line);
            }

            if project.active_sessions.is_empty() {
                let empty_line = StyledText::new(theme)
                    .text("  ")
                    .status_text("(no running sessions)")
                    .build();
                lines.push(empty_line);
            }

            let is_selected = project_index_offset + i == app.selected_index();
            let style = if is_selected {
                Style::default()
                    .bg(theme.selection_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(lines).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Projects ({} files)", app.projects.len())),
    );

    f.render_widget(list, area);
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let mut status_text = app
        .status_message
        .as_ref()
        .map(|msg| msg.text().to_string())
        .unwrap_or_else(|| "Ready".to_string());

    if let Some(last_refresh) = app.last_refresh {
        let refresh_info = format!(" | Last refresh: {}", last_refresh.format("%H:%M:%S"));
        status_text.push_str(&refresh_info);
    }

    // Choose color based on message severity
    let fg_color = app
        .status_message
        .as_ref()
        .map(|msg| match msg {
            crate::app::StatusMessage::Error(_) => app.color_scheme.status_error_fg,
            crate::app::StatusMessage::Warning(_) => app.color_scheme.status_paused_fg,
            crate::app::StatusMessage::Info(_) => app.color_scheme.status_message_fg,
        })
        .unwrap_or(app.color_scheme.status_message_fg);

    let status = Paragraph::new(status_text)
        .style(Style::default().fg(fg_color))
        .block(Block::default().borders(Borders::ALL).title("Status"))
        .wrap(Wrap { trim: true });

    f.render_widget(status, area);
}

fn draw_help(f: &mut Frame, app: &App, area: Rect) {
    let is_project_selected = app.selected_index() < app.projects.len();

    let mut help_bar = HelpBar::new(&app.color_scheme)
        .item("↑/↓", "Nav")
        .item("Tab", "Switch Area")
        .item("m", "Mode");

    if is_project_selected {
        // Project-specific commands
        help_bar = help_bar
            .item("↵", "Edit")
            .item("s", "Start/Stop")
            .item("p", "Push")
            .item("Space", "Pause/Resume");
    } else {
        // Session-specific commands
        help_bar = help_bar
            .item("Space", "Pause/Resume")
            .item("f", "Flush")
            .item("t", "Terminate")
            .item("c", "Conflicts");
    }

    // Common commands
    help_bar = help_bar.item("r", "Refresh").item("q", "Quit");

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
        .style(Style::default().bg(app.color_scheme.selection_bg));

    f.render_widget(modal_block, overlay_area);

    // Inner area for content
    let inner_area = overlay_area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });

    // Static hourglass indicator (spinner won't animate since we only draw once)
    let message = format!("⏳ {}\n\nPlease wait...", blocking_op.message);

    let paragraph = Paragraph::new(message)
        .alignment(Alignment::Center)
        .style(Style::default().fg(app.color_scheme.status_message_fg));

    f.render_widget(paragraph, inner_area);
}

fn draw_conflict_detail(f: &mut Frame, app: &App) {
    use ratatui::layout::{Alignment, Margin};

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

    // Clear the overlay area with a background
    let overlay_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.color_scheme.help_key_fg))
        .title(" Conflict Details (press 'c' to close) ")
        .title_alignment(Alignment::Center)
        .style(Style::default().bg(app.color_scheme.selection_bg));

    f.render_widget(overlay_block, overlay_area);

    // Draw conflict list inside the overlay
    let inner_area = overlay_area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });

    if let Some(conflicts) = app.get_selected_session_conflicts() {
        if conflicts.is_empty() {
            let no_conflicts = Paragraph::new("No conflicts found")
                .style(Style::default().fg(app.color_scheme.session_status_fg))
                .alignment(Alignment::Center);
            f.render_widget(no_conflicts, inner_area);
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
                    ])];

                    if !conflict.alpha_changes.is_empty() {
                        lines.push(Line::from(vec![Span::styled(
                            "  Alpha changes:",
                            Style::default()
                                .fg(app.color_scheme.session_name_fg)
                                .add_modifier(Modifier::BOLD),
                        )]));
                        for change in &conflict.alpha_changes {
                            lines.push(Line::from(vec![
                                Span::raw("    "),
                                Span::styled(
                                    &change.path,
                                    Style::default().fg(app.color_scheme.session_alpha_fg),
                                ),
                            ]));

                            // Format the change description, handling optional FileState
                            let old_str = format_file_state(&change.old);
                            let new_str = format_file_state(&change.new);

                            lines.push(Line::from(vec![
                                Span::raw("      "),
                                Span::styled(
                                    old_str,
                                    Style::default().fg(app.color_scheme.session_status_fg),
                                ),
                                Span::raw(" → "),
                                Span::styled(
                                    new_str,
                                    Style::default().fg(app.color_scheme.session_status_fg),
                                ),
                            ]));
                        }
                    }

                    if !conflict.beta_changes.is_empty() {
                        lines.push(Line::from(vec![Span::styled(
                            "  Beta changes:",
                            Style::default()
                                .fg(app.color_scheme.session_name_fg)
                                .add_modifier(Modifier::BOLD),
                        )]));
                        for change in &conflict.beta_changes {
                            lines.push(Line::from(vec![
                                Span::raw("    "),
                                Span::styled(
                                    &change.path,
                                    Style::default().fg(app.color_scheme.session_beta_fg),
                                ),
                            ]));

                            // Format the change description, handling optional FileState
                            let old_str = format_file_state(&change.old);
                            let new_str = format_file_state(&change.new);

                            lines.push(Line::from(vec![
                                Span::raw("      "),
                                Span::styled(
                                    old_str,
                                    Style::default().fg(app.color_scheme.session_status_fg),
                                ),
                                Span::raw(" → "),
                                Span::styled(
                                    new_str,
                                    Style::default().fg(app.color_scheme.session_status_fg),
                                ),
                            ]));
                        }
                    }

                    lines.push(Line::from(""));

                    ListItem::new(lines)
                })
                .collect();

            let conflict_list = List::new(conflict_items).block(Block::default());
            f.render_widget(conflict_list, inner_area);
        }
    } else {
        let error = Paragraph::new("No session selected")
            .style(Style::default().fg(app.color_scheme.session_status_fg))
            .alignment(Alignment::Center);
        f.render_widget(error, inner_area);
    }
}
