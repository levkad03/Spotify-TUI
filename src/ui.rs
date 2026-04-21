use crate::model::{ControlCommand, NowPlaying};
use crossterm::{
    event::{self, Event as CEvent, KeyCode, KeyEvent},
    execute, terminal,
};
use rand::{Rng, RngExt};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
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
    let mut terminal = init_terminal()?;
    let mut current: Option<NowPlaying> = None;
    let mut bar_states = Vec::new();
    let mut rng = rand::rng();

    loop {
        // 1. Sync state: resize bars based on terminal width + check for new spotify data
        let _ = sync_bars(&mut terminal, &mut bar_states);

        match rx.try_recv() {
            Ok(now) => current = Some(now),
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
            _ => {}
        }

        // 2. Update animation: animate the visualizer bars
        let is_playing = current.as_ref().map(|n| n.is_playing).unwrap_or(false);
        update_animation(&mut bar_states, is_playing, &mut rng);

        // 3. Draw: render all components
        terminal.draw(|f| {
            let area = f.area();
            render_outer_frame(f, area);

            // Create layout inside the border
            let inner_area = Block::default().borders(Borders::ALL).inner(area);
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(5), // Track info
                    Constraint::Length(3), // Progress
                    Constraint::Min(5),    // Visualizer
                    Constraint::Length(1), // Help
                ])
                .split(inner_area);

            if let Some(now) = &current {
                render_track_info(f, chunks[0], now);
                render_progress_gauge(f, chunks[1], now);
            } else {
                render_empty_state(f, chunks[0]);
            }

            render_visualizer(f, chunks[2], &bar_states);
            render_help_bar(f, chunks[3]);
        })?;

        // 4. Handle input: check for key presses
        if event::poll(Duration::from_millis(50))? {
            if let CEvent::Key(key) = event::read()? {
                if handle_input(key, &control_tx) {
                    break;
                }
            }
        }
    }

    cleanup_terminal(&mut terminal)?;

    Ok(())
}

fn render_outer_frame(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(Span::styled(
            " Spotify TUI ",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    f.render_widget(block, area);
}

fn render_track_info(f: &mut Frame, area: Rect, now: &NowPlaying) {
    let play_icon = if now.is_playing { "▶  " } else { "⏸  " };
    let text = Text::from(vec![
        Line::from(vec![
            Span::raw(play_icon),
            Span::styled(
                &now.title,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("    "),
            Span::styled(now.artists.join(", "), Style::default().fg(Color::Cyan)),
        ]),
    ]);

    let block = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(block, area);
}

fn render_progress_gauge(f: &mut Frame, area: Rect, now: &NowPlaying) {
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
                .title(format!(" {} ", now.album))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .gauge_style(Style::default().fg(Color::Green).bg(Color::Black))
        .ratio(ratio.clamp(0.0, 1.0))
        .label(label);

    f.render_widget(gauge, area);
}

fn render_visualizer(f: &mut Frame, area: Rect, bar_states: &[f64]) {
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

    let visualizer = BarChart::default()
        .block(
            Block::default()
                .title(" Visualizer")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .data(BarGroup::default().bars(&bar_data))
        .bar_width(2)
        .bar_gap(1);

    f.render_widget(visualizer, area);
}

fn render_empty_state(f: &mut Frame, area: Rect) {
    let empty = Paragraph::new("No track playing")
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);

    f.render_widget(empty, area);
}

fn render_help_bar(f: &mut Frame, area: Rect) {
    let help = Paragraph::new("[space] play/pause   [n] next   [p] prev   [q] quit")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(help, area);
}

fn update_animation(bar_states: &mut [f64], is_playing: bool, rng: &mut impl Rng) {
    for bar in bar_states.iter_mut() {
        if is_playing {
            let impulse = rng.random_range(0.0..15.0);
            if impulse > *bar {
                *bar = impulse;
            } else {
                *bar *= 0.9;
            }
        } else {
            *bar *= 0.8;
        }
        if *bar < 0.1 {
            *bar = 0.0;
        }
    }
}

fn handle_input(key: KeyEvent, control_tx: &Sender<ControlCommand>) -> bool {
    match key.code {
        KeyCode::Char('q') => {
            let _ = control_tx.try_send(ControlCommand::Quit);
            return true;
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
    false
}

fn sync_bars(
    terminal: &Terminal<CrosstermBackend<io::Stdout>>,
    bar_states: &mut Vec<f64>,
) -> io::Result<()> {
    let term_size = terminal.size()?;
    let available_width = term_size.width.saturating_sub(4);
    let ideal_bar_count = (available_width / 3) as usize;

    if bar_states.len() != ideal_bar_count {
        bar_states.resize(ideal_bar_count, 0.0);
    }

    Ok(())
}

fn init_terminal() -> anyhow::Result<Terminal<CrosstermBackend<std::io::Stdout>>> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
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
