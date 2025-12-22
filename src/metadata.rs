use anyhow::Context;
use id3::{frame::Picture, Tag, TagLike, Version};
use image::{ImageReader, GenericImageView, ImageEncoder};
use image::codecs::jpeg::JpegEncoder;
use std::path::Path;

use crate::cli::PortableConfig;

/// Tag an MP3 (ID3v2.3), optionally embed cover art from `cover_path` if present.
pub fn tag_mp3(
    file_path: &Path,
    artist: &str,
    album: &str,
    title: &str,
    track: u32,
    cover_path: Option<&Path>,
    config: &PortableConfig,
) -> anyhow::Result<()> {
    let mut tag = Tag::new();
    tag.set_artist(artist);
    tag.set_album(album);
    tag.set_title(title);
    tag.set_track(track);

    if let Some(cover) = cover_path {
        if cover.exists() {
            if let Ok(data) = resize_and_read_image(cover, config) {
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

fn encode_jpeg(img: &image::DynamicImage, quality: u8) -> anyhow::Result<Vec<u8>> {
    let mut buf: Vec<u8> = Vec::new();
    let rgb_img = img.to_rgb8();
    let encoder = JpegEncoder::new_with_quality(&mut buf, quality);
    encoder.write_image(
        rgb_img.as_raw(),
        rgb_img.width(),
        rgb_img.height(),
        image::ExtendedColorType::Rgb8,
    )?;
    Ok(buf)
}

fn resize_and_read_image(cover: &Path, config: &PortableConfig) -> anyhow::Result<Vec<u8>> {
    let img = ImageReader::open(cover)?.decode()?;
    let (w, h) = img.dimensions();
    let max_dim = config.max_cover_dim;
    let max_bytes = config.max_cover_bytes;

    let scale = ((max_dim as f32) / (w.max(h) as f32)).min(1.0);
    let new_w = (w as f32 * scale).round() as u32;
    let new_h = (h as f32 * scale).round() as u32;
    let resized = img.resize_exact(new_w, new_h, image::imageops::FilterType::Lanczos3);

    // encode to JPEG
    let mut buf = encode_jpeg(&resized, 85)?;

    // if still too large, re-encode at lower quality
    if buf.len() > max_bytes {
        let mut q = 80u8;
        loop {
            buf = encode_jpeg(&resized, q)?;
            if buf.len() <= max_bytes || q <= 30 {
                break;
            }
            q -= 10;
        }
    }
    Ok(buf)
}
