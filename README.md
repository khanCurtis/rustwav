# rustwav

Rust-based music downloader and library manager. Downloads and organizes music from Spotify playlists/albums via YouTube with full metadata, album art, and ID3v2.3 tagging.

## Branch: `dashboard`

This branch is for the **future UI/visual layer** - a web or TUI frontend for managing your music library.

### Dashboard Goals

- **Web or TUI frontend** - Visual interface for library management
- **Consumes core as library** - No duplication of downloader logic
- **Progress visualization** - Real-time download progress
- **Library browser** - Browse, search, and manage downloaded music
- **Stats and analytics** - Download history, library statistics

### Planned Features

- **Download queue** - Visual queue management
- **Library view** - Browse by artist, album, playlist
- **Search** - Find tracks in your library
- **Progress bars** - Real-time download progress
- **History** - View past downloads and errors

### Architecture

```
┌─────────────────────────────────────┐
│           Dashboard UI              │
│      (Web / TUI Frontend)           │
├─────────────────────────────────────┤
│         rustwav-core                │
│   (Library - no CLI, no I/O)        │
├─────────────────────────────────────┤
│    Spotify API  │  YouTube (yt-dlp) │
└─────────────────────────────────────┘
```

### Technology Options

| Option | Pros | Cons |
|--------|------|------|
| **Ratatui (TUI)** | No browser needed, terminal native | Limited styling |
| **Leptos (Web)** | Rich UI, Rust WASM | Requires browser |
| **Tauri** | Desktop app, native feel | Larger binary |

## Requirements

- Rust 1.70+
- `yt-dlp` in PATH
- Spotify API credentials (`RSPOTIFY_CLIENT_ID`, `RSPOTIFY_CLIENT_SECRET`)

## Branch Structure

| Branch | Purpose |
|--------|---------|
| `master` | Stable, release-ready core |
| `portable` | Constrained devices (3DS, car stereos) |
| `headless` | Automation / server mode |
| `dashboard` | Future UI layer (this branch) |
| `dev` | Active development |
