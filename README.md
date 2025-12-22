# rustwav

Rust-based music downloader and library manager. Downloads and organizes music from Spotify playlists/albums via YouTube with full metadata, album art, and ID3v2.3 tagging.

## Features

- **Spotify integration** - Download albums and playlists from Spotify links
- **YouTube audio** - Sources audio from YouTube via yt-dlp
- **ID3v2.3 tagging** - Full metadata with embedded album art
- **Download cache** - Skip already-downloaded tracks
- **M3U playlists** - Auto-generated playlist files
- **Portable mode** - Optimized for constrained devices (3DS, car stereos, FAT32)

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
# Download an album
rustwav album <spotify-album-link>

# Download a playlist
rustwav playlist <spotify-playlist-link>

# Specify format (default: mp3)
rustwav album --format flac <link>
```

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
| `master` | Stable, release-ready (this branch) |
| `headless` | Automation / server mode |
| `dashboard` | Future UI layer |

Feature branches are created as needed and merged into master when complete.
