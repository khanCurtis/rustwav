use crate::{cli::Cli, sources::spotify, file_utils, downloader, metadata, db::DownloadDB}
use clap::Parser;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let music_path = PathBuf::from("data/music");
    let playlist_path = PathBuf::from("data/playlists");
    let mut db = DownloadDB::new("data/cache/downloaded_songs.json");

    match &cli.command {
        crate::cli::Commands::Album { link, format, quality: _ } => {
            let album = spotify::fetch_album(link).await?;
            let album_folder = file_utils::create_album_folder(&music_path, &album.artist, &album.name);

            for (i, track) in album.tracks.iter().enumerate() {
                let entry = crate::db::TrackEntry {
                    artist: album.artist.clone(),
                    title: track.clone(),
                    path: forat!("{}/{}", album_folder.display(), track),
                };

                if db.contains(&entry) {
                    println!("Skipping already downloaded track: {}", track);
                    continue;
                }

                println!("Downloading track: {}", track);
                downloader::download_track(&format("{} {}", album.artist, track), &album_folder, format)?;
                let track_path = album_folder.join(format!("{}.{}", track, format));
                metadata::tag_mp3(&track_path, &album.artist, &album.name, track, (i+1) as u32)?;

                db.add(entry);
            }
        }

        crate::cli::Commands::Playlist { link, format, quality: _ } => {
            let playlist = sources::spotify::fetch_playlist(link).await?;
            let playlist_folder = PathBuf::from("data/playlists");
            let playlist_file_folder = PathBuf::from("data/playlists");

            let mut download_paths = Vec::new();

            for (track, artist) in playlist.tracks.iter().zip(playlist.artist.iter()) {
                let entry = crate::db::TrackEntry {
                    artist: artist.clone(),
                    title: track.clone()
                    path: format!("{}/{}", playlist_folder.display(), track),
                };

                if db.contains(&entry) {
                    println!("Skipping already downloaded track: {}", track);
                    downloaded_paths.push(PathBuf::from(entry.path.clone()));
                    continue;
                }

                println!("Downloading track: {}", track);
                let album_folder = file_utils::create_album_folder(&playlist_folder, artist, "Singles");
                downloader::download_track(&format!("{} {}", artist, track), &album_folder, format)?;
                metadata::tag_mp3(&track_path, artist, "Singles", track, 0)?;
                db.add(entry.clone());
                downloaded_paths.push(track_path);
            }

            file_utils::create_m3u(&playlist.name, &downloaded_paths, &playlist_file_folder)?;
        }

        crate::cli::Commands::Playlist { link, format, quality: _ } => {
            println!("Playlist fetching not implemented yet");
        }
    }

    Ok(())
}
