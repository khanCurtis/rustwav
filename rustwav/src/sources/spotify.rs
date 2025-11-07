use anyhow::Result;
use rspotify::{AuthCodeSpotify, clients::SpotifyBuilder, model::{AlbumId, PlaylstId}};
use std::env;

pub async fn fetch_album(link: &str) -> Result<()> {
    // Initialize Spotify client
    let spotify = AuthCodeSpotify::default()
        .client_credentials_manager()
        .build();

    // Extract album ID from link
    let album_id = link
        .split("/album/")
        .nth(1)
        .and_then(|s| s.split('?').next())
        .ok_or_else(|| anyhow::anyhow!("Invalid album link"))?;

    let album = spotify.album(AlbumId::from_id(album_id)?).await?;

    println!("Album: {}", album.name);
    println!(
        "Artist(s): {:?}",
        album.artists.iter().map(|a| &a.name).collect::<Vec<_>>()
    );
    println!("Tracks:");
    for track in album.tracks.items.iter() {
        println!(" - {} ({:?})", track.name, track.id);
    }

    Ok(())
}

pub struct Playlist {
    pub name: String,
    pub tracks: Vec<String>, // track titles
    pub artist: Vec<String>, // artist name(s)
}

pub async fn fetch_playlist(link: &str) -> Result<(Playlist)> {
    let spotify = AuthCodeSpotify::default().client_credentials_manager().build();

    // Extract playlist ID from link
    let playlist_id = link.split("/playlist/").nth(1)
        .and_then(|s| s.split('?').next())
        .ok_or_else(|| anyhow::anyhow!("Invalid playlist link"))?;

    let playlist_data = spotify.playlist(PlaylistId::from_id(playlist_id)?).await?;

    let tracks = playlist_data.tracks.items.iter()
        .filter_map(|item| item.track.as_ref())
        .map(|track| track.name.clone())
        .collect::<Vec<_>>();

    let artists = playlist_data.tracks_items.iter()
        .filter_map(|item| item.track.as_ref())
        .map(|track| track.name.clone())
        .collect::Vec<_>>();

    Ok(Playlist {
        name: playlist_data.name,
        tracks,
        artist: artist,
    })
}
