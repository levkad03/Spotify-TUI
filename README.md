# Spotify TUI (Rust)

A lightweight, terminal-based Spotify controller and visualizer built with **Rust** and **Ratatui**. This tool allows you to see what's currently playing, control playback, and enjoy a dynamic equalizer-style visualization directly in your terminal.

## ✨ Features

- **Real-time Track Info:** Displays current track title, artists, and album name.
- **Dynamic Progress Bar:** Smoothly estimates track progress between API polls.
- **Equalizer Visualizer:** A responsive "dancing bars" visualization that reacts to the playback state and terminal size.
- **Playback Controls:** Simple keyboard shortcuts to play/pause, skip, and go back.
- **OAuth2 Authentication:** Securely authenticates with your Spotify account via a local callback server.

## 🚀 Getting Started

### 1. Prerequisites
- [Rust](https://www.rust-lang.org/tools/install) (latest stable version)
- A [Spotify Developer](https://developer.spotify.com/dashboard/) account.

### 2. Spotify API Setup
1. Go to the [Spotify Developer Dashboard](https://developer.spotify.com/dashboard/).
2. Create a new App.
3. Add `http://localhost:8888/callback` to the **Redirect URIs** list.
4. Note your **Client ID** and **Client Secret**.

### 3. Installation & Configuration
Clone the repository and create a `.env` file in the root directory:

```bash
git clone https://github.com/your-username/spotify-tui.git
cd spotify-tui
```

Create a `.env` file:
```env
SPOTIFY_CLIENT_ID=your_client_id_here
SPOTIFY_CLIENT_SECRET=your_client_secret_here
```

### 4. Running the App
```bash
cargo run
```
*Your browser will open to authorize the app. Once authorized, the TUI will start automatically.*

## 🎮 Controls

| Key | Action |
| :--- | :--- |
| `Space` | Play / Pause |
| `n` | Next Track |
| `p` | Previous Track |
| `q` | Quit Application |

## 🛠 Tech Stack

- **[Ratatui](https://ratatui.rs/):** The terminal UI framework.
- **[Tokio](https://tokio.rs/):** Async runtime for non-blocking API polling.
- **[Reqwest](https://docs.rs/reqwest/):** Handles HTTP requests to the Spotify Web API.
- **[Crossterm](https://docs.rs/crossterm/):** Cross-platform terminal manipulation.

