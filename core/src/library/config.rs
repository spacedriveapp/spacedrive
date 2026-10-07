//! Library configuration types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::ops::indexing::sources::{SourceConfig, StorePlacement};

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

	/// File extensions to ignore during indexing
	pub ignored_extensions: Vec<String>,

	/// TODO: ai slop config pls remove this
	pub max_file_size: Option<u64>,

	/// Whether to automatically track system volumes
	pub auto_track_system_volumes: bool,

	/// Whether to automatically track external volumes when connected
	pub auto_track_external_volumes: bool,

	/// Indexer settings (rule toggles and related)
	#[serde(default)]
	pub indexer: IndexerSettings,

	/// What a new source starts with when nothing on the add overrides it.
	#[serde(default)]
	pub adding: AddDefaults,
}

/// Defaults for Add to Library, under Library Settings > Adding content.
///
/// Every entry point resolves the same effective settings from these plus the
/// add's own overrides, so a folder, a whole volume and a remote path do not
/// each carry their own notion of what a source starts as. A one-off override
/// never writes back here; changing these initializes later sources and
/// policies and moves no existing store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct AddDefaults {
	/// Where a new source's catalog lives.
	#[serde(default)]
	pub placement: StorePlacement,
	/// For an on-source store, keep a replica in the library so the catalog
	/// answers while the source is disconnected.
	#[serde(default = "AddDefaults::default_true")]
	pub keep_offline_copy: bool,
	/// Record everything readable instead of applying the rules that hide
	/// system files, `.git` and dev directories.
	#[serde(default)]
	pub unfiltered: bool,
	/// Identify the bytes behind each record once the walk lands.
	#[serde(default = "AddDefaults::default_true")]
	pub identify_content: bool,
}

impl AddDefaults {
	fn default_true() -> bool {
		true
	}

	/// The settings a source starts with: these, with each override the add
	/// names taking the place of its default.
	pub fn resolve(&self, overrides: &AddOverrides) -> SourceConfig {
		SourceConfig {
			unfiltered: overrides.unfiltered.unwrap_or(self.unfiltered),
			placement: overrides.placement.unwrap_or(self.placement),
			keep_offline_copy: overrides
				.keep_offline_copy
				.unwrap_or(self.keep_offline_copy),
			identify_content: overrides.identify_content.unwrap_or(self.identify_content),
		}
	}
}

impl Default for AddDefaults {
	fn default() -> Self {
		Self {
			placement: StorePlacement::InLibrary,
			keep_offline_copy: true,
			unfiltered: false,
			identify_content: true,
		}
	}
}

/// What one add changes from the library's defaults. Every field absent
/// means the default; the add itself is the only thing an override touches.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct AddOverrides {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub placement: Option<StorePlacement>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub keep_offline_copy: Option<bool>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub unfiltered: Option<bool>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub identify_content: Option<bool>,
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
			adding: AddDefaults::default(),
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

	/// Number of sources registered in this library. Recomputed by every
	/// statistics pass, so a config without it starts from zero.
	#[serde(default)]
	pub source_count: u32,

	/// Number of tags created
	pub tag_count: u32,

	/// Number of devices in this library (v2 field, defaults to 0 for old configs)
	#[serde(default)]
	pub device_count: u32,

	/// Number of unique content identities in this library (v2 field, defaults to 0 for old configs)
	#[serde(default)]
	pub unique_content_count: u64,

	/// Total storage capacity across all volumes in bytes (v2 field, defaults to 0 for old configs)
	#[serde(default)]
	pub total_capacity: u64,

	/// Available storage across all volumes in bytes (v2 field, defaults to 0 for old configs)
	#[serde(default)]
	pub available_capacity: u64,

	/// Database file size in bytes
	pub database_size: u64,

	/// Total number of sidecar files (all types: thumbnails, embeddings, etc.)
	#[serde(default)]
	pub sidecar_count: u64,

	/// Total size of all sidecar files in bytes
	#[serde(default)]
	pub sidecar_size: u64,

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
			source_count: 0,
			tag_count: 0,
			device_count: 0,
			unique_content_count: 0,
			total_capacity: 0,
			available_capacity: 0,
			database_size: 0,
			sidecar_count: 0,
			sidecar_size: 0,
			last_indexed: None,
			updated_at: Utc::now(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// The plan's proposed defaults, built as proposed: in-library placement,
	/// an offline copy kept, filtered capture, content identified.
	#[test]
	fn add_defaults_are_the_plans_proposal() {
		let defaults = AddDefaults::default();
		assert_eq!(defaults.placement, StorePlacement::InLibrary);
		assert!(defaults.keep_offline_copy);
		assert!(!defaults.unfiltered);
		assert!(defaults.identify_content);

		// A settings file written before the section existed gets them too.
		let settings: LibrarySettings =
			serde_json::from_str(&serde_json::to_string(&LibrarySettings::default()).unwrap())
				.unwrap();
		assert_eq!(settings.adding, defaults);
	}

	/// An override replaces only the default it names; the rest of the add
	/// still follows the library.
	#[test]
	fn an_override_replaces_one_default_and_leaves_the_rest() {
		let defaults = AddDefaults {
			unfiltered: true,
			..AddDefaults::default()
		};
		let effective = defaults.resolve(&AddOverrides {
			placement: Some(StorePlacement::OnSource),
			..AddOverrides::default()
		});
		assert_eq!(effective.placement, StorePlacement::OnSource);
		assert!(effective.unfiltered, "untouched defaults still apply");
		assert!(effective.keep_offline_copy);
		assert!(effective.identify_content);

		let effective = defaults.resolve(&AddOverrides {
			unfiltered: Some(false),
			identify_content: Some(false),
			..AddOverrides::default()
		});
		assert!(!effective.unfiltered);
		assert!(!effective.identify_content);
		assert_eq!(effective.placement, StorePlacement::InLibrary);
	}
}
