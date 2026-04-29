use crate::model::{ControlCommand, NowPlaying};
use crossterm::{
    event::{self, Event as CEvent, KeyCode, KeyEvent, KeyEventKind},
    execute, terminal,
};
use rand::{Rng, RngExt};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Bar, BarChart, BarGroup, Block, Borders, Gauge, List, ListItem, ListState, Paragraph, Wrap,
    },
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

    let mut queue_state = ListState::default();

    loop {
        // 1. Sync state: resize bars based on terminal width + check for new spotify data
        let _ = sync_bars(&terminal, &mut bar_states);

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

            let default_color = (30, 215, 96); // Spotify Green
            let theme_color = current
                .as_ref()
                .map(|n| n.theme_color)
                .unwrap_or(default_color);

            render_outer_frame(f, area, theme_color);

            // Create layout inside the border
            let inner_area = Block::default().borders(Borders::ALL).inner(area);

            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(0), Constraint::Length(40)])
                .split(inner_area);

            let left_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(5), // Track info
                    Constraint::Length(3), // Progress
                    Constraint::Min(5),    // Visualizer
                    Constraint::Length(1), // Help
                ])
                .split(main_chunks[0]);

            if let Some(now) = &current {
                render_track_info(f, left_chunks[0], now);
                render_progress_gauge(f, left_chunks[1], now);
                render_visualizer(f, left_chunks[2], &bar_states, now);

                render_queue(f, main_chunks[1], now, &mut queue_state);
            } else {
                render_empty_state(f, left_chunks[0]);
            }

            render_help_bar(f, left_chunks[3]);
        })?;

        // 4. Handle input: check for key presses
        if event::poll(Duration::from_millis(50))? {
            if let CEvent::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    // Pass the queue state and length to handle scrolling

                    let queue_len = current.as_ref().map(|n| n.queue.len()).unwrap_or(0);
                    if handle_input(key, &control_tx, &mut queue_state, queue_len) {
                        break;
                    }
                }
            }
        }
    }

    cleanup_terminal(&mut terminal)?;

    Ok(())
}

fn render_outer_frame(f: &mut Frame, area: Rect, theme_rgb: (u8, u8, u8)) {
    let theme_color = Color::Rgb(theme_rgb.0, theme_rgb.1, theme_rgb.2);
    let block = Block::default()
        .title(Span::styled(
            " Spotify TUI ",
            Style::default()
                .fg(theme_color)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme_color));

    f.render_widget(block, area);
}

fn render_track_info(f: &mut Frame, area: Rect, now: &NowPlaying) {
    let (r, g, b) = now.theme_color;
    let theme_color = Color::Rgb(r, g, b);
    let play_icon = if now.is_playing { "▶  " } else { "⏸  " };
    let text = Text::from(vec![
        Line::from(vec![
            Span::raw(play_icon),
            Span::styled(
                &now.title,
                Style::default()
                    .fg(theme_color)
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
    let (r, g, b) = now.theme_color;
    let theme_color = Color::Rgb(r, g, b);
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
        .gauge_style(Style::default().fg(theme_color).bg(Color::Black))
        .ratio(ratio.clamp(0.0, 1.0))
        .label(label);

    f.render_widget(gauge, area);
}

fn render_visualizer(f: &mut Frame, area: Rect, bar_states: &[f64], now: &NowPlaying) {
    let (r_base, g_base, b_base) = now.theme_color;
    let bar_data: Vec<Bar> = bar_states
        .iter()
        .enumerate()
        .map(|(i, &h)| {
            let intensity = 0.6 + (0.4 * (i as f32 / bar_states.len().max(1) as f32));
            let color = Color::Rgb(
                (r_base as f32 * intensity) as u8,
                (g_base as f32 * intensity) as u8,
                (b_base as f32 * intensity) as u8,
            );
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

fn render_queue(f: &mut Frame, area: Rect, now: &NowPlaying, state: &mut ListState) {
    let (r, g, b) = now.theme_color;
    let theme_color = Color::Rgb(r, g, b);

    let items: Vec<ListItem> = now
        .queue
        .iter()
        .map(|t| {
            ListItem::new(vec![
                Line::from(Span::styled(
                    &t.title,
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    t.artists.join(", "),
                    Style::default().fg(Color::Cyan),
                )),
            ])
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Next Up ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .highlight_style(Style::default().bg(theme_color).fg(Color::Black))
        .highlight_symbol(">> ");

    f.render_stateful_widget(list, area, state);
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

fn handle_input(
    key: KeyEvent,
    control_tx: &Sender<ControlCommand>,
    queue_state: &mut ListState,
    queue_len: usize,
) -> bool {
    match key.code {
        KeyCode::Char('q') => {
            let _ = control_tx.try_send(ControlCommand::Quit);
            return true;
        }
        KeyCode::Down => {
            let i = match queue_state.selected() {
                Some(i) => {
                    if i >= queue_len.saturating_sub(1) {
                        0
                    } else {
                        i + 1
                    }
                }
                None => 0,
            };
            queue_state.select(Some(i));
        }
        KeyCode::Up => {
            let i = match queue_state.selected() {
                Some(i) => {
                    if i == 0 {
                        queue_len.saturating_sub(1)
                    } else {
                        i - 1
                    }
                }
                None => 0,
            };
            queue_state.select(Some(i));
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
