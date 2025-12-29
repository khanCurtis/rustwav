mod sources {
    pub mod spotify;
    pub mod models;
}
mod converter;
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

        crate::cli::Commands::Convert {
            input,
            to,
            quality,
            refresh_metadata,
            recursive,
        } => {
            // Check FFmpeg availability
            if !converter::check_ffmpeg_available() {
                anyhow::bail!("FFmpeg is not installed or not in PATH. Please install FFmpeg to use the converter.");
            }

            // Validate target format
            if !converter::is_supported_format(to) {
                anyhow::bail!(
                    "Unsupported format: {}. Supported formats: {:?}",
                    to,
                    converter::SUPPORTED_FORMATS
                );
            }

            // Collect files to convert
            let input_path = std::path::Path::new(input);
            let files: Vec<PathBuf> = if input_path.is_file() {
                vec![input_path.to_path_buf()]
            } else if input_path.is_dir() {
                collect_audio_files(input_path, *recursive)?
            } else {
                anyhow::bail!("Input path does not exist: {}", input);
            };

            if files.is_empty() {
                println!("No audio files found to convert.");
                return Ok(());
            }

            println!("Found {} file(s) to convert to {}", files.len(), to);

            let mut converted_count = 0;
            let mut failed_count = 0;

            for file_path in &files {
                let current_format = converter::get_format_from_path(file_path);
                if current_format.as_deref() == Some(to.as_str()) {
                    println!("Skipping {} (already in {} format)", file_path.display(), to);
                    continue;
                }

                println!("\nConverting: {}", file_path.display());

                // Convert the file
                let result = converter::convert_audio(file_path, to, quality, |msg| {
                    println!("  {}", msg);
                });

                match result {
                    Ok(new_path) => {
                        converted_count += 1;

                        // Refresh metadata from Spotify if requested
                        if *refresh_metadata {
                            if let Some(entry) = db.find_by_path(&file_path.display().to_string()) {
                                let artist = entry.artist.clone();
                                let title = entry.title.clone();

                                println!("  Refreshing metadata for: {} - {}", artist, title);

                                match tokio::runtime::Handle::current()
                                    .block_on(spotify::search_track(&artist, &title))
                                {
                                    Ok(Some(meta)) => {
                                        // Download cover art if available
                                        let cover_path = if let Some(url) = &meta.cover_url {
                                            let cover_file = new_path.with_file_name("temp_cover.jpg");
                                            if let Ok(response) = reqwest::blocking::get(url) {
                                                if let Ok(bytes) = response.bytes() {
                                                    let _ = std::fs::write(&cover_file, &bytes);
                                                    Some(cover_file)
                                                } else {
                                                    None
                                                }
                                            } else {
                                                None
                                            }
                                        } else {
                                            None
                                        };

                                        // Apply metadata
                                        if let Err(e) = metadata::tag_audio(
                                            &new_path,
                                            &meta.artist,
                                            &meta.album,
                                            &meta.title,
                                            meta.track_number,
                                            cover_path.as_deref(),
                                            &config,
                                        ) {
                                            println!("  Warning: Failed to apply metadata: {}", e);
                                        } else {
                                            println!("  Metadata refreshed successfully");
                                        }

                                        // Clean up temp cover
                                        if let Some(cover) = cover_path {
                                            let _ = std::fs::remove_file(cover);
                                        }
                                    }
                                    Ok(None) => {
                                        println!("  Could not find track on Spotify, keeping existing metadata");
                                    }
                                    Err(e) => {
                                        println!("  Spotify search failed: {}", e);
                                    }
                                }
                            }
                        }

                        // Update database with new path
                        let old_path_str = file_path.display().to_string();
                        let new_path_str = new_path.display().to_string();
                        db.update_path(&old_path_str, &new_path_str);

                        // Prompt for deletion
                        print!("  Delete original file? [y/N]: ");
                        use std::io::Write;
                        std::io::stdout().flush()?;

                        let mut response = String::new();
                        std::io::stdin().read_line(&mut response)?;

                        if response.trim().eq_ignore_ascii_case("y") {
                            match converter::delete_file(file_path) {
                                Ok(()) => println!("  Original deleted."),
                                Err(e) => println!("  Failed to delete original: {}", e),
                            }
                        } else {
                            println!("  Original kept.");
                        }
                    }
                    Err(e) => {
                        failed_count += 1;
                        println!("  Error: {}", e);
                    }
                }
            }

            println!(
                "\nConversion complete: {} succeeded, {} failed",
                converted_count, failed_count
            );
        }

        crate::cli::Commands::Cleanup { dry_run, verbose } => {
            println!("Scanning download database for missing files...\n");

            if *dry_run {
                // Dry run: show what would be removed without actually removing
                let missing: Vec<_> = db
                    .all_tracks()
                    .into_iter()
                    .filter(|entry| !std::path::Path::new(&entry.path).exists())
                    .collect();

                if missing.is_empty() {
                    println!("Database is clean. All {} entries point to existing files.", db.all_tracks().len());
                } else {
                    println!("Would remove {} entries (files no longer exist):\n", missing.len());
                    for entry in &missing {
                        println!("  {} - {}", entry.artist, entry.title);
                        if *verbose {
                            println!("    Path: {}", entry.path);
                        }
                    }
                    println!("\nRun without --dry-run to remove these entries.");
                }
            } else {
                // Collect entries to show before cleanup if verbose
                let missing_entries: Vec<_> = if *verbose {
                    db.all_tracks()
                        .into_iter()
                        .filter(|entry| !std::path::Path::new(&entry.path).exists())
                        .cloned()
                        .collect()
                } else {
                    Vec::new()
                };

                // Actually perform cleanup
                let (removed, total_before) = db.cleanup();

                if removed == 0 {
                    println!("Database is clean. All {} entries point to existing files.", total_before);
                } else {
                    if *verbose {
                        println!("Removed {} entries:\n", removed);
                        for entry in &missing_entries {
                            println!("  {} - {}", entry.artist, entry.title);
                            println!("    Path: {}", entry.path);
                        }
                        println!();
                    }
                    println!(
                        "Cleanup complete: removed {} of {} entries. {} entries remaining.",
                        removed,
                        total_before,
                        total_before - removed
                    );
                }
            }
        }
    }

    Ok(())
}

/// Collect audio files from a directory, optionally recursively
fn collect_audio_files(dir: &std::path::Path, recursive: bool) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let extensions = ["mp3", "flac", "wav", "aac", "m4a"];

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if extensions.contains(&ext.to_lowercase().as_str()) {
                    files.push(path);
                }
            }
        } else if path.is_dir() && recursive {
            files.extend(collect_audio_files(&path, recursive)?);
        }
    }

    Ok(files)
} 
