use crate::model::NowPlaying;
use anyhow::Error;
use reqwest::Client;
use tokio::sync::mpsc::Sender;
use tokio::time::{self, Duration};

/// Poll Spotify's "currently playing" endpoint repeatedly and send updates to "tx".
/// Replace `fetch_currently_playing` call with your real fetcher (returns `NowPlaying`).
pub async fn spotify_poller(
    tx: Sender<NowPlaying>,
    client: Client,
    mut get_access_token: impl FnMut() -> Option<String> + Send + 'static,
    interval_secs: u64,
) {
    let mut interval = time::interval(Duration::from_secs(interval_secs));
    loop {
        interval.tick().await;

        let token = match get_access_token() {
            Some(t) => t,
            None => {
                // No token: skip until next tick
                let _ = tx
                    .send(NowPlaying {
                        title: "Not authenticated".into(),
                        artists: vec![],
                        album: "".into(),
                        progress_ms: 0,
                        duration_ms: 0,
                        is_playing: false,
                        album_art_url: None,
                        fetched_at: std::time::Instant::now(),
                    })
                    .await;
                continue;
            }
        };

        match fetch_currently_playing(&client, &token).await {
            Ok(mut now) => {
                now.fetched_at = std::time::Instant::now();
                let _ = tx.send(now).await;
            }

            Err(e) => {
                // Send a lightweight error state (optional)
                eprintln!("spotify_poller: fetch error: {}", e);
                // Optionally send a minimal NowPlaying with error message in title
                let _ = tx
                    .send(NowPlaying {
                        title: format!("Error: {}", e),
                        artists: vec![],
                        album: "".into(),
                        progress_ms: 0,
                        duration_ms: 0,
                        is_playing: false,
                        album_art_url: None,
                        fetched_at: std::time::Instant::now(),
                    })
                    .await;
            }
        }
    }
}

async fn fetch_currently_playing(client: &Client, token: &str) -> Result<NowPlaying, Error> {
    crate::spotify::get_current_track(client, token)
        .await
        .map_err(|e| anyhow::anyhow!("Spotify fetch error: {}", e))
}
