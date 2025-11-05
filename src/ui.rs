use crate::app::{App, SessionDisplayMode};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
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

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
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
}

fn draw_empty_state(f: &mut Frame, app: &App, area: Rect) {
    let message = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "No Mutagen sessions or projects found",
            Style::default()
                .fg(app.color_scheme.session_status_fg)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "• Start a new session with: mutagen sync create",
            Style::default().fg(app.color_scheme.help_text_fg),
        )),
        Line::from(Span::styled(
            "• Or create a mutagen.yml file in your project directory",
            Style::default().fg(app.color_scheme.help_text_fg),
        )),
    ])
    .block(Block::default().borders(Borders::ALL).title("Welcome"))
    .style(Style::default());

    f.render_widget(message, area);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let title = Paragraph::new("Mutagen TUI")
        .style(
            Style::default()
                .fg(app.color_scheme.header_fg)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, area);
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

            let mut spans = vec![
                Span::styled(
                    format!("{} ", status_icon),
                    Style::default().fg(status_color),
                ),
                Span::styled(
                    format!("{:<18}", session.name),
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

            let is_selected = i + app.projects.len() == app.selected_index;
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

    let items: Vec<ListItem> = app
        .projects
        .iter()
        .enumerate()
        .map(|(i, project)| {
            let status_icon = project.status_icon();
            let status_color = if project.is_active() {
                app.color_scheme.status_running_fg
            } else {
                app.color_scheme.status_paused_fg
            };

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

            let header = Line::from(vec![
                Span::styled(
                    format!("{} ", status_icon),
                    Style::default().fg(status_color),
                ),
                Span::styled(
                    project.file.display_name(),
                    Style::default()
                        .fg(app.color_scheme.session_name_fg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("({})", short_path),
                    Style::default().fg(app.color_scheme.session_status_fg),
                ),
            ]);
            lines.push(header);

            for session in &project.active_sessions {
                let session_line = Line::from(vec![
                    Span::raw("  └─ "),
                    Span::styled(
                        &session.name,
                        Style::default().fg(app.color_scheme.session_alpha_fg),
                    ),
                    Span::raw(": "),
                    Span::styled(
                        &session.status,
                        Style::default().fg(app.color_scheme.session_status_fg),
                    ),
                ]);
                lines.push(session_line);
            }

            if project.active_sessions.is_empty() {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        "(no running sessions)",
                        Style::default().fg(app.color_scheme.session_status_fg),
                    ),
                ]));
            }

            let is_selected = project_index_offset + i == app.selected_index;
            let style = if is_selected {
                Style::default()
                    .bg(app.color_scheme.selection_bg)
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
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Ready".to_string());

    if let Some(last_refresh) = app.last_refresh {
        let refresh_info = format!(" | Last refresh: {}", last_refresh.format("%H:%M:%S"));
        status_text.push_str(&refresh_info);
    }

    let status = Paragraph::new(status_text)
        .style(Style::default().fg(app.color_scheme.status_message_fg))
        .block(Block::default().borders(Borders::ALL).title("Status"));

    f.render_widget(status, area);
}

fn draw_help(f: &mut Frame, app: &App, area: Rect) {
    let is_project_selected = app.selected_index < app.projects.len();

    let mut spans = vec![
        Span::styled("↑/↓", Style::default().fg(app.color_scheme.help_key_fg)),
        Span::styled(
            " Nav | ",
            Style::default().fg(app.color_scheme.help_text_fg),
        ),
        Span::styled("Tab", Style::default().fg(app.color_scheme.help_key_fg)),
        Span::styled(
            " Switch Area | ",
            Style::default().fg(app.color_scheme.help_text_fg),
        ),
        Span::styled("m", Style::default().fg(app.color_scheme.help_key_fg)),
        Span::styled(
            " Mode | ",
            Style::default().fg(app.color_scheme.help_text_fg),
        ),
    ];

    if is_project_selected {
        // Project-specific commands
        spans.extend(vec![
            Span::styled("s", Style::default().fg(app.color_scheme.help_key_fg)),
            Span::styled(
                " Start/Stop | ",
                Style::default().fg(app.color_scheme.help_text_fg),
            ),
            Span::styled("p", Style::default().fg(app.color_scheme.help_key_fg)),
            Span::styled(
                " Push | ",
                Style::default().fg(app.color_scheme.help_text_fg),
            ),
            Span::styled("Space", Style::default().fg(app.color_scheme.help_key_fg)),
            Span::styled(
                " Pause/Resume | ",
                Style::default().fg(app.color_scheme.help_text_fg),
            ),
        ]);
    } else {
        // Session-specific commands
        spans.extend(vec![
            Span::styled("Space", Style::default().fg(app.color_scheme.help_key_fg)),
            Span::styled(
                " Pause/Resume | ",
                Style::default().fg(app.color_scheme.help_text_fg),
            ),
            Span::styled("f", Style::default().fg(app.color_scheme.help_key_fg)),
            Span::styled(
                " Flush | ",
                Style::default().fg(app.color_scheme.help_text_fg),
            ),
            Span::styled("t", Style::default().fg(app.color_scheme.help_key_fg)),
            Span::styled(
                " Terminate | ",
                Style::default().fg(app.color_scheme.help_text_fg),
            ),
            Span::styled("c", Style::default().fg(app.color_scheme.help_key_fg)),
            Span::styled(
                " Conflicts | ",
                Style::default().fg(app.color_scheme.help_text_fg),
            ),
        ]);
    }

    // Common commands
    spans.extend(vec![
        Span::styled("r", Style::default().fg(app.color_scheme.help_key_fg)),
        Span::styled(
            " Refresh | ",
            Style::default().fg(app.color_scheme.help_text_fg),
        ),
        Span::styled("q", Style::default().fg(app.color_scheme.help_key_fg)),
        Span::styled(" Quit", Style::default().fg(app.color_scheme.help_text_fg)),
    ]);

    let help_text = Line::from(spans);
    let help =
        Paragraph::new(help_text).block(Block::default().borders(Borders::ALL).title("Help"));

    f.render_widget(help, area);
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
