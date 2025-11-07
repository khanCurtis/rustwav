use std::fs::File;
use std::io::{BufWriter, Write};
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

pub fn create_m3u(
    playlist_name: &str,
    tracks: &[PathBuf],
    output_folder: &Path,
) -> anyhow::Result<()> {
    let playlist_file = output_folder.join(format!("{}.m3u", playlist_name));
    let file = File::create(&playlist_file)?;
    let mut writer = BufWriter::new(file);

    for track in tracks {
        // Write relative paths
        let relative_path = track.strip_prefix(output_folder.parent().unwrap_or(output_folder))?;
        writeln!(writer, "{}", relative_path.display())?;
    }

    println!("Playlist saved: {}", playlist_file.display());
    Ok(())
}
