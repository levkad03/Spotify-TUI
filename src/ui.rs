use crate::model::{ControlCommand, NowPlaying};
use crossterm::{
    event::{self, Event as CEvent, KeyCode},
    execute, terminal,
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Text,
    widgets::{Block, Gauge, Paragraph, Wrap},
};
use std::io;
use std::time::Duration;
use tokio::sync::mpsc::{Receiver, Sender};

/// Run the blocking UI. Intended to run on the main thread.
/// `rx` receives `NowPlaying` updates from the poller.
/// `control_tx` is used to send playback commands back to a control worker.
pub fn run_ui(
    mut rx: Receiver<NowPlaying>,
    mut control_tx: Sender<ControlCommand>,
) -> anyhow::Result<()> {
    // terminal init
    let mut stdout = io::stdout();
    terminal::enable_raw_mode()?;
    execute!(stdout, terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut current: Option<NowPlaying> = None;

    loop {
        // Drain incoming updates (non-blocking)
        match rx.try_recv() {
            Ok(now) => current = Some(now),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {}
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                // Poller hung up; exit UI
                terminal.draw(|f| {
                    let block = Block::default()
                        .title("Disconnected")
                        .borders(ratatui::widgets::Borders::ALL);
                    f.render_widget(block, f.size());
                })?;
                cleanup_terminal(&mut terminal)?;
                return Ok(());
            }
        }

        // Draw
        terminal.draw(|f| {
            let size = f.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints(
                    [
                        Constraint::Length(3),
                        Constraint::Length(3),
                        Constraint::Min(3),
                    ]
                    .as_ref(),
                )
                .split(size);

            let header = Block::default()
                .title("Spotify TUI")
                .borders(ratatui::widgets::Borders::ALL);

            f.render_widget(header, chunks[0]);

            if let Some(now) = &current {
                let title = Paragraph::new(Text::from(format!(
                    "{} — {}",
                    now.title,
                    now.artists.join(", ")
                )))
                .wrap(Wrap { trim: true });
                f.render_widget(title, chunks[1]);

                let ratio = if now.duration_ms > 0 {
                    now.elapsed_progress() as f64 / now.duration_ms as f64
                } else {
                    0.0
                };
                let gauge = Gauge::default()
                    .block(
                        Block::default()
                            .title(format!("{} • {}", now.album, fmt_ms(now.duration_ms)))
                            .borders(ratatui::widgets::Borders::ALL),
                    )
                    .gauge_style(Style::default().fg(Color::Green))
                    .ratio(ratio);
                f.render_widget(gauge, chunks[2]);
            } else {
                let empty = Paragraph::new("No track data yet")
                    .block(Block::default().borders(ratatui::widgets::Borders::ALL));
                f.render_widget(empty, chunks[1]);
            }
        })?;

        // Input handling with a small timeout so the UI remains responsive
        if event::poll(Duration::from_millis(200))? {
            if let CEvent::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => {
                        let _ = control_tx.try_send(ControlCommand::Quit);
                        break;
                    }
                    KeyCode::Char(' ') => {
                        let _ = control_tx.try_send(ControlCommand::PlayPause);
                    }
                    KeyCode::Char('n') => {
                        let _ = control_tx.try_send(ControlCommand::Next);
                    }
                    KeyCode::Char('p') => {
                        let _ = control_tx.try_send(ControlCommand::Prev);
                    }

                    _ => {}
                }
            }
        }
    }

    cleanup_terminal(&mut terminal)?;
    Ok(())
}

fn cleanup_terminal(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) -> anyhow::Result<()> {
    terminal.show_cursor()?;
    execute!(terminal.backend_mut(), terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    Ok(())
}

fn fmt_ms(ms: u64) -> String {
    let secs = ms / 1000;
    let m = secs / 60;
    let s = secs % 60;
    format!("{:02}:{:02}", m, s)
}
