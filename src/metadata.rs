use anyhow::Context;
use id3::{frame::Picture, Tag, TagLike, Version};
use image::codecs::jpeg::JpegEncoder;
use image::{GenericImageView, ImageEncoder, ImageReader};
use metaflac::block::PictureType;
use std::path::Path;

use crate::cli::PortableConfig;

/// Struct holding all tag information from an audio file
#[derive(Debug, Clone, Default)]
pub struct AudioTags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub track: Option<u32>,
    pub year: Option<i32>,
    pub has_cover: bool,
}

impl std::fmt::Display for AudioTags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "  Title:  {}", self.title.as_deref().unwrap_or("(none)"))?;
        writeln!(f, "  Artist: {}", self.artist.as_deref().unwrap_or("(none)"))?;
        writeln!(f, "  Album:  {}", self.album.as_deref().unwrap_or("(none)"))?;
        writeln!(f, "  Genre:  {}", self.genre.as_deref().unwrap_or("(none)"))?;
        writeln!(f, "  Track:  {}", self.track.map(|t| t.to_string()).unwrap_or_else(|| "(none)".to_string()))?;
        writeln!(f, "  Year:   {}", self.year.map(|y| y.to_string()).unwrap_or_else(|| "(none)".to_string()))?;
        writeln!(f, "  Cover:  {}", if self.has_cover { "Yes" } else { "No" })?;
        Ok(())
    }
}

/// Read tags from an audio file and return them as AudioTags
pub fn read_tags(file_path: &Path) -> anyhow::Result<AudioTags> {
    let extension = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    match extension.as_deref() {
        Some("flac") => read_flac_tags(file_path),
        Some("mp3") => read_id3_tags(file_path),
        Some("wav") => read_wav_tags(file_path),
        Some("aiff" | "aif") => read_aiff_tags(file_path),
        _ => anyhow::bail!("Unsupported format for tag reading: {:?}", extension),
    }
}

fn read_flac_tags(file_path: &Path) -> anyhow::Result<AudioTags> {
    let tag = metaflac::Tag::read_from_path(file_path)
        .context("reading FLAC file")?;

    let vorbis = tag.vorbis_comments();

    let get_first = |key: &str| -> Option<String> {
        vorbis.and_then(|v| v.get(key).and_then(|vals| vals.first().cloned()))
    };

    let track = get_first("TRACKNUMBER")
        .and_then(|s| s.parse::<u32>().ok());

    let year = get_first("DATE")
        .or_else(|| get_first("YEAR"))
        .and_then(|s| s.chars().take(4).collect::<String>().parse::<i32>().ok());

    let has_cover = tag.pictures().next().is_some();

    Ok(AudioTags {
        title: get_first("TITLE"),
        artist: get_first("ARTIST"),
        album: get_first("ALBUM"),
        genre: get_first("GENRE"),
        track,
        year,
        has_cover,
    })
}

fn read_id3_tags(file_path: &Path) -> anyhow::Result<AudioTags> {
    let tag = Tag::read_from_path(file_path)
        .context("reading ID3 tags")?;

    let has_cover = tag.pictures().next().is_some();

    Ok(AudioTags {
        title: tag.title().map(|s| s.to_string()),
        artist: tag.artist().map(|s| s.to_string()),
        album: tag.album().map(|s| s.to_string()),
        genre: tag.genre_parsed().map(|g| g.to_string()),
        track: tag.track(),
        year: tag.year(),
        has_cover,
    })
}

#[allow(deprecated)]
fn read_wav_tags(file_path: &Path) -> anyhow::Result<AudioTags> {
    match Tag::read_from_wav_path(file_path) {
        Ok(tag) => Ok(AudioTags {
            title: tag.title().map(|s| s.to_string()),
            artist: tag.artist().map(|s| s.to_string()),
            album: tag.album().map(|s| s.to_string()),
            genre: tag.genre_parsed().map(|g| g.to_string()),
            track: tag.track(),
            year: tag.year(),
            has_cover: tag.pictures().next().is_some(),
        }),
        Err(_) => Ok(AudioTags::default()), // WAV might have no tags
    }
}

#[allow(deprecated)]
fn read_aiff_tags(file_path: &Path) -> anyhow::Result<AudioTags> {
    match Tag::read_from_aiff_path(file_path) {
        Ok(tag) => Ok(AudioTags {
            title: tag.title().map(|s| s.to_string()),
            artist: tag.artist().map(|s| s.to_string()),
            album: tag.album().map(|s| s.to_string()),
            genre: tag.genre_parsed().map(|g| g.to_string()),
            track: tag.track(),
            year: tag.year(),
            has_cover: tag.pictures().next().is_some(),
        }),
        Err(_) => Ok(AudioTags::default()),
    }
}

/// Sanitize a string for Vorbis comments: UTF-8 only, no null bytes
fn sanitize_vorbis_string(s: &str) -> String {
    s.chars().filter(|&c| c != '\0').collect()
}

/// Tag a FLAC file with Vorbis comments.
/// Field names: ARTIST, ALBUM, TITLE, TRACKNUMBER, GENRE (uppercase, UTF-8, no nulls)
fn tag_flac(
    file_path: &Path,
    artist: &str,
    album: &str,
    title: &str,
    track: u32,
    genre: Option<&str>,
    cover_path: Option<&Path>,
    config: &PortableConfig,
) -> anyhow::Result<()> {
    let mut flac_tag = metaflac::Tag::read_from_path(file_path)
        .context("reading FLAC file")?;

    // Remove existing Vorbis comments for these fields to avoid duplicates
    flac_tag.remove_vorbis("ARTIST");
    flac_tag.remove_vorbis("ALBUM");
    flac_tag.remove_vorbis("TITLE");
    flac_tag.remove_vorbis("TRACKNUMBER");
    flac_tag.remove_vorbis("GENRE");

    // Set Vorbis comments with sanitized UTF-8 strings (no null bytes)
    flac_tag.set_vorbis("ARTIST", vec![sanitize_vorbis_string(artist)]);
    flac_tag.set_vorbis("ALBUM", vec![sanitize_vorbis_string(album)]);
    flac_tag.set_vorbis("TITLE", vec![sanitize_vorbis_string(title)]);
    flac_tag.set_vorbis("TRACKNUMBER", vec![track.to_string()]);

    // Set genre if provided
    if let Some(g) = genre {
        flac_tag.set_vorbis("GENRE", vec![sanitize_vorbis_string(g)]);
    }

    // Add cover art if provided
    if let Some(cover) = cover_path {
        if cover.exists() {
            if let Ok(data) = resize_and_read_image(cover, config) {
                // Remove existing pictures first
                flac_tag.remove_picture_type(PictureType::CoverFront);

                let picture = metaflac::block::Picture {
                    picture_type: PictureType::CoverFront,
                    mime_type: "image/jpeg".to_string(),
                    description: String::new(),
                    width: 0,
                    height: 0,
                    depth: 0,
                    num_colors: 0,
                    data,
                };
                flac_tag.add_picture(
                    picture.mime_type,
                    picture.picture_type,
                    picture.data,
                );
            }
        }
    }

    flac_tag.write_to_path(file_path)
        .context("writing FLAC Vorbis comments")?;

    Ok(())
}

/// Tag an audio file with appropriate metadata format.
/// - FLAC files: Vorbis comments (ARTIST, ALBUM, TITLE, TRACKNUMBER, GENRE)
/// - WAV/AIFF/MP3/etc: ID3v2.3 tags
pub fn tag_audio(
    file_path: &Path,
    artist: &str,
    album: &str,
    title: &str,
    track: u32,
    genre: Option<&str>,
    cover_path: Option<&Path>,
    config: &PortableConfig,
) -> anyhow::Result<()> {
    let extension = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    // Use Vorbis comments for FLAC files
    if extension.as_deref() == Some("flac") {
        return tag_flac(file_path, artist, album, title, track, genre, cover_path, config);
    }

    // Use ID3 tags for other formats
    let mut tag = Tag::new();
    tag.set_artist(artist);
    tag.set_album(album);
    tag.set_title(title);
    tag.set_track(track);

    // Set genre if provided
    if let Some(g) = genre {
        tag.set_genre(g);
    }

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

    #[allow(deprecated)]
    match extension.as_deref() {
        Some("wav") => tag
            .write_to_wav_path(file_path, Version::Id3v23)
            .context("writing ID3 tag to WAV")?,
        Some("aif" | "aiff") => tag
            .write_to_aiff_path(file_path, Version::Id3v23)
            .context("writing ID3 tag to AIFF")?,
        _ => tag
            .write_to_path(file_path, Version::Id3v23)
            .context("writing ID3 tag")?,
    }

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
