use std::path::Path;
use std::process::Command;

pub fn download_track(query: &str, output_path: &Path, format: &str) -> anyhow::Result<()> {
    let output_template = output_path
        .join("%(title)s.%(ext)s")
        .to_string_lossy()
        .to_string();

    let status = Command::new("yt-dlp")
        .args([
            "-x", // extract audio
            "--audio-format",
            format, //mp3, flac, wav
            "-o",
            &output_template, //output template
            query,
        ])
        .status()?;

    if !status.success() {
        anyhow::bail!("yt-dlp failed for query: {}", query);
    }

    Ok(())
}
