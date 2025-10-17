mod app;
mod mutagen;
mod project;
mod theme;
mod ui;

use anyhow::Result;
use app::App;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "mutagen-tui")]
#[command(about = "Terminal UI for managing Mutagen sync sessions", long_about = None)]
struct Cli {
    /// Directory to search for mutagen project files (default: current directory)
    #[arg(short = 'd', long, value_name = "DIR")]
    project_dir: Option<PathBuf>,
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
                    match key.code {
                        KeyCode::Char('q') => {
                            app.quit();
                        }
                        KeyCode::Tab => {
                            app.toggle_view();
                        }
                        KeyCode::Char('r') => {
                            app.refresh_sessions().await?;
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.select_previous();
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.select_next();
                        }
                        KeyCode::Char('p') => {
                            app.pause_selected();
                            app.refresh_sessions().await?;
                        }
                        KeyCode::Char('u') => {
                            app.resume_selected();
                            app.refresh_sessions().await?;
                        }
                        KeyCode::Char('t') => {
                            app.terminate_selected();
                        }
                        KeyCode::Char('f') => {
                            app.flush_selected();
                            app.refresh_sessions().await?;
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
