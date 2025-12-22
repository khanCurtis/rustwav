mod sources {
    pub mod spotify;
    pub mod models;
}
mod downloader;
mod metadata;
mod file_utils;
mod db;
mod cli;

use crate::{cli::{Cli, PortableConfig}, db::DownloadDB, sources::spotify};
use clap::Parser;
use rspotify::model::PlayableItem;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = PortableConfig::from_cli(&cli);

    if config.enabled {
        println!("[portable mode] MP3 only, FAT32-safe names, shallow folders, small covers");
    }

    // Base paths
    let music_path = PathBuf::from("data/music");
    let playlist_path = PathBuf::from("data/playlists");
    let cache_path = "data/cache/downloaded_songs.json";

    // Ensure runtime directories exist
    std::fs::create_dir_all(&music_path)?;
    std::fs::create_dir_all(&playlist_path)?;
    std::fs::create_dir_all(std::path::Path::new("data/cache"))?;

    let mut db = DownloadDB::new(cache_path);

    match &cli.command {
        crate::cli::Commands::Album { link, format, quality: _ } => {
            // Portable mode forces MP3
            let actual_format = if config.enabled { "mp3".to_string() } else { format.clone() };

            let album = spotify::fetch_album(&link).await?;
            let main_artist = album
                .artists
                .get(0)
                .and_then(|a| a.name.clone().into())
                .unwrap_or_else(|| "Unknown Artist".to_string());
            let album_name = album.name.clone();

            // Portable: shallow folders (just music_path), Normal: artist/album nesting
            let album_folder = if config.enabled {
                file_utils::create_portable_folder(&music_path, &config)
            } else {
                file_utils::create_album_folder(&music_path, &main_artist, &album_name)
            };

            let cover_path: Option<std::path::PathBuf> = {
                if let Some(image) = album.images.get(0) {
                    let p = album_folder.join("cover.jpg");
                    if !p.exists() {
                        if let Ok(response) = reqwest::blocking::get(&image.url) {
                            if let Ok(bytes) = response.bytes() {
                                let _ = std::fs::write(&p, &bytes);
                            }
                        }
                    }
                    if p.exists() { Some(p) } else { None }
                } else {
                    None
                }
            };

            for (i, track) in album.tracks.items.iter().enumerate() {
                let track_title = track.name.clone();
                let track_artist = track
                    .artists
                    .get(0)
                    .and_then(|a| a.name.clone().into())
                    .unwrap_or_else(|| main_artist.clone());

                let safe_file_name = file_utils::build_filename(
                    &track_artist,
                    &track_title,
                    &actual_format,
                    &config,
                );

                let file_path = album_folder.join(&safe_file_name);

                let entry = crate::db::TrackEntry {
                    artist: track_artist.clone(),
                    title: track_title.clone(),
                    path: file_path.display().to_string(),
                };

                if db.contains(&entry) {
                    println!("Skipping: {} — {}", track_artist, track_title);
                    continue;
                }

                println!("Downloading: {} — {}", track_artist, track_title);
                let query = format!("{} {}", track_artist, track_title);

                let album_folder_clone = album_folder.clone();
                let format_clone = actual_format.clone();
                let query_clone = query.clone();
                tokio::task::spawn_blocking(move || {
                    downloader::download_track(&query_clone, &album_folder_clone, &format_clone)
                }).await??;

                metadata::tag_mp3(
                    &file_path,
                    &track_artist,
                    &album_name,
                    &track_title,
                    (i + 1) as u32,
                    cover_path.as_deref(),
                    &config,
                )?;

                db.add(entry);
            }

            println!("Album '{}' by {} finished.", album_name, main_artist);
        }

        crate::cli::Commands::Playlist { link, format, quality: _ } => {
            let actual_format = if config.enabled { "mp3".to_string() } else { format.clone() };

            let playlist = spotify::fetch_playlist(&link).await?;
            let playlist_name = playlist.name.clone();

            std::fs::create_dir_all(&playlist_path)?;
            let mut downloaded_paths: Vec<PathBuf> = Vec::new();

            for item in playlist.tracks.items.iter() {
                let track_obj = match &item.track {
                    Some(t) => t,
                    None => continue,
                };

                let (track_title, track_artist) = match track_obj {
                    PlayableItem::Track(track) => {
                        let title = track.name.clone();
                        let artist = track
                            .artists
                            .get(0)
                            .map(|a| a.name.clone())
                            .unwrap_or_else(|| "Unknown Artist".to_string());
                        (title, artist)
                    }
                    PlayableItem::Episode(_) => continue,
                };

                // Portable: shallow folders, Normal: artist/Singles nesting
                let output_folder = if config.enabled {
                    file_utils::create_portable_folder(&playlist_path, &config)
                } else {
                    file_utils::create_album_folder(&playlist_path, &track_artist, "Singles")
                };

                let safe_file_name = file_utils::build_filename(
                    &track_artist,
                    &track_title,
                    &actual_format,
                    &config,
                );
                let file_path = output_folder.join(&safe_file_name);

                let entry = crate::db::TrackEntry {
                    artist: track_artist.clone(),
                    title: track_title.clone(),
                    path: file_path.display().to_string(),
                };

                if db.contains(&entry) {
                    println!("Skipping: {} — {}", track_artist, track_title);
                    downloaded_paths.push(std::path::PathBuf::from(entry.path.clone()));
                    continue;
                }

                println!("Downloading: {} — {}", track_artist, track_title);
                let query = format!("{} {}", track_artist, track_title);
                let folder_clone = output_folder.clone();
                let format_clone = actual_format.clone();
                let query_clone = query.clone();
                tokio::task::spawn_blocking(move || {
                    downloader::download_track(&query_clone, &folder_clone, &format_clone)
                })
                .await??;

                metadata::tag_mp3(
                    &file_path,
                    &track_artist,
                    "Singles",
                    &track_title,
                    0,
                    None,
                    &config,
                )?;
                db.add(entry.clone());
                downloaded_paths.push(file_path);
            }

            file_utils::create_m3u(&playlist_name, &downloaded_paths, &playlist_path)?;
            println!("Playlist '{}' with {} tracks finished.", playlist_name, downloaded_paths.len());
        }
    }

    Ok(())
} 
