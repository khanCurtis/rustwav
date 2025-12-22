# rustwav

Rust-based music downloader and library manager. Downloads and organizes music from Spotify playlists/albums via YouTube with full metadata, album art, and ID3v2.3 tagging.

## Branch: `headless`

This branch is for **automation and server mode** - designed for scripts, cron jobs, Docker containers, and CI/CD pipelines.

### Headless Mode Features

When running with `--headless`:

- **No prompts** - Never blocks waiting for user input
- **Deterministic output** - Consistent, parseable logs
- **Script-friendly** - Exit codes, JSON output options
- **Daemon-ready** - Suitable for Docker, systemd, cron

### Usage

```bash
# Headless mode with structured logging
rustwav --headless album <link>

# JSON output for parsing
rustwav --headless --json playlist <link>

# Quiet mode (errors only)
rustwav --headless --quiet album <link>

# Portable mode also available
rustwav --headless --portable album <link>
```

### Use Cases

- **Cron jobs** - Scheduled playlist syncing
- **Docker containers** - Automated music library building
- **CI/CD** - Testing and automated downloads
- **Server deployment** - Headless music management

### Output Modes

| Flag | Description |
|------|-------------|
| (default) | Human-readable progress |
| `--json` | Machine-parseable JSON lines |
| `--quiet` | Errors only |
| `--verbose` | Debug-level logging |

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Invalid arguments |
| 3 | Network/API error |
| 4 | Download failed |

## Requirements

- Rust 1.70+
- `yt-dlp` in PATH
- Spotify API credentials:
  ```bash
  export RSPOTIFY_CLIENT_ID="your_client_id"
  export RSPOTIFY_CLIENT_SECRET="your_client_secret"
  ```

## Branch Structure

| Branch | Purpose |
|--------|---------|
| `master` | UI / Dashboard |
| `cli` | Command-line interface |
| `headless` | Automation / server mode (this branch) |
