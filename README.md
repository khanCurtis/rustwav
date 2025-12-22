# rustwav

Rust-based music downloader and library manager. Downloads and organizes music from Spotify playlists/albums via YouTube with full metadata, album art, and ID3v2.3 tagging.

## Branch: `headless`

This branch is for **automation and server mode** - designed for scripts, cron jobs, Docker containers, and CI/CD pipelines.

### Headless Mode Goals

- **No prompts** - Never blocks waiting for user input
- **Deterministic output** - Consistent, parseable logs
- **Script-friendly** - Exit codes, JSON output options
- **Daemon-ready** - Suitable for Docker, systemd, cron

### Planned Features

```bash
# Run with structured logging
rustwav --headless album <link>

# JSON output for parsing
rustwav --headless --json playlist <link>

# Quiet mode (errors only)
rustwav --headless --quiet album <link>
```

### Use Cases

- **Cron jobs** - Scheduled playlist syncing
- **Docker containers** - Automated music library building
- **CI/CD** - Testing and automated downloads
- **Server deployment** - Headless music management

### Output Modes

| Mode | Description |
|------|-------------|
| Default | Human-readable progress |
| `--json` | Machine-parseable JSON lines |
| `--quiet` | Errors only |
| `--verbose` | Debug-level logging |

## Requirements

- Rust 1.70+
- `yt-dlp` in PATH
- Spotify API credentials (`RSPOTIFY_CLIENT_ID`, `RSPOTIFY_CLIENT_SECRET`)

## Branch Structure

| Branch | Purpose |
|--------|---------|
| `master` | Stable, release-ready core |
| `portable` | Constrained devices (3DS, car stereos) |
| `headless` | Automation / server mode (this branch) |
| `dashboard` | Future UI layer |
| `dev` | Active development |
