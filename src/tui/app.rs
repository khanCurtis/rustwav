use crate::db::{DownloadDB, TrackEntry};
use std::path::PathBuf;
use tokio::sync::mpsc;

use super::worker::{DownloadEvent, DownloadRequest};

#[derive(Debug, Clone, PartialEq)]
pub enum View {
    Main,
    AddLink,
    Queue,
    Library,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LinkType {
    Album,
    Playlist,
}

#[derive(Debug, Clone)]
pub struct QueueItem {
    pub id: usize,
    pub name: String,
    pub status: JobStatus,
    pub current_track: Option<String>,
    pub progress: (usize, usize), // (completed, total)
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum JobStatus {
    Pending,
    Fetching,
    Downloading,
    Complete,
    Failed(String),
}

pub struct App {
    pub running: bool,
    pub view: View,
    pub input: String,
    pub input_mode: bool,
    pub link_type: LinkType,
    pub portable_mode: bool,
    pub queue: Vec<QueueItem>,
    pub queue_selected: usize,
    pub library: Vec<TrackEntry>,
    pub library_selected: usize,
    pub status_message: String,
    pub db: DownloadDB,
    #[allow(dead_code)]
    pub music_path: PathBuf,
    #[allow(dead_code)]
    pub playlist_path: PathBuf,
    // Channels
    pub download_tx: mpsc::Sender<DownloadRequest>,
    pub event_rx: mpsc::Receiver<DownloadEvent>,
    next_id: usize,
}

impl App {
    pub fn new(
        download_tx: mpsc::Sender<DownloadRequest>,
        event_rx: mpsc::Receiver<DownloadEvent>,
    ) -> Self {
        let music_path = PathBuf::from("data/music");
        let playlist_path = PathBuf::from("data/playlists");
        let cache_path = "data/cache/downloaded_songs.json";

        let _ = std::fs::create_dir_all(&music_path);
        let _ = std::fs::create_dir_all(&playlist_path);
        let _ = std::fs::create_dir_all("data/cache");

        let db = DownloadDB::new(cache_path);
        let library: Vec<TrackEntry> = db.tracks.iter().cloned().collect();

        Self {
            running: true,
            view: View::Main,
            input: String::new(),
            input_mode: false,
            link_type: LinkType::Album,
            portable_mode: false,
            queue: Vec::new(),
            queue_selected: 0,
            library,
            library_selected: 0,
            status_message: "Welcome! Press 'a' for album, 'p' for playlist, 'P' for portable mode"
                .to_string(),
            db,
            music_path,
            playlist_path,
            download_tx,
            event_rx,
            next_id: 0,
        }
    }

    pub fn process_events(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                DownloadEvent::Started {
                    id,
                    name,
                    total_tracks,
                } => {
                    if let Some(item) = self.queue.iter_mut().find(|q| q.id == id) {
                        item.name = name;
                        item.status = JobStatus::Downloading;
                        item.progress = (0, total_tracks);
                    }
                }
                DownloadEvent::TrackStarted {
                    id, artist, title, ..
                } => {
                    if let Some(item) = self.queue.iter_mut().find(|q| q.id == id) {
                        item.current_track = Some(format!("{} - {}", artist, title));
                    }
                    self.status_message = format!("Downloading: {} - {}", artist, title);
                }
                DownloadEvent::TrackComplete {
                    id,
                    artist,
                    title,
                    path,
                } => {
                    if let Some(item) = self.queue.iter_mut().find(|q| q.id == id) {
                        item.progress.0 += 1;
                        item.current_track = None;
                    }
                    // Add to library
                    let entry = TrackEntry {
                        artist: artist.clone(),
                        title: title.clone(),
                        path,
                    };
                    if !self
                        .library
                        .iter()
                        .any(|t| t.artist == artist && t.title == title)
                    {
                        self.library.push(entry);
                    }
                    self.status_message = format!("Complete: {} - {}", artist, title);
                }
                DownloadEvent::TrackSkipped { id, artist, title } => {
                    if let Some(item) = self.queue.iter_mut().find(|q| q.id == id) {
                        item.progress.0 += 1;
                    }
                    self.status_message = format!("Skipped (exists): {} - {}", artist, title);
                }
                DownloadEvent::TrackFailed {
                    id,
                    artist,
                    title,
                    error,
                } => {
                    if let Some(item) = self.queue.iter_mut().find(|q| q.id == id) {
                        item.progress.0 += 1;
                    }
                    self.status_message = format!("Failed: {} - {} ({})", artist, title, error);
                }
                DownloadEvent::Complete { id, name } => {
                    if let Some(item) = self.queue.iter_mut().find(|q| q.id == id) {
                        item.status = JobStatus::Complete;
                        item.current_track = None;
                    }
                    self.status_message = format!("Finished: {}", name);
                }
                DownloadEvent::Error { id, error } => {
                    if let Some(item) = self.queue.iter_mut().find(|q| q.id == id) {
                        item.status = JobStatus::Failed(error.clone());
                    }
                    self.status_message = format!("Error: {}", error);
                }
            }
        }
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub fn next_view(&mut self) {
        self.view = match self.view {
            View::Main => View::Queue,
            View::Queue => View::Library,
            View::Library => View::Main,
            View::AddLink => View::Main,
        };
    }

    pub fn toggle_portable(&mut self) {
        self.portable_mode = !self.portable_mode;
        self.status_message = if self.portable_mode {
            "Portable mode: ON (FAT32-safe, small covers)".to_string()
        } else {
            "Portable mode: OFF".to_string()
        };
    }

    pub fn start_add_album(&mut self) {
        self.view = View::AddLink;
        self.input_mode = true;
        self.input.clear();
        self.link_type = LinkType::Album;
        let mode = if self.portable_mode {
            " [portable]"
        } else {
            ""
        };
        self.status_message = format!("Enter Spotify album link{}:", mode);
    }

    pub fn start_add_playlist(&mut self) {
        self.view = View::AddLink;
        self.input_mode = true;
        self.input.clear();
        self.link_type = LinkType::Playlist;
        let mode = if self.portable_mode {
            " [portable]"
        } else {
            ""
        };
        self.status_message = format!("Enter Spotify playlist link{}:", mode);
    }

    pub fn cancel_input(&mut self) {
        self.input_mode = false;
        self.input.clear();
        self.view = View::Main;
        self.status_message = "Cancelled".to_string();
    }

    pub fn submit_input(&mut self) {
        let link = self.input.clone();
        self.input_mode = false;
        self.input.clear();
        self.view = View::Queue;

        if link.is_empty() {
            self.status_message = "No link provided".to_string();
            return;
        }

        self.next_id += 1;
        let id = self.next_id;

        let request = match self.link_type {
            LinkType::Album => {
                self.queue.push(QueueItem {
                    id,
                    name: "Fetching album...".to_string(),
                    status: JobStatus::Fetching,
                    current_track: None,
                    progress: (0, 0),
                });
                DownloadRequest::Album {
                    link,
                    portable: self.portable_mode,
                }
            }
            LinkType::Playlist => {
                self.queue.push(QueueItem {
                    id,
                    name: "Fetching playlist...".to_string(),
                    status: JobStatus::Fetching,
                    current_track: None,
                    progress: (0, 0),
                });
                DownloadRequest::Playlist {
                    link,
                    portable: self.portable_mode,
                }
            }
        };

        // Send to worker (non-blocking)
        let tx = self.download_tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(request).await;
        });

        self.status_message = "Added to queue".to_string();
    }

    pub fn queue_up(&mut self) {
        if self.queue_selected > 0 {
            self.queue_selected -= 1;
        }
    }

    pub fn queue_down(&mut self) {
        if !self.queue.is_empty() && self.queue_selected < self.queue.len() - 1 {
            self.queue_selected += 1;
        }
    }

    pub fn library_up(&mut self) {
        if self.library_selected > 0 {
            self.library_selected -= 1;
        }
    }

    pub fn library_down(&mut self) {
        if !self.library.is_empty() && self.library_selected < self.library.len() - 1 {
            self.library_selected += 1;
        }
    }

    pub fn refresh_library(&mut self) {
        self.db = DownloadDB::new("data/cache/downloaded_songs.json");
        self.library = self.db.tracks.iter().cloned().collect();
        self.status_message = format!("Library refreshed: {} tracks", self.library.len());
    }
}
