//! Library configuration types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Library configuration stored in library.json
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LibraryConfig {
	/// Version of the configuration format
	pub version: u32,

	/// Unique identifier for this library
	pub id: Uuid,

	/// Human-readable name
	pub name: String,

	/// Optional description
	pub description: Option<String>,

	/// When the library was created
	pub created_at: DateTime<Utc>,

	/// When the library was last modified
	pub updated_at: DateTime<Utc>,

	/// Library-specific settings
	pub settings: LibrarySettings,

	/// Library statistics
	pub statistics: LibraryStatistics,
}

/// Library-specific settings
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LibrarySettings {
	/// Whether to generate thumbnails for media files
	pub generate_thumbnails: bool,

	/// Thumbnail quality (0-100)
	pub thumbnail_quality: u8,

	/// Whether to enable AI-powered tagging
	pub enable_ai_tagging: bool,

	/// Whether sync is enabled for this library
	pub sync_enabled: bool,

	/// Whether the library is encrypted at rest
	pub encryption_enabled: bool,

	/// Custom thumbnail sizes to generate
	pub thumbnail_sizes: Vec<u32>,

	/// File extensions to ignore during indexing
	pub ignored_extensions: Vec<String>,

	/// Maximum file size to index (in bytes)
	pub max_file_size: Option<u64>,

	/// Whether to automatically track system volumes
	pub auto_track_system_volumes: bool,

	/// Whether to automatically track external volumes when connected
	pub auto_track_external_volumes: bool,

	/// Indexer settings (rule toggles and related)
	#[serde(default)]
	pub indexer: IndexerSettings,
}

impl LibraryConfig {
	/// Load library configuration from a JSON file
	pub async fn load(path: &std::path::Path) -> Result<Self, super::error::LibraryError> {
		let config_data = tokio::fs::read_to_string(path)
			.await
			.map_err(|e| super::error::LibraryError::IoError(e))?;
		let config: LibraryConfig = serde_json::from_str(&config_data)
			.map_err(|e| super::error::LibraryError::JsonError(e))?;
		Ok(config)
	}
}

impl Default for LibrarySettings {
	fn default() -> Self {
		Self {
			generate_thumbnails: true,
			thumbnail_quality: 85,
			enable_ai_tagging: false,
			sync_enabled: false,
			encryption_enabled: false,
			thumbnail_sizes: vec![128, 256, 512],
			ignored_extensions: vec![
				".tmp".to_string(),
				".temp".to_string(),
				".cache".to_string(),
				".part".to_string(),
			],
			max_file_size: Some(100 * 1024 * 1024 * 1024), // 100GB
			auto_track_system_volumes: true,               // Default to true for user convenience
			auto_track_external_volumes: false,            // Default to false for privacy
			indexer: IndexerSettings::default(),
		}
	}
}

/// Indexer settings controlling rule toggles
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct IndexerSettings {
	#[serde(default = "IndexerSettings::default_true")]
	pub no_system_files: bool,
	#[serde(default = "IndexerSettings::default_true")]
	pub no_git: bool,
	#[serde(default = "IndexerSettings::default_true")]
	pub no_dev_dirs: bool,
	#[serde(default)]
	pub no_hidden: bool,
	#[serde(default = "IndexerSettings::default_true")]
	pub gitignore: bool,
	#[serde(default)]
	pub only_images: bool,
}

impl IndexerSettings {
	fn default_true() -> bool {
		true
	}
}

impl Default for IndexerSettings {
	fn default() -> Self {
		Self {
			no_system_files: true,
			no_git: true,
			no_dev_dirs: true,
			no_hidden: false,
			gitignore: true,
			only_images: false,
		}
	}
}

/// Library statistics
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LibraryStatistics {
	/// Total number of files indexed
	pub total_files: u64,

	/// Total size of all files in bytes
	pub total_size: u64,

	/// Number of locations in this library
	pub location_count: u32,

	/// Number of tags created
	pub tag_count: u32,

	/// Number of thumbnails generated
	pub thumbnail_count: u64,

	/// Database file size in bytes
	pub database_size: u64,

	/// Last time the library was fully indexed
	pub last_indexed: Option<DateTime<Utc>>,

	/// When these statistics were last updated
	pub updated_at: DateTime<Utc>,
}

impl Default for LibraryStatistics {
	fn default() -> Self {
		Self {
			total_files: 0,
			total_size: 0,
			location_count: 0,
			tag_count: 0,
			thumbnail_count: 0,
			database_size: 0,
			last_indexed: None,
			updated_at: Utc::now(),
		}
	}
}

/// Thumbnail generation metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThumbnailMetadata {
	/// Version of the thumbnail format
	pub version: u32,

	/// Quality setting used for generation
	pub quality: u8,

	/// Sizes that were generated
	pub sizes: Vec<u32>,

	/// When this metadata was created
	pub created_at: DateTime<Utc>,
}

impl Default for ThumbnailMetadata {
	fn default() -> Self {
		Self {
			version: 1,
			quality: 85,
			sizes: vec![128, 256, 512],
			created_at: Utc::now(),
		}
	}
}
