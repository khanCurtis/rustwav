use anyhow::Context;
use std::path::Path;
use std::process::Command;

pub fn download_track(query: &str, output_path: &Path, format: &str) -> anyhow::Result<()> {
    let output_template = output_path
        .join("%(title)s.%(ext)s")
        .to_string_lossy()
        .to_string();

    // Use ytsearch: prefix to search YouTube for the track
    let search_query = format!("ytsearch1:{}", query);

    let status = Command::new("yt-dlp")
        .args([
            "-x", // extract audio
            "--no-playlist",
            "--audio-format",
            format, // mp3, flac, wav
            "-o",
            &output_template,
            &search_query,
        ])
        .status()
        .context("failed to spawn yt-dlp")?;

    if !status.success() {
        anyhow::bail!("yt-dlp failed for query: {}", query);
    }

    Ok(())
}
