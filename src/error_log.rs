use chrono::{DateTime, Local, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Error types for categorizing errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorType {
    Download,
    Convert,
    Refresh,
}

impl ErrorType {
    pub fn filename(&self) -> &'static str {
        match self {
            ErrorType::Download => "download.json",
            ErrorType::Convert => "convert.json",
            ErrorType::Refresh => "refresh.json",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ErrorType::Download => "Download",
            ErrorType::Convert => "Convert",
            ErrorType::Refresh => "Refresh",
        }
    }
}

/// Error entry for failed download operations (album/playlist tracks)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DownloadErrorEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub link: String,
    pub link_type: String, // "album" or "playlist"
    pub format: String,
    pub quality: String,
    pub portable: bool,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub error: String,
    pub retry_count: u32,
}

impl DownloadErrorEntry {
    pub fn new(
        link: String,
        link_type: String,
        format: String,
        quality: String,
        portable: bool,
        artist: Option<String>,
        title: Option<String>,
        error: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            link,
            link_type,
            format,
            quality,
            portable,
            artist,
            title,
            error,
            retry_count: 0,
        }
    }
}

/// Error entry for failed conversion operations
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ConvertErrorEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub input_path: String,
    pub target_format: String,
    pub quality: String,
    pub refresh_metadata: bool,
    pub artist: String,
    pub title: String,
    pub error: String,
    pub retry_count: u32,
}

impl ConvertErrorEntry {
    pub fn new(
        input_path: String,
        target_format: String,
        quality: String,
        refresh_metadata: bool,
        artist: String,
        title: String,
        error: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            input_path,
            target_format,
            quality,
            refresh_metadata,
            artist,
            title,
            error,
            retry_count: 0,
        }
    }
}

/// Error entry for failed metadata refresh operations
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RefreshErrorEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub input_path: String,
    pub artist: String,
    pub title: String,
    pub error: String,
    pub retry_count: u32,
}

impl RefreshErrorEntry {
    pub fn new(input_path: String, artist: String, title: String, error: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            input_path,
            artist,
            title,
            error,
            retry_count: 0,
        }
    }
}

/// Manages error logs organized by date and error type
pub struct ErrorLogManager {
    base_path: PathBuf,
}

impl ErrorLogManager {
    pub fn new(base_path: &str) -> Self {
        let path = PathBuf::from(base_path);
        let _ = fs::create_dir_all(&path);
        Self { base_path: path }
    }

    /// Get today's date as a string (YYYY-MM-DD)
    fn today_str() -> String {
        Local::now().format("%Y-%m-%d").to_string()
    }

    /// Get the path for a specific date and error type
    fn get_log_path(&self, date: &str, error_type: ErrorType) -> PathBuf {
        self.base_path.join(date).join(error_type.filename())
    }

    /// Ensure the date directory exists
    fn ensure_date_dir(&self, date: &str) {
        let date_dir = self.base_path.join(date);
        let _ = fs::create_dir_all(&date_dir);
    }

    /// Add a download error entry
    pub fn add_download_error(&self, entry: DownloadErrorEntry) {
        let date = Self::today_str();
        self.ensure_date_dir(&date);
        let path = self.get_log_path(&date, ErrorType::Download);

        let mut entries = self.load_download_errors_from_path(&path);
        entries.push(entry);
        self.save_entries(&path, &entries);
    }

    /// Add a convert error entry
    pub fn add_convert_error(&self, entry: ConvertErrorEntry) {
        let date = Self::today_str();
        self.ensure_date_dir(&date);
        let path = self.get_log_path(&date, ErrorType::Convert);

        let mut entries = self.load_convert_errors_from_path(&path);
        entries.push(entry);
        self.save_entries(&path, &entries);
    }

    /// Add a refresh error entry
    pub fn add_refresh_error(&self, entry: RefreshErrorEntry) {
        let date = Self::today_str();
        self.ensure_date_dir(&date);
        let path = self.get_log_path(&date, ErrorType::Refresh);

        let mut entries = self.load_refresh_errors_from_path(&path);
        entries.push(entry);
        self.save_entries(&path, &entries);
    }

    /// Remove a download error by ID and date
    pub fn remove_download_error(&self, date: &str, id: &str) -> bool {
        let path = self.get_log_path(date, ErrorType::Download);
        let mut entries = self.load_download_errors_from_path(&path);
        let original_len = entries.len();
        entries.retain(|e| e.id != id);

        if entries.len() != original_len {
            if entries.is_empty() {
                let _ = fs::remove_file(&path);
                self.cleanup_empty_date_dir(date);
            } else {
                self.save_entries(&path, &entries);
            }
            true
        } else {
            false
        }
    }

    /// Remove a convert error by ID and date
    pub fn remove_convert_error(&self, date: &str, id: &str) -> bool {
        let path = self.get_log_path(date, ErrorType::Convert);
        let mut entries = self.load_convert_errors_from_path(&path);
        let original_len = entries.len();
        entries.retain(|e| e.id != id);

        if entries.len() != original_len {
            if entries.is_empty() {
                let _ = fs::remove_file(&path);
                self.cleanup_empty_date_dir(date);
            } else {
                self.save_entries(&path, &entries);
            }
            true
        } else {
            false
        }
    }

    /// Remove a refresh error by ID and date
    pub fn remove_refresh_error(&self, date: &str, id: &str) -> bool {
        let path = self.get_log_path(date, ErrorType::Refresh);
        let mut entries = self.load_refresh_errors_from_path(&path);
        let original_len = entries.len();
        entries.retain(|e| e.id != id);

        if entries.len() != original_len {
            if entries.is_empty() {
                let _ = fs::remove_file(&path);
                self.cleanup_empty_date_dir(date);
            } else {
                self.save_entries(&path, &entries);
            }
            true
        } else {
            false
        }
    }

    /// Get a download error by ID (searches all dates)
    pub fn get_download_error(&self, id: &str) -> Option<(String, DownloadErrorEntry)> {
        for date in self.list_dates() {
            let path = self.get_log_path(&date, ErrorType::Download);
            let entries = self.load_download_errors_from_path(&path);
            if let Some(entry) = entries.into_iter().find(|e| e.id == id) {
                return Some((date, entry));
            }
        }
        None
    }

    /// Get a convert error by ID (searches all dates)
    pub fn get_convert_error(&self, id: &str) -> Option<(String, ConvertErrorEntry)> {
        for date in self.list_dates() {
            let path = self.get_log_path(&date, ErrorType::Convert);
            let entries = self.load_convert_errors_from_path(&path);
            if let Some(entry) = entries.into_iter().find(|e| e.id == id) {
                return Some((date, entry));
            }
        }
        None
    }

    /// Get a refresh error by ID (searches all dates)
    pub fn get_refresh_error(&self, id: &str) -> Option<(String, RefreshErrorEntry)> {
        for date in self.list_dates() {
            let path = self.get_log_path(&date, ErrorType::Refresh);
            let entries = self.load_refresh_errors_from_path(&path);
            if let Some(entry) = entries.into_iter().find(|e| e.id == id) {
                return Some((date, entry));
            }
        }
        None
    }

    /// Increment retry count for a download error
    pub fn increment_download_retry(&self, date: &str, id: &str) {
        let path = self.get_log_path(date, ErrorType::Download);
        let mut entries = self.load_download_errors_from_path(&path);
        if let Some(entry) = entries.iter_mut().find(|e| e.id == id) {
            entry.retry_count += 1;
            entry.timestamp = Utc::now();
            self.save_entries(&path, &entries);
        }
    }

    /// Increment retry count for a convert error
    pub fn increment_convert_retry(&self, date: &str, id: &str) {
        let path = self.get_log_path(date, ErrorType::Convert);
        let mut entries = self.load_convert_errors_from_path(&path);
        if let Some(entry) = entries.iter_mut().find(|e| e.id == id) {
            entry.retry_count += 1;
            entry.timestamp = Utc::now();
            self.save_entries(&path, &entries);
        }
    }

    /// Increment retry count for a refresh error
    pub fn increment_refresh_retry(&self, date: &str, id: &str) {
        let path = self.get_log_path(date, ErrorType::Refresh);
        let mut entries = self.load_refresh_errors_from_path(&path);
        if let Some(entry) = entries.iter_mut().find(|e| e.id == id) {
            entry.retry_count += 1;
            entry.timestamp = Utc::now();
            self.save_entries(&path, &entries);
        }
    }

    /// Get all download errors for a specific date
    pub fn get_download_errors_for_date(&self, date: &str) -> Vec<DownloadErrorEntry> {
        let path = self.get_log_path(date, ErrorType::Download);
        self.load_download_errors_from_path(&path)
    }

    /// Get all convert errors for a specific date
    pub fn get_convert_errors_for_date(&self, date: &str) -> Vec<ConvertErrorEntry> {
        let path = self.get_log_path(date, ErrorType::Convert);
        self.load_convert_errors_from_path(&path)
    }

    /// Get all refresh errors for a specific date
    pub fn get_refresh_errors_for_date(&self, date: &str) -> Vec<RefreshErrorEntry> {
        let path = self.get_log_path(date, ErrorType::Refresh);
        self.load_refresh_errors_from_path(&path)
    }

    /// Get all download errors across all dates
    pub fn get_all_download_errors(&self) -> Vec<(String, DownloadErrorEntry)> {
        let mut all = Vec::new();
        for date in self.list_dates() {
            let entries = self.get_download_errors_for_date(&date);
            for entry in entries {
                all.push((date.clone(), entry));
            }
        }
        // Sort by timestamp descending (newest first)
        all.sort_by(|a, b| b.1.timestamp.cmp(&a.1.timestamp));
        all
    }

    /// Get all convert errors across all dates
    pub fn get_all_convert_errors(&self) -> Vec<(String, ConvertErrorEntry)> {
        let mut all = Vec::new();
        for date in self.list_dates() {
            let entries = self.get_convert_errors_for_date(&date);
            for entry in entries {
                all.push((date.clone(), entry));
            }
        }
        all.sort_by(|a, b| b.1.timestamp.cmp(&a.1.timestamp));
        all
    }

    /// Get all refresh errors across all dates
    pub fn get_all_refresh_errors(&self) -> Vec<(String, RefreshErrorEntry)> {
        let mut all = Vec::new();
        for date in self.list_dates() {
            let entries = self.get_refresh_errors_for_date(&date);
            for entry in entries {
                all.push((date.clone(), entry));
            }
        }
        all.sort_by(|a, b| b.1.timestamp.cmp(&a.1.timestamp));
        all
    }

    /// List all dates that have error logs (sorted newest first)
    pub fn list_dates(&self) -> Vec<String> {
        let mut dates = Vec::new();

        if let Ok(entries) = fs::read_dir(&self.base_path) {
            for entry in entries.flatten() {
                if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                    if let Some(name) = entry.file_name().to_str() {
                        // Validate it's a date format
                        if NaiveDate::parse_from_str(name, "%Y-%m-%d").is_ok() {
                            dates.push(name.to_string());
                        }
                    }
                }
            }
        }

        // Sort by date descending (newest first)
        dates.sort_by(|a, b| b.cmp(a));
        dates
    }

    /// Get error counts for a specific date
    pub fn get_error_counts(&self, date: &str) -> (usize, usize, usize) {
        let download_count = self.get_download_errors_for_date(date).len();
        let convert_count = self.get_convert_errors_for_date(date).len();
        let refresh_count = self.get_refresh_errors_for_date(date).len();
        (download_count, convert_count, refresh_count)
    }

    /// Get total error counts across all dates
    pub fn get_total_error_counts(&self) -> (usize, usize, usize) {
        let mut download_total = 0;
        let mut convert_total = 0;
        let mut refresh_total = 0;

        for date in self.list_dates() {
            let (d, c, r) = self.get_error_counts(&date);
            download_total += d;
            convert_total += c;
            refresh_total += r;
        }

        (download_total, convert_total, refresh_total)
    }

    /// Clear all errors for a specific date
    pub fn clear_date(&self, date: &str) {
        let date_dir = self.base_path.join(date);
        if date_dir.exists() {
            let _ = fs::remove_dir_all(&date_dir);
        }
    }

    /// Clear all errors of a specific type
    pub fn clear_error_type(&self, error_type: ErrorType) {
        for date in self.list_dates() {
            let path = self.get_log_path(&date, error_type);
            let _ = fs::remove_file(&path);
            self.cleanup_empty_date_dir(&date);
        }
    }

    /// Clear all error logs
    pub fn clear_all(&self) {
        if self.base_path.exists() {
            let _ = fs::remove_dir_all(&self.base_path);
            let _ = fs::create_dir_all(&self.base_path);
        }
    }

    /// Check if a date directory is empty and remove it if so
    fn cleanup_empty_date_dir(&self, date: &str) {
        let date_dir = self.base_path.join(date);
        if let Ok(mut entries) = fs::read_dir(&date_dir) {
            if entries.next().is_none() {
                let _ = fs::remove_dir(&date_dir);
            }
        }
    }

    // Helper methods for loading entries from specific paths

    fn load_download_errors_from_path(&self, path: &Path) -> Vec<DownloadErrorEntry> {
        if path.exists() {
            let data = fs::read_to_string(path).unwrap_or_default();
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    fn load_convert_errors_from_path(&self, path: &Path) -> Vec<ConvertErrorEntry> {
        if path.exists() {
            let data = fs::read_to_string(path).unwrap_or_default();
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    fn load_refresh_errors_from_path(&self, path: &Path) -> Vec<RefreshErrorEntry> {
        if path.exists() {
            let data = fs::read_to_string(path).unwrap_or_default();
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    fn save_entries<T: Serialize>(&self, path: &Path, entries: &[T]) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(data) = serde_json::to_string_pretty(entries) {
            let _ = fs::write(path, data);
        }
    }
}
