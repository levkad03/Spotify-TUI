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

    let mut last_art_url: Option<String> = None;
    let mut current_theme = (30, 215, 96);

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
                        theme_color: current_theme,
                    })
                    .await;
                continue;
            }
        };

        match fetch_currently_playing(&client, &token).await {
            Ok(mut now) => {
                now.fetched_at = std::time::Instant::now();

                // Adaptive UI color logic
                // Only fetch new color if album art URL has changed
                if now.album_art_url != last_art_url {
                    if let Some(url) = &now.album_art_url {
                        // Try to get dominant color from the new image
                        if let Some(color) = crate::spotify::fetch_dominant_color(url).await {
                            current_theme = color;
                        }
                    } else {
                        // Fallback to Spotify Green if there is no album art
                        current_theme = (30, 215, 96);
                    }
                    // Update our tracker
                    last_art_url = now.album_art_url.clone();
                }

                // Apply the theme color to  the current update
                now.theme_color = current_theme;
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
                        theme_color: current_theme,
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
