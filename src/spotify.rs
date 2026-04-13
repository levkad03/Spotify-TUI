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

pub async fn get_current_track(token: &str) -> Option<String> {
    let client = Client::new();

    let res = client
        .get("https://api.spotify.com/v1/me/player/currently-playing")
        .bearer_auth(token)
        .send()
        .await
        .ok()?;

    let json: serde_json::Value = res.json().await.ok()?;

    Some(json["item"]["name"].as_str()?.to_string())
}
