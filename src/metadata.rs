use anyhow::Context;
use id3::{frame::Picture, Tag, TagLike, Version}; 
use image::io::Reader as ImageReader;
use std::{fs, path::{Path, PathBuf}};

const MAX_COVER_DIM: u32 = 500;
const MAX_COVER_BYTES: usize = 300 * 1024;

/// Tag an MP3 (ID3v2.3), optionally embed cover art from `cover_path` if present.
/// `cover_path` may be Some(path) if you have a downloaded cover image.
pub fn tag_mp3(
    file_path: &Path,
    artist: &str,
    album: &str,
    title: &str,
    track: u32,
    cover_path: Option<&Path>,
) -> anyhow::Result<()> {
    let mut tag = Tag::new();
    tag.set_artist(artist);
    tag.set_album(album);
    tag.set_title(title);
    tag.set_track(track);

    if let Some(cover) = cover_path {
        if cover.exists() {
            if letOk(data) = resize_and_read_image(cover) {
                let picture = Picture {
                    mime_type: "image/jpeg".to_string(),
                    picture_type: id3::frame::PictureType::CoverFront,
                    description: "cover".to_string(),
                    data,
                };
                tag.add_frame(picture);
            }
        }
    }

    tag.write_to_path(file_path, Version::Id3v23).context("writing ID3 tag")?;

    Ok(())
}

fn resize_and_read_image(cover: &Path) -> anyhow::Result<Vec<u8>> {
    let img = ImageReader::open(cover)?.decode()?;
    let (w, h) = img.dimensions();
    let scale = ((MAX_COVER_DIM as f32) / (w.max(h) as f32)).min(1.0);
    let new_w = (w as f32 * scale).round() as u32;
    let new_h = (h as f32 * scale).round() as u32;
    let resized = img.resize_exact(new_w, new_h, image::imageops::FilterType::Lanczos3);
    
    // encode to JPEG
    let mut buf: Vec<u8> = Vec::new();
    resized.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageOutputFormat::Jpeg(85))?;
    
    // if still too large, re-encode at lower quality
    if buf.len() > MAX_COVER_BYTES {
        let mut q = 80;
        loop {
            buf.clear();
            resized.write_to(
                &mut std::io::Cursor::new(&mut buf),
                image::ImageOutputFormat::Jpeg(q),
            )?;
            if buf.len() <= MAX_COVER_BYTES || q <= 30 {
                break;
            }
            q -= 10;
        }
    }
    Ok(buf)
}
