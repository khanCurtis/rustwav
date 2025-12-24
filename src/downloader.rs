use anyhow::Context;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

/// Convert quality string to yt-dlp audio quality value
/// yt-dlp uses 0 (best) to 10 (worst)
fn quality_to_ytdlp(quality: &str) -> &str {
    match quality {
        "high" => "0",
        "medium" => "5",
        "low" => "9",
        _ => "0",
    }
}

/// Download a track using yt-dlp (legacy version without output capture)
#[allow(dead_code)]
pub fn download_track(query: &str, output_path: &Path, format: &str) -> anyhow::Result<()> {
    download_track_with_output(query, output_path, format, "high", |_| {})
}

/// Download a track to a specific file path using yt-dlp with output streaming.
///
/// The `output_file` should be the full path including filename and extension.
/// The `on_output` callback is called for each line of output from yt-dlp,
/// allowing real-time progress updates in the TUI.
pub fn download_track_with_output<F>(
    query: &str,
    output_file: &Path,
    format: &str,
    quality: &str,
    on_output: F,
) -> anyhow::Result<()>
where
    F: Fn(&str) + Send + Clone + 'static,
{
    // Use the exact output path provided (strip extension as yt-dlp adds it)
    let output_template = output_file
        .with_extension(format)
        .to_string_lossy()
        .to_string();

    // Use ytsearch: prefix to search YouTube for the track
    let search_query = format!("ytsearch1:{}", query);
    let audio_quality = quality_to_ytdlp(quality);

    let mut child = Command::new("yt-dlp")
        .args([
            "-x",            // extract audio
            "--no-playlist", // don't download playlists
            "--audio-format",
            format, // mp3, flac, wav, aac
            "--audio-quality",
            audio_quality, // 0=best, 10=worst
            "--newline",   // output progress on new lines (easier to parse)
            "--progress",  // show progress
            "-o",
            &output_template,
            &search_query,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn yt-dlp")?;

    // Read stdout in a separate thread
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let on_output_clone = on_output.clone();

    // Process stdout
    if let Some(stdout) = stdout {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                on_output(trimmed);
            }
        }
    }

    // Process stderr (yt-dlp outputs progress here)
    if let Some(stderr) = stderr {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                on_output_clone(trimmed);
            }
        }
    }

    let status = child.wait().context("failed to wait for yt-dlp")?;

    if !status.success() {
        anyhow::bail!("yt-dlp failed for query: {}", query);
    }

    Ok(())
}
