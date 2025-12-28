use anyhow::Result;
use futures::stream::TryStreamExt;
use rspotify::clients::BaseClient;
use rspotify::model::{AlbumId, FullAlbum, FullPlaylist, PlaylistId, PlaylistItem, SearchType};
use rspotify::{ClientCredsSpotify, Credentials};

/// Metadata fetched from Spotify for a track
#[derive(Debug, Clone)]
pub struct TrackMetadata {
    pub artist: String,
    pub album: String,
    pub title: String,
    pub track_number: u32,
    pub cover_url: Option<String>,
}

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

/// Search for a track on Spotify by artist and title.
/// Returns metadata if found, None if no results.
pub async fn search_track(artist: &str, title: &str) -> Result<Option<TrackMetadata>, anyhow::Error> {
    let spotify = get_spotify_client().await?;

    // Build search query with artist and track filters
    let query = format!("artist:{} track:{}", artist, title);

    let result = spotify
        .search(&query, SearchType::Track, None, None, Some(1), None)
        .await?;

    // Extract track from search results
    if let rspotify::model::SearchResult::Tracks(tracks) = result {
        if let Some(track) = tracks.items.into_iter().next() {
            let artist_name = track
                .artists
                .first()
                .map(|a| a.name.clone())
                .unwrap_or_else(|| artist.to_string());

            let album_name = track.album.name.clone();
            let track_title = track.name.clone();
            let track_number = track.track_number;

            let cover_url = track
                .album
                .images
                .first()
                .map(|img| img.url.clone());

            return Ok(Some(TrackMetadata {
                artist: artist_name,
                album: album_name,
                title: track_title,
                track_number,
                cover_url,
            }));
        }
    }

    Ok(None)
}
