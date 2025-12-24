mod sources {
    pub mod models;
    pub mod spotify;
}
mod cli;
mod db;
mod downloader;
mod file_utils;
mod metadata;
mod tui;

use crate::{
    cli::{Cli, PortableConfig},
    db::DownloadDB,
    sources::spotify,
    tui::{App, DownloadWorker},
};
use clap::Parser;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use rspotify::model::PlayableItem;
use std::io::stdout;
use std::path::PathBuf;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if present (won't error if missing)
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    match &cli.command {
        Some(cmd) => run_cli(cmd, &cli).await,
        None => run_tui().await,
    }
}

async fn run_tui() -> anyhow::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create channels for download communication
    let (download_tx, download_rx) = mpsc::channel(32);
    let (event_tx, event_rx) = mpsc::channel(32);

    // Spawn the download worker
    let worker = DownloadWorker::new(download_rx, event_tx);
    tokio::spawn(async move {
        worker.run().await;
    });

    // Create app state with channels
    let mut app = App::new(download_tx, event_rx);

    // Main loop
    while app.running {
        // Process any pending download events
        app.process_events();

        terminal.draw(|frame| tui::ui::draw(frame, &app))?;
        tui::event::handle_events(&mut app)?;
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    Ok(())
}

async fn run_cli(command: &cli::Commands, cli_args: &Cli) -> anyhow::Result<()> {
    let config = PortableConfig::from_cli(cli_args);

    if config.enabled {
        println!("[portable mode] MP3 only, FAT32-safe names, shallow folders, small covers");
    }

    let music_path = PathBuf::from("data/music");
    let playlist_path = PathBuf::from("data/playlists");
    let cache_path = "data/cache/downloaded_songs.json";

    std::fs::create_dir_all(&music_path)?;
    std::fs::create_dir_all(&playlist_path)?;
    std::fs::create_dir_all(std::path::Path::new("data/cache"))?;

    let mut db = DownloadDB::new(cache_path);

    match command {
        cli::Commands::Album {
            link,
            format,
            quality: _,
        } => {
            let actual_format = if config.enabled {
                "mp3".to_string()
            } else {
                format.clone()
            };

            let album = spotify::fetch_album(link).await?;
            let main_artist = album
                .artists
                .first()
                .and_then(|a| a.name.clone().into())
                .unwrap_or_else(|| "Unknown Artist".to_string());
            let album_name = album.name.clone();

            let album_folder = if config.enabled {
                file_utils::create_portable_folder(&music_path, &config)
            } else {
                file_utils::create_album_folder(&music_path, &main_artist, &album_name)
            };

            let cover_path: Option<std::path::PathBuf> = {
                if let Some(image) = album.images.first() {
                    let p = album_folder.join("cover.jpg");
                    if !p.exists() {
                        if let Ok(response) = reqwest::blocking::get(&image.url) {
                            if let Ok(bytes) = response.bytes() {
                                let _ = std::fs::write(&p, &bytes);
                            }
                        }
                    }
                    if p.exists() {
                        Some(p)
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            for (i, track) in album.tracks.items.iter().enumerate() {
                let track_title = track.name.clone();
                let track_artist = track
                    .artists
                    .first()
                    .and_then(|a| a.name.clone().into())
                    .unwrap_or_else(|| main_artist.clone());

                let safe_file_name = file_utils::build_filename(
                    &track_artist,
                    &track_title,
                    &actual_format,
                    &config,
                );

                let file_path = album_folder.join(&safe_file_name);

                let entry = db::TrackEntry {
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

                let file_path_clone = file_path.clone();
                let format_clone = actual_format.clone();
                let query_clone = query.clone();
                tokio::task::spawn_blocking(move || {
                    downloader::download_track(&query_clone, &file_path_clone, &format_clone)
                })
                .await??;

                metadata::tag_audio(
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

        cli::Commands::Playlist {
            link,
            format,
            quality: _,
        } => {
            let actual_format = if config.enabled {
                "mp3".to_string()
            } else {
                format.clone()
            };

            let playlist = spotify::fetch_playlist(link).await?;
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
                            .first()
                            .map(|a| a.name.clone())
                            .unwrap_or_else(|| "Unknown Artist".to_string());
                        (title, artist)
                    }
                    PlayableItem::Episode(_) => continue,
                };

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

                let entry = db::TrackEntry {
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
                let file_path_clone = file_path.clone();
                let format_clone = actual_format.clone();
                let query_clone = query.clone();
                tokio::task::spawn_blocking(move || {
                    downloader::download_track(&query_clone, &file_path_clone, &format_clone)
                })
                .await??;

                metadata::tag_audio(
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
            println!(
                "Playlist '{}' with {} tracks finished.",
                playlist_name,
                downloaded_paths.len()
            );
        }
    }

    Ok(())
}
