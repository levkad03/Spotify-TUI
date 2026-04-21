use crate::model::NowPlaying;
use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

pub async fn get_token(client_id: &str, client_secret: &str, code: &str) -> String {
    let client = Client::new();

    let res = client
        .post("https://accounts.spotify.com/api/token")
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", "http://127.0.0.1:8888/callback"),
            ("client_id", client_id),
            ("client_secret", client_secret),
        ])
        .send()
        .await
        .unwrap();

    let json: TokenResponse = res.json().await.unwrap();

    json.access_token
}

pub async fn get_current_track(
    client: &reqwest::Client,
    token: &str,
) -> Result<NowPlaying, Box<dyn std::error::Error>> {
    let res = client
        .get("https://api.spotify.com/v1/me/player/currently-playing")
        .bearer_auth(token)
        .send()
        .await?;

    if res.status() == 204 {
        // 204 No Content = nothing playing
        return Ok(NowPlaying {
            title: "No track playing".into(),
            artists: vec![],
            album: "".into(),
            progress_ms: 0,
            duration_ms: 0,
            is_playing: false,
            album_art_url: None,
            fetched_at: std::time::Instant::now(),
            theme_color: (30, 215, 96), // Default spotify green
        });
    }

    let json: serde_json::Value = res.json().await?;

    let is_playing = json["is_playing"].as_bool().unwrap_or(false);
    let progress_ms = json["progress_ms"].as_u64().unwrap_or(0);

    let item = &json["item"];
    let title = item["name"].as_str().unwrap_or("Unknown").to_string();

    let artists: Vec<String> = item["artists"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a["name"].as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let album = item["album"]["name"]
        .as_str()
        .unwrap_or("Unknown")
        .to_string();
    let duration_ms = item["duration_ms"].as_u64().unwrap_or(0);

    let album_art_url = item["album"]["images"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|img| img["url"].as_str())
        .map(|s| s.to_string());

    Ok(NowPlaying {
        title,
        artists,
        album,
        progress_ms,
        duration_ms,
        is_playing,
        album_art_url,
        fetched_at: std::time::Instant::now(),
        theme_color: (30, 215, 96), // Default spotify green
    })
}

pub async fn fetch_dominant_color(url: &str) -> Option<(u8, u8, u8)> {
    let client = reqwest::Client::new();
    let bytes = client.get(url).send().await.ok()?.bytes().await.ok()?;

    let img = image::load_from_memory(&bytes).ok()?;

    // Resize to 1x1 to effectively get the average color of the whole image
    let rgb = img.thumbnail(1, 1).to_rgb8();
    let pixel = rgb.get_pixel(0, 0);

    Some((pixel[0], pixel[1], pixel[2]))
}
