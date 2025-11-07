use id3::{Tag, Version};
use std::path::Path;

pub fn tag_mp3(
    file_path: &Path,
    artist: &str,
    album: &str,
    title: &str,
    track: u32,
) -> anyhow::Result<()> {
    let mut tag = Tag::new();
    tag.set_artist(artist);
    tag.set_album(album);
    tag.set_title(title);
    tag.set_track(track);
    tag.write_to_path(file_path, Version::Id3v24)?;

    Ok(())
}
