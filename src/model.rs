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
    pub theme_color: (u8, u8, u8),
}

impl NowPlaying {
    pub fn elapsed_progress(&self) -> u64 {
        // If not playing, just return the server's progress_ms (don't advance)
        if !self.is_playing {
            return self.progress_ms;
        }

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
