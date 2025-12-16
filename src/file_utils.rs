use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// Sanitize filenames to remove invalid characters
pub fn sanitize_filename(name: &str) -> String {
    let mut s = String::with_capacity(name.len());
    for ch in name.chars() {
        match ch {
            // safe characters
            'A'..='Z' | 'a'..='z' | '0'..='9' | ' ' | '-' | '_' | '.' | '(' | ')' => s.push(ch),
            '/' | '\\' | '?' | '%' | '*' | ':' | '|' | '"' | '<' | '>' | '\n' | '\r' | '\t' => {
                s.push('_')
            }
            other => {
                // strip control and non-printable; keep basic punctuation as underscore
                if other.is_control() {
                    s.push('_');
                } else {
                    // convert non-ascii to ascii fallback
                    if other.is_ascii() {
                        s.push(other);
                    } else {
                        s.push('_');
                    }
                }
            }
        }
    }
    // Trim and limit length for 3DS compatibility
    let trimmed = s.trim();
    if trimmed.len() > 100 {
        trimmed[..100].to_string()
    } else {
        trimmed.to_string()
    }
}

/// Create folder for album
pub fn create_album_folder(base_path: &Path, artist: &str, album: &str) -> PathBuf {
    let artist_s = sanitize_filename(artist);
    let album_s = sanitize_filename(album);
    let folder = base_path.join(artist_s).join(album_s);
    std::fs::create_dir_all(&folder).expect("Failed to create album folder");
    folder
}

pub fn create_m3u(playlist_name: &str, tracks: &[PathBuf], out_dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(out_dir)?;
    let playlist_file = out_dir.join(format!("{}.m3u", sanitize_filename(playlist_name)));
    let file = File::create(&playlist_file)?;
    let mut writer = BufWriter::new(file);

   // write header
   writeln!(writer, "#EXTM3U")?;

   for track in tracks {
       // attempt to write path relative to out_dir
       let rel = match track.strip_prefix(out_dir) {
           Ok(r) => r.to_owned(),
           Err(_) => match track.strip_prefix(std::path::new(".")) {
               Ok(r2) => r2.to_owned(),
               Err(_) => track.clone(),
           },
       };
       writeln!(writer, "{}", rel.display())?;
    }

    println!("Playlist saved: {}", playlist_file.display());
    Ok(())
}
