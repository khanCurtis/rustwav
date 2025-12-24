use crate::db::{DownloadDB, TrackEntry};
use std::collections::VecDeque;
use std::path::PathBuf;
use tokio::sync::mpsc;

use super::worker::{DownloadEvent, DownloadRequest};

// Format and quality options
pub const FORMAT_OPTIONS: [&str; 4] = ["mp3", "flac", "wav", "aac"];
pub const QUALITY_OPTIONS: [&str; 3] = ["high", "medium", "low"];

#[derive(Debug, Clone, PartialEq)]
pub enum View {
    Main,
    AddLink,
    LinkSettings,
    Queue,
    Library,
    Logs,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LinkType {
    Album,
    Playlist,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SettingsField {
    Format,
    Quality,
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
    // Settings state
    pub pending_link: Option<String>,
    pub selected_format: usize,
    pub selected_quality: usize,
    pub settings_field: SettingsField,
    // Logs state
    pub download_logs: VecDeque<String>,
    pub log_scroll: usize,
    pub log_auto_scroll: bool,
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
            // Settings defaults
            pending_link: None,
            selected_format: 0,  // mp3
            selected_quality: 0, // high
            settings_field: SettingsField::Format,
            // Logs
            download_logs: VecDeque::with_capacity(500),
            log_scroll: 0,
            log_auto_scroll: true,
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
                        item.name = name.clone();
                        item.status = JobStatus::Downloading;
                        item.progress = (0, total_tracks);
                    }
                    self.add_log(format!(
                        "[{}] Started: {} ({} tracks)",
                        id, name, total_tracks
                    ));
                }
                DownloadEvent::TrackStarted {
                    id, artist, title, ..
                } => {
                    if let Some(item) = self.queue.iter_mut().find(|q| q.id == id) {
                        item.current_track = Some(format!("{} - {}", artist, title));
                    }
                    self.status_message = format!("Downloading: {} - {}", artist, title);
                    self.add_log(format!("[{}] Downloading: {} - {}", id, artist, title));
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
                    self.add_log(format!("[{}] Complete: {} - {}", id, artist, title));
                }
                DownloadEvent::TrackSkipped { id, artist, title } => {
                    if let Some(item) = self.queue.iter_mut().find(|q| q.id == id) {
                        item.progress.0 += 1;
                    }
                    self.status_message = format!("Skipped (exists): {} - {}", artist, title);
                    self.add_log(format!("[{}] Skipped: {} - {}", id, artist, title));
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
                    self.add_log(format!(
                        "[{}] FAILED: {} - {} - {}",
                        id, artist, title, error
                    ));
                }
                DownloadEvent::Complete { id, name } => {
                    if let Some(item) = self.queue.iter_mut().find(|q| q.id == id) {
                        item.status = JobStatus::Complete;
                        item.current_track = None;
                    }
                    self.status_message = format!("Finished: {}", name);
                    self.add_log(format!("[{}] Finished: {}", id, name));
                }
                DownloadEvent::Error { id, error } => {
                    if let Some(item) = self.queue.iter_mut().find(|q| q.id == id) {
                        item.status = JobStatus::Failed(error.clone());
                    }
                    self.status_message = format!("Error: {}", error);
                    self.add_log(format!("[{}] ERROR: {}", id, error));
                }
                DownloadEvent::LogLine { id, line } => {
                    self.add_log(format!("[{}] {}", id, line));
                }
            }
        }
    }

    fn add_log(&mut self, line: String) {
        self.download_logs.push_back(line);
        if self.download_logs.len() > 500 {
            self.download_logs.pop_front();
        }
        // Auto-scroll to bottom if enabled
        if self.log_auto_scroll && !self.download_logs.is_empty() {
            self.log_scroll = self.download_logs.len().saturating_sub(1);
        }
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub fn next_view(&mut self) {
        self.view = match self.view {
            View::Main => View::Queue,
            View::Queue => View::Library,
            View::Library => View::Logs,
            View::Logs => View::Main,
            View::AddLink => View::Main,
            View::LinkSettings => View::Main,
        };
    }

    pub fn toggle_portable(&mut self) {
        self.portable_mode = !self.portable_mode;
        self.status_message = if self.portable_mode {
            "Portable mode: ON (FAT32-safe, small covers, MP3 only)".to_string()
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
        self.pending_link = None;
        self.view = View::Main;
        self.status_message = "Cancelled".to_string();
    }

    pub fn submit_input(&mut self) {
        let link = self.input.clone();
        self.input_mode = false;
        self.input.clear();

        if link.is_empty() {
            self.view = View::Main;
            self.status_message = "No link provided".to_string();
            return;
        }

        // Store the link and go to settings
        self.pending_link = Some(link);
        self.view = View::LinkSettings;
        self.settings_field = SettingsField::Format;
        // Reset to defaults (but keep previous selections for convenience)
        self.status_message = "Select format and quality, then press Enter".to_string();
    }

    pub fn cancel_settings(&mut self) {
        self.pending_link = None;
        self.view = View::Main;
        self.status_message = "Cancelled".to_string();
    }

    pub fn submit_settings(&mut self) {
        let link = match self.pending_link.take() {
            Some(l) => l,
            None => {
                self.view = View::Main;
                self.status_message = "No link pending".to_string();
                return;
            }
        };

        self.view = View::Queue;
        self.next_id += 1;
        let id = self.next_id;

        // Get selected format and quality
        let format = if self.portable_mode {
            "mp3".to_string() // Portable mode forces MP3
        } else {
            FORMAT_OPTIONS[self.selected_format].to_string()
        };
        let quality = QUALITY_OPTIONS[self.selected_quality].to_string();

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
                    id,
                    link,
                    portable: self.portable_mode,
                    format: format.clone(),
                    quality: quality.clone(),
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
                    id,
                    link,
                    portable: self.portable_mode,
                    format: format.clone(),
                    quality: quality.clone(),
                }
            }
        };

        // Send to worker (non-blocking)
        let tx = self.download_tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(request).await;
        });

        self.status_message = format!("Added to queue ({}, {})", format, quality);
    }

    // Settings navigation
    pub fn settings_up(&mut self) {
        self.settings_field = SettingsField::Format;
    }

    pub fn settings_down(&mut self) {
        self.settings_field = SettingsField::Quality;
    }

    pub fn settings_left(&mut self) {
        match self.settings_field {
            SettingsField::Format => {
                if self.selected_format > 0 {
                    self.selected_format -= 1;
                }
            }
            SettingsField::Quality => {
                if self.selected_quality > 0 {
                    self.selected_quality -= 1;
                }
            }
        }
    }

    pub fn settings_right(&mut self) {
        match self.settings_field {
            SettingsField::Format => {
                if self.selected_format < FORMAT_OPTIONS.len() - 1 {
                    self.selected_format += 1;
                }
            }
            SettingsField::Quality => {
                if self.selected_quality < QUALITY_OPTIONS.len() - 1 {
                    self.selected_quality += 1;
                }
            }
        }
    }

    // Queue navigation
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

    // Library navigation
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

    // Logs navigation
    pub fn logs_up(&mut self) {
        self.log_auto_scroll = false;
        if self.log_scroll > 0 {
            self.log_scroll -= 1;
        }
    }

    pub fn logs_down(&mut self) {
        if self.log_scroll < self.download_logs.len().saturating_sub(1) {
            self.log_scroll += 1;
        }
        // Re-enable auto-scroll if at bottom
        if self.log_scroll >= self.download_logs.len().saturating_sub(1) {
            self.log_auto_scroll = true;
        }
    }

    pub fn logs_top(&mut self) {
        self.log_auto_scroll = false;
        self.log_scroll = 0;
    }

    pub fn logs_bottom(&mut self) {
        self.log_auto_scroll = true;
        self.log_scroll = self.download_logs.len().saturating_sub(1);
    }

    pub fn show_logs(&mut self) {
        self.view = View::Logs;
    }
}
