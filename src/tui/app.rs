use crate::db::{DownloadDB, TrackEntry};
use crate::error_log::{
    ConvertErrorEntry, DownloadErrorEntry, ErrorLogManager, RefreshErrorEntry,
};
use crate::file_utils;
use crate::sources::{spotify, youtube};
use std::collections::VecDeque;
use std::path::PathBuf;
use tokio::sync::{mpsc, watch};

use super::worker::{ConvertTrackInfo, DownloadEvent, DownloadRequest};

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
    ConvertBatchConfirm,
    CleanupConfirm,
    ErrorLog,
}

/// Tab for error log view (Download/Convert/Refresh)
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorTab {
    Download,
    Convert,
    Refresh,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LinkType {
    Album,
    Playlist,
    YouTubePlaylist,
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
    pub convert_batch_delete_pending: Option<Vec<(String, String)>>,
    // Cleanup state
    pub cleanup_preview: Option<CleanupPreview>,
    // Error log state
    pub error_log: ErrorLogManager,
    pub error_dates: Vec<String>,
    pub error_date_selected: usize,
    pub error_tab: ErrorTab,
    pub error_selected: usize,
    pub download_errors: Vec<DownloadErrorEntry>,
    pub convert_errors: Vec<ConvertErrorEntry>,
    pub refresh_errors: Vec<RefreshErrorEntry>,
}

/// Preview of what cleanup will remove
#[derive(Debug, Clone)]
pub struct CleanupPreview {
    pub missing_count: usize,
    pub total_count: usize,
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

        let error_log = ErrorLogManager::new("data/errors");
        let error_dates = error_log.list_dates();

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
            convert_batch_delete_pending: None,
            cleanup_preview: None,
            // Error log
            error_log,
            error_dates,
            error_date_selected: 0,
            error_tab: ErrorTab::Download,
            error_selected: 0,
            download_errors: Vec::new(),
            convert_errors: Vec::new(),
            refresh_errors: Vec::new(),
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
                DownloadEvent::ConvertBatchComplete { total, successful, .. } => {
                    self.add_log(format!(
                        "Batch conversion complete: {}/{} successful",
                        successful, total
                    ));
                    self.status_message = format!(
                        "Batch conversion complete: {}/{} tracks converted",
                        successful, total
                    );
                }
                DownloadEvent::RefreshStarted { id, artist, title } => {
                    self.add_log(format!("[{}] Refreshing metadata: {} - {}", id, artist, title));
                    self.status_message = format!("Refreshing metadata: {} - {}", artist, title);
                }
                DownloadEvent::RefreshComplete { id, artist, title } => {
                    self.add_log(format!("[{}] Metadata refreshed: {} - {}", id, artist, title));
                    self.status_message = format!("Metadata refreshed: {} - {}", artist, title);
                }
                DownloadEvent::RefreshFailed { id, artist, title, error } => {
                    self.add_log(format!(
                        "[{}] Metadata refresh failed: {} - {} - {}",
                        id, artist, title, error
                    ));
                    self.status_message = format!("Refresh failed: {} - {}", artist, title);
                }
                DownloadEvent::RefreshBatchComplete { total, successful, .. } => {
                    self.add_log(format!(
                        "Batch metadata refresh complete: {}/{} successful",
                        successful, total
                    ));
                    self.status_message = format!(
                        "Batch metadata refresh complete: {}/{} tracks refreshed",
                        successful, total
                    );
                }
                DownloadEvent::ConvertBatchDeleteConfirm { converted_files } => {
                    let count = converted_files.len();
                    self.convert_batch_delete_pending = Some(converted_files);
                    self.view = View::ConvertBatchConfirm;
                    self.status_message = format!(
                        "Delete {} original file(s)? Press 'y' to delete all, 'n' to keep all.",
                        count
                    );
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
            View::ConvertBatchConfirm => View::Main,
            View::CleanupConfirm => View::Main,
            View::ErrorLog => View::Main,
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

    pub fn start_add_youtube_playlist(&mut self) {
        self.view = View::AddLink;
        self.input_mode = true;
        self.input.clear();
        self.link_type = LinkType::YouTubePlaylist;
        let mode = if self.portable_mode {
            " [portable]"
        } else {
            ""
        };
        self.status_message = format!("Enter YouTube playlist link{}:", mode);
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

        // Auto-detect YouTube playlist URLs
        if youtube::is_youtube_playlist(&link) {
            self.link_type = LinkType::YouTubePlaylist;
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
            LinkType::YouTubePlaylist => {
                self.queue.push(QueueItem {
                    id,
                    name: "Fetching YouTube playlist...".to_string(),
                    status: JobStatus::Fetching,
                    current_track: None,
                    progress: (0, 0),
                });
                DownloadRequest::YouTubePlaylist {
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
                LinkType::YouTubePlaylist => {
                    // YouTube metadata is fetched by the worker, skip here
                    None
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
            // Queue batch conversion for all tracks in the library
            let tracks: Vec<ConvertTrackInfo> = self
                .library
                .iter()
                .map(|t| ConvertTrackInfo {
                    input_path: t.path.clone(),
                    artist: t.artist.clone(),
                    title: t.title.clone(),
                })
                .collect();
            let track_count = tracks.len();

            self.next_id += 1;
            let id = self.next_id;

            let request = DownloadRequest::ConvertBatch {
                id,
                tracks,
                target_format: format.clone(),
                quality: quality.clone(),
                refresh_metadata,
            };

            let tx = self.download_tx.clone();
            tokio::spawn(async move {
                let _ = tx.send(request).await;
            });

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

    pub fn confirm_batch_delete_originals(&mut self) {
        if let Some(files) = self.convert_batch_delete_pending.take() {
            let mut deleted = 0;
            let mut failed = 0;
            for (old_path, _) in &files {
                if let Err(_) = std::fs::remove_file(old_path) {
                    failed += 1;
                } else {
                    deleted += 1;
                }
            }
            if failed > 0 {
                self.status_message = format!(
                    "Deleted {} files, {} failed to delete",
                    deleted, failed
                );
            } else {
                self.status_message = format!("Deleted {} original files", deleted);
            }
        }
        self.view = View::Library;
        self.refresh_library();
    }

    pub fn cancel_batch_delete_originals(&mut self) {
        let count = self
            .convert_batch_delete_pending
            .as_ref()
            .map(|f| f.len())
            .unwrap_or(0);
        self.convert_batch_delete_pending = None;
        self.view = View::Library;
        self.status_message = format!("Kept {} original files", count);
        self.refresh_library();
    }

    // Metadata refresh methods
    pub fn start_refresh_metadata(&mut self) {
        if self.library.is_empty() {
            self.status_message = "Library is empty, nothing to refresh".to_string();
            return;
        }

        let selected = &self.library[self.library_selected];
        self.next_id += 1;
        let id = self.next_id;

        let request = DownloadRequest::RefreshMetadata {
            id,
            input_path: selected.path.clone(),
            artist: selected.artist.clone(),
            title: selected.title.clone(),
        };

        let tx = self.download_tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(request).await;
        });

        self.view = View::Logs;
        self.status_message = format!(
            "Refreshing metadata for: {} - {}",
            selected.artist, selected.title
        );
    }

    pub fn start_refresh_all_metadata(&mut self) {
        if self.library.is_empty() {
            self.status_message = "Library is empty, nothing to refresh".to_string();
            return;
        }

        self.next_id += 1;
        let id = self.next_id;

        let tracks: Vec<ConvertTrackInfo> = self
            .library
            .iter()
            .map(|t| ConvertTrackInfo {
                input_path: t.path.clone(),
                artist: t.artist.clone(),
                title: t.title.clone(),
            })
            .collect();
        let track_count = tracks.len();

        let request = DownloadRequest::RefreshMetadataBatch { id, tracks };

        let tx = self.download_tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(request).await;
        });

        self.view = View::Logs;
        self.status_message = format!("Refreshing metadata for {} tracks...", track_count);
    }

    /// Start the cleanup process - shows confirmation with preview
    pub fn start_cleanup_database(&mut self) {
        // Count how many entries have missing files
        let total_count = self.db.tracks.len();
        let missing_count = self
            .db
            .tracks
            .iter()
            .filter(|entry| !std::path::Path::new(&entry.path).exists())
            .count();

        self.cleanup_preview = Some(CleanupPreview {
            missing_count,
            total_count,
        });
        self.view = View::CleanupConfirm;

        if missing_count == 0 {
            self.status_message =
                "Database is clean! All entries point to existing files.".to_string();
        } else {
            self.status_message = format!(
                "Found {} entries with missing files. Press 'y' to clean up, 'n' to cancel.",
                missing_count
            );
        }
    }

    /// Confirm and execute the cleanup
    pub fn confirm_cleanup(&mut self) {
        let (removed, total_before) = self.db.cleanup();

        // Refresh the library view
        self.library = self.db.tracks.iter().cloned().collect();
        if self.library_selected >= self.library.len() && !self.library.is_empty() {
            self.library_selected = self.library.len() - 1;
        }

        self.cleanup_preview = None;
        self.view = View::Library;
        self.status_message = format!(
            "Cleanup complete: removed {} of {} entries. {} remaining.",
            removed,
            total_before,
            total_before - removed
        );

        self.add_log(format!(
            "Database cleanup: removed {} entries ({} remaining)",
            removed,
            total_before - removed
        ));
    }

    /// Cancel the cleanup and return to library
    pub fn cancel_cleanup(&mut self) {
        self.cleanup_preview = None;
        self.view = View::Library;
        self.status_message = "Cleanup cancelled.".to_string();
    }

    // ============ Error Log Methods ============

    /// Show the error log view
    pub fn show_error_log(&mut self) {
        // Refresh dates list
        self.error_dates = self.error_log.list_dates();

        // Load errors for the first date if available
        if !self.error_dates.is_empty() {
            self.error_date_selected = 0;
            self.load_errors_for_current_date();
        } else {
            self.download_errors = Vec::new();
            self.convert_errors = Vec::new();
            self.refresh_errors = Vec::new();
        }

        self.error_tab = ErrorTab::Download;
        self.error_selected = 0;
        self.view = View::ErrorLog;

        let (d, c, r) = self.error_log.get_total_error_counts();
        let total = d + c + r;
        if total == 0 {
            self.status_message = "No errors logged.".to_string();
        } else {
            self.status_message = format!(
                "Error Log: {} download, {} convert, {} refresh errors",
                d, c, r
            );
        }
    }

    /// Load errors for the currently selected date
    fn load_errors_for_current_date(&mut self) {
        if self.error_dates.is_empty() {
            return;
        }
        let date = &self.error_dates[self.error_date_selected];
        self.download_errors = self.error_log.get_download_errors_for_date(date);
        self.convert_errors = self.error_log.get_convert_errors_for_date(date);
        self.refresh_errors = self.error_log.get_refresh_errors_for_date(date);
        self.error_selected = 0;
    }

    /// Get current error list length based on selected tab
    pub fn current_error_count(&self) -> usize {
        match self.error_tab {
            ErrorTab::Download => self.download_errors.len(),
            ErrorTab::Convert => self.convert_errors.len(),
            ErrorTab::Refresh => self.refresh_errors.len(),
        }
    }

    /// Navigate to next date
    pub fn error_date_next(&mut self) {
        if !self.error_dates.is_empty() {
            self.error_date_selected = (self.error_date_selected + 1) % self.error_dates.len();
            self.load_errors_for_current_date();
        }
    }

    /// Navigate to previous date
    pub fn error_date_prev(&mut self) {
        if !self.error_dates.is_empty() {
            if self.error_date_selected == 0 {
                self.error_date_selected = self.error_dates.len() - 1;
            } else {
                self.error_date_selected -= 1;
            }
            self.load_errors_for_current_date();
        }
    }

    /// Switch to next error tab
    pub fn error_tab_next(&mut self) {
        self.error_tab = match self.error_tab {
            ErrorTab::Download => ErrorTab::Convert,
            ErrorTab::Convert => ErrorTab::Refresh,
            ErrorTab::Refresh => ErrorTab::Download,
        };
        self.error_selected = 0;
    }

    /// Switch to previous error tab
    pub fn error_tab_prev(&mut self) {
        self.error_tab = match self.error_tab {
            ErrorTab::Download => ErrorTab::Refresh,
            ErrorTab::Convert => ErrorTab::Download,
            ErrorTab::Refresh => ErrorTab::Convert,
        };
        self.error_selected = 0;
    }

    /// Navigate error list up
    pub fn error_up(&mut self) {
        let count = self.current_error_count();
        if count > 0 && self.error_selected > 0 {
            self.error_selected -= 1;
        }
    }

    /// Navigate error list down
    pub fn error_down(&mut self) {
        let count = self.current_error_count();
        if count > 0 && self.error_selected < count - 1 {
            self.error_selected += 1;
        }
    }

    /// Delete selected error from log
    pub fn delete_selected_error(&mut self) {
        if self.error_dates.is_empty() {
            return;
        }
        let date = self.error_dates[self.error_date_selected].clone();

        let removed = match self.error_tab {
            ErrorTab::Download => {
                if self.error_selected < self.download_errors.len() {
                    let id = self.download_errors[self.error_selected].id.clone();
                    self.error_log.remove_download_error(&date, &id)
                } else {
                    false
                }
            }
            ErrorTab::Convert => {
                if self.error_selected < self.convert_errors.len() {
                    let id = self.convert_errors[self.error_selected].id.clone();
                    self.error_log.remove_convert_error(&date, &id)
                } else {
                    false
                }
            }
            ErrorTab::Refresh => {
                if self.error_selected < self.refresh_errors.len() {
                    let id = self.refresh_errors[self.error_selected].id.clone();
                    self.error_log.remove_refresh_error(&date, &id)
                } else {
                    false
                }
            }
        };

        if removed {
            // Refresh the view
            self.error_dates = self.error_log.list_dates();
            if self.error_date_selected >= self.error_dates.len() && !self.error_dates.is_empty() {
                self.error_date_selected = self.error_dates.len() - 1;
            }
            self.load_errors_for_current_date();
            if self.error_selected >= self.current_error_count() && self.current_error_count() > 0
            {
                self.error_selected = self.current_error_count() - 1;
            }
            self.status_message = "Error deleted.".to_string();
        }
    }

    /// Clear all errors for current date
    pub fn clear_current_date_errors(&mut self) {
        if self.error_dates.is_empty() {
            return;
        }
        let date = self.error_dates[self.error_date_selected].clone();
        self.error_log.clear_date(&date);

        // Refresh
        self.error_dates = self.error_log.list_dates();
        self.error_date_selected = 0;
        self.load_errors_for_current_date();
        self.status_message = format!("Cleared all errors from {}.", date);
    }

    /// Get the selected error's ID and date for retry
    pub fn get_selected_error_info(&self) -> Option<(String, String, ErrorTab)> {
        if self.error_dates.is_empty() {
            return None;
        }
        let date = self.error_dates[self.error_date_selected].clone();

        match self.error_tab {
            ErrorTab::Download => {
                if self.error_selected < self.download_errors.len() {
                    Some((
                        self.download_errors[self.error_selected].id.clone(),
                        date,
                        ErrorTab::Download,
                    ))
                } else {
                    None
                }
            }
            ErrorTab::Convert => {
                if self.error_selected < self.convert_errors.len() {
                    Some((
                        self.convert_errors[self.error_selected].id.clone(),
                        date,
                        ErrorTab::Convert,
                    ))
                } else {
                    None
                }
            }
            ErrorTab::Refresh => {
                if self.error_selected < self.refresh_errors.len() {
                    Some((
                        self.refresh_errors[self.error_selected].id.clone(),
                        date,
                        ErrorTab::Refresh,
                    ))
                } else {
                    None
                }
            }
        }
    }

    /// Refresh error log data
    pub fn refresh_error_logs(&mut self) {
        self.error_dates = self.error_log.list_dates();
        if self.error_date_selected >= self.error_dates.len() {
            self.error_date_selected = 0;
        }
        self.load_errors_for_current_date();
    }

    /// Retry the currently selected error
    pub fn retry_selected_error(&mut self) {
        if self.error_dates.is_empty() {
            self.status_message = "No errors to retry".to_string();
            return;
        }

        let date = self.error_dates[self.error_date_selected].clone();

        match self.error_tab {
            ErrorTab::Download => {
                if self.error_selected >= self.download_errors.len() {
                    self.status_message = "No download error selected".to_string();
                    return;
                }

                let error = self.download_errors[self.error_selected].clone();
                let error_id = error.id.clone();

                // Determine link type from the error's link_type field
                let link_type = match error.link_type.as_str() {
                    "album" => LinkType::Album,
                    "playlist" => LinkType::Playlist,
                    "youtube_playlist" => LinkType::YouTubePlaylist,
                    _ => {
                        self.status_message =
                            format!("Unknown link type: {}", error.link_type);
                        return;
                    }
                };

                // Create new job
                self.next_id += 1;
                let id = self.next_id;

                let name = match (&error.artist, &error.title) {
                    (Some(artist), Some(title)) => format!("{} - {} (retry)", artist, title),
                    _ => format!("Retry: {}", &error.link[..error.link.len().min(40)]),
                };

                self.queue.push(QueueItem {
                    id,
                    name: name.clone(),
                    status: JobStatus::Fetching,
                    current_track: None,
                    progress: (0, 0),
                });

                let request = match link_type {
                    LinkType::Album => DownloadRequest::Album {
                        id,
                        link: error.link.clone(),
                        portable: error.portable,
                        format: error.format.clone(),
                        quality: error.quality.clone(),
                    },
                    LinkType::Playlist => DownloadRequest::Playlist {
                        id,
                        link: error.link.clone(),
                        portable: error.portable,
                        format: error.format.clone(),
                        quality: error.quality.clone(),
                    },
                    LinkType::YouTubePlaylist => DownloadRequest::YouTubePlaylist {
                        id,
                        link: error.link.clone(),
                        portable: error.portable,
                        format: error.format.clone(),
                        quality: error.quality.clone(),
                    },
                };

                let tx = self.download_tx.clone();
                tokio::spawn(async move {
                    let _ = tx.send(request).await;
                });

                // Increment retry count and remove from error log
                self.error_log.increment_download_retry(&date, &error_id);
                self.error_log.remove_download_error(&date, &error_id);
                self.refresh_error_logs();

                self.view = View::Queue;
                self.status_message = format!("Retrying: {}", name);
            }
            ErrorTab::Convert => {
                if self.error_selected >= self.convert_errors.len() {
                    self.status_message = "No convert error selected".to_string();
                    return;
                }

                let error = self.convert_errors[self.error_selected].clone();
                let error_id = error.id.clone();

                // Check if input file still exists
                if !std::path::Path::new(&error.input_path).exists() {
                    self.status_message =
                        format!("Source file no longer exists: {}", error.input_path);
                    return;
                }

                self.next_id += 1;
                let id = self.next_id;

                let name = format!("{} - {}", error.artist, error.title);

                let request = DownloadRequest::Convert {
                    id,
                    input_path: error.input_path.clone(),
                    target_format: error.target_format.clone(),
                    quality: error.quality.clone(),
                    refresh_metadata: error.refresh_metadata,
                    artist: error.artist.clone(),
                    title: error.title.clone(),
                };

                let tx = self.download_tx.clone();
                tokio::spawn(async move {
                    let _ = tx.send(request).await;
                });

                // Increment retry count and remove from error log
                self.error_log.increment_convert_retry(&date, &error_id);
                self.error_log.remove_convert_error(&date, &error_id);
                self.refresh_error_logs();

                self.view = View::Logs;
                self.status_message = format!("Retrying conversion: {}", name);
            }
            ErrorTab::Refresh => {
                if self.error_selected >= self.refresh_errors.len() {
                    self.status_message = "No refresh error selected".to_string();
                    return;
                }

                let error = self.refresh_errors[self.error_selected].clone();
                let error_id = error.id.clone();

                // Check if input file still exists
                if !std::path::Path::new(&error.input_path).exists() {
                    self.status_message =
                        format!("Source file no longer exists: {}", error.input_path);
                    return;
                }

                self.next_id += 1;
                let id = self.next_id;

                let name = format!("{} - {}", error.artist, error.title);

                let request = DownloadRequest::RefreshMetadata {
                    id,
                    input_path: error.input_path.clone(),
                    artist: error.artist.clone(),
                    title: error.title.clone(),
                };

                let tx = self.download_tx.clone();
                tokio::spawn(async move {
                    let _ = tx.send(request).await;
                });

                // Increment retry count and remove from error log
                self.error_log.increment_refresh_retry(&date, &error_id);
                self.error_log.remove_refresh_error(&date, &error_id);
                self.refresh_error_logs();

                self.view = View::Logs;
                self.status_message = format!("Retrying metadata refresh: {}", name);
            }
        }
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
