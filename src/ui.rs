use crate::app::App;
use crate::selection::SelectableItem;
use crate::project::SyncSpecState;
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
                        let spans = render_spec_row(app, spec);

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

    let title = format!(" Sync Projects ({} projects, {} specs) ", app.projects.len(), total_specs);
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

    // Count conflicts across all running specs
    let conflict_count: usize = project.specs.iter()
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

    // Add running status
    if running_count == 0 {
        spans.push(Span::styled(
            "  Not running".to_string(),
            Style::default().fg(theme.session_status_fg),
        ));
    } else if running_count == total_count {
        spans.push(Span::styled(
            "  Running".to_string(),
            Style::default().fg(theme.session_status_fg),
        ));
    } else {
        spans.push(Span::styled(
            format!("  {}/{} running", running_count, total_count),
            Style::default().fg(theme.session_status_fg),
        ));
    }

    // Add conflict indicator if there are conflicts
    if conflict_count > 0 {
        spans.push(Span::raw("  ".to_string()));
        spans.push(Span::styled(
            format!("⚠ {} conflict{}", conflict_count, if conflict_count == 1 { "" } else { "s" }),
            Style::default()
                .fg(theme.status_paused_fg)
                .add_modifier(Modifier::BOLD),
        ));
    }

    spans
}

/// Render a spec row with state indicator and details
fn render_spec_row(app: &App, spec: &crate::project::SyncSpec) -> Vec<Span<'static>> {
    let theme = &app.color_scheme;

    let mut spans = vec![Span::raw("    ".to_string())]; // Indent for spec under project

    match &spec.state {
        SyncSpecState::NotRunning => {
            // Not running: show ○ icon and "Not running" status
            spans.push(Span::styled(
                "○ ".to_string(),
                Style::default().fg(theme.status_paused_fg),
            ));
            spans.push(Span::styled(
                format!("{:<30}", spec.name),
                Style::default().fg(theme.session_name_fg),
            ));
            spans.push(Span::styled(
                "  Not running".to_string(),
                Style::default().fg(theme.session_status_fg),
            ));
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

                spans.push(Span::styled(
                    format!("{:<30}", spec.name),
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

                // Arrow (⇄ for two-way, ⬆ for push)
                let arrow = if spec.state == SyncSpecState::RunningPush {
                    "⬆ "
                } else {
                    "⇄ "
                };
                spans.push(Span::raw(arrow.to_string()));

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
                            if session.conflict_count() == 1 { "" } else { "s" }
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
                    let mut parts = vec![session.name.clone(), ": ".to_string(), session.status_text().to_string()];

                    // Add progress percentage if available
                    if let Some(pct) = session.progress_percentage() {
                        parts.push(format!(" ({}%)", pct));
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
                    // Spec not running
                    (format!("{}: Not running", spec.name), app.color_scheme.status_message_fg)
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
    } else {
        // No spec selected - show status message
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
        .item("h/l", "Fold")
        .item("r", "Refresh");

    if is_project_selected {
        // Project-specific commands
        help_bar = help_bar
            .item("↵", "Edit")
            .item("s", "Start/Stop")
            .item("p", "Push")
            .item("Space", "Pause/Resume");
    } else if is_spec_selected {
        // Spec-specific commands
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

    if let Some(conflicts) = app.get_selected_spec_conflicts() {
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
