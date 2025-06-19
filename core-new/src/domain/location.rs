//! Location - an indexed directory within a library
//! 
//! Locations are directories that Spacedrive actively monitors and indexes.
//! They can be on any device and are addressed using SdPath.

use crate::domain::entry::SdPathSerialized;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

/// An indexed directory that Spacedrive monitors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    /// Unique identifier
    pub id: Uuid,
    
    /// Library this location belongs to
    pub library_id: Uuid,
    
    /// Root path of this location (includes device!)
    pub sd_path: SdPathSerialized,
    
    /// Human-friendly name
    pub name: String,
    
    /// Indexing configuration
    pub index_mode: IndexMode,
    
    /// How often to rescan (None = manual only)
    pub scan_interval: Option<Duration>,
    
    /// Statistics
    pub total_size: u64,
    pub file_count: u64,
    pub directory_count: u64,
    
    /// Current state
    pub scan_state: ScanState,
    
    /// Timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_scan_at: Option<DateTime<Utc>>,
    
    /// Whether this location is currently available
    pub is_available: bool,
    
    /// Hidden glob patterns (e.g., [".*", "node_modules"])
    pub ignore_patterns: Vec<String>,
}

/// How deeply to index files in this location
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum IndexMode {
    /// Just filesystem metadata (name, size, dates)
    Shallow,
    
    /// Generate content IDs for deduplication
    Content,
    
    /// Full indexing - content IDs, text extraction, thumbnails
    Deep,
}

/// Current scanning state of a location
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ScanState {
    /// Not currently being scanned
    Idle,
    
    /// Currently scanning
    Scanning {
        /// Progress percentage (0-100)
        progress: u8,
    },
    
    /// Scan completed successfully
    Completed,
    
    /// Scan failed with error
    Failed,
    
    /// Scan was paused
    Paused,
}

impl Location {
    /// Create a new location
    pub fn new(
        library_id: Uuid,
        name: String,
        sd_path: SdPathSerialized,
        index_mode: IndexMode,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            library_id,
            sd_path,
            name,
            index_mode,
            scan_interval: None,
            total_size: 0,
            file_count: 0,
            directory_count: 0,
            scan_state: ScanState::Idle,
            created_at: now,
            updated_at: now,
            last_scan_at: None,
            is_available: true,
            ignore_patterns: vec![
                ".*".to_string(),           // Hidden files
                "*.tmp".to_string(),         // Temporary files
                "node_modules".to_string(),  // Node.js
                "__pycache__".to_string(),   // Python
                ".git".to_string(),          // Git
            ],
        }
    }
    
    /// Check if this location is currently being scanned
    pub fn is_scanning(&self) -> bool {
        matches!(self.scan_state, ScanState::Scanning { .. })
    }
    
    /// Check if this location needs scanning based on interval
    pub fn needs_scan(&self) -> bool {
        if !self.is_available {
            return false;
        }
        
        match (self.scan_interval, self.last_scan_at) {
            (Some(interval), Some(last_scan)) => {
                let next_scan = last_scan + chrono::Duration::from_std(interval).unwrap();
                Utc::now() >= next_scan
            }
            (Some(_), None) => true, // Never scanned but has interval
            (None, _) => false,      // Manual scan only
        }
    }
    
    /// Update scan progress
    pub fn set_scan_progress(&mut self, progress: u8) {
        self.scan_state = ScanState::Scanning {
            progress: progress.min(100),
        };
        self.updated_at = Utc::now();
    }
    
    /// Mark scan as completed
    pub fn complete_scan(&mut self, file_count: u64, directory_count: u64, total_size: u64) {
        self.scan_state = ScanState::Completed;
        self.file_count = file_count;
        self.directory_count = directory_count;
        self.total_size = total_size;
        self.last_scan_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }
    
    /// Mark scan as failed
    pub fn fail_scan(&mut self) {
        self.scan_state = ScanState::Failed;
        self.updated_at = Utc::now();
    }
    
    /// Check if a path should be ignored
    pub fn should_ignore(&self, path: &str) -> bool {
        self.ignore_patterns.iter().any(|pattern| {
            // Simple glob matching (could use glob crate for full support)
            if pattern.starts_with("*.") {
                path.ends_with(&pattern[1..])
            } else if pattern.starts_with('.') {
                path.split('/').any(|part| part == pattern)
            } else {
                path.contains(pattern)
            }
        })
    }
}

impl Default for IndexMode {
    fn default() -> Self {
        IndexMode::Content
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::types::SdPath;
    
    #[test]
    fn test_location_creation() {
        let sd_path = SdPathSerialized::from_sdpath(&SdPath::local("/Users/test/Documents"));
        let location = Location::new(
            Uuid::new_v4(),
            "My Documents".to_string(),
            sd_path,
            IndexMode::Deep,
        );
        
        assert_eq!(location.name, "My Documents");
        assert_eq!(location.index_mode, IndexMode::Deep);
        assert!(location.is_available);
        assert!(!location.is_scanning());
    }
    
    #[test]
    fn test_ignore_patterns() {
        let sd_path = SdPathSerialized::from_sdpath(&SdPath::local("/test"));
        let location = Location::new(
            Uuid::new_v4(),
            "Test".to_string(),
            sd_path,
            IndexMode::Shallow,
        );
        
        assert!(location.should_ignore(".hidden_file"));
        assert!(location.should_ignore("file.tmp"));
        assert!(location.should_ignore("/path/to/node_modules/file.js"));
        assert!(!location.should_ignore("normal_file.txt"));
    }
}