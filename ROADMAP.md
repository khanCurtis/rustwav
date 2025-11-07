# Rustwav 🚀 Roadmap

**Rustwav** is a Rust-based music downloader and library manager designed for personal use.  
It rebuilds and organizes music from Spotify, YouTube, and local sources, with full metadata, playlists, and optional high-quality audio.  

---

## Table of Contents
1. [Phases](#phases)
2. [Branching Strategy](#branching-strategy)
3. [Tech Stack](#tech-stack)
4. [Future Ideas](#future-ideas)
5. [Notes](#notes)

---

## Phases

### Phase 1 — Core Features
- Album downloader from Spotify links using YouTube audio.
- Playlist creation with `.m3u` files (relative paths).
- Metadata tagging (artist, album, track number, cover art).
- Duplicate prevention via `downloaded_songs.db` or JSON caching.
- Structured library layout: `/Music/{Artist}/{Album}/`.
- Minimal logging for duplicate checking.

---

### Phase 2 — Advanced Features
- CLI flags: `--skip-existing`, `--force-redownload`, `--dry-run`.
- Parallel downloads with `rayon` or async (`tokio`).
- Playlist sync mode: `--sync` and `--clean`.
- Discography mode: download all albums, singles, and EPs from an artist.
- Audio quality & format control:
  - `--format [mp3|flac|wav|opus|m4a]`
  - `--quality [low|medium|high|lossless]`
  - `--require-quality <level>` to skip low-quality tracks.
  - Configurable fallback: skip, transcode, or warn.
- Automatic transcoding using `ffmpeg`.
- Custom `config.toml` for defaults (paths, threads, quality, API keys).
- SQLite caching (scalable, faster than JSON).
- Modular backends: Spotify, YouTube, Local (expandable for SoundCloud/Bandcamp).

---

### Phase 3 — Web & Playlist Expansion
- Web dashboard (Tauri or Leptos) for library management.
- Integrated search and queueing of Spotify/YouTube content.
- YouTube playlist downloader for unreleased or private tracks.
- Cross-source linking: Spotify metadata + YouTube audio.

---

### Phase 4 — Performance & UX Enhancements
- SQLite / Redis caching for faster lookups.
- File integrity check (`--verify`) for metadata and audio.
- File naming templates.
- Error recovery and resume for interrupted downloads.
- Verbose / silent console modes.
- Configurable storage paths for libraries or SD cards.
- Version checking for new releases.
- Audio normalization: optional `--normalize` for consistent loudness.
- Portable mode: self-contained, runs from USB or SD card.

---

### Phase 5 — Automation & APIs
- Local REST / GraphQL API for external scripts:
  - `GET /library` → all artists/albums/tracks
  - `POST /download` → queue Spotify/YouTube link
  - `GET /status` → current download state
  - `POST /playlist` → create or update playlists
  - `GET /config` / `POST /config` → read/update settings
- Headless / server mode: Raspberry Pi or minimal server sync.
- File watcher daemon for automatic metadata and playlist updates.
- Optional cloud sync mode for backup.

---

## Branching Strategy
| Branch | Purpose |
|--------|---------|
| `main` | Stable release branch. |
| `dev` | Active development branch. |
| `dashboard` | Web UI and REST/GraphQL API. |
| `headless` | Headless/server build for Raspberry Pi or servers. |
| `portable` | Self-contained USB/SD builds with relative paths and local DB. |

💡 **Tip:** Use Cargo features to toggle modes:
```toml
[features]
default = ["desktop"]
desktop = ["tauri", "web"]
headless = ["axum", "sqlite"]
portable = []
```
Build example:
```
cargo build --release --features headless
cargo build --release --features portable
```

Tech Stack

    Language: Rust

    Async Runtime: tokio

    CLI Parsing: clap

    Networking & APIs: reqwest, serde_json

    Metadata: id3, ffmpeg

    Parallelism: rayon

    Web Backend: axum or actix-web

    Web UI: leptos or tauri

    Database: rusqlite or redis

    Downloader: yt-dlp
