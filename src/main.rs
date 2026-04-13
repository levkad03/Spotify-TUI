mod app;
mod auth;
mod model;
mod spotify;
use app::App;
use auth::{build_auth_url, load_env, wait_for_code};
use spotify::{get_current_track, get_token};

#[tokio::main]
async fn main() {
    let (client_id, client_secret) = load_env();

    let url = build_auth_url(&client_id);
    println!("Opening browser for Spotify login...");
    open::that(url).unwrap();

    let code = wait_for_code();
    let token = get_token(&client_id, &client_secret, &code).await;

    println!("Got token!");

    let mut app = App::new();
    loop {
        if let Some(track) = get_current_track(&token).await {
            app.track = track;
        }
        println!("Current track: {}", app.track);

        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}
