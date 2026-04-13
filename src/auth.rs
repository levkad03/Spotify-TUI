use dotenvy::dotenv;
use std::env;
use tiny_http::{Response, Server};

pub fn load_env() -> (String, String) {
    dotenv().ok();

    let client_id = env::var("CLIENT_ID").unwrap();
    let client_secret = env::var("CLIENT_SECRET").unwrap();

    (client_id, client_secret)
}

pub fn build_auth_url(client_id: &str) -> String {
    let redirect_uri = "http://127.0.0.1:8888/callback";
    let scopes = "user-read-playback-state user-read-currently-playing";

    format!(
        "https://accounts.spotify.com/authorize?response_type=code&client_id={}&scope={}&redirect_uri={}",
        client_id,
        urlencoding::encode(scopes),
        urlencoding::encode(redirect_uri)
    )
}

pub fn wait_for_code() -> String {
    let server = Server::http("0.0.0.0:8888").unwrap();

    for request in server.incoming_requests() {
        let url = request.url().to_string();

        if url.contains("code=") {
            let code = url
                .split("code=")
                .nth(1)
                .unwrap()
                .split('&')
                .next()
                .unwrap();

            let response = Response::from_string("You can close this tab.");
            let _ = request.respond(response);

            return code.to_string();
        }
    }

    panic!("No code received");
}
