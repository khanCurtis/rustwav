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

    /// Find a track entry by its file path
    pub fn find_by_path(&self, path: &str) -> Option<&TrackEntry> {
        self.tracks.iter().find(|t| t.path == path)
    }

    /// Update the path for a track (after format conversion).
    /// Returns true if the entry was found and updated.
    pub fn update_path(&mut self, old_path: &str, new_path: &str) -> bool {
        // Find the entry with the old path
        let entry = self.tracks.iter().find(|t| t.path == old_path).cloned();

        if let Some(old_entry) = entry {
            // Remove old entry and insert updated one
            self.tracks.remove(&old_entry);
            let new_entry = TrackEntry {
                artist: old_entry.artist,
                title: old_entry.title,
                path: new_path.to_string(),
            };
            self.tracks.insert(new_entry);
            self.save();
            true
        } else {
            false
        }
    }

    /// Remove a track entry by its file path.
    /// Returns true if the entry was found and removed.
    pub fn remove_by_path(&mut self, path: &str) -> bool {
        let entry = self.tracks.iter().find(|t| t.path == path).cloned();

        if let Some(old_entry) = entry {
            self.tracks.remove(&old_entry);
            self.save();
            true
        } else {
            false
        }
    }

    /// Clean up the database by removing entries for files that no longer exist.
    /// Returns a tuple of (removed_count, total_before_cleanup).
    pub fn cleanup(&mut self) -> (usize, usize) {
        let total_before = self.tracks.len();

        // Collect entries to remove (files that don't exist)
        let missing: Vec<TrackEntry> = self
            .tracks
            .iter()
            .filter(|entry| !Path::new(&entry.path).exists())
            .cloned()
            .collect();

        let removed_count = missing.len();

        // Remove missing entries
        for entry in missing {
            self.tracks.remove(&entry);
        }

        if removed_count > 0 {
            self.save();
        }

        (removed_count, total_before)
    }

    /// Get all track entries (for listing purposes)
    pub fn all_tracks(&self) -> Vec<&TrackEntry> {
        self.tracks.iter().collect()
    }

    fn save(&self) {
        if let Some(parent) = std::path::Path::new(&self.file_path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let data = serde_json::to_string_pretty(&self.tracks).unwrap();
        fs::write(&self.file_path, data).expect("Failed to save downloaded_songs.json");
    }
}
