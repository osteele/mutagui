use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

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
    draw_sessions(f, app, chunks[1]);
    draw_status(f, app, chunks[2]);
    draw_help(f, app, chunks[3]);
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

            let alpha_stats = session.alpha.stats_display();
            let beta_stats = session.beta.stats_display();

            let mut spans = vec![
                Span::styled(
                    format!("{} ", status_icon),
                    Style::default().fg(status_color),
                ),
                Span::styled(
                    format!("{:<18}", session.name),
                    Style::default().fg(app.color_scheme.session_name_fg).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    session.alpha.status_icon(),
                    Style::default().fg(if session.alpha.connected {
                        app.color_scheme.status_running_fg
                    } else {
                        app.color_scheme.status_paused_fg
                    }),
                ),
                Span::styled(
                    format!("{:<25}", session.alpha_display()),
                    Style::default().fg(app.color_scheme.session_alpha_fg),
                ),
            ];

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

            spans.extend(vec![
                Span::raw("• "),
                Span::styled(
                    &session.status,
                    Style::default().fg(app.color_scheme.session_status_fg),
                ),
            ]);

            let content = Line::from(spans);

            let style = if i == app.selected_index {
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
    let help_text = Line::from(vec![
        Span::styled("↑/↓", Style::default().fg(app.color_scheme.help_key_fg)),
        Span::styled(" Navigate | ", Style::default().fg(app.color_scheme.help_text_fg)),
        Span::styled("r", Style::default().fg(app.color_scheme.help_key_fg)),
        Span::styled(" Refresh | ", Style::default().fg(app.color_scheme.help_text_fg)),
        Span::styled("p", Style::default().fg(app.color_scheme.help_key_fg)),
        Span::styled(" Pause | ", Style::default().fg(app.color_scheme.help_text_fg)),
        Span::styled("u", Style::default().fg(app.color_scheme.help_key_fg)),
        Span::styled(" Resume | ", Style::default().fg(app.color_scheme.help_text_fg)),
        Span::styled("f", Style::default().fg(app.color_scheme.help_key_fg)),
        Span::styled(" Flush | ", Style::default().fg(app.color_scheme.help_text_fg)),
        Span::styled("t", Style::default().fg(app.color_scheme.help_key_fg)),
        Span::styled(" Terminate | ", Style::default().fg(app.color_scheme.help_text_fg)),
        Span::styled("q", Style::default().fg(app.color_scheme.help_key_fg)),
        Span::styled(" Quit", Style::default().fg(app.color_scheme.help_text_fg)),
    ]);

    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Help"));

    f.render_widget(help, area);
}
