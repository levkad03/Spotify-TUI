use std::time::Instant;

#[derive(Clone, Debug)]
pub struct NowPlaying {
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    pub progress_ms: u64,
    pub duration_ms: u64,
    pub is_playing: bool,
    pub album_art_url: Option<String>,

    // when the server returned this snapshot (used to advance the progress locally)
    pub fetched_at: Instant,
}

impl NowPlaying {
    pub fn elapsed_progress(&self) -> u64 {
        let since = Instant::now().saturating_duration_since(self.fetched_at);
        self.progress_ms
            .saturating_add(since.as_millis() as u64)
            .min(self.duration_ms)
    }
}

#[derive(Debug, Clone)]
pub enum ControlCommand {
    PlayPause,
    Next,
    Prev,
    Quit,
}
