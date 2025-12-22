use crate::cli::{HeadlessConfig, OutputFormat};
use serde::Serialize;

#[derive(Clone)]
pub struct Logger {
    config: HeadlessConfig,
}

#[derive(Serialize)]
struct JsonEvent {
    #[serde(rename = "type")]
    event_type: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

impl Logger {
    pub fn new(config: HeadlessConfig) -> Self {
        Self { config }
    }

    pub fn info(&self, message: &str) {
        if self.config.quiet {
            return;
        }
        self.output("info", message, None);
    }

    pub fn progress(&self, message: &str) {
        if self.config.quiet {
            return;
        }
        self.output("progress", message, None);
    }

    pub fn success(&self, message: &str) {
        if self.config.quiet {
            return;
        }
        self.output("success", message, None);
    }

    pub fn warn(&self, message: &str) {
        self.output("warn", message, None);
    }

    pub fn error(&self, message: &str) {
        self.output("error", message, None);
    }

    pub fn debug(&self, message: &str) {
        if !self.config.verbose {
            return;
        }
        self.output("debug", message, None);
    }

    pub fn track_start(&self, artist: &str, title: &str) {
        if self.config.quiet {
            return;
        }
        let data = serde_json::json!({
            "artist": artist,
            "title": title
        });
        self.output("track_start", &format!("Downloading: {} — {}", artist, title), Some(data));
    }

    pub fn track_skip(&self, artist: &str, title: &str) {
        if self.config.quiet {
            return;
        }
        let data = serde_json::json!({
            "artist": artist,
            "title": title
        });
        self.output("track_skip", &format!("Skipping: {} — {}", artist, title), Some(data));
    }

    pub fn track_complete(&self, artist: &str, title: &str, path: &str) {
        if self.config.quiet {
            return;
        }
        let data = serde_json::json!({
            "artist": artist,
            "title": title,
            "path": path
        });
        self.output("track_complete", &format!("Complete: {} — {}", artist, title), Some(data));
    }

    pub fn album_complete(&self, name: &str, artist: &str, track_count: usize) {
        let data = serde_json::json!({
            "album": name,
            "artist": artist,
            "tracks": track_count
        });
        self.output("album_complete", &format!("Album '{}' by {} finished.", name, artist), Some(data));
    }

    pub fn playlist_complete(&self, name: &str, track_count: usize) {
        let data = serde_json::json!({
            "playlist": name,
            "tracks": track_count
        });
        self.output("playlist_complete", &format!("Playlist '{}' with {} tracks finished.", name, track_count), Some(data));
    }

    fn output(&self, event_type: &str, message: &str, data: Option<serde_json::Value>) {
        match self.config.output_format {
            OutputFormat::Json => {
                let event = JsonEvent {
                    event_type: event_type.to_string(),
                    message: message.to_string(),
                    data,
                };
                if let Ok(json) = serde_json::to_string(&event) {
                    println!("{}", json);
                }
            }
            OutputFormat::Text => {
                let prefix = match event_type {
                    "error" => "[ERROR]",
                    "warn" => "[WARN]",
                    "debug" => "[DEBUG]",
                    "progress" | "track_start" => "[...]",
                    "success" | "track_complete" | "album_complete" | "playlist_complete" => "[OK]",
                    "track_skip" => "[SKIP]",
                    _ => "[INFO]",
                };
                if self.config.enabled {
                    println!("{} {}", prefix, message);
                } else {
                    println!("{}", message);
                }
            }
        }
    }
}
