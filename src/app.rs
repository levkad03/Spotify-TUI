pub struct App {
    pub track: String,
    pub bars: Vec<u64>,
}

impl App {
    pub fn new() -> Self {
        Self {
            track: "Nothing playing".to_string(),
            bars: vec![0; 10],
        }
    }
}
