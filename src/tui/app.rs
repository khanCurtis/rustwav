use crate::db::{DownloadDB, TrackEntry};
use crate::file_utils;
use crate::spotify;
use std::collections::VecDeque;
use std::path::PathBuf;
use tokio::sync::{mpsc, watch};

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
    GenerateM3U,
    M3UConfirm,
    ConvertSettings,
    ConvertConfirm,
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
    pub event_tx: mpsc::Sender<DownloadEvent>,
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
    // Pause state
    pub paused: bool,
    pub pause_tx: watch::Sender<bool>,
    // M3U generation state
    pub m3u_generating: bool,
    pub m3u_pending: Option<M3UPending>,
    // Conversion state
    pub convert_pending: Option<ConvertPending>,
    pub convert_target_format: usize,
    pub convert_quality: usize,
    pub convert_refresh_metadata: bool,
    pub convert_delete_pending: Option<ConvertDeletePending>,
    pub convert_all_mode: bool,
}

/// Pending M3U data waiting for user confirmation
#[derive(Debug, Clone)]
pub struct M3UPending {
    pub name: String,
    pub found: usize,
    pub missing: usize,
    pub paths: Vec<PathBuf>,
}

/// Pending conversion data
#[derive(Debug, Clone)]
pub struct ConvertPending {
    pub track_path: String,
    pub artist: String,
    pub title: String,
}

/// Pending deletion confirmation after conversion
#[derive(Debug, Clone)]
pub struct ConvertDeletePending {
    pub old_path: String,
    pub new_path: String,
}

impl App {
    pub fn new(
        download_tx: mpsc::Sender<DownloadRequest>,
        event_tx: mpsc::Sender<DownloadEvent>,
        event_rx: mpsc::Receiver<DownloadEvent>,
        pause_tx: watch::Sender<bool>,
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
            event_tx,
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
            // Pause
            paused: false,
            pause_tx,
            // M3U
            m3u_generating: false,
            m3u_pending: None,
            // Conversion
            convert_pending: None,
            convert_target_format: 0,
            convert_quality: 0,
            convert_refresh_metadata: true,
            convert_delete_pending: None,
            convert_all_mode: false,
        }
    }

    pub fn process_events(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                DownloadEvent::MetadataFetched { id, name } => {
                    // Update name while still in Fetching state
                    if let Some(item) = self.queue.iter_mut().find(|q| q.id == id) {
                        item.name = format!("Fetching: {}", name);
                    }
                }
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
                DownloadEvent::M3UGenerated { result } => {
                    self.m3u_generating = false;
                    self.status_message = result;
                }
                DownloadEvent::M3UConfirm {
                    name,
                    found,
                    missing,
                    paths,
                } => {
                    self.m3u_generating = false;
                    self.m3u_pending = Some(M3UPending {
                        name,
                        found,
                        missing,
                        paths,
                    });
                    self.view = View::M3UConfirm;
                    self.status_message =
                        "Some tracks are missing. Press Enter to generate anyway, Esc to cancel."
                            .to_string();
                }
                DownloadEvent::ConvertStarted {
                    id,
                    path,
                    target_format,
                } => {
                    self.add_log(format!(
                        "[{}] Converting: {} -> {}",
                        id, path, target_format
                    ));
                    self.status_message = format!("Converting to {}...", target_format);
                }
                DownloadEvent::ConvertComplete {
                    id,
                    old_path,
                    new_path,
                } => {
                    self.add_log(format!("[{}] Converted: {} -> {}", id, old_path, new_path));
                    self.status_message = format!("Conversion complete: {}", new_path);
                    // Refresh library to show updated path
                    self.refresh_library();
                }
                DownloadEvent::ConvertFailed { id, path, error } => {
                    self.add_log(format!("[{}] Conversion failed: {} - {}", id, path, error));
                    self.status_message = format!("Conversion failed: {}", error);
                }
                DownloadEvent::ConvertDeleteConfirm { old_path, new_path, .. } => {
                    self.convert_delete_pending = Some(ConvertDeletePending {
                        old_path,
                        new_path,
                    });
                    self.view = View::ConvertConfirm;
                    self.status_message =
                        "Delete original file? Press 'y' to delete, 'n' to keep.".to_string();
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
            View::GenerateM3U => View::Main,
            View::M3UConfirm => View::Main,
            View::ConvertSettings => View::Main,
            View::ConvertConfirm => View::Main,
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

        let link_clone = link.clone();
        let event_tx = self.event_tx.clone();
        let link_type = self.link_type.clone();

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

        // Spawn immediate metadata fetch (doesn't wait for download worker)
        tokio::spawn(async move {
            let name = match link_type {
                LinkType::Album => {
                    if let Ok(album) = spotify::fetch_album(&link_clone).await {
                        let artist = album
                            .artists
                            .first()
                            .map(|a| a.name.clone())
                            .unwrap_or_else(|| "Unknown Artist".to_string());
                        Some(format!("{} - {}", artist, album.name))
                    } else {
                        None
                    }
                }
                LinkType::Playlist => {
                    if let Ok(playlist) = spotify::fetch_playlist(&link_clone).await {
                        Some(playlist.name)
                    } else {
                        None
                    }
                }
            };
            if let Some(name) = name {
                let _ = event_tx
                    .send(DownloadEvent::MetadataFetched { id, name })
                    .await;
            }
        });

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

    // Pause control
    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        let _ = self.pause_tx.send(self.paused);
        self.status_message = if self.paused {
            "PAUSED - press Space to resume".to_string()
        } else {
            "Resumed".to_string()
        };
    }

    // M3U generation
    pub fn start_generate_m3u(&mut self) {
        self.view = View::GenerateM3U;
        self.input_mode = true;
        self.input.clear();
        self.status_message = "Enter Spotify album/playlist link to generate M3U:".to_string();
    }

    pub fn submit_m3u_input(&mut self) {
        let link = self.input.clone();
        self.input_mode = false;
        self.input.clear();

        if link.is_empty() {
            self.view = View::Main;
            self.status_message = "No link provided".to_string();
            return;
        }

        self.m3u_generating = true;
        self.status_message = "Fetching track list from Spotify...".to_string();
        self.view = View::Main;

        // Clone needed data for async task
        let db_tracks: Vec<TrackEntry> = self.db.tracks.iter().cloned().collect();
        let playlist_path = self.playlist_path.clone();
        let event_tx = self.event_tx.clone();

        tokio::spawn(async move {
            match check_m3u_tracks(&link, &db_tracks).await {
                M3UCheckResult::Error(msg) => {
                    let _ = event_tx.send(DownloadEvent::M3UGenerated { result: msg }).await;
                }
                M3UCheckResult::AllFound { name, paths } => {
                    // All tracks found, generate directly
                    let result = do_generate_m3u(&name, &paths, &playlist_path);
                    let _ = event_tx.send(DownloadEvent::M3UGenerated { result }).await;
                }
                M3UCheckResult::SomeMissing {
                    name,
                    found,
                    missing,
                    paths,
                } => {
                    // Ask for confirmation
                    let _ = event_tx
                        .send(DownloadEvent::M3UConfirm {
                            name,
                            found,
                            missing,
                            paths,
                        })
                        .await;
                }
                M3UCheckResult::NoneFound { total } => {
                    let _ = event_tx
                        .send(DownloadEvent::M3UGenerated {
                            result: format!("No tracks found in library (0/{} total)", total),
                        })
                        .await;
                }
            }
        });
    }

    pub fn confirm_m3u(&mut self) {
        if let Some(pending) = self.m3u_pending.take() {
            let result = do_generate_m3u(&pending.name, &pending.paths, &self.playlist_path);
            self.status_message = result;
        }
        self.view = View::Main;
    }

    pub fn cancel_m3u(&mut self) {
        self.m3u_pending = None;
        self.view = View::Main;
        self.status_message = "M3U generation cancelled".to_string();
    }

    // Conversion methods
    pub fn start_convert(&mut self) {
        if self.library.is_empty() {
            self.status_message = "Library is empty, nothing to convert".to_string();
            return;
        }

        self.convert_all_mode = false;
        let selected = &self.library[self.library_selected];
        self.convert_pending = Some(ConvertPending {
            track_path: selected.path.clone(),
            artist: selected.artist.clone(),
            title: selected.title.clone(),
        });
        self.view = View::ConvertSettings;
        self.status_message = format!(
            "Convert: {} - {}. Select format and press Enter.",
            selected.artist, selected.title
        );
    }

    pub fn start_convert_all(&mut self) {
        if self.library.is_empty() {
            self.status_message = "Library is empty, nothing to convert".to_string();
            return;
        }

        self.convert_all_mode = true;
        // Use a placeholder pending entry - we'll process all tracks on submit
        self.convert_pending = Some(ConvertPending {
            track_path: String::new(),
            artist: String::new(),
            title: String::new(),
        });
        self.view = View::ConvertSettings;
        self.status_message = format!(
            "Convert ALL {} tracks. Select format and press Enter.",
            self.library.len()
        );
    }

    pub fn cancel_convert(&mut self) {
        self.convert_pending = None;
        self.view = View::Library;
        self.status_message = "Conversion cancelled".to_string();
    }

    pub fn submit_convert(&mut self) {
        if self.convert_pending.is_none() {
            self.view = View::Library;
            self.status_message = "No conversion pending".to_string();
            return;
        }
        self.convert_pending = None;

        self.view = View::Logs;

        let format = FORMAT_OPTIONS[self.convert_target_format].to_string();
        let quality = QUALITY_OPTIONS[self.convert_quality].to_string();
        let refresh_metadata = self.convert_refresh_metadata;

        if self.convert_all_mode {
            // Queue conversion for all tracks in the library
            let tracks: Vec<_> = self.library.iter().cloned().collect();
            let track_count = tracks.len();

            for track in tracks {
                self.next_id += 1;
                let id = self.next_id;

                let request = DownloadRequest::Convert {
                    id,
                    input_path: track.path,
                    target_format: format.clone(),
                    quality: quality.clone(),
                    refresh_metadata,
                    artist: track.artist,
                    title: track.title,
                };

                let tx = self.download_tx.clone();
                tokio::spawn(async move {
                    let _ = tx.send(request).await;
                });
            }

            self.convert_all_mode = false;
            self.status_message = format!(
                "Converting {} tracks to {} (quality: {})...",
                track_count, format, quality
            );
        } else {
            // Single track conversion (use selected track)
            let selected = &self.library[self.library_selected];
            self.next_id += 1;
            let id = self.next_id;

            let request = DownloadRequest::Convert {
                id,
                input_path: selected.path.clone(),
                target_format: format.clone(),
                quality: quality.clone(),
                refresh_metadata,
                artist: selected.artist.clone(),
                title: selected.title.clone(),
            };

            let tx = self.download_tx.clone();
            tokio::spawn(async move {
                let _ = tx.send(request).await;
            });

            self.status_message = format!("Converting to {} (quality: {})...", format, quality);
        }
    }

    pub fn convert_settings_up(&mut self) {
        // Cycle through: Format -> Quality -> Refresh Metadata
        // Currently on refresh metadata, go to quality
        // Just use a simple toggle for now
    }

    pub fn convert_settings_down(&mut self) {
        // Cycle through settings fields
    }

    pub fn convert_settings_left(&mut self) {
        if self.convert_target_format > 0 {
            self.convert_target_format -= 1;
        }
    }

    pub fn convert_settings_right(&mut self) {
        if self.convert_target_format < FORMAT_OPTIONS.len() - 1 {
            self.convert_target_format += 1;
        }
    }

    pub fn convert_toggle_refresh(&mut self) {
        self.convert_refresh_metadata = !self.convert_refresh_metadata;
    }

    pub fn convert_quality_left(&mut self) {
        if self.convert_quality > 0 {
            self.convert_quality -= 1;
        }
    }

    pub fn convert_quality_right(&mut self) {
        if self.convert_quality < QUALITY_OPTIONS.len() - 1 {
            self.convert_quality += 1;
        }
    }

    pub fn confirm_delete_original(&mut self) {
        if let Some(pending) = self.convert_delete_pending.take() {
            if let Err(e) = std::fs::remove_file(&pending.old_path) {
                self.status_message = format!("Failed to delete original: {}", e);
            } else {
                self.status_message = "Original file deleted".to_string();
            }
        }
        self.view = View::Library;
    }

    pub fn cancel_delete_original(&mut self) {
        self.convert_delete_pending = None;
        self.view = View::Library;
        self.status_message = "Original file kept".to_string();
    }
}

/// Result of checking M3U tracks against the database
enum M3UCheckResult {
    Error(String),
    AllFound {
        name: String,
        paths: Vec<PathBuf>,
    },
    SomeMissing {
        name: String,
        found: usize,
        missing: usize,
        paths: Vec<PathBuf>,
    },
    NoneFound {
        total: usize,
    },
}

/// Check which tracks from a Spotify link are in the database
async fn check_m3u_tracks(link: &str, db_tracks: &[TrackEntry]) -> M3UCheckResult {
    let is_album = link.contains("/album/");
    let is_playlist = link.contains("/playlist/");

    if !is_album && !is_playlist {
        return M3UCheckResult::Error(
            "Error: Invalid Spotify link (must be album or playlist)".to_string(),
        );
    }

    // Fetch tracks from Spotify and get the name
    let (spotify_tracks, m3u_name): (Vec<(String, String)>, String) = if is_album {
        match spotify::fetch_album(link).await {
            Ok(album) => {
                let album_name = album.name.clone();
                let main_artist = album
                    .artists
                    .first()
                    .map(|a| a.name.clone())
                    .unwrap_or_else(|| "Unknown".to_string());
                let tracks: Vec<(String, String)> = album
                    .tracks
                    .items
                    .iter()
                    .map(|t| {
                        let artist = t
                            .artists
                            .first()
                            .map(|a| a.name.clone())
                            .unwrap_or_else(|| main_artist.clone());
                        (artist, t.name.clone())
                    })
                    .collect();
                (tracks, format!("{} - {}", main_artist, album_name))
            }
            Err(e) => return M3UCheckResult::Error(format!("Spotify error: {}", e)),
        }
    } else {
        match spotify::fetch_playlist(link).await {
            Ok(playlist) => {
                let playlist_name = playlist.name.clone();
                match spotify::fetch_all_playlist_items(link).await {
                    Ok(items) => {
                        let tracks: Vec<(String, String)> = items
                            .iter()
                            .filter_map(|item| {
                                if let Some(rspotify::model::PlayableItem::Track(t)) = &item.track {
                                    let artist = t
                                        .artists
                                        .first()
                                        .map(|a| a.name.clone())
                                        .unwrap_or_else(|| "Unknown".to_string());
                                    Some((artist, t.name.clone()))
                                } else {
                                    None
                                }
                            })
                            .collect();
                        (tracks, playlist_name)
                    }
                    Err(e) => return M3UCheckResult::Error(format!("Spotify error: {}", e)),
                }
            }
            Err(e) => return M3UCheckResult::Error(format!("Spotify error: {}", e)),
        }
    };

    // Match against database
    let mut found_paths: Vec<PathBuf> = Vec::new();
    let mut missing = 0;

    for (artist, title) in &spotify_tracks {
        let found = db_tracks.iter().find(|e| {
            e.artist.to_lowercase() == artist.to_lowercase()
                && e.title.to_lowercase() == title.to_lowercase()
        });

        if let Some(entry) = found {
            found_paths.push(PathBuf::from(&entry.path));
        } else {
            missing += 1;
        }
    }

    if found_paths.is_empty() {
        M3UCheckResult::NoneFound {
            total: spotify_tracks.len(),
        }
    } else if missing == 0 {
        M3UCheckResult::AllFound {
            name: m3u_name,
            paths: found_paths,
        }
    } else {
        M3UCheckResult::SomeMissing {
            name: m3u_name,
            found: found_paths.len(),
            missing,
            paths: found_paths,
        }
    }
}

/// Actually generate the M3U file
fn do_generate_m3u(name: &str, paths: &[PathBuf], playlist_path: &std::path::Path) -> String {
    match file_utils::create_m3u(name, paths, playlist_path) {
        Ok(_) => format!(
            "Created: {}.m3u ({} tracks)",
            file_utils::sanitize_filename(name),
            paths.len()
        ),
        Err(e) => format!("Failed: {}", e),
    }
}
