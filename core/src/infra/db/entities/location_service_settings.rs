//! Location service settings entity
//!
//! Stores per-location service configuration for watcher, stale detector, and sync.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "location_service_settings")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = false)]
	pub location_id: i32,

	// Watcher settings
	pub watcher_enabled: bool,
	/// JSON-serialized WatcherConfig: { "debounce_ms": 150, "batch_size": 10000, "recursive": true }
	pub watcher_config: Option<String>,

	// Stale detector settings
	pub stale_detector_enabled: bool,
	/// JSON-serialized StaleDetectorConfig: { "check_interval_secs": 3600, "aggressiveness": "normal", ... }
	pub stale_detector_config: Option<String>,

	// Sync settings
	pub sync_enabled: bool,
	/// JSON-serialized SyncConfig: { "mode": "mirror", "conflict_resolution": "newest_wins" }
	pub sync_config: Option<String>,

	// Timestamps
	pub created_at: DateTimeUtc,
	pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(
		belongs_to = "super::location::Entity",
		from = "Column::LocationId",
		to = "super::location::Column::Id",
		on_delete = "Cascade"
	)]
	Location,
}

impl Related<super::location::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::Location.def()
	}
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
	/// Create default settings for a new location
	pub fn default_for_location(location_id: i32) -> Self {
		Self {
			location_id,
			watcher_enabled: true,
			watcher_config: None, // Use system defaults
			stale_detector_enabled: true,
			stale_detector_config: None, // Use system defaults
			sync_enabled: false,
			sync_config: None,
			created_at: chrono::Utc::now().into(),
			updated_at: chrono::Utc::now().into(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_default_settings() {
		let settings = Model::default_for_location(1);
		assert_eq!(settings.location_id, 1);
		assert!(settings.watcher_enabled);
		assert!(settings.stale_detector_enabled);
		assert!(!settings.sync_enabled);
	}
}
