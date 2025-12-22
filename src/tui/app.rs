use crate::db::{DownloadDB, TrackEntry};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum View {
    Main,
    AddLink,
    Queue,
    Library,
}

#[derive(Debug, Clone)]
pub struct QueueItem {
    pub artist: String,
    pub title: String,
    pub status: DownloadStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DownloadStatus {
    Pending,
    Downloading,
    Complete,
    Failed(String),
}

pub struct App {
    pub running: bool,
    pub view: View,
    pub input: String,
    pub input_mode: bool,
    pub queue: Vec<QueueItem>,
    pub queue_selected: usize,
    pub library: Vec<TrackEntry>,
    pub library_selected: usize,
    pub status_message: String,
    pub db: DownloadDB,
    pub music_path: PathBuf,
    pub playlist_path: PathBuf,
}

impl App {
    pub fn new() -> Self {
        let music_path = PathBuf::from("data/music");
        let playlist_path = PathBuf::from("data/playlists");
        let cache_path = "data/cache/downloaded_songs.json";

        // Ensure directories exist
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
            queue: Vec::new(),
            queue_selected: 0,
            library,
            library_selected: 0,
            status_message: "Welcome to rustwav! Press 'a' to add album, 'p' for playlist".to_string(),
            db,
            music_path,
            playlist_path,
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

    pub fn start_add_album(&mut self) {
        self.view = View::AddLink;
        self.input_mode = true;
        self.input.clear();
        self.status_message = "Enter Spotify album link:".to_string();
    }

    pub fn start_add_playlist(&mut self) {
        self.view = View::AddLink;
        self.input_mode = true;
        self.input.clear();
        self.status_message = "Enter Spotify playlist link:".to_string();
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

        if link.contains("album") {
            self.status_message = format!("Added album to queue: {}", link);
            self.queue.push(QueueItem {
                artist: "Loading...".to_string(),
                title: link,
                status: DownloadStatus::Pending,
            });
        } else if link.contains("playlist") {
            self.status_message = format!("Added playlist to queue: {}", link);
            self.queue.push(QueueItem {
                artist: "Loading...".to_string(),
                title: link,
                status: DownloadStatus::Pending,
            });
        } else {
            self.status_message = "Invalid link - must be a Spotify album or playlist URL".to_string();
        }
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
        self.library = self.db.tracks.iter().cloned().collect();
    }
}
