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
            println!("Playlist fetching not implemented yet");
        }
    }

    Ok(())
}
