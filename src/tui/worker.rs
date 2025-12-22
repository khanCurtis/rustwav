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
    Album { link: String, portable: bool },
    Playlist { link: String, portable: bool },
}

#[derive(Debug, Clone)]
pub enum DownloadEvent {
    Started { id: usize, name: String, total_tracks: usize },
    TrackStarted { id: usize, artist: String, title: String, track_num: usize },
    TrackComplete { id: usize, artist: String, title: String, path: String },
    TrackSkipped { id: usize, artist: String, title: String },
    TrackFailed { id: usize, artist: String, title: String, error: String },
    Complete { id: usize, name: String },
    Error { id: usize, error: String },
}

pub struct DownloadWorker {
    rx: mpsc::Receiver<DownloadRequest>,
    tx: mpsc::Sender<DownloadEvent>,
    music_path: PathBuf,
    playlist_path: PathBuf,
    db: DownloadDB,
    job_id: usize,
}

impl DownloadWorker {
    pub fn new(
        rx: mpsc::Receiver<DownloadRequest>,
        tx: mpsc::Sender<DownloadEvent>,
    ) -> Self {
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
            job_id: 0,
        }
    }

    pub async fn run(mut self) {
        while let Some(request) = self.rx.recv().await {
            self.job_id += 1;
            let id = self.job_id;

            match request {
                DownloadRequest::Album { link, portable } => {
                    self.process_album(id, &link, portable).await;
                }
                DownloadRequest::Playlist { link, portable } => {
                    self.process_playlist(id, &link, portable).await;
                }
            }
        }
    }

    async fn process_album(&mut self, id: usize, link: &str, portable: bool) {
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

        let album = match spotify::fetch_album(link).await {
            Ok(a) => a,
            Err(e) => {
                let _ = self.tx.send(DownloadEvent::Error {
                    id,
                    error: format!("Failed to fetch album: {}", e),
                }).await;
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

        let _ = self.tx.send(DownloadEvent::Started {
            id,
            name: format!("{} - {}", main_artist, album_name),
            total_tracks,
        }).await;

        let album_folder = if config.enabled {
            file_utils::create_portable_folder(&self.music_path, &config)
        } else {
            file_utils::create_album_folder(&self.music_path, &main_artist, &album_name)
        };

        // Download cover
        let cover_path: Option<PathBuf> = if let Some(image) = album.images.first() {
            let p = album_folder.join("cover.jpg");
            if !p.exists() {
                if let Ok(response) = reqwest::get(&image.url).await {
                    if let Ok(bytes) = response.bytes().await {
                        let _ = std::fs::write(&p, &bytes);
                    }
                }
            }
            if p.exists() { Some(p) } else { None }
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

            let safe_file_name = file_utils::build_filename(
                &track_artist,
                &track_title,
                "mp3",
                &config,
            );
            let file_path = album_folder.join(&safe_file_name);

            let entry = TrackEntry {
                artist: track_artist.clone(),
                title: track_title.clone(),
                path: file_path.display().to_string(),
            };

            if self.db.contains(&entry) {
                let _ = self.tx.send(DownloadEvent::TrackSkipped {
                    id,
                    artist: track_artist,
                    title: track_title,
                }).await;
                continue;
            }

            let _ = self.tx.send(DownloadEvent::TrackStarted {
                id,
                artist: track_artist.clone(),
                title: track_title.clone(),
                track_num: i + 1,
            }).await;

            let query = format!("{} {}", track_artist, track_title);
            let album_folder_clone = album_folder.clone();

            match tokio::task::spawn_blocking(move || {
                downloader::download_track(&query, &album_folder_clone, "mp3")
            }).await {
                Ok(Ok(_)) => {
                    if let Err(e) = metadata::tag_mp3(
                        &file_path,
                        &track_artist,
                        &album_name,
                        &track_title,
                        (i + 1) as u32,
                        cover_path.as_deref(),
                        &config,
                    ) {
                        let _ = self.tx.send(DownloadEvent::TrackFailed {
                            id,
                            artist: track_artist,
                            title: track_title,
                            error: format!("Tagging failed: {}", e),
                        }).await;
                        continue;
                    }

                    self.db.add(entry);
                    let _ = self.tx.send(DownloadEvent::TrackComplete {
                        id,
                        artist: track_artist,
                        title: track_title,
                        path: file_path.display().to_string(),
                    }).await;
                }
                Ok(Err(e)) => {
                    let _ = self.tx.send(DownloadEvent::TrackFailed {
                        id,
                        artist: track_artist,
                        title: track_title,
                        error: e.to_string(),
                    }).await;
                }
                Err(e) => {
                    let _ = self.tx.send(DownloadEvent::TrackFailed {
                        id,
                        artist: track_artist,
                        title: track_title,
                        error: e.to_string(),
                    }).await;
                }
            }
        }

        let _ = self.tx.send(DownloadEvent::Complete {
            id,
            name: format!("{} - {}", main_artist, album_name),
        }).await;
    }

    async fn process_playlist(&mut self, id: usize, link: &str, portable: bool) {
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

        let playlist = match spotify::fetch_playlist(link).await {
            Ok(p) => p,
            Err(e) => {
                let _ = self.tx.send(DownloadEvent::Error {
                    id,
                    error: format!("Failed to fetch playlist: {}", e),
                }).await;
                return;
            }
        };

        let playlist_name = playlist.name.clone();
        let total_tracks = playlist.tracks.items.len();

        let _ = self.tx.send(DownloadEvent::Started {
            id,
            name: playlist_name.clone(),
            total_tracks,
        }).await;

        let mut downloaded_paths: Vec<PathBuf> = Vec::new();

        for (i, item) in playlist.tracks.items.iter().enumerate() {
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

            let output_folder = if config.enabled {
                file_utils::create_portable_folder(&self.playlist_path, &config)
            } else {
                file_utils::create_album_folder(&self.playlist_path, &track_artist, "Singles")
            };

            let safe_file_name = file_utils::build_filename(
                &track_artist,
                &track_title,
                "mp3",
                &config,
            );
            let file_path = output_folder.join(&safe_file_name);

            let entry = TrackEntry {
                artist: track_artist.clone(),
                title: track_title.clone(),
                path: file_path.display().to_string(),
            };

            if self.db.contains(&entry) {
                let _ = self.tx.send(DownloadEvent::TrackSkipped {
                    id,
                    artist: track_artist,
                    title: track_title,
                }).await;
                downloaded_paths.push(file_path);
                continue;
            }

            let _ = self.tx.send(DownloadEvent::TrackStarted {
                id,
                artist: track_artist.clone(),
                title: track_title.clone(),
                track_num: i + 1,
            }).await;

            let query = format!("{} {}", track_artist, track_title);
            let folder_clone = output_folder.clone();

            match tokio::task::spawn_blocking(move || {
                downloader::download_track(&query, &folder_clone, "mp3")
            }).await {
                Ok(Ok(_)) => {
                    if let Err(e) = metadata::tag_mp3(
                        &file_path,
                        &track_artist,
                        "Singles",
                        &track_title,
                        0,
                        None,
                        &config,
                    ) {
                        let _ = self.tx.send(DownloadEvent::TrackFailed {
                            id,
                            artist: track_artist,
                            title: track_title,
                            error: format!("Tagging failed: {}", e),
                        }).await;
                        continue;
                    }

                    self.db.add(entry);
                    downloaded_paths.push(file_path.clone());
                    let _ = self.tx.send(DownloadEvent::TrackComplete {
                        id,
                        artist: track_artist,
                        title: track_title,
                        path: file_path.display().to_string(),
                    }).await;
                }
                Ok(Err(e)) => {
                    let _ = self.tx.send(DownloadEvent::TrackFailed {
                        id,
                        artist: track_artist,
                        title: track_title,
                        error: e.to_string(),
                    }).await;
                }
                Err(e) => {
                    let _ = self.tx.send(DownloadEvent::TrackFailed {
                        id,
                        artist: track_artist,
                        title: track_title,
                        error: e.to_string(),
                    }).await;
                }
            }
        }

        let _ = file_utils::create_m3u(&playlist_name, &downloaded_paths, &self.playlist_path);

        let _ = self.tx.send(DownloadEvent::Complete {
            id,
            name: playlist_name,
        }).await;
    }
}
