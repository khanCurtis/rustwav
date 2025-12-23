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
- `ffmpeg` in PATH (for audio extraction)
- Spotify API credentials (see setup below)

## Setup

### 1. Install yt-dlp and ffmpeg

**Arch Linux:**
```bash
sudo pacman -S yt-dlp ffmpeg
```

**Debian/Ubuntu:**
```bash
sudo apt install yt-dlp ffmpeg
```

**macOS:**
```bash
brew install yt-dlp ffmpeg
```

### 2. Get Spotify API Credentials

1. Go to [Spotify Developer Dashboard](https://developer.spotify.com/dashboard)
2. Log in and click **Create App**
3. Fill in any name/description, set Redirect URI to `https://127.0.0.1:8080`
4. Copy your **Client ID** and **Client Secret**

### 3. Configure Credentials

Create a `.env` file in the project root (recommended - keeps secrets out of your shell config):

```bash
cp .env.example .env
```

Then edit `.env` with your credentials:
```
RSPOTIFY_CLIENT_ID=your_client_id_here
RSPOTIFY_CLIENT_SECRET=your_client_secret_here
```

Alternatively, export as environment variables:
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
