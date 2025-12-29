mod sources {
    pub mod models;
    pub mod spotify;
}
mod cli;
mod converter;
mod db;
mod downloader;
pub mod error_log;
mod file_utils;
mod metadata;
mod tui;

use crate::{
    cli::{Cli, PortableConfig},
    db::DownloadDB,
    error_log::{ErrorLogManager, ErrorType},
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
use tokio::sync::{mpsc, watch};

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
    let (pause_tx, pause_rx) = watch::channel(false);

    // Spawn the download worker
    let worker = DownloadWorker::new(download_rx, event_tx.clone(), pause_rx);
    tokio::spawn(async move {
        worker.run().await;
    });

    // Create app state with channels
    let mut app = App::new(download_tx, event_tx, event_rx, pause_tx);

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

        cli::Commands::Convert {
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

                                // Search Spotify for metadata
                                match tokio::runtime::Handle::current()
                                    .block_on(spotify::search_track(&artist, &title))
                                {
                                    Ok(Some(meta)) => {
                                        // Download cover art if available
                                        let cover_path = if let Some(url) = &meta.cover_url {
                                            let cover_file =
                                                new_path.with_file_name("temp_cover.jpg");
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
                                        println!(
                                            "  Could not find track on Spotify, keeping existing metadata"
                                        );
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

        cli::Commands::Cleanup { dry_run, verbose } => {
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

        cli::Commands::Retry {
            error_type,
            id,
            date,
            list,
            clear,
            clear_date,
        } => {
            let error_log = ErrorLogManager::new("data/errors");

            // Handle date filter - if specified, only show/retry errors from that date
            let date_filter = date.as_deref();

            // Handle clear operations first
            if *clear {
                match error_type.as_str() {
                    "download" => {
                        error_log.clear_error_type(ErrorType::Download);
                        println!("Cleared all download errors.");
                    }
                    "convert" => {
                        error_log.clear_error_type(ErrorType::Convert);
                        println!("Cleared all convert errors.");
                    }
                    "refresh" => {
                        error_log.clear_error_type(ErrorType::Refresh);
                        println!("Cleared all refresh errors.");
                    }
                    "all" => {
                        error_log.clear_all();
                        println!("Cleared all error logs.");
                    }
                    _ => {
                        anyhow::bail!("Unknown error type: {}. Use: download, convert, refresh, or all", error_type);
                    }
                }
                return Ok(());
            }

            if let Some(date_str) = clear_date {
                error_log.clear_date(date_str);
                println!("Cleared all errors from {}.", date_str);
                return Ok(());
            }

            // Handle list operation
            if *list {
                let dates = if let Some(d) = date_filter {
                    vec![d.to_string()]
                } else {
                    error_log.list_dates()
                };

                if dates.is_empty() {
                    println!("No errors logged.");
                    return Ok(());
                }

                let (download_total, convert_total, refresh_total) = if date_filter.is_some() {
                    error_log.get_error_counts(date_filter.unwrap())
                } else {
                    error_log.get_total_error_counts()
                };
                println!("Error Log Summary{}:",
                    if let Some(d) = date_filter { format!(" ({})", d) } else { String::new() });
                println!("  Download errors: {}", download_total);
                println!("  Convert errors:  {}", convert_total);
                println!("  Refresh errors:  {}", refresh_total);
                println!();

                for date_str in &dates {
                    let (d, c, r) = error_log.get_error_counts(date_str);
                    if d == 0 && c == 0 && r == 0 {
                        continue;
                    }

                    println!("=== {} ===", date_str);

                    // Show download errors
                    if (error_type == "all" || error_type == "download") && d > 0 {
                        println!("\n  Download Errors ({}):", d);
                        for entry in error_log.get_download_errors_for_date(date_str) {
                            let track_info = match (&entry.artist, &entry.title) {
                                (Some(a), Some(t)) => format!("{} - {}", a, t),
                                _ => entry.link_type.clone(),
                            };
                            println!("    [{}] {} (retries: {})",
                                &entry.id[..8], track_info, entry.retry_count);
                            println!("      Error: {}", entry.error);
                            println!("      Link: {}", entry.link);
                        }
                    }

                    // Show convert errors
                    if (error_type == "all" || error_type == "convert") && c > 0 {
                        println!("\n  Convert Errors ({}):", c);
                        for entry in error_log.get_convert_errors_for_date(date_str) {
                            println!("    [{}] {} - {} (retries: {})",
                                &entry.id[..8], entry.artist, entry.title, entry.retry_count);
                            println!("      Error: {}", entry.error);
                            println!("      Path: {}", entry.input_path);
                        }
                    }

                    // Show refresh errors
                    if (error_type == "all" || error_type == "refresh") && r > 0 {
                        println!("\n  Refresh Errors ({}):", r);
                        for entry in error_log.get_refresh_errors_for_date(date_str) {
                            println!("    [{}] {} - {} (retries: {})",
                                &entry.id[..8], entry.artist, entry.title, entry.retry_count);
                            println!("      Error: {}", entry.error);
                            println!("      Path: {}", entry.input_path);
                        }
                    }
                    println!();
                }
                return Ok(());
            }

            // Handle retry operation
            // For now, print a message - full retry implementation requires reusing download/convert logic
            let (download_total, convert_total, refresh_total) = error_log.get_total_error_counts();
            let total = download_total + convert_total + refresh_total;

            if total == 0 {
                println!("No errors to retry.");
                return Ok(());
            }

            if let Some(error_id) = id {
                // Retry specific error by ID
                println!("Retrying error: {}...", error_id);

                // Try to find the error in each log type
                if let Some((found_date, entry)) = error_log.get_download_error(error_id) {
                    println!("Found download error: {} - {:?}",
                        entry.artist.as_deref().unwrap_or("Unknown"),
                        entry.title.as_deref().unwrap_or("Unknown"));
                    println!("To retry, use the TUI (press 'e' for error logs) or re-run the original command:");
                    println!("  rustwav {} {}", entry.link_type, entry.link);
                    error_log.remove_download_error(&found_date, error_id);
                    return Ok(());
                }

                if let Some((found_date, entry)) = error_log.get_convert_error(error_id) {
                    println!("Found convert error: {} - {}", entry.artist, entry.title);
                    println!("Re-running conversion...");

                    // Actually retry the conversion
                    let input_path = std::path::Path::new(&entry.input_path);
                    if input_path.exists() {
                        match converter::convert_audio(input_path, &entry.target_format, &entry.quality, |msg| {
                            println!("  {}", msg);
                        }) {
                            Ok(new_path) => {
                                println!("Conversion successful: {}", new_path.display());
                                error_log.remove_convert_error(&found_date, error_id);
                                db.update_path(&entry.input_path, &new_path.display().to_string());
                            }
                            Err(e) => {
                                println!("Conversion failed again: {}", e);
                                error_log.increment_convert_retry(&found_date, error_id);
                            }
                        }
                    } else {
                        println!("Input file no longer exists: {}", entry.input_path);
                        error_log.remove_convert_error(&found_date, error_id);
                    }
                    return Ok(());
                }

                if let Some((found_date, entry)) = error_log.get_refresh_error(error_id) {
                    println!("Found refresh error: {} - {}", entry.artist, entry.title);
                    println!("Re-running metadata refresh...");

                    let input_path = std::path::Path::new(&entry.input_path);
                    if input_path.exists() {
                        match spotify::search_track(&entry.artist, &entry.title).await {
                            Ok(Some(meta)) => {
                                let cover_path = if let Some(url) = &meta.cover_url {
                                    let cover_file = input_path.with_file_name("temp_cover.jpg");
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

                                if let Err(e) = metadata::tag_audio(
                                    input_path,
                                    &meta.artist,
                                    &meta.album,
                                    &meta.title,
                                    meta.track_number,
                                    cover_path.as_deref(),
                                    &config,
                                ) {
                                    println!("Failed to apply metadata: {}", e);
                                    error_log.increment_refresh_retry(&found_date, error_id);
                                } else {
                                    println!("Metadata refreshed successfully!");
                                    error_log.remove_refresh_error(&found_date, error_id);
                                }

                                if let Some(cover) = cover_path {
                                    let _ = std::fs::remove_file(cover);
                                }
                            }
                            Ok(None) => {
                                println!("Track not found on Spotify.");
                                error_log.increment_refresh_retry(&found_date, error_id);
                            }
                            Err(e) => {
                                println!("Spotify search failed: {}", e);
                                error_log.increment_refresh_retry(&found_date, error_id);
                            }
                        }
                    } else {
                        println!("Input file no longer exists: {}", entry.input_path);
                        error_log.remove_refresh_error(&found_date, error_id);
                    }
                    return Ok(());
                }

                println!("Error ID not found: {}", error_id);
                return Ok(());
            }

            // No specific ID - show summary and suggest using TUI or --id
            println!("Found {} error(s) to retry:", total);
            println!("  Download: {}", download_total);
            println!("  Convert:  {}", convert_total);
            println!("  Refresh:  {}", refresh_total);
            println!();
            println!("To retry specific errors:");
            println!("  rustwav retry --list                # List all errors with IDs");
            println!("  rustwav retry --id <error-id>       # Retry specific error");
            println!("  rustwav retry --date 2025-12-29     # Retry all errors from a date");
            println!();
            println!("Or use the TUI (press 'e' for error logs view).");
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
