use anyhow::Result;
use futures::stream::TryStreamExt;
use rspotify::clients::BaseClient;
use rspotify::model::{AlbumId, FullAlbum, FullPlaylist, PlaylistId, PlaylistItem};
use rspotify::{ClientCredsSpotify, Credentials};

async fn get_spotify_client() -> Result<ClientCredsSpotify, anyhow::Error> {
    let creds = Credentials::from_env().ok_or_else(|| {
        anyhow::anyhow!(
            "Missing RSPOTIFY_CLIENT_ID or RSPOTIFY_CLIENT_SECRET environment variables"
        )
    })?;
    let spotify = ClientCredsSpotify::new(creds);
    spotify.request_token().await?;
    Ok(spotify)
}

pub async fn fetch_album(link: &str) -> Result<FullAlbum, anyhow::Error> {
    let album_id = AlbumId::from_id(extract_id(link, "album")?)?;
    let spotify = get_spotify_client().await?;
    let album = spotify.album(album_id, None).await?;
    Ok(album)
}

pub async fn fetch_playlist(link: &str) -> Result<FullPlaylist, anyhow::Error> {
    let playlist_id = PlaylistId::from_id(extract_id(link, "playlist")?)?;
    let spotify = get_spotify_client().await?;
    let playlist = spotify.playlist(playlist_id.clone(), None, None).await?;
    Ok(playlist)
}

/// Fetch all playlist items with pagination (no 100 track limit)
pub async fn fetch_all_playlist_items(link: &str) -> Result<Vec<PlaylistItem>, anyhow::Error> {
    let playlist_id = PlaylistId::from_id(extract_id(link, "playlist")?)?;
    let spotify = get_spotify_client().await?;

    // Use playlist_items stream which handles pagination automatically
    let items: Vec<PlaylistItem> = spotify
        .playlist_items(playlist_id, None, None)
        .try_collect()
        .await?;

    Ok(items)
}

fn extract_id<'a>(link: &'a str, kind: &str) -> Result<&'a str, anyhow::Error> {
    // Handle both full URLs and bare IDs
    // e.g., "https://open.spotify.com/album/abc123?si=xyz" -> "abc123"
    // or just "abc123" -> "abc123"
    if link.contains("spotify.com") {
        let pattern = format!("/{}/", kind);
        if let Some(pos) = link.find(&pattern) {
            let start = pos + pattern.len();
            let rest = &link[start..];
            let end = rest.find('?').unwrap_or(rest.len());
            return Ok(&rest[..end]);
        }
        anyhow::bail!("Could not extract {} ID from link: {}", kind, link);
    }
    Ok(link)
}
