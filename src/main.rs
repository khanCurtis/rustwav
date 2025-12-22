mod sources {
    pub mod spotify;
    pub mod models;
}
mod downloader;
mod metadata;
mod file_utils;
mod db;
mod cli;
mod logger;

use crate::{
    cli::{Cli, HeadlessConfig, PortableConfig},
    db::DownloadDB,
    logger::Logger,
    sources::spotify,
};
use clap::Parser;
use rspotify::model::PlayableItem;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let portable_config = PortableConfig::from_cli(&cli);
    let headless_config = HeadlessConfig::from_cli(&cli);
    let log = Logger::new(headless_config.clone());

    if portable_config.enabled {
        log.info("[portable mode] MP3 only, FAT32-safe names, shallow folders, small covers");
    }
    if headless_config.enabled {
        log.debug("[headless mode] Script-friendly output enabled");
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
            let actual_format = if portable_config.enabled { "mp3".to_string() } else { format.clone() };

            let album = spotify::fetch_album(&link).await?;
            let main_artist = album
                .artists
                .get(0)
                .and_then(|a| a.name.clone().into())
                .unwrap_or_else(|| "Unknown Artist".to_string());
            let album_name = album.name.clone();

            log.info(&format!("Fetching album: {} by {}", album_name, main_artist));

            let album_folder = if portable_config.enabled {
                file_utils::create_portable_folder(&music_path, &portable_config)
            } else {
                file_utils::create_album_folder(&music_path, &main_artist, &album_name)
            };

            let cover_path: Option<std::path::PathBuf> = {
                if let Some(image) = album.images.get(0) {
                    let p = album_folder.join("cover.jpg");
                    if !p.exists() {
                        log.debug("Downloading cover art...");
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

            let total_tracks = album.tracks.items.len();
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
                    &portable_config,
                );

                let file_path = album_folder.join(&safe_file_name);

                let entry = crate::db::TrackEntry {
                    artist: track_artist.clone(),
                    title: track_title.clone(),
                    path: file_path.display().to_string(),
                };

                if db.contains(&entry) {
                    log.track_skip(&track_artist, &track_title);
                    continue;
                }

                log.track_start(&track_artist, &track_title);
                log.debug(&format!("Track {}/{}", i + 1, total_tracks));

                let query = format!("{} {}", track_artist, track_title);
                let album_folder_clone = album_folder.clone();
                let format_clone = actual_format.clone();
                let query_clone = query.clone();

                match tokio::task::spawn_blocking(move || {
                    downloader::download_track(&query_clone, &album_folder_clone, &format_clone)
                }).await? {
                    Ok(_) => {
                        metadata::tag_mp3(
                            &file_path,
                            &track_artist,
                            &album_name,
                            &track_title,
                            (i + 1) as u32,
                            cover_path.as_deref(),
                            &portable_config,
                        )?;
                        log.track_complete(&track_artist, &track_title, &file_path.display().to_string());
                        db.add(entry);
                    }
                    Err(e) => {
                        log.error(&format!("Failed to download {} — {}: {}", track_artist, track_title, e));
                    }
                }
            }

            log.album_complete(&album_name, &main_artist, total_tracks);
        }

        crate::cli::Commands::Playlist { link, format, quality: _ } => {
            let actual_format = if portable_config.enabled { "mp3".to_string() } else { format.clone() };

            let playlist = spotify::fetch_playlist(&link).await?;
            let playlist_name = playlist.name.clone();

            log.info(&format!("Fetching playlist: {}", playlist_name));

            std::fs::create_dir_all(&playlist_path)?;
            let mut downloaded_paths: Vec<PathBuf> = Vec::new();
            let mut track_count = 0;

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

                let output_folder = if portable_config.enabled {
                    file_utils::create_portable_folder(&playlist_path, &portable_config)
                } else {
                    file_utils::create_album_folder(&playlist_path, &track_artist, "Singles")
                };

                let safe_file_name = file_utils::build_filename(
                    &track_artist,
                    &track_title,
                    &actual_format,
                    &portable_config,
                );
                let file_path = output_folder.join(&safe_file_name);

                let entry = crate::db::TrackEntry {
                    artist: track_artist.clone(),
                    title: track_title.clone(),
                    path: file_path.display().to_string(),
                };

                if db.contains(&entry) {
                    log.track_skip(&track_artist, &track_title);
                    downloaded_paths.push(std::path::PathBuf::from(entry.path.clone()));
                    continue;
                }

                log.track_start(&track_artist, &track_title);

                let query = format!("{} {}", track_artist, track_title);
                let folder_clone = output_folder.clone();
                let format_clone = actual_format.clone();
                let query_clone = query.clone();

                match tokio::task::spawn_blocking(move || {
                    downloader::download_track(&query_clone, &folder_clone, &format_clone)
                }).await? {
                    Ok(_) => {
                        metadata::tag_mp3(
                            &file_path,
                            &track_artist,
                            "Singles",
                            &track_title,
                            0,
                            None,
                            &portable_config,
                        )?;
                        log.track_complete(&track_artist, &track_title, &file_path.display().to_string());
                        db.add(entry.clone());
                        downloaded_paths.push(file_path);
                        track_count += 1;
                    }
                    Err(e) => {
                        log.error(&format!("Failed to download {} — {}: {}", track_artist, track_title, e));
                    }
                }
            }

            file_utils::create_m3u(&playlist_name, &downloaded_paths, &playlist_path)?;
            log.playlist_complete(&playlist_name, track_count);
        }
    }

    Ok(())
} 
