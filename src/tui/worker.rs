use std::path::PathBuf;
use tokio::sync::{mpsc, watch};

use crate::{
    cli::PortableConfig,
    converter,
    db::{DownloadDB, TrackEntry},
    downloader, file_utils, metadata,
    sources::spotify,
};

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
    Convert {
        id: usize,
        input_path: String,
        target_format: String,
        quality: String,
        refresh_metadata: bool,
        artist: String,
        title: String,
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
    /// Ask user to confirm deletion of original
    ConvertDeleteConfirm {
        id: usize,
        old_path: String,
        new_path: String,
    },
}

pub struct DownloadWorker {
    rx: mpsc::Receiver<DownloadRequest>,
    tx: mpsc::Sender<DownloadEvent>,
    pause_rx: watch::Receiver<bool>,
    music_path: PathBuf,
    playlist_path: PathBuf,
    db: DownloadDB,
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
            if !p.exists() {
                self.send_log(id, "Downloading cover art...".to_string())
                    .await;
                if let Ok(response) = reqwest::get(&image.url).await {
                    if let Ok(bytes) = response.bytes().await {
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
                        cover_path.as_deref(),
                        &config,
                    ) {
                        let _ = self
                            .tx
                            .send(DownloadEvent::TrackFailed {
                                id,
                                artist: track_artist,
                                title: track_title,
                                error: format!("Tagging failed: {}", e),
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
                    let _ = self
                        .tx
                        .send(DownloadEvent::TrackFailed {
                            id,
                            artist: track_artist,
                            title: track_title,
                            error: e.to_string(),
                        })
                        .await;
                }
                Err(e) => {
                    let _ = self
                        .tx
                        .send(DownloadEvent::TrackFailed {
                            id,
                            artist: track_artist,
                            title: track_title,
                            error: e.to_string(),
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
                    if let Err(e) = metadata::tag_audio(
                        &file_path,
                        &track_artist,
                        &album_name,
                        &track_title,
                        track.track_number,
                        None,
                        &config,
                    ) {
                        let _ = self
                            .tx
                            .send(DownloadEvent::TrackFailed {
                                id,
                                artist: track_artist,
                                title: track_title,
                                error: format!("Tagging failed: {}", e),
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
                    let _ = self
                        .tx
                        .send(DownloadEvent::TrackFailed {
                            id,
                            artist: track_artist,
                            title: track_title,
                            error: e.to_string(),
                        })
                        .await;
                }
                Err(e) => {
                    let _ = self
                        .tx
                        .send(DownloadEvent::TrackFailed {
                            id,
                            artist: track_artist,
                            title: track_title,
                            error: e.to_string(),
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
                                if let Ok(response) = reqwest::get(url).await {
                                    if let Ok(bytes) = response.bytes().await {
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
                self.send_log(id, format!("Conversion failed: {}", e)).await;
                let _ = self
                    .tx
                    .send(DownloadEvent::ConvertFailed {
                        id,
                        path: input_path.to_string(),
                        error: e.to_string(),
                    })
                    .await;
            }
            Err(e) => {
                self.send_log(id, format!("Conversion task failed: {}", e))
                    .await;
                let _ = self
                    .tx
                    .send(DownloadEvent::ConvertFailed {
                        id,
                        path: input_path.to_string(),
                        error: e.to_string(),
                    })
                    .await;
            }
        }
    }
}
