# rustwav

Rust-based music downloader and library manager with a terminal UI. Downloads and organizes music from Spotify playlists/albums via YouTube with full metadata, album art, and ID3v2.3 tagging.

## Features

- **Terminal UI (TUI)** - Beautiful terminal interface built with Ratatui
- **Spotify integration** - Download albums and playlists from Spotify links
- **Download queue** - Visual queue management with progress bars
- **Library browser** - Browse by artist, album, playlist
- **Real-time progress** - Watch downloads as they happen
- **Portable mode** - Optimized output for constrained devices

## Installation

```bash
git clone https://github.com/khanCurtis/rustwav.git
cd rustwav
cargo build --release
```

## Requirements

- Rust 1.70+
- `yt-dlp` in PATH
- Spotify API credentials:
  ```bash
  export RSPOTIFY_CLIENT_ID="your_client_id"
  export RSPOTIFY_CLIENT_SECRET="your_client_secret"
  ```

## Usage

```bash
# Launch the TUI
rustwav

# Or use CLI mode directly
rustwav album <spotify-album-link>
rustwav playlist <spotify-playlist-link>
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `q` | Quit |
| `Tab` | Switch panels |
| `Enter` | Select / Confirm |
| `a` | Add album |
| `p` | Add playlist |
| `↑/↓` | Navigate |

## Architecture

```
┌─────────────────────────────────────┐
│           Dashboard UI              │
│        (Ratatui TUI)                │
├─────────────────────────────────────┤
│         rustwav-core                │
│   (Library - no CLI, no I/O)        │
├─────────────────────────────────────┤
│    Spotify API  │  YouTube (yt-dlp) │
└─────────────────────────────────────┘
```

## Branch Structure

| Branch | Purpose |
|--------|---------|
| `master` | TUI Dashboard (this branch) |
| `cli` | Command-line interface only |
| `headless` | Automation / server mode |
