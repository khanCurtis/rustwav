use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Hash, Eq, PartialEq, Clone)]
pub struct TrackEntry {
    pub artist: String,
    pub title: String,
    pub path: String,
}

pub struct DownloadDB {
    pub tracks: HashSet<TrackEntry>,
    file_path: String,
}

impl DownloadDB {
    pub fn new(file_path: &str) -> Self {
        let tracks = if Path::new(file_path).exists() {
            let data = fs::read_to_string(file_path).unwrap_or_default();
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            HashSet::new()
        };

        Self {
            tracks,
            file_path: file_path.to_string(),
        }
    }

    pub fn add(&mut self, entry: TrackEntry) {
        self.tracks.insert(entry);
        self.save();
    }

    pub fn contains(&self, entry: &TrackEntry) -> bool {
        self.tracks.contains(entry)
    }

    fn save(&self) {
        if let Some(parent) = std::path::Path::new(&self.file_path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let data = serde_json::to_string_pretty(&self.tracks).unwrap();
        fs::write(&self.file_path, data).expect("Failed to save downloaded_songs.json");
    }
}
