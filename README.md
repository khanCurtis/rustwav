# rustwav

Rust-based music downloader and library manager. Downloads and organizes music from Spotify playlists/albums via YouTube with full metadata, album art, and ID3v2.3 tagging.

## Branch: `cli`

Command-line interface for rustwav. This branch contains the terminal-based downloader without any UI.

## Features

- **Spotify integration** - Download albums and playlists from Spotify links
- **YouTube audio** - Sources audio from YouTube via yt-dlp
- **ID3v2.3 tagging** - Full metadata with embedded album art
- **Download cache** - Skip already-downloaded tracks
- **M3U playlists** - Auto-generated playlist files
- **Portable mode** - Optimized for constrained devices (3DS, car stereos, FAT32)
- **Audio converter** - Convert between MP3, FLAC, WAV, AAC formats with metadata refresh

## Installation

```bash
git clone https://github.com/khanCurtis/rustwav.git
cd rustwav
git checkout cli
cargo build --release
```

## Requirements

- Rust 1.70+
- `yt-dlp` in PATH
- `ffmpeg` in PATH (required for audio extraction and format conversion)
- Spotify API credentials:

> **Note:** FFmpeg is required for both downloading (audio extraction) and the audio converter feature. Make sure it's installed and accessible in your PATH.
  ```bash
  export RSPOTIFY_CLIENT_ID="your_client_id"
  export RSPOTIFY_CLIENT_SECRET="your_client_secret"
  ```

## Usage

```bash
# Download an album
rustwav album <spotify-album-link>

# Download a playlist
rustwav playlist <spotify-playlist-link>

# Specify format (default: mp3)
rustwav album --format flac <link>

# Convert audio files between formats
rustwav convert -i "path/to/file.wav" -t mp3 --quality high
rustwav convert -i "path/to/directory" -t flac -r  # recursive
```

### Convert Options

| Option | Description |
|--------|-------------|
| `-i, --input` | Input file or directory |
| `-t, --to` | Target format: mp3, flac, wav, aac (default: mp3) |
| `-q, --quality` | Quality: high, medium, low (default: high) |
| `--refresh-metadata` | Refresh ID3 tags from Spotify (default: true) |
| `-r, --recursive` | Process directories recursively |

### Portable Mode

For constrained devices (Nintendo 3DS, car stereos, old MP3 players, FAT32 storage):

```bash
rustwav --portable album <link>
rustwav -p playlist <link>
```

Portable mode enforces:
- MP3 only (ignores --format)
- FAT32-safe filenames (alphanumeric, underscores, hyphens)
- Shallow folder structure (no artist/album nesting)
- Small cover art (128x128 max, 64KB limit)
- Short filenames (64 char max)

## Output Structure

**Normal mode:**
```
data/music/
  Artist Name/
    Album Name/
      Artist - Track.mp3
      cover.jpg
```

**Portable mode:**
```
data/music/
  Artist_-_Track.mp3
  cover.jpg
```

## Branch Structure

| Branch | Purpose |
|--------|---------|
| `master` | UI / Dashboard (web or TUI) |
| `cli` | Command-line interface (this branch) |
| `headless` | Automation / server mode |
