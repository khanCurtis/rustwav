# rustwav

Rust-based music downloader and library manager. Downloads and organizes music from Spotify playlists/albums via YouTube with full metadata, album art, and ID3v2.3 tagging.

## Branch: `portable`

This branch targets **constrained devices** like Nintendo 3DS, car stereos, old MP3 players, and FAT32 storage.

### Portable Mode Features

When running with `--portable` or `-p`:

- **MP3 only** - Forces MP3 format regardless of `--format` flag
- **FAT32-safe filenames** - Alphanumeric, underscores, hyphens only (no spaces or special chars)
- **Shallow folders** - No nested `Artist/Album` structure, all files in base directory
- **Small cover art** - 128x128 max, 64KB limit for embedded album art
- **Short filenames** - 64 character max to avoid path length issues

### Usage

```bash
# Normal mode (desktop)
rustwav album <spotify-link>
rustwav playlist <spotify-link>

# Portable mode (3DS, car stereo, etc.)
rustwav --portable album <spotify-link>
rustwav -p playlist <spotify-link>
```

### Output Structure

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

## Requirements

- Rust 1.70+
- `yt-dlp` in PATH
- Spotify API credentials (`RSPOTIFY_CLIENT_ID`, `RSPOTIFY_CLIENT_SECRET`)

## Branch Structure

| Branch | Purpose |
|--------|---------|
| `master` | Stable, release-ready core |
| `portable` | Constrained devices (this branch) |
| `headless` | Automation / server mode |
| `dashboard` | Future UI layer |
| `dev` | Active development |
