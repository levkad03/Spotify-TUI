mod app;
mod auth;
mod model;
mod poller;
mod spotify;
mod ui;
use auth::{build_auth_url, load_env, wait_for_code};
use spotify::get_token;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (client_id, client_secret) = load_env();
    let url = build_auth_url(&client_id);
    println!("Opening browser for Spotify login...");
    open::that(url).ok();

    let code = wait_for_code();
    let token = get_token(&client_id, &client_secret, &code).await;
    println!("Got token!");

    let client = reqwest::Client::new();
    let (now_playing_tx, now_playing_rx) = mpsc::channel(8);
    let (control_tx, mut control_rx) = mpsc::channel(4);

    let poller_client = client.clone();
    let poller_token = token.clone();

    tokio::spawn(async move {
        poller::spotify_poller(
            now_playing_tx,
            poller_client,
            move || Some(poller_token.clone()),
            5, // poll interval in seconds
        )
        .await;
    });

    // For now, just logs commands; implement Spotify controls later
    let control_client = client.clone();
    tokio::spawn(async move {
        while let Some(cmd) = control_rx.recv().await {
            match cmd {
                model::ControlCommand::PlayPause => {
                    eprintln!("→ Play/Pause (not yet implemented)");
                    // TODO: call spotify::play_pause(&control_client, &token).await
                }
                model::ControlCommand::Next => {
                    eprintln!("→ Skip Next (not yet implemented)");
                    // TODO: call spotify::next_track(&control_client, &token).await
                }
                model::ControlCommand::Prev => {
                    eprintln!("→ Previous (not yet implemented)");
                    // TODO: call spotify::prev_track(&control_client, &token).await
                }
                model::ControlCommand::Quit => {
                    println!("Quitting...");
                    break;
                }
            }
        }
    });

    println!("Starting UI... Press 'q' to quit");
    ui::run_ui(now_playing_rx, control_tx)?;

    println!("✓ Goodbye!");
    Ok(())
}
