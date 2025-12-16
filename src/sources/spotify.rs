use anyhow::Result;
use rspotify::clients::{AuthCodeSpotify, BaseClient};
use rspotify::model::{
    FullAlbum, FullPlaylist, AlbumId, PlaylistId, SimplifiedTrack, Market,
};

pub async fn fetch_album(link: &str) -> Result<FullAlbum, anyhow::Error> {
    let album_id = AlbumId::from_id(link)?;
    let spotify = AuthCodeSpotify::default();
    let album = spotify.album(album_id, None).await?;
    Ok(album)
}

pub async fn fetch_playlist(link: &str) -> Result<FullPlaylist, anyhow::Error> {
    let playlist_id = PlaylistId::from_id(link)?;
    let spotify = AuthCodeSpotify::default();
    let playlist = spotify.playlist(playlist_id, None, None).await?;
    Ok(playlist)
}

