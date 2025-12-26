use std::path::PathBuf;
use tokio::sync::mpsc;

use crate::{
    cli::PortableConfig,
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
}

#[derive(Debug, Clone)]
pub enum DownloadEvent {
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
}

pub struct DownloadWorker {
    rx: mpsc::Receiver<DownloadRequest>,
    tx: mpsc::Sender<DownloadEvent>,
    music_path: PathBuf,
    playlist_path: PathBuf,
    db: DownloadDB,
}

impl DownloadWorker {
    pub fn new(rx: mpsc::Receiver<DownloadRequest>, tx: mpsc::Sender<DownloadEvent>) -> Self {
        let music_path = PathBuf::from("data/music");
        let playlist_path = PathBuf::from("data/playlists");
        let _ = std::fs::create_dir_all(&music_path);
        let _ = std::fs::create_dir_all(&playlist_path);
        let _ = std::fs::create_dir_all("data/cache");

        Self {
            rx,
            tx,
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
            }
        }
    }

    async fn send_log(&self, id: usize, line: String) {
        let _ = self.tx.send(DownloadEvent::LogLine { id, line }).await;
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
                let _ = self
                    .tx
                    .send(DownloadEvent::Error {
                        id,
                        error: format!("Failed to fetch album ({}): {}", link, e),
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
                name: format!("{} - {}", main_artist, album_name),
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
                let _ = self
                    .tx
                    .send(DownloadEvent::Error {
                        id,
                        error: format!("Failed to fetch playlist ({}): {}", link, e),
                    })
                    .await;
                return;
            }
        };

        let playlist_name = playlist.name.clone();

        // Fetch ALL playlist items with pagination (no 100 track limit)
        self.send_log(id, "Fetching all playlist tracks...".to_string())
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
}
