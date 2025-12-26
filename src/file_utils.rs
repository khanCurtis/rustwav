use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::cli::PortableConfig;

/// Sanitize filenames to remove invalid characters
pub fn sanitize_filename(name: &str) -> String {
    sanitize_filename_with_len(name, 100)
}

/// Sanitize filenames with configurable max length
pub fn sanitize_filename_with_len(name: &str, max_len: usize) -> String {
    let mut s = String::with_capacity(name.len());
    for ch in name.chars() {
        match ch {
            // safe characters
            'A'..='Z' | 'a'..='z' | '0'..='9' | ' ' | '-' | '_' | '.' | '(' | ')' => s.push(ch),
            '/' | '\\' | '?' | '%' | '*' | ':' | '|' | '"' | '<' | '>' | '\n' | '\r' | '\t' => {
                s.push('_')
            }
            other => {
                if other.is_control() {
                    s.push('_');
                } else if other.is_ascii() {
                    s.push(other);
                } else {
                    s.push('_');
                }
            }
        }
    }
    let trimmed = s.trim();
    if trimmed.len() > max_len {
        trimmed[..max_len].to_string()
    } else {
        trimmed.to_string()
    }
}

/// Stricter FAT32-safe sanitization for portable mode
/// Only allows alphanumeric, underscore, hyphen - no spaces
pub fn sanitize_filename_portable(name: &str, max_len: usize) -> String {
    let mut s = String::with_capacity(name.len());
    for ch in name.chars() {
        match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' => s.push(ch),
            ' ' => s.push('_'),
            _ => {}
        }
    }
    // Collapse multiple underscores
    let mut result = String::with_capacity(s.len());
    let mut last_was_underscore = false;
    for ch in s.chars() {
        if ch == '_' {
            if !last_was_underscore {
                result.push(ch);
            }
            last_was_underscore = true;
        } else {
            result.push(ch);
            last_was_underscore = false;
        }
    }
    let trimmed = result.trim_matches('_');
    if trimmed.len() > max_len {
        trimmed[..max_len].to_string()
    } else {
        trimmed.to_string()
    }
}

/// Create folder for album - with portable mode support
pub fn create_album_folder(base_path: &Path, artist: &str, album: &str) -> PathBuf {
    let artist_s = sanitize_filename(artist);
    let album_s = sanitize_filename(album);
    let folder = base_path.join(artist_s).join(album_s);
    std::fs::create_dir_all(&folder).expect("Failed to create album folder");
    folder
}

/// Create folder for portable mode - shallow structure (no artist/album nesting)
pub fn create_portable_folder(base_path: &Path, config: &PortableConfig) -> PathBuf {
    let folder = if config.enabled {
        // Shallow: just use base_path directly
        base_path.to_path_buf()
    } else {
        base_path.to_path_buf()
    };
    std::fs::create_dir_all(&folder).expect("Failed to create folder");
    folder
}

/// Build a filename based on portable mode settings
pub fn build_filename(artist: &str, title: &str, ext: &str, config: &PortableConfig) -> String {
    if config.enabled {
        // Portable: "Artist_-_Title.ext" with strict sanitization
        let artist_s = sanitize_filename_portable(artist, 20);
        let title_s = sanitize_filename_portable(title, config.max_filename_len - 25);
        format!("{}_-_{}.{}", artist_s, title_s, ext)
    } else {
        // Normal: "Artist - Title.ext"
        let artist_s = sanitize_filename(artist);
        let title_s = sanitize_filename(title);
        format!("{} - {}.{}", artist_s, title_s, ext)
    }
}

/// Calculate relative path from one directory to a file
fn relative_path_from(from_dir: &Path, to_file: &Path) -> PathBuf {
    // Canonicalize both paths to get absolute paths
    let from_abs = std::fs::canonicalize(from_dir).unwrap_or_else(|_| from_dir.to_path_buf());
    let to_abs = std::fs::canonicalize(to_file).unwrap_or_else(|_| to_file.to_path_buf());

    // Find common ancestor
    let from_parts: Vec<_> = from_abs.components().collect();
    let to_parts: Vec<_> = to_abs.components().collect();

    let mut common_len = 0;
    for (a, b) in from_parts.iter().zip(to_parts.iter()) {
        if a == b {
            common_len += 1;
        } else {
            break;
        }
    }

    // Build relative path: go up from 'from_dir' then down to 'to_file'
    let mut result = PathBuf::new();
    for _ in common_len..from_parts.len() {
        result.push("..");
    }
    for part in &to_parts[common_len..] {
        result.push(part);
    }

    result
}

pub fn create_m3u(playlist_name: &str, tracks: &[PathBuf], playlist_dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(playlist_dir)?;
    let playlist_file = playlist_dir.join(format!("{}.m3u", sanitize_filename(playlist_name)));
    let file = File::create(&playlist_file)?;
    let mut writer = BufWriter::new(file);

    // write header
    writeln!(writer, "#EXTM3U")?;

    for track in tracks {
        // Calculate relative path from playlist directory to the track
        let rel = relative_path_from(playlist_dir, track);
        writeln!(writer, "{}", rel.display())?;
    }

    println!("Playlist saved: {}", playlist_file.display());
    Ok(())
}
