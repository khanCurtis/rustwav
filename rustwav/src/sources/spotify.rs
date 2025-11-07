use anyhow::Result;
use rspotify::{AuthCodeSpotify, clients::SpotifyBuilder, model::AlbumId};
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

pub async fn fetch_playlist(_link: &str) -> Result<()> {
    //Placeholder
    println!("Playlist fetching not implemented yet");
    Ok(())
}
