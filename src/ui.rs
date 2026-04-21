use crate::model::{ControlCommand, NowPlaying};
use crossterm::{
    event::{self, Event as CEvent, KeyCode},
    execute, terminal,
};
use rand::RngExt;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Bar, BarChart, BarGroup, Block, Borders, Gauge, Paragraph, Wrap},
};
use std::io;
use std::time::Duration;
use tokio::sync::mpsc::{Receiver, Sender};

/// Run the blocking UI. Intended to run on the main thread.
/// `rx` receives `NowPlaying` updates from the poller.
/// `control_tx` is used to send playback commands back to a control worker.
pub fn run_ui(
    mut rx: Receiver<NowPlaying>,
    control_tx: Sender<ControlCommand>,
) -> anyhow::Result<()> {
    // terminal init
    let mut stdout = io::stdout();
    terminal::enable_raw_mode()?;
    execute!(stdout, terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut current: Option<NowPlaying> = None;

    let mut bar_states = vec![0.0f64; 40]; // Adjust number of bars
    let mut rng = rand::rng();

    loop {
        // Recalculate bar count
        // Get current terminal width to see how many bars we can fit
        // Each bar is 2 wide + 1 gap = 3 columns per bar
        let term_size = terminal.size()?;
        let available_width = term_size.width.saturating_sub(4); // Subtract padding/borders
        let ideal_bar_count = (available_width / 3) as usize;

        // Resize the vector dynamically.
        // .resize() keeps existing values and adds 0s if expanding.
        if bar_states.len() != ideal_bar_count {
            bar_states.resize(ideal_bar_count, 0.0f64);
        }

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
                    f.render_widget(block, f.area());
                })?;
                cleanup_terminal(&mut terminal)?;
                return Ok(());
            }
        }

        // Update visualizer heights
        if let Some(now) = &current {
            for i in 0..bar_states.len() {
                if now.is_playing {
                    // Generate a random impulse
                    let impulse = rng.random_range(0.0..15.0);

                    // Fast rise: if the impulse is higher than the bar, jump to it
                    if impulse > bar_states[i] {
                        bar_states[i] = impulse;
                    } else {
                        // Slow fall: otherwise, let the bar float down
                        // Multiply by a factor < 1.0
                        bar_states[i] *= 0.9;
                    }
                } else {
                    // Fade to zero when paused
                    bar_states[i] *= 0.8;
                }
                // Ensure it doesn't stay at micro-values forever
                if bar_states[i] < 0.1 {
                    bar_states[i] = 0.0;
                }
            }
        }

        // Draw
        terminal.draw(|f| {
            let size = f.area();

            // 1. Render an outer border that wraps everything
            let outer_block = Block::default()
                .title(Span::styled(
                    " Spotify TUI ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green));

            // 2. Get the area *inside* the outer border
            let inner_area = outer_block.inner(size);

            // 3. Render the outer block first (just the border)
            f.render_widget(outer_block, size);

            // 4. Now split the inner area for your content (no extra margin needed)
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1) // small padding inside the outer border
                .constraints(
                    [
                        Constraint::Length(5), // track info
                        Constraint::Length(3), // progress gauge
                        Constraint::Min(5),    // Visualizer (Dynamic Space)
                        Constraint::Length(1), // help bar
                    ]
                    .as_ref(),
                )
                .split(inner_area);

            if let Some(now) = &current {
                // Track info block
                let play_icon = if now.is_playing { "▶  " } else { "⏸  " };
                let track_block = Paragraph::new(Text::from(vec![
                    Line::from(vec![
                        Span::raw(play_icon),
                        Span::styled(
                            now.title.clone(),
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(vec![
                        Span::raw("    "),
                        Span::styled(now.artists.join(", "), Style::default().fg(Color::Cyan)),
                    ]),
                ]))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::DarkGray)),
                )
                .wrap(Wrap { trim: true });
                f.render_widget(track_block, chunks[0]);

                // Progress gauge
                let elapsed = now.elapsed_progress();
                let ratio = if now.duration_ms > 0 {
                    elapsed as f64 / now.duration_ms as f64
                } else {
                    0.0
                };
                let label = format!("{} / {}", fmt_ms(elapsed), fmt_ms(now.duration_ms));
                let gauge = Gauge::default()
                    .block(
                        Block::default()
                            .title(Span::styled(
                                format!(" {} ", now.album),
                                Style::default().fg(Color::DarkGray),
                            ))
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::DarkGray)),
                    )
                    .gauge_style(Style::default().fg(Color::Green).bg(Color::Black))
                    .ratio(ratio.clamp(0.0, 1.0))
                    .label(label);
                f.render_widget(gauge, chunks[1]);
            } else {
                let empty = Paragraph::new(Span::styled(
                    "No track playing",
                    Style::default().fg(Color::DarkGray),
                ))
                .block(Block::default().borders(Borders::ALL))
                .alignment(Alignment::Center);
                f.render_widget(empty, chunks[0]);
            }

            // Visualizer
            let bar_data: Vec<Bar> = bar_states
                .iter()
                .enumerate()
                .map(|(i, &h)| {
                    let r = (i * 255 / bar_states.len()) as u8;
                    let g = 255 - r;
                    let color = Color::Rgb(r, g, 150);
                    Bar::new(h.round() as u64).style(Style::default().fg(color))
                })
                .collect();

            let group = BarGroup::default().bars(&bar_data);

            let visualizer = BarChart::default()
                .block(
                    Block::default()
                        .title(" Visualizer")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::DarkGray)),
                )
                .data(group)
                .bar_width(2)
                .bar_gap(1);

            f.render_widget(visualizer, chunks[2]);

            // Help bar
            let help = Paragraph::new(Span::styled(
                "[space] play/pause   [n] next   [p] prev   [q] quit",
                Style::default().fg(Color::DarkGray),
            ))
            .alignment(Alignment::Center);
            f.render_widget(help, chunks[3]);
        })?;

        // Input handling with a small timeout so the UI remains responsive
        if event::poll(Duration::from_millis(50))? {
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
