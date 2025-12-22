use anyhow::Result;
use rspotify::{ClientCredsSpotify, Credentials};
use rspotify::clients::BaseClient;
use rspotify::model::{FullAlbum, FullPlaylist, AlbumId, PlaylistId};

async fn get_spotify_client() -> Result<ClientCredsSpotify, anyhow::Error> {
    let creds = Credentials::from_env()
        .ok_or_else(|| anyhow::anyhow!("Missing RSPOTIFY_CLIENT_ID or RSPOTIFY_CLIENT_SECRET environment variables"))?;
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
    let playlist = spotify.playlist(playlist_id, None, None).await?;
    Ok(playlist)
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

