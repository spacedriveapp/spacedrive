//! Location - an indexed directory within a library
//!
//! Locations are directories that Spacedrive actively monitors and indexes.
//! They can be on any device and are addressed using SdPath.

use crate::domain::addressing::SdPath;
use crate::domain::resource::Identifiable;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::time::Duration;
use uuid::Uuid;

/// An indexed directory that Spacedrive monitors
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Location {
	/// Unique identifier
	pub id: Uuid,

	/// Library this location belongs to
	pub library_id: Uuid,

	/// Root path of this location (includes device!)
	pub sd_path: SdPath,

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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Type)]
pub enum IndexMode {
	/// Just filesystem metadata (name, size, dates)
	Shallow,

	/// Generate content IDs for deduplication
	Content,

	/// Full indexing - content IDs, text extraction, thumbnails
	Deep,
}

/// Current scanning state of a location
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Type)]
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
		sd_path: SdPath,
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
				"*.tmp".to_string(),        // Temporary files
				"node_modules".to_string(), // Node.js
				"__pycache__".to_string(),  // Python
				".git".to_string(),         // Git
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
		IndexMode::Deep
	}
}

impl Identifiable for Location {
	fn id(&self) -> Uuid {
		self.id
	}

	fn resource_type() -> &'static str {
		"location"
	}
}

impl Location {
	/// Build Location from database model (for event emission)
	pub fn from_db_model(
		model: &crate::infra::db::entities::location::Model,
		library_id: Uuid,
		sd_path: SdPath,
	) -> Self {
		let index_mode = match model.index_mode.as_str() {
			"shallow" => IndexMode::Shallow,
			"content" => IndexMode::Content,
			"deep" => IndexMode::Deep,
			_ => IndexMode::Deep,
		};

		let scan_state = match model.scan_state.as_str() {
			"pending" | "idle" => ScanState::Idle,
			"scanning" => ScanState::Scanning { progress: 0 },
			"completed" => ScanState::Completed,
			"error" | "failed" => ScanState::Failed,
			_ => ScanState::Idle,
		};

		Self {
			id: model.uuid,
			library_id,
			sd_path,
			name: model.name.clone().unwrap_or_else(|| "Unknown".to_string()),
			index_mode,
			scan_interval: None,
			total_size: model.total_byte_size as u64,
			file_count: model.total_file_count as u64,
			directory_count: 0, // Not tracked in DB model yet
			scan_state,
			created_at: model.created_at.into(),
			updated_at: model.updated_at.into(),
			last_scan_at: model.last_scan_at.map(|dt| dt.into()),
			is_available: true,
			ignore_patterns: vec![
				".*".to_string(),
				"*.tmp".to_string(),
				"node_modules".to_string(),
				"__pycache__".to_string(),
				".git".to_string(),
			],
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::domain::addressing::SdPath;

	#[test]
	fn test_location_creation() {
		let sd_path = SdPath::local("/Users/test/Documents");
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
		let sd_path = SdPath::local("/test");
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
