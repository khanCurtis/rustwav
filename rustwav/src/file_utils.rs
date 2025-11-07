use std::fs;
use std::path::{Path, PathBuf};

/// Sanitize filenames to remove invalid characters
pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .filter(|c| !['/', '\\', '?', '%', '*', ':', '|', '"', '<', '>'].contains(c))
        .collect()
}

/// Create folder for album
pub fn create_album_folder(base_path: &Path, artist: &str, album: &str) -> PathBuf {
    let folder = base_path
        .join(sanitize_filename(artist))
        .join(sanitize_filename(album));
    fs::create_dir_all(&folder).expect("Failed to create album folder");
    folder
}
