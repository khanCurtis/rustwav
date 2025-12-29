use std::path::PathBuf;
use tokio::sync::{mpsc, watch};

use crate::{
    cli::PortableConfig,
    converter,
    db::{DownloadDB, TrackEntry},
    downloader,
    error_log::{ConvertErrorEntry, DownloadErrorEntry, ErrorLogManager, RefreshErrorEntry},
    file_utils, metadata,
    sources::{spotify, youtube},
};

#[derive(Debug, Clone)]
pub struct ConvertTrackInfo {
    pub input_path: String,
    pub artist: String,
    pub title: String,
}

#[derive(Debug, Clone)]
pub enum DownloadRequest {
    Album {
        id: usize,
        link: String,
        portable: bool,
        format: String,
        quality: String,
    },
    Playlist {
        id: usize,
        link: String,
        portable: bool,
        format: String,
        quality: String,
    },
    YouTubePlaylist {
        id: usize,
        link: String,
        portable: bool,
        format: String,
        quality: String,
    },
    Convert {
        id: usize,
        input_path: String,
        target_format: String,
        quality: String,
        refresh_metadata: bool,
        artist: String,
        title: String,
    },
    ConvertBatch {
        id: usize,
        tracks: Vec<ConvertTrackInfo>,
        target_format: String,
        quality: String,
        refresh_metadata: bool,
    },
    RefreshMetadata {
        id: usize,
        input_path: String,
        artist: String,
        title: String,
    },
    RefreshMetadataBatch {
        id: usize,
        tracks: Vec<ConvertTrackInfo>,
    },
}

#[derive(Debug, Clone)]
pub enum DownloadEvent {
    /// Update name while still fetching (before we know track count)
    MetadataFetched {
        id: usize,
        name: String,
    },
    Started {
        id: usize,
        name: String,
        total_tracks: usize,
    },
    #[allow(dead_code)]
    TrackStarted {
        id: usize,
        artist: String,
        title: String,
        track_num: usize,
    },
    TrackComplete {
        id: usize,
        artist: String,
        title: String,
        path: String,
    },
    TrackSkipped {
        id: usize,
        artist: String,
        title: String,
    },
    TrackFailed {
        id: usize,
        artist: String,
        title: String,
        error: String,
    },
    Complete {
        id: usize,
        name: String,
    },
    Error {
        id: usize,
        error: String,
    },
    LogLine {
        id: usize,
        line: String,
    },
    /// M3U generation completed
    M3UGenerated {
        result: String,
    },
    /// M3U confirmation needed (some tracks missing)
    M3UConfirm {
        name: String,
        found: usize,
        missing: usize,
        paths: Vec<std::path::PathBuf>,
    },
    /// Conversion started
    ConvertStarted {
        id: usize,
        path: String,
        target_format: String,
    },
    /// Conversion complete
    ConvertComplete {
        id: usize,
        old_path: String,
        new_path: String,
    },
    /// Conversion failed
    ConvertFailed {
        id: usize,
        path: String,
        error: String,
    },
    /// Ask user to confirm deletion of original (single file)
    ConvertDeleteConfirm {
        id: usize,
        old_path: String,
        new_path: String,
    },
    /// Ask user to confirm deletion of all originals (batch conversion)
    ConvertBatchDeleteConfirm {
        converted_files: Vec<(String, String)>, // Vec of (old_path, new_path)
    },
    /// Batch conversion complete
    ConvertBatchComplete {
        id: usize,
        total: usize,
        successful: usize,
    },
    /// Metadata refresh started
    RefreshStarted {
        id: usize,
        artist: String,
        title: String,
    },
    /// Metadata refresh complete
    RefreshComplete {
        id: usize,
        artist: String,
        title: String,
    },
    /// Metadata refresh failed
    RefreshFailed {
        id: usize,
        artist: String,
        title: String,
        error: String,
    },
    /// Batch metadata refresh complete
    RefreshBatchComplete {
        id: usize,
        total: usize,
        successful: usize,
    },
}

pub struct DownloadWorker {
    rx: mpsc::Receiver<DownloadRequest>,
    tx: mpsc::Sender<DownloadEvent>,
    pause_rx: watch::Receiver<bool>,
    music_path: PathBuf,
    playlist_path: PathBuf,
    db: DownloadDB,
    error_log: ErrorLogManager,
}

impl DownloadWorker {
    pub fn new(
        rx: mpsc::Receiver<DownloadRequest>,
        tx: mpsc::Sender<DownloadEvent>,
        pause_rx: watch::Receiver<bool>,
    ) -> Self {
        let music_path = PathBuf::from("data/music");
        let playlist_path = PathBuf::from("data/playlists");
        let _ = std::fs::create_dir_all(&music_path);
        let _ = std::fs::create_dir_all(&playlist_path);
        let _ = std::fs::create_dir_all("data/cache");

        Self {
            rx,
            tx,
            pause_rx,
            music_path,
            playlist_path,
            db: DownloadDB::new("data/cache/downloaded_songs.json"),
            error_log: ErrorLogManager::new("data/errors"),
        }
    }

    pub async fn run(mut self) {
        while let Some(request) = self.rx.recv().await {
            match request {
                DownloadRequest::Album {
                    id,
                    link,
                    portable,
                    format,
                    quality,
                } => {
                    self.process_album(id, &link, portable, &format, &quality)
                        .await;
                }
                DownloadRequest::Playlist {
                    id,
                    link,
                    portable,
                    format,
                    quality,
                } => {
                    self.process_playlist(id, &link, portable, &format, &quality)
                        .await;
                }
                DownloadRequest::YouTubePlaylist {
                    id,
                    link,
                    portable,
                    format,
                    quality,
                } => {
                    self.process_youtube_playlist(id, &link, portable, &format, &quality)
                        .await;
                }
                DownloadRequest::Convert {
                    id,
                    input_path,
                    target_format,
                    quality,
                    refresh_metadata,
                    artist,
                    title,
                } => {
                    self.process_convert(
                        id,
                        &input_path,
                        &target_format,
                        &quality,
                        refresh_metadata,
                        &artist,
                        &title,
                    )
                    .await;
                }
                DownloadRequest::ConvertBatch {
                    id,
                    tracks,
                    target_format,
                    quality,
                    refresh_metadata,
                } => {
                    self.process_convert_batch(
                        id,
                        tracks,
                        &target_format,
                        &quality,
                        refresh_metadata,
                    )
                    .await;
                }
                DownloadRequest::RefreshMetadata {
                    id,
                    input_path,
                    artist,
                    title,
                } => {
                    self.process_refresh_metadata(id, &input_path, &artist, &title)
                        .await;
                }
                DownloadRequest::RefreshMetadataBatch { id, tracks } => {
                    self.process_refresh_metadata_batch(id, tracks).await;
                }
            }
        }
    }

    async fn send_log(&self, id: usize, line: String) {
        let _ = self.tx.send(DownloadEvent::LogLine { id, line }).await;
    }

    /// Format error message with hint if it looks like a 404/not found error
    fn format_error_with_hint(error: &anyhow::Error, item_type: &str) -> String {
        let error_str = error.to_string().to_lowercase();
        if error_str.contains("404") || error_str.contains("not found") {
            format!("{} (Hint: Is the {} private? Only public {}s can be downloaded)",
                error, item_type, item_type)
        } else {
            error.to_string()
        }
    }

    /// Download cover art from a URL to a file path, with proper error logging
    async fn download_cover_art(&self, id: usize, url: &str, dest: &std::path::Path) -> Option<PathBuf> {
        match reqwest::get(url).await {
            Ok(response) => {
                if !response.status().is_success() {
                    self.send_log(
                        id,
                        format!("Cover art download failed: HTTP {}", response.status()),
                    )
                    .await;
                    return None;
                }
                match response.bytes().await {
                    Ok(bytes) => {
                        if let Err(e) = std::fs::write(dest, &bytes) {
                            self.send_log(
                                id,
                                format!("Failed to save cover art: {}", e),
                            )
                            .await;
                            return None;
                        }
                        Some(dest.to_path_buf())
                    }
                    Err(e) => {
                        self.send_log(
                            id,
                            format!("Failed to read cover art response: {}", e),
                        )
                        .await;
                        None
                    }
                }
            }
            Err(e) => {
                self.send_log(
                    id,
                    format!("Cover art download failed: {}", e),
                )
                .await;
                None
            }
        }
    }

    async fn process_album(
        &mut self,
        id: usize,
        link: &str,
        portable: bool,
        format: &str,
        quality: &str,
    ) {
        let config = if portable {
            PortableConfig {
                enabled: true,
                max_cover_dim: 128,
                max_cover_bytes: 64 * 1024,
                max_filename_len: 64,
            }
        } else {
            PortableConfig {
                enabled: false,
                max_cover_dim: 500,
                max_cover_bytes: 300 * 1024,
                max_filename_len: 100,
            }
        };

        // Use mp3 for portable mode, otherwise use selected format
        let actual_format = if portable { "mp3" } else { format };

        self.send_log(id, "Fetching album info from Spotify...".to_string())
            .await;

        let album = match spotify::fetch_album(link).await {
            Ok(a) => a,
            Err(e) => {
                let error_msg = Self::format_error_with_hint(&e, "album");
                // Log error for retry
                self.error_log.add_download_error(DownloadErrorEntry::new(
                    link.to_string(),
                    "album".to_string(),
                    format.to_string(),
                    quality.to_string(),
                    portable,
                    None,
                    None,
                    format!("Failed to fetch album: {}", error_msg),
                ));
                let _ = self
                    .tx
                    .send(DownloadEvent::Error {
                        id,
                        error: format!("Failed to fetch album ({}): {}", link, error_msg),
                    })
                    .await;
                return;
            }
        };

        let main_artist = album
            .artists
            .first()
            .and_then(|a| a.name.clone().into())
            .unwrap_or_else(|| "Unknown Artist".to_string());
        let album_name = album.name.clone();
        let total_tracks = album.tracks.items.len();
        let display_name = format!("{} - {}", main_artist, album_name);

        // Fetch genre for the album
        let album_genre = spotify::fetch_album_genres(&album).await;

        // Update queue with album name while still processing
        let _ = self
            .tx
            .send(DownloadEvent::MetadataFetched {
                id,
                name: display_name.clone(),
            })
            .await;

        self.send_log(
            id,
            format!(
                "Found: {} - {} ({} tracks, format: {}, quality: {})",
                main_artist, album_name, total_tracks, actual_format, quality
            ),
        )
        .await;

        let _ = self
            .tx
            .send(DownloadEvent::Started {
                id,
                name: display_name,
                total_tracks,
            })
            .await;

        let album_folder = if config.enabled {
            file_utils::create_portable_folder(&self.music_path, &config)
        } else {
            file_utils::create_album_folder(&self.music_path, &main_artist, &album_name)
        };

        // Download cover
        let cover_path: Option<PathBuf> = if let Some(image) = album.images.first() {
            let p = album_folder.join("cover.jpg");
            if p.exists() {
                Some(p)
            } else {
                self.send_log(id, "Downloading cover art...".to_string())
                    .await;
                self.download_cover_art(id, &image.url, &p).await
            }
        } else {
            None
        };

        for (i, track) in album.tracks.items.iter().enumerate() {
            // Check for pause before starting each track
            while *self.pause_rx.borrow() {
                if self.pause_rx.changed().await.is_err() {
                    return;
                }
            }

            let track_title = track.name.clone();
            let track_artist = track
                .artists
                .first()
                .and_then(|a| a.name.clone().into())
                .unwrap_or_else(|| main_artist.clone());

            let safe_file_name =
                file_utils::build_filename(&track_artist, &track_title, actual_format, &config);
            let file_path = album_folder.join(&safe_file_name);

            let entry = TrackEntry {
                artist: track_artist.clone(),
                title: track_title.clone(),
                path: file_path.display().to_string(),
            };

            if self.db.contains(&entry) {
                let _ = self
                    .tx
                    .send(DownloadEvent::TrackSkipped {
                        id,
                        artist: track_artist,
                        title: track_title,
                    })
                    .await;
                continue;
            }

            let _ = self
                .tx
                .send(DownloadEvent::TrackStarted {
                    id,
                    artist: track_artist.clone(),
                    title: track_title.clone(),
                    track_num: i + 1,
                })
                .await;

            let query = format!("{} {}", track_artist, track_title);
            let file_path_clone = file_path.clone();
            let format_clone = actual_format.to_string();
            let quality_clone = quality.to_string();
            let tx_clone = self.tx.clone();

            match tokio::task::spawn_blocking(move || {
                downloader::download_track_with_output(
                    &query,
                    &file_path_clone,
                    &format_clone,
                    &quality_clone,
                    move |line| {
                        // Send log lines from the blocking context
                        let tx = tx_clone.clone();
                        let line = line.to_string();
                        // We can't await here, so we use try_send or spawn
                        let _ = tx.blocking_send(DownloadEvent::LogLine { id, line });
                    },
                )
            })
            .await
            {
                Ok(Ok(_)) => {
                    if let Err(e) = metadata::tag_audio(
                        &file_path,
                        &track_artist,
                        &album_name,
                        &track_title,
                        (i + 1) as u32,
                        album_genre.as_deref(),
                        cover_path.as_deref(),
                        &config,
                    ) {
                        let error_msg = format!("Tagging failed: {}", e);
                        // Log error for retry
                        self.error_log.add_download_error(DownloadErrorEntry::new(
                            link.to_string(),
                            "album".to_string(),
                            actual_format.to_string(),
                            quality.to_string(),
                            portable,
                            Some(track_artist.clone()),
                            Some(track_title.clone()),
                            error_msg.clone(),
                        ));
                        let _ = self
                            .tx
                            .send(DownloadEvent::TrackFailed {
                                id,
                                artist: track_artist,
                                title: track_title,
                                error: error_msg,
                            })
                            .await;
                        continue;
                    }

                    self.db.add(entry);
                    let _ = self
                        .tx
                        .send(DownloadEvent::TrackComplete {
                            id,
                            artist: track_artist,
                            title: track_title,
                            path: file_path.display().to_string(),
                        })
                        .await;
                }
                Ok(Err(e)) => {
                    let error_msg = e.to_string();
                    // Log error for retry
                    self.error_log.add_download_error(DownloadErrorEntry::new(
                        link.to_string(),
                        "album".to_string(),
                        actual_format.to_string(),
                        quality.to_string(),
                        portable,
                        Some(track_artist.clone()),
                        Some(track_title.clone()),
                        error_msg.clone(),
                    ));
                    let _ = self
                        .tx
                        .send(DownloadEvent::TrackFailed {
                            id,
                            artist: track_artist,
                            title: track_title,
                            error: error_msg,
                        })
                        .await;
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    // Log error for retry
                    self.error_log.add_download_error(DownloadErrorEntry::new(
                        link.to_string(),
                        "album".to_string(),
                        actual_format.to_string(),
                        quality.to_string(),
                        portable,
                        Some(track_artist.clone()),
                        Some(track_title.clone()),
                        error_msg.clone(),
                    ));
                    let _ = self
                        .tx
                        .send(DownloadEvent::TrackFailed {
                            id,
                            artist: track_artist,
                            title: track_title,
                            error: error_msg,
                        })
                        .await;
                }
            }
        }

        let _ = self
            .tx
            .send(DownloadEvent::Complete {
                id,
                name: format!("{} - {}", main_artist, album_name),
            })
            .await;
    }

    async fn process_playlist(
        &mut self,
        id: usize,
        link: &str,
        portable: bool,
        format: &str,
        quality: &str,
    ) {
        let config = if portable {
            PortableConfig {
                enabled: true,
                max_cover_dim: 128,
                max_cover_bytes: 64 * 1024,
                max_filename_len: 64,
            }
        } else {
            PortableConfig {
                enabled: false,
                max_cover_dim: 500,
                max_cover_bytes: 300 * 1024,
                max_filename_len: 100,
            }
        };

        // Use mp3 for portable mode, otherwise use selected format
        let actual_format = if portable { "mp3" } else { format };

        self.send_log(id, "Fetching playlist info from Spotify...".to_string())
            .await;

        // Fetch playlist metadata
        let playlist = match spotify::fetch_playlist(link).await {
            Ok(p) => p,
            Err(e) => {
                let error_msg = Self::format_error_with_hint(&e, "playlist");
                // Log error for retry
                self.error_log.add_download_error(DownloadErrorEntry::new(
                    link.to_string(),
                    "playlist".to_string(),
                    format.to_string(),
                    quality.to_string(),
                    portable,
                    None,
                    None,
                    format!("Failed to fetch playlist: {}", error_msg),
                ));
                let _ = self
                    .tx
                    .send(DownloadEvent::Error {
                        id,
                        error: format!("Failed to fetch playlist ({}): {}", link, error_msg),
                    })
                    .await;
                return;
            }
        };

        let playlist_name = playlist.name.clone();

        // Update queue with playlist name while still fetching tracks
        let _ = self
            .tx
            .send(DownloadEvent::MetadataFetched {
                id,
                name: playlist_name.clone(),
            })
            .await;

        // Fetch ALL playlist items with pagination (no 100 track limit)
        self.send_log(id, format!("Fetching tracks for '{}'...", playlist_name))
            .await;

        let all_items = match spotify::fetch_all_playlist_items(link).await {
            Ok(items) => items,
            Err(e) => {
                // Log error for retry
                self.error_log.add_download_error(DownloadErrorEntry::new(
                    link.to_string(),
                    "playlist".to_string(),
                    format.to_string(),
                    quality.to_string(),
                    portable,
                    None,
                    Some(playlist_name.clone()),
                    format!("Failed to fetch playlist tracks: {}", e),
                ));
                let _ = self
                    .tx
                    .send(DownloadEvent::Error {
                        id,
                        error: format!("Failed to fetch playlist tracks for '{}': {}", playlist_name, e),
                    })
                    .await;
                return;
            }
        };

        let total_tracks = all_items.len();

        self.send_log(
            id,
            format!(
                "Found: {} ({} tracks, format: {}, quality: {})",
                playlist_name, total_tracks, actual_format, quality
            ),
        )
        .await;

        let _ = self
            .tx
            .send(DownloadEvent::Started {
                id,
                name: playlist_name.clone(),
                total_tracks,
            })
            .await;

        let mut downloaded_paths: Vec<PathBuf> = Vec::new();

        for (i, item) in all_items.iter().enumerate() {
            // Check for pause before starting each track
            while *self.pause_rx.borrow() {
                if self.pause_rx.changed().await.is_err() {
                    return;
                }
            }

            let track = match &item.track {
                Some(rspotify::model::PlayableItem::Track(t)) => t,
                _ => continue,
            };

            let track_title = track.name.clone();
            let track_artist = track
                .artists
                .first()
                .map(|a| a.name.clone())
                .unwrap_or_else(|| "Unknown Artist".to_string());

            // Get album name from track metadata
            let album_name = track.album.name.clone();

            // Use music path (like albums) and organize by artist/album
            let output_folder = if config.enabled {
                file_utils::create_portable_folder(&self.music_path, &config)
            } else {
                file_utils::create_album_folder(&self.music_path, &track_artist, &album_name)
            };

            let safe_file_name =
                file_utils::build_filename(&track_artist, &track_title, actual_format, &config);
            let file_path = output_folder.join(&safe_file_name);

            let entry = TrackEntry {
                artist: track_artist.clone(),
                title: track_title.clone(),
                path: file_path.display().to_string(),
            };

            if self.db.contains(&entry) {
                let _ = self
                    .tx
                    .send(DownloadEvent::TrackSkipped {
                        id,
                        artist: track_artist,
                        title: track_title,
                    })
                    .await;
                downloaded_paths.push(file_path);
                continue;
            }

            let _ = self
                .tx
                .send(DownloadEvent::TrackStarted {
                    id,
                    artist: track_artist.clone(),
                    title: track_title.clone(),
                    track_num: i + 1,
                })
                .await;

            let query = format!("{} {}", track_artist, track_title);
            let file_path_clone = file_path.clone();
            let format_clone = actual_format.to_string();
            let quality_clone = quality.to_string();
            let tx_clone = self.tx.clone();

            match tokio::task::spawn_blocking(move || {
                downloader::download_track_with_output(
                    &query,
                    &file_path_clone,
                    &format_clone,
                    &quality_clone,
                    move |line| {
                        let tx = tx_clone.clone();
                        let line = line.to_string();
                        let _ = tx.blocking_send(DownloadEvent::LogLine { id, line });
                    },
                )
            })
            .await
            {
                Ok(Ok(_)) => {
                    // For playlists, we don't have album-level genre info
                    if let Err(e) = metadata::tag_audio(
                        &file_path,
                        &track_artist,
                        &album_name,
                        &track_title,
                        track.track_number,
                        None, // genre - can be added via retag command
                        None,
                        &config,
                    ) {
                        let error_msg = format!("Tagging failed: {}", e);
                        // Log error for retry
                        self.error_log.add_download_error(DownloadErrorEntry::new(
                            link.to_string(),
                            "playlist".to_string(),
                            actual_format.to_string(),
                            quality.to_string(),
                            portable,
                            Some(track_artist.clone()),
                            Some(track_title.clone()),
                            error_msg.clone(),
                        ));
                        let _ = self
                            .tx
                            .send(DownloadEvent::TrackFailed {
                                id,
                                artist: track_artist,
                                title: track_title,
                                error: error_msg,
                            })
                            .await;
                        continue;
                    }

                    self.db.add(entry);
                    downloaded_paths.push(file_path.clone());
                    let _ = self
                        .tx
                        .send(DownloadEvent::TrackComplete {
                            id,
                            artist: track_artist,
                            title: track_title,
                            path: file_path.display().to_string(),
                        })
                        .await;
                }
                Ok(Err(e)) => {
                    let error_msg = e.to_string();
                    // Log error for retry
                    self.error_log.add_download_error(DownloadErrorEntry::new(
                        link.to_string(),
                        "playlist".to_string(),
                        actual_format.to_string(),
                        quality.to_string(),
                        portable,
                        Some(track_artist.clone()),
                        Some(track_title.clone()),
                        error_msg.clone(),
                    ));
                    let _ = self
                        .tx
                        .send(DownloadEvent::TrackFailed {
                            id,
                            artist: track_artist,
                            title: track_title,
                            error: error_msg,
                        })
                        .await;
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    // Log error for retry
                    self.error_log.add_download_error(DownloadErrorEntry::new(
                        link.to_string(),
                        "playlist".to_string(),
                        actual_format.to_string(),
                        quality.to_string(),
                        portable,
                        Some(track_artist.clone()),
                        Some(track_title.clone()),
                        error_msg.clone(),
                    ));
                    let _ = self
                        .tx
                        .send(DownloadEvent::TrackFailed {
                            id,
                            artist: track_artist,
                            title: track_title,
                            error: error_msg,
                        })
                        .await;
                }
            }
        }

        let _ = file_utils::create_m3u(&playlist_name, &downloaded_paths, &self.playlist_path);

        let _ = self
            .tx
            .send(DownloadEvent::Complete {
                id,
                name: playlist_name,
            })
            .await;
    }

    async fn process_youtube_playlist(
        &mut self,
        id: usize,
        link: &str,
        portable: bool,
        format: &str,
        quality: &str,
    ) {
        let config = if portable {
            PortableConfig {
                enabled: true,
                max_cover_dim: 128,
                max_cover_bytes: 64 * 1024,
                max_filename_len: 64,
            }
        } else {
            PortableConfig {
                enabled: false,
                max_cover_dim: 500,
                max_cover_bytes: 300 * 1024,
                max_filename_len: 100,
            }
        };

        let actual_format = if portable { "mp3" } else { format };

        self.send_log(id, format!("Fetching YouTube playlist: {}", link))
            .await;

        // Fetch playlist info using yt-dlp (blocking operation)
        let link_clone = link.to_string();
        let playlist_result = tokio::task::spawn_blocking(move || {
            youtube::fetch_playlist(&link_clone)
        })
        .await;

        let playlist = match playlist_result {
            Ok(Ok(p)) => p,
            Ok(Err(e)) => {
                let error_msg = format!("Failed to fetch YouTube playlist: {}", e);
                self.error_log.add_download_error(DownloadErrorEntry::new(
                    link.to_string(),
                    "youtube_playlist".to_string(),
                    actual_format.to_string(),
                    quality.to_string(),
                    portable,
                    None,
                    None,
                    error_msg.clone(),
                ));
                let _ = self
                    .tx
                    .send(DownloadEvent::Error { id, error: error_msg })
                    .await;
                return;
            }
            Err(e) => {
                let error_msg = format!("Task error: {}", e);
                let _ = self
                    .tx
                    .send(DownloadEvent::Error { id, error: error_msg })
                    .await;
                return;
            }
        };

        let playlist_name = playlist.title.clone();
        let total_tracks = playlist.tracks.len();

        self.send_log(
            id,
            format!(
                "Found: {} ({} tracks, format: {}, quality: {})",
                playlist_name, total_tracks, actual_format, quality
            ),
        )
        .await;

        let _ = self
            .tx
            .send(DownloadEvent::Started {
                id,
                name: playlist_name.clone(),
                total_tracks,
            })
            .await;

        let mut downloaded_paths: Vec<PathBuf> = Vec::new();

        for (i, track) in playlist.tracks.iter().enumerate() {
            // Check for pause before starting each track
            while *self.pause_rx.borrow() {
                if self.pause_rx.changed().await.is_err() {
                    return;
                }
            }

            let track_title = track.title.clone();
            let track_artist = track.artist.clone();

            // Use music path and organize by artist/album (using playlist name as album)
            let output_folder = if config.enabled {
                file_utils::create_portable_folder(&self.music_path, &config)
            } else {
                file_utils::create_album_folder(&self.music_path, &track_artist, &playlist_name)
            };

            let safe_file_name =
                file_utils::build_filename(&track_artist, &track_title, actual_format, &config);
            let file_path = output_folder.join(&safe_file_name);

            let entry = TrackEntry {
                artist: track_artist.clone(),
                title: track_title.clone(),
                path: file_path.display().to_string(),
            };

            if self.db.contains(&entry) {
                let _ = self
                    .tx
                    .send(DownloadEvent::TrackSkipped {
                        id,
                        artist: track_artist,
                        title: track_title,
                    })
                    .await;
                continue;
            }

            self.send_log(
                id,
                format!(
                    "[{}/{}] Downloading: {} - {}",
                    i + 1,
                    total_tracks,
                    track_artist,
                    track_title
                ),
            )
            .await;

            let _ = self
                .tx
                .send(DownloadEvent::TrackStarted {
                    id,
                    artist: track_artist.clone(),
                    title: track_title.clone(),
                    track_num: i + 1,
                })
                .await;

            // Download directly from YouTube URL instead of searching
            let file_path_clone = file_path.clone();
            let format_clone = actual_format.to_string();
            let quality_clone = quality.to_string();
            let video_url = track.url.clone();
            let tx_clone = self.tx.clone();

            match tokio::task::spawn_blocking(move || {
                // Use the direct URL instead of search query
                downloader::download_track_with_output(
                    &video_url,
                    &file_path_clone,
                    &format_clone,
                    &quality_clone,
                    move |line| {
                        let tx = tx_clone.clone();
                        let line = line.to_string();
                        let _ = tx.blocking_send(DownloadEvent::LogLine { id, line });
                    },
                )
            })
            .await
            {
                Ok(Ok(_)) => {
                    // Tag with basic metadata (no cover art for YouTube)
                    if let Err(e) = metadata::tag_audio(
                        &file_path,
                        &track_artist,
                        &playlist_name, // Use playlist name as album
                        &track_title,
                        (i + 1) as u32,
                        None, // No genre
                        None, // No cover art
                        &config,
                    ) {
                        self.send_log(id, format!("Warning: Tagging failed: {}", e))
                            .await;
                    }

                    self.db.add(entry);
                    downloaded_paths.push(file_path.clone());

                    let _ = self
                        .tx
                        .send(DownloadEvent::TrackComplete {
                            id,
                            artist: track_artist,
                            title: track_title,
                            path: file_path.display().to_string(),
                        })
                        .await;
                }
                Ok(Err(e)) => {
                    let error_msg = e.to_string();
                    self.error_log.add_download_error(DownloadErrorEntry::new(
                        link.to_string(),
                        "youtube_playlist".to_string(),
                        actual_format.to_string(),
                        quality.to_string(),
                        portable,
                        Some(track_artist.clone()),
                        Some(track_title.clone()),
                        error_msg.clone(),
                    ));
                    let _ = self
                        .tx
                        .send(DownloadEvent::TrackFailed {
                            id,
                            artist: track_artist,
                            title: track_title,
                            error: error_msg,
                        })
                        .await;
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    self.error_log.add_download_error(DownloadErrorEntry::new(
                        link.to_string(),
                        "youtube_playlist".to_string(),
                        actual_format.to_string(),
                        quality.to_string(),
                        portable,
                        Some(track_artist.clone()),
                        Some(track_title.clone()),
                        error_msg.clone(),
                    ));
                    let _ = self
                        .tx
                        .send(DownloadEvent::TrackFailed {
                            id,
                            artist: track_artist,
                            title: track_title,
                            error: error_msg,
                        })
                        .await;
                }
            }
        }

        let _ = file_utils::create_m3u(&playlist_name, &downloaded_paths, &self.playlist_path);

        let _ = self
            .tx
            .send(DownloadEvent::Complete {
                id,
                name: playlist_name,
            })
            .await;
    }

    async fn process_convert(
        &mut self,
        id: usize,
        input_path: &str,
        target_format: &str,
        quality: &str,
        refresh_metadata: bool,
        artist: &str,
        title: &str,
    ) {
        let input = std::path::Path::new(input_path);

        // Send started event
        let _ = self
            .tx
            .send(DownloadEvent::ConvertStarted {
                id,
                path: input_path.to_string(),
                target_format: target_format.to_string(),
            })
            .await;

        self.send_log(
            id,
            format!("Converting {} to {}", input_path, target_format),
        )
        .await;

        // Perform conversion in blocking thread
        let input_clone = input.to_path_buf();
        let format_clone = target_format.to_string();
        let quality_clone = quality.to_string();
        let tx_clone = self.tx.clone();

        let result = tokio::task::spawn_blocking(move || {
            converter::convert_audio(&input_clone, &format_clone, &quality_clone, move |line| {
                let tx = tx_clone.clone();
                let line = line.to_string();
                let _ = tx.blocking_send(DownloadEvent::LogLine { id, line });
            })
        })
        .await;

        match result {
            Ok(Ok(new_path)) => {
                let new_path_str = new_path.display().to_string();
                self.send_log(id, format!("Conversion complete: {}", new_path_str))
                    .await;

                // Refresh metadata if requested
                if refresh_metadata {
                    self.send_log(id, format!("Refreshing metadata for: {} - {}", artist, title))
                        .await;

                    match spotify::search_track(artist, title).await {
                        Ok(Some(meta)) => {
                            // Download cover art if available
                            let cover_path = if let Some(url) = &meta.cover_url {
                                let cover_file = new_path.with_file_name("temp_cover.jpg");
                                self.download_cover_art(id, url, &cover_file).await
                            } else {
                                None
                            };

                            // Apply metadata
                            let config = PortableConfig {
                                enabled: false,
                                max_cover_dim: 500,
                                max_cover_bytes: 300 * 1024,
                                max_filename_len: 100,
                            };

                            if let Err(e) = metadata::tag_audio(
                                &new_path,
                                &meta.artist,
                                &meta.album,
                                &meta.title,
                                meta.track_number,
                                meta.genre.as_deref(),
                                cover_path.as_deref(),
                                &config,
                            ) {
                                self.send_log(id, format!("Warning: Failed to apply metadata: {}", e))
                                    .await;
                            } else {
                                self.send_log(id, "Metadata refreshed successfully".to_string())
                                    .await;
                            }

                            // Clean up temp cover
                            if let Some(cover) = cover_path {
                                let _ = std::fs::remove_file(cover);
                            }
                        }
                        Ok(None) => {
                            self.send_log(
                                id,
                                "Could not find track on Spotify, keeping existing metadata"
                                    .to_string(),
                            )
                            .await;
                        }
                        Err(e) => {
                            self.send_log(id, format!("Spotify search failed: {}", e))
                                .await;
                        }
                    }
                }

                // Update database with new path
                self.db.update_path(input_path, &new_path_str);

                // Ask for deletion confirmation
                let _ = self
                    .tx
                    .send(DownloadEvent::ConvertDeleteConfirm {
                        id,
                        old_path: input_path.to_string(),
                        new_path: new_path_str.clone(),
                    })
                    .await;

                let _ = self
                    .tx
                    .send(DownloadEvent::ConvertComplete {
                        id,
                        old_path: input_path.to_string(),
                        new_path: new_path_str,
                    })
                    .await;
            }
            Ok(Err(e)) => {
                let error_msg = e.to_string();
                self.send_log(id, format!("Conversion failed: {}", error_msg)).await;
                // Log error for retry
                self.error_log.add_convert_error(ConvertErrorEntry::new(
                    input_path.to_string(),
                    target_format.to_string(),
                    quality.to_string(),
                    refresh_metadata,
                    artist.to_string(),
                    title.to_string(),
                    error_msg.clone(),
                ));
                let _ = self
                    .tx
                    .send(DownloadEvent::ConvertFailed {
                        id,
                        path: input_path.to_string(),
                        error: error_msg,
                    })
                    .await;
            }
            Err(e) => {
                let error_msg = e.to_string();
                self.send_log(id, format!("Conversion task failed: {}", error_msg))
                    .await;
                // Log error for retry
                self.error_log.add_convert_error(ConvertErrorEntry::new(
                    input_path.to_string(),
                    target_format.to_string(),
                    quality.to_string(),
                    refresh_metadata,
                    artist.to_string(),
                    title.to_string(),
                    error_msg.clone(),
                ));
                let _ = self
                    .tx
                    .send(DownloadEvent::ConvertFailed {
                        id,
                        path: input_path.to_string(),
                        error: error_msg,
                    })
                    .await;
            }
        }
    }

    async fn process_convert_batch(
        &mut self,
        id: usize,
        tracks: Vec<ConvertTrackInfo>,
        target_format: &str,
        quality: &str,
        refresh_metadata: bool,
    ) {
        let total = tracks.len();
        let mut successful = 0;
        let mut converted_files: Vec<(String, String)> = Vec::new();

        self.send_log(
            id,
            format!("Starting batch conversion of {} tracks to {}", total, target_format),
        )
        .await;

        for (i, track) in tracks.iter().enumerate() {
            let input = std::path::Path::new(&track.input_path);

            self.send_log(
                id,
                format!(
                    "[{}/{}] Converting: {} - {}",
                    i + 1,
                    total,
                    track.artist,
                    track.title
                ),
            )
            .await;

            let _ = self
                .tx
                .send(DownloadEvent::ConvertStarted {
                    id,
                    path: track.input_path.clone(),
                    target_format: target_format.to_string(),
                })
                .await;

            // Perform conversion
            let input_clone = input.to_path_buf();
            let format_clone = target_format.to_string();
            let quality_clone = quality.to_string();
            let tx_clone = self.tx.clone();

            let result = tokio::task::spawn_blocking(move || {
                converter::convert_audio(&input_clone, &format_clone, &quality_clone, move |line| {
                    let tx = tx_clone.clone();
                    let line = line.to_string();
                    let _ = tx.blocking_send(DownloadEvent::LogLine { id, line });
                })
            })
            .await;

            match result {
                Ok(Ok(new_path)) => {
                    let new_path_str = new_path.display().to_string();

                    // Refresh metadata if requested
                    if refresh_metadata {
                        match spotify::search_track(&track.artist, &track.title).await {
                            Ok(Some(meta)) => {
                                let cover_path = if let Some(url) = &meta.cover_url {
                                    let cover_file = new_path.with_file_name("temp_cover.jpg");
                                    self.download_cover_art(id, url, &cover_file).await
                                } else {
                                    None
                                };

                                let config = PortableConfig {
                                    enabled: false,
                                    max_cover_dim: 500,
                                    max_cover_bytes: 300 * 1024,
                                    max_filename_len: 100,
                                };

                                let _ = metadata::tag_audio(
                                    &new_path,
                                    &meta.artist,
                                    &meta.album,
                                    &meta.title,
                                    meta.track_number,
                                    meta.genre.as_deref(),
                                    cover_path.as_deref(),
                                    &config,
                                );

                                if let Some(cover) = cover_path {
                                    let _ = std::fs::remove_file(cover);
                                }
                            }
                            _ => {}
                        }
                    }

                    // Update database with new path
                    self.db.update_path(&track.input_path, &new_path_str);

                    converted_files.push((track.input_path.clone(), new_path_str.clone()));
                    successful += 1;

                    let _ = self
                        .tx
                        .send(DownloadEvent::ConvertComplete {
                            id,
                            old_path: track.input_path.clone(),
                            new_path: new_path_str,
                        })
                        .await;
                }
                Ok(Err(e)) => {
                    let error_msg = e.to_string();
                    self.send_log(
                        id,
                        format!("Failed to convert {} - {}: {}", track.artist, track.title, error_msg),
                    )
                    .await;
                    // Log error for retry
                    self.error_log.add_convert_error(ConvertErrorEntry::new(
                        track.input_path.clone(),
                        target_format.to_string(),
                        quality.to_string(),
                        refresh_metadata,
                        track.artist.clone(),
                        track.title.clone(),
                        error_msg.clone(),
                    ));
                    let _ = self
                        .tx
                        .send(DownloadEvent::ConvertFailed {
                            id,
                            path: track.input_path.clone(),
                            error: error_msg,
                        })
                        .await;
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    self.send_log(
                        id,
                        format!("Task failed for {} - {}: {}", track.artist, track.title, error_msg),
                    )
                    .await;
                    // Log error for retry
                    self.error_log.add_convert_error(ConvertErrorEntry::new(
                        track.input_path.clone(),
                        target_format.to_string(),
                        quality.to_string(),
                        refresh_metadata,
                        track.artist.clone(),
                        track.title.clone(),
                        error_msg.clone(),
                    ));
                    let _ = self
                        .tx
                        .send(DownloadEvent::ConvertFailed {
                            id,
                            path: track.input_path.clone(),
                            error: error_msg,
                        })
                        .await;
                }
            }
        }

        self.send_log(
            id,
            format!(
                "Batch conversion complete: {}/{} successful",
                successful, total
            ),
        )
        .await;

        // Send batch complete event
        let _ = self
            .tx
            .send(DownloadEvent::ConvertBatchComplete {
                id,
                total,
                successful,
            })
            .await;

        // If any files were successfully converted, ask about deletion
        if !converted_files.is_empty() {
            let _ = self
                .tx
                .send(DownloadEvent::ConvertBatchDeleteConfirm { converted_files })
                .await;
        }
    }

    async fn process_refresh_metadata(
        &mut self,
        id: usize,
        input_path: &str,
        artist: &str,
        title: &str,
    ) {
        let input = std::path::Path::new(input_path);

        let _ = self
            .tx
            .send(DownloadEvent::RefreshStarted {
                id,
                artist: artist.to_string(),
                title: title.to_string(),
            })
            .await;

        self.send_log(
            id,
            format!("Refreshing metadata for: {} - {}", artist, title),
        )
        .await;

        match spotify::search_track(artist, title).await {
            Ok(Some(meta)) => {
                // Download cover art if available
                let cover_path = if let Some(url) = &meta.cover_url {
                    let cover_file = input.with_file_name("temp_cover.jpg");
                    self.download_cover_art(id, url, &cover_file).await
                } else {
                    None
                };

                // Apply metadata
                let config = PortableConfig {
                    enabled: false,
                    max_cover_dim: 500,
                    max_cover_bytes: 300 * 1024,
                    max_filename_len: 100,
                };

                if let Err(e) = metadata::tag_audio(
                    input,
                    &meta.artist,
                    &meta.album,
                    &meta.title,
                    meta.track_number,
                    meta.genre.as_deref(),
                    cover_path.as_deref(),
                    &config,
                ) {
                    let error_msg = e.to_string();
                    self.send_log(id, format!("Failed to apply metadata: {}", error_msg))
                        .await;
                    // Log error for retry
                    self.error_log.add_refresh_error(RefreshErrorEntry::new(
                        input_path.to_string(),
                        artist.to_string(),
                        title.to_string(),
                        format!("Failed to apply metadata: {}", error_msg),
                    ));
                    let _ = self
                        .tx
                        .send(DownloadEvent::RefreshFailed {
                            id,
                            artist: artist.to_string(),
                            title: title.to_string(),
                            error: error_msg,
                        })
                        .await;
                } else {
                    self.send_log(id, "Metadata refreshed successfully".to_string())
                        .await;
                    let _ = self
                        .tx
                        .send(DownloadEvent::RefreshComplete {
                            id,
                            artist: artist.to_string(),
                            title: title.to_string(),
                        })
                        .await;
                }

                // Clean up temp cover
                if let Some(cover) = cover_path {
                    let _ = std::fs::remove_file(cover);
                }
            }
            Ok(None) => {
                let error_msg = "Track not found on Spotify".to_string();
                self.send_log(
                    id,
                    format!("Could not find {} - {} on Spotify", artist, title),
                )
                .await;
                // Log error for retry
                self.error_log.add_refresh_error(RefreshErrorEntry::new(
                    input_path.to_string(),
                    artist.to_string(),
                    title.to_string(),
                    error_msg.clone(),
                ));
                let _ = self
                    .tx
                    .send(DownloadEvent::RefreshFailed {
                        id,
                        artist: artist.to_string(),
                        title: title.to_string(),
                        error: error_msg,
                    })
                    .await;
            }
            Err(e) => {
                let error_msg = e.to_string();
                self.send_log(id, format!("Spotify search failed: {}", error_msg))
                    .await;
                // Log error for retry
                self.error_log.add_refresh_error(RefreshErrorEntry::new(
                    input_path.to_string(),
                    artist.to_string(),
                    title.to_string(),
                    format!("Spotify search failed: {}", error_msg),
                ));
                let _ = self
                    .tx
                    .send(DownloadEvent::RefreshFailed {
                        id,
                        artist: artist.to_string(),
                        title: title.to_string(),
                        error: error_msg,
                    })
                    .await;
            }
        }
    }

    async fn process_refresh_metadata_batch(&mut self, id: usize, tracks: Vec<ConvertTrackInfo>) {
        let total = tracks.len();
        let mut successful = 0;

        self.send_log(
            id,
            format!("Starting batch metadata refresh for {} tracks", total),
        )
        .await;

        for (i, track) in tracks.iter().enumerate() {
            let input = std::path::Path::new(&track.input_path);

            self.send_log(
                id,
                format!(
                    "[{}/{}] Refreshing: {} - {}",
                    i + 1,
                    total,
                    track.artist,
                    track.title
                ),
            )
            .await;

            let _ = self
                .tx
                .send(DownloadEvent::RefreshStarted {
                    id,
                    artist: track.artist.clone(),
                    title: track.title.clone(),
                })
                .await;

            match spotify::search_track(&track.artist, &track.title).await {
                Ok(Some(meta)) => {
                    let cover_path = if let Some(url) = &meta.cover_url {
                        let cover_file = input.with_file_name("temp_cover.jpg");
                        self.download_cover_art(id, url, &cover_file).await
                    } else {
                        None
                    };

                    let config = PortableConfig {
                        enabled: false,
                        max_cover_dim: 500,
                        max_cover_bytes: 300 * 1024,
                        max_filename_len: 100,
                    };

                    if metadata::tag_audio(
                        input,
                        &meta.artist,
                        &meta.album,
                        &meta.title,
                        meta.track_number,
                        meta.genre.as_deref(),
                        cover_path.as_deref(),
                        &config,
                    )
                    .is_ok()
                    {
                        successful += 1;
                        let _ = self
                            .tx
                            .send(DownloadEvent::RefreshComplete {
                                id,
                                artist: track.artist.clone(),
                                title: track.title.clone(),
                            })
                            .await;
                    } else {
                        let error_msg = "Failed to apply metadata".to_string();
                        // Log error for retry
                        self.error_log.add_refresh_error(RefreshErrorEntry::new(
                            track.input_path.clone(),
                            track.artist.clone(),
                            track.title.clone(),
                            error_msg.clone(),
                        ));
                        let _ = self
                            .tx
                            .send(DownloadEvent::RefreshFailed {
                                id,
                                artist: track.artist.clone(),
                                title: track.title.clone(),
                                error: error_msg,
                            })
                            .await;
                    }

                    if let Some(cover) = cover_path {
                        let _ = std::fs::remove_file(cover);
                    }
                }
                Ok(None) => {
                    let error_msg = "Track not found on Spotify".to_string();
                    // Log error for retry
                    self.error_log.add_refresh_error(RefreshErrorEntry::new(
                        track.input_path.clone(),
                        track.artist.clone(),
                        track.title.clone(),
                        error_msg.clone(),
                    ));
                    let _ = self
                        .tx
                        .send(DownloadEvent::RefreshFailed {
                            id,
                            artist: track.artist.clone(),
                            title: track.title.clone(),
                            error: error_msg,
                        })
                        .await;
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    // Log error for retry
                    self.error_log.add_refresh_error(RefreshErrorEntry::new(
                        track.input_path.clone(),
                        track.artist.clone(),
                        track.title.clone(),
                        format!("Spotify search failed: {}", error_msg),
                    ));
                    let _ = self
                        .tx
                        .send(DownloadEvent::RefreshFailed {
                            id,
                            artist: track.artist.clone(),
                            title: track.title.clone(),
                            error: e.to_string(),
                        })
                        .await;
                }
            }
        }

        self.send_log(
            id,
            format!(
                "Batch metadata refresh complete: {}/{} successful",
                successful, total
            ),
        )
        .await;

        let _ = self
            .tx
            .send(DownloadEvent::RefreshBatchComplete {
                id,
                total,
                successful,
            })
            .await;
    }
}
