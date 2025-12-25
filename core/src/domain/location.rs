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

	/// Job execution policies for this location
	#[serde(default)]
	pub job_policies: JobPolicies,
}

/// How deeply to index files in this location
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Type)]
pub enum IndexMode {
	/// Location exists but is not indexed
	None,

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
	pub fn new(library_id: Uuid, name: String, sd_path: SdPath, index_mode: IndexMode) -> Self {
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
			job_policies: JobPolicies::default(),
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
			if pattern == ".*" {
				// Match files/directories starting with a dot
				path.split('/')
					.any(|part| part.starts_with('.') && part != ".")
			} else if pattern.starts_with("*.") {
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

	async fn from_ids(
		db: &sea_orm::DatabaseConnection,
		ids: &[Uuid],
	) -> crate::common::errors::Result<Vec<Self>>
	where
		Self: Sized,
	{
		use crate::domain::addressing::SdPath;
		use crate::infra::db::entities::{device, directory_paths, entry, location};
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let locations_with_entries = location::Entity::find()
			.filter(location::Column::Uuid.is_in(ids.to_vec()))
			.find_also_related(entry::Entity)
			.all(db)
			.await?;

		let mut results = Vec::new();

		for (loc, entry_opt) in locations_with_entries {
			let Some(entry) = entry_opt else {
				tracing::warn!("Location {} has no root entry, skipping", loc.uuid);
				continue;
			};

			let Some(dir_path) = directory_paths::Entity::find_by_id(entry.id)
				.one(db)
				.await?
			else {
				tracing::warn!(
					"No directory path for location {} entry {}",
					loc.uuid,
					entry.id
				);
				continue;
			};

			let Some(device_model) = device::Entity::find_by_id(loc.device_id).one(db).await?
			else {
				tracing::warn!("Device not found for location {}", loc.uuid);
				continue;
			};

			// Note: Each library has its own database, so all locations in this DB
			// belong to the same library. The library_id field is populated from
			// context when needed, here we use Uuid::nil as a placeholder.
			let library_id = Uuid::nil();

			let sd_path = SdPath::Physical {
				device_slug: device_model.slug.clone(),
				path: dir_path.path.clone().into(),
			};

			results.push(Location::from_db_model(&loc, library_id, sd_path));
		}

		Ok(results)
	}
}

// Register Location as a simple resource
crate::register_resource!(Location);

impl Location {
	/// Build Location from database model (for event emission)
	pub fn from_db_model(
		model: &crate::infra::db::entities::location::Model,
		library_id: Uuid,
		sd_path: SdPath,
	) -> Self {
		let index_mode = match model.index_mode.as_str() {
			"none" => IndexMode::None,
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

		let job_policies = model
			.job_policies
			.as_ref()
			.and_then(|json| serde_json::from_str(json).ok())
			.unwrap_or_default();

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
			job_policies,
		}
	}
}

/// Job execution policies for a location
///
/// Controls which automated jobs run on this location and their configuration.
/// This allows per-location customization of thumbnail generation, OCR, speech-to-text, etc.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct JobPolicies {
	/// Thumbnail generation policy
	#[serde(default)]
	pub thumbnail: ThumbnailPolicy,

	/// Thumbstrip generation policy
	#[serde(default)]
	pub thumbstrip: ThumbstripPolicy,

	/// Proxy/sidecar generation policy (video scrubbing)
	#[serde(default)]
	pub proxy: ProxyPolicy,

	/// OCR (text extraction) policy
	#[serde(default)]
	pub ocr: OcrPolicy,

	/// Speech-to-text transcription policy
	#[serde(default)]
	pub speech_to_text: SpeechPolicy,

	/// Object detection policy (future)
	#[serde(default)]
	pub object_detection: ObjectDetectionPolicy,
}

impl Default for JobPolicies {
	fn default() -> Self {
		Self {
			thumbnail: ThumbnailPolicy::default(),
			thumbstrip: ThumbstripPolicy::default(),
			proxy: ProxyPolicy::default(),
			ocr: OcrPolicy::default(),
			speech_to_text: SpeechPolicy::default(),
			object_detection: ObjectDetectionPolicy::default(),
		}
	}
}

/// Thumbnail generation policy
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ThumbnailPolicy {
	/// Whether to generate thumbnails for this location
	pub enabled: bool,

	/// Specific thumbnail sizes to generate (empty = use defaults)
	pub sizes: Vec<u32>,

	/// JPEG quality (0-100)
	pub quality: u8,

	/// Whether to regenerate existing thumbnails
	pub regenerate: bool,
}

impl Default for ThumbnailPolicy {
	fn default() -> Self {
		Self {
			enabled: true,
			sizes: vec![], // Empty = use defaults from ThumbnailVariants
			quality: 85,
			regenerate: false,
		}
	}
}

impl ThumbnailPolicy {
	/// Convert this policy to a ThumbnailJobConfig for job dispatch
	#[cfg(feature = "ffmpeg")]
	pub fn to_job_config(&self) -> crate::ops::media::thumbnail::ThumbnailJobConfig {
		use crate::ops::media::thumbnail::{ThumbnailJobConfig, ThumbnailVariants};

		let variants = if self.sizes.is_empty() {
			// Use defaults if no specific sizes configured
			ThumbnailVariants::defaults()
		} else {
			// Map configured sizes to variants
			self.sizes
				.iter()
				.filter_map(|&size| ThumbnailVariants::from_size(size))
				.collect()
		};

		ThumbnailJobConfig {
			variants,
			regenerate: self.regenerate,
			batch_size: 50,
			max_concurrent: 4,
			run_in_background: false,
		}
	}
}

/// Thumbstrip generation policy
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ThumbstripPolicy {
	/// Whether to generate thumbstrips for this location
	pub enabled: bool,

	/// Whether to regenerate existing thumbstrips
	pub regenerate: bool,
}

impl Default for ThumbstripPolicy {
	fn default() -> Self {
		Self {
			enabled: false, // Disabled by default (expensive operation)
			regenerate: false,
		}
	}
}

impl ThumbstripPolicy {
	/// Convert this policy to a ThumbstripJobConfig for job dispatch
	#[cfg(feature = "ffmpeg")]
	pub fn to_job_config(&self) -> crate::ops::media::thumbstrip::ThumbstripJobConfig {
		crate::ops::media::thumbstrip::ThumbstripJobConfig {
			variants: crate::ops::media::thumbstrip::ThumbstripVariants::defaults(),
			regenerate: self.regenerate,
			batch_size: 10,
		}
	}
}

/// Proxy/sidecar generation policy (video scrubbing)
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProxyPolicy {
	/// Whether to generate proxy files for this location
	pub enabled: bool,

	/// Whether to regenerate existing proxies
	pub regenerate: bool,
}

impl Default for ProxyPolicy {
	fn default() -> Self {
		Self {
			enabled: false, // Disabled by default (expensive operation)
			regenerate: false,
		}
	}
}

/// OCR (text extraction) policy
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct OcrPolicy {
	/// Whether to run OCR on this location
	pub enabled: bool,

	/// Languages to use for OCR (e.g., ["eng", "spa"])
	pub languages: Vec<String>,

	/// Minimum confidence threshold (0.0 - 1.0)
	pub min_confidence: f32,

	/// Whether to reprocess files that already have text
	pub reprocess: bool,
}

impl Default for OcrPolicy {
	fn default() -> Self {
		Self {
			enabled: false,
			languages: vec!["eng".to_string()],
			min_confidence: 0.6,
			reprocess: false,
		}
	}
}

impl OcrPolicy {
	/// Convert this policy to an OcrJobConfig for job dispatch
	pub fn to_job_config(&self, location_id: Option<Uuid>) -> crate::ops::media::ocr::OcrJobConfig {
		crate::ops::media::ocr::OcrJobConfig {
			location_id,
			entry_uuid: None,
			languages: self.languages.clone(),
			min_confidence: self.min_confidence,
			reprocess: self.reprocess,
		}
	}
}

/// Speech-to-text transcription policy
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpeechPolicy {
	/// Whether to run speech-to-text on this location
	pub enabled: bool,

	/// Language for transcription
	pub language: Option<String>,

	/// Model to use (e.g., "base", "small", "medium", "large")
	pub model: String,

	/// Whether to reprocess files that already have transcriptions
	pub reprocess: bool,
}

impl Default for SpeechPolicy {
	fn default() -> Self {
		Self {
			enabled: false,
			language: None, // Auto-detect
			model: "base".to_string(),
			reprocess: false,
		}
	}
}

impl SpeechPolicy {
	/// Convert this policy to a SpeechToTextJobConfig for job dispatch
	#[cfg(feature = "ffmpeg")]
	pub fn to_job_config(
		&self,
		location_id: Option<Uuid>,
	) -> crate::ops::media::speech::SpeechToTextJobConfig {
		crate::ops::media::speech::SpeechToTextJobConfig {
			location_id,
			entry_uuid: None,
			language: self.language.clone(),
			model: self.model.clone(),
			reprocess: self.reprocess,
		}
	}
}

/// Object detection policy (for future AI features)
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ObjectDetectionPolicy {
	/// Whether to run object detection on this location
	pub enabled: bool,

	/// Minimum confidence threshold (0.0 - 1.0)
	pub min_confidence: f32,

	/// Categories to detect (empty = all)
	pub categories: Vec<String>,

	/// Whether to reprocess files that already have object data
	pub reprocess: bool,
}

impl Default for ObjectDetectionPolicy {
	fn default() -> Self {
		Self {
			enabled: false,
			min_confidence: 0.7,
			categories: vec![],
			reprocess: false,
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
