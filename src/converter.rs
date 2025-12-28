use anyhow::Context;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Supported audio formats for conversion
pub const SUPPORTED_FORMATS: [&str; 4] = ["mp3", "flac", "wav", "aac"];

/// Check if FFmpeg is available on the system
pub fn check_ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Convert quality string to FFmpeg bitrate for lossy formats
pub fn quality_to_bitrate(format: &str, quality: &str) -> Option<&'static str> {
    match format {
        "mp3" => Some(match quality {
            "high" => "320k",
            "medium" => "192k",
            "low" => "128k",
            _ => "320k",
        }),
        "aac" => Some(match quality {
            "high" => "256k",
            "medium" => "192k",
            "low" => "128k",
            _ => "256k",
        }),
        // FLAC and WAV are lossless, no bitrate setting
        _ => None,
    }
}

/// Get the FFmpeg codec for a given format
fn format_to_codec(format: &str) -> &'static str {
    match format {
        "mp3" => "libmp3lame",
        "flac" => "flac",
        "wav" => "pcm_s16le",
        "aac" => "aac",
        _ => "libmp3lame",
    }
}

/// Check if a format is supported
pub fn is_supported_format(format: &str) -> bool {
    SUPPORTED_FORMATS.contains(&format.to_lowercase().as_str())
}

/// Get the format from a file extension
pub fn get_format_from_path(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
}

/// Convert an audio file to a different format using FFmpeg.
///
/// Returns the path to the newly created file on success.
/// The `on_output` callback receives progress lines from FFmpeg.
pub fn convert_audio<F>(
    input_path: &Path,
    output_format: &str,
    quality: &str,
    on_output: F,
) -> anyhow::Result<PathBuf>
where
    F: Fn(&str) + Send + Clone + 'static,
{
    let output_format = output_format.to_lowercase();

    if !is_supported_format(&output_format) {
        anyhow::bail!(
            "Unsupported output format: {}. Supported: {:?}",
            output_format,
            SUPPORTED_FORMATS
        );
    }

    if !input_path.exists() {
        anyhow::bail!("Input file does not exist: {}", input_path.display());
    }

    // Generate output path by changing extension
    let output_path = input_path.with_extension(&output_format);

    // Don't overwrite if same file
    if input_path == output_path {
        anyhow::bail!("Input and output formats are the same");
    }

    // Build FFmpeg arguments
    let codec = format_to_codec(&output_format);
    let mut args = vec![
        "-i".to_string(),
        input_path.to_string_lossy().to_string(),
        "-codec:a".to_string(),
        codec.to_string(),
    ];

    // Add bitrate for lossy formats
    if let Some(bitrate) = quality_to_bitrate(&output_format, quality) {
        args.push("-b:a".to_string());
        args.push(bitrate.to_string());
    }

    // Overwrite output without asking, show progress
    args.push("-y".to_string());
    args.push("-progress".to_string());
    args.push("pipe:1".to_string());
    args.push(output_path.to_string_lossy().to_string());

    on_output(&format!(
        "Converting {} -> {} (codec: {})",
        input_path.display(),
        output_path.display(),
        codec
    ));

    let mut child = Command::new("ffmpeg")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn FFmpeg. Is it installed?")?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let on_output_clone = on_output.clone();

    // Process stdout (progress output)
    if let Some(stdout) = stdout {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let trimmed = line.trim();
            // FFmpeg progress output includes lines like "out_time=00:01:23.456"
            if trimmed.starts_with("out_time=") {
                if let Some(time) = trimmed.strip_prefix("out_time=") {
                    on_output(&format!("Progress: {}", time));
                }
            }
        }
    }

    // Process stderr (FFmpeg logs and errors)
    if let Some(stderr) = stderr {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                // Filter out verbose FFmpeg output, keep important messages
                if trimmed.contains("Error")
                    || trimmed.contains("error")
                    || trimmed.contains("Warning")
                    || trimmed.contains("Output")
                    || trimmed.contains("Stream")
                {
                    on_output_clone(trimmed);
                }
            }
        }
    }

    let status = child.wait().context("Failed to wait for FFmpeg")?;

    if !status.success() {
        // Clean up partial output file if it exists
        let _ = std::fs::remove_file(&output_path);
        anyhow::bail!(
            "FFmpeg conversion failed for: {}",
            input_path.display()
        );
    }

    // Verify output file was created
    if !output_path.exists() {
        anyhow::bail!(
            "FFmpeg completed but output file was not created: {}",
            output_path.display()
        );
    }

    on_output(&format!("Conversion complete: {}", output_path.display()));

    Ok(output_path)
}

/// Delete a file (used after successful conversion when user confirms)
pub fn delete_file(path: &Path) -> anyhow::Result<()> {
    std::fs::remove_file(path).context(format!("Failed to delete: {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quality_to_bitrate() {
        assert_eq!(quality_to_bitrate("mp3", "high"), Some("320k"));
        assert_eq!(quality_to_bitrate("mp3", "medium"), Some("192k"));
        assert_eq!(quality_to_bitrate("mp3", "low"), Some("128k"));
        assert_eq!(quality_to_bitrate("aac", "high"), Some("256k"));
        assert_eq!(quality_to_bitrate("flac", "high"), None);
        assert_eq!(quality_to_bitrate("wav", "high"), None);
    }

    #[test]
    fn test_is_supported_format() {
        assert!(is_supported_format("mp3"));
        assert!(is_supported_format("MP3"));
        assert!(is_supported_format("flac"));
        assert!(is_supported_format("wav"));
        assert!(is_supported_format("aac"));
        assert!(!is_supported_format("ogg"));
        assert!(!is_supported_format("wma"));
    }

    #[test]
    fn test_format_to_codec() {
        assert_eq!(format_to_codec("mp3"), "libmp3lame");
        assert_eq!(format_to_codec("flac"), "flac");
        assert_eq!(format_to_codec("wav"), "pcm_s16le");
        assert_eq!(format_to_codec("aac"), "aac");
    }

    #[test]
    fn test_check_ffmpeg_available() {
        // This test depends on the environment, just ensure it doesn't panic
        let _ = check_ffmpeg_available();
    }
}
