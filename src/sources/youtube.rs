use anyhow::{Context, Result};
use serde::Deserialize;
use std::process::Command;

/// A track from a YouTube playlist
#[derive(Debug, Clone)]
pub struct YouTubeTrack {
    pub title: String,
    pub artist: String,
    pub url: String,
    pub duration: Option<u64>,
}

/// A YouTube playlist with its tracks
#[derive(Debug, Clone)]
pub struct YouTubePlaylist {
    pub title: String,
    pub uploader: String,
    pub tracks: Vec<YouTubeTrack>,
}

#[derive(Deserialize)]
struct YtDlpPlaylistEntry {
    title: Option<String>,
    uploader: Option<String>,
    channel: Option<String>,
    url: Option<String>,
    webpage_url: Option<String>,
    duration: Option<f64>,
}

#[derive(Deserialize)]
struct YtDlpPlaylistInfo {
    title: Option<String>,
    uploader: Option<String>,
    channel: Option<String>,
    entries: Option<Vec<YtDlpPlaylistEntry>>,
}

/// Check if a URL is a YouTube URL
pub fn is_youtube_url(url: &str) -> bool {
    url.contains("youtube.com") || url.contains("youtu.be")
}

/// Check if a URL is a YouTube playlist URL
pub fn is_youtube_playlist(url: &str) -> bool {
    is_youtube_url(url) && (url.contains("playlist?list=") || url.contains("&list="))
}

/// Extract playlist ID from a YouTube playlist URL
pub fn extract_playlist_id(url: &str) -> Option<String> {
    // Handle both formats:
    // https://www.youtube.com/playlist?list=PLxxxxx
    // https://www.youtube.com/watch?v=xxxxx&list=PLxxxxx
    if let Some(pos) = url.find("list=") {
        let start = pos + 5;
        let rest = &url[start..];
        let end = rest.find('&').unwrap_or(rest.len());
        Some(rest[..end].to_string())
    } else {
        None
    }
}

/// Fetch playlist information from YouTube using yt-dlp
pub fn fetch_playlist(url: &str) -> Result<YouTubePlaylist> {
    // Use yt-dlp to get playlist info as JSON
    let output = Command::new("yt-dlp")
        .args([
            "--flat-playlist",  // Don't download, just get info
            "--dump-json",      // Output as JSON
            "-i",               // Ignore errors for unavailable videos
            url,
        ])
        .output()
        .context("Failed to run yt-dlp. Is it installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("yt-dlp failed: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // yt-dlp with --flat-playlist outputs one JSON object per line for each video
    let mut tracks = Vec::new();
    let mut playlist_title = String::new();
    let mut playlist_uploader = String::new();

    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }

        if let Ok(entry) = serde_json::from_str::<YtDlpPlaylistEntry>(line) {
            // Get artist from uploader/channel
            let artist = entry.uploader
                .or(entry.channel)
                .unwrap_or_else(|| "Unknown Artist".to_string());

            // Get video URL
            let video_url = entry.webpage_url
                .or(entry.url)
                .unwrap_or_default();

            if video_url.is_empty() {
                continue;
            }

            // Use first entry's uploader as playlist uploader if not set
            if playlist_uploader.is_empty() {
                playlist_uploader = artist.clone();
            }

            tracks.push(YouTubeTrack {
                title: entry.title.unwrap_or_else(|| "Unknown Title".to_string()),
                artist,
                url: video_url,
                duration: entry.duration.map(|d| d as u64),
            });
        }
    }

    // If we couldn't get tracks, try parsing as a single playlist object
    if tracks.is_empty() {
        // Try to get playlist info with a different approach
        let output = Command::new("yt-dlp")
            .args([
                "--flat-playlist",
                "-J",  // Single JSON object for entire playlist
                "-i",
                url,
            ])
            .output()
            .context("Failed to run yt-dlp")?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Ok(info) = serde_json::from_str::<YtDlpPlaylistInfo>(&stdout) {
                playlist_title = info.title.unwrap_or_else(|| "YouTube Playlist".to_string());
                playlist_uploader = info.uploader
                    .or(info.channel)
                    .unwrap_or_else(|| "Unknown".to_string());

                if let Some(entries) = info.entries {
                    for entry in entries {
                        let artist = entry.uploader
                            .or(entry.channel)
                            .unwrap_or_else(|| playlist_uploader.clone());

                        let video_url = entry.webpage_url
                            .or(entry.url)
                            .unwrap_or_default();

                        if video_url.is_empty() {
                            continue;
                        }

                        tracks.push(YouTubeTrack {
                            title: entry.title.unwrap_or_else(|| "Unknown Title".to_string()),
                            artist,
                            url: video_url,
                            duration: entry.duration.map(|d| d as u64),
                        });
                    }
                }
            }
        }
    }

    if tracks.is_empty() {
        anyhow::bail!("No tracks found in playlist. Is the URL correct?");
    }

    // Default playlist title if not set
    if playlist_title.is_empty() {
        playlist_title = format!("YouTube Playlist ({} tracks)", tracks.len());
    }

    Ok(YouTubePlaylist {
        title: playlist_title,
        uploader: playlist_uploader,
        tracks,
    })
}
