//! Syncable trait for database models that participate in synchronization
//!
//! Models that implement `Syncable` can be automatically logged in the sync log
//! when they are created, updated, or deleted via the TransactionManager.

use sea_orm::{ActiveModelTrait, DatabaseConnection};
use serde::Serialize;
use uuid::Uuid;

/// Trait for database models that can be synchronized across devices
///
/// This trait enables automatic sync log creation when a model is modified
/// through the TransactionManager. Each syncable model must have:
/// - A globally unique ID (UUID) for cross-device identification
/// - A version number for optimistic concurrency control
/// - A stable model type identifier used in sync logs
///
/// # Example
///
/// ```rust,ignore
/// use sd_core::infra::sync::Syncable;
/// use sea_orm::entity::prelude::*;
///
/// #[derive(Clone, Debug, DeriveEntityModel, Serialize, Deserialize)]
/// #[sea_orm(table_name = "albums")]
/// pub struct Model {
///     pub id: i32,              // Database primary key (not synced)
///     pub uuid: Uuid,           // Sync identifier (synced)
///     pub name: String,
///     pub version: i64,         // For conflict resolution
///     pub created_at: DateTime<Utc>,  // Not synced (platform-specific)
///     pub updated_at: DateTime<Utc>,  // Not synced (platform-specific)
/// }
///
/// impl Syncable for Model {
///     const SYNC_MODEL: &'static str = "album";
///
///     fn sync_id(&self) -> Uuid {
///         self.uuid
///     }
///
///     fn version(&self) -> i64 {
///         self.version
///     }
///
///     fn exclude_fields() -> Option<&'static [&'static str]> {
///         // Don't sync database IDs or timestamps
///         Some(&["id", "created_at", "updated_at"])
///     }
/// }
/// ```
pub trait Syncable: Serialize + Clone {
	/// Stable model identifier used in sync logs
	///
	/// This must be unique across all syncable models and should never change.
	/// Examples: "album", "tag", "entry", "location"
	const SYNC_MODEL: &'static str;

	/// Get the globally unique ID for this resource
	///
	/// This ID must be consistent across all devices syncing this resource.
	/// Typically this is a UUID field on the model.
	fn sync_id(&self) -> Uuid;

	/// Get the version number for optimistic concurrency control
	///
	/// This is incremented with each update and used to resolve conflicts.
	/// The higher version wins in case of concurrent modifications.
	fn version(&self) -> i64;

	/// Optional: Exclude platform-specific or derived fields from sync
	///
	/// Fields listed here will be filtered out before serialization for sync.
	/// Common exclusions:
	/// - Database auto-increment IDs (e.g., "id")
	/// - Platform-specific timestamps (e.g., "created_at", "updated_at")
	/// - Derived/computed fields
	/// - Local-only state
	///
	/// # Example
	///
	/// ```rust,ignore
	/// fn exclude_fields() -> Option<&'static [&'static str]> {
	///     Some(&["id", "created_at", "updated_at"])
	/// }
	/// ```
	fn exclude_fields() -> Option<&'static [&'static str]> {
		None
	}

	/// Convert to sync-safe JSON representation
	///
	/// By default, this serializes the full model to JSON. Override this
	/// method to customize the serialization (e.g., to apply field exclusions
	/// or transform data before syncing).
	///
	/// # Errors
	///
	/// Returns an error if serialization fails.
	fn to_sync_json(&self) -> Result<serde_json::Value, serde_json::Error> {
		let mut value = serde_json::to_value(self)?;

		// Apply field exclusions if specified
		if let Some(excluded) = Self::exclude_fields() {
			if let Some(obj) = value.as_object_mut() {
				for field in excluded {
					obj.remove(*field);
				}
			}
		}

		Ok(value)
	}

	// TODO: Reimplement with leaderless architecture
	// Old apply_sync_entry removed - will use PeerSync directly
}

/// Helper to validate that a model's sync_id is unique
///
/// This is used in tests to ensure no two records of the same model type
/// have the same sync_id.
#[cfg(test)]
pub fn validate_unique_sync_ids<T: Syncable>(models: &[T]) -> bool {
	use std::collections::HashSet;
	let mut seen = HashSet::new();
	models.iter().all(|m| seen.insert(m.sync_id()))
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde::{Deserialize, Serialize};

	#[derive(Clone, Debug, Serialize, Deserialize)]
	struct TestModel {
		id: i32,
		uuid: Uuid,
		name: String,
		version: i64,
		created_at: String,
	}

	impl Syncable for TestModel {
		const SYNC_MODEL: &'static str = "test_model";

		fn sync_id(&self) -> Uuid {
			self.uuid
		}

		fn version(&self) -> i64 {
			self.version
		}

		fn exclude_fields() -> Option<&'static [&'static str]> {
			Some(&["id", "created_at"])
		}
	}

	#[test]
	fn test_sync_json_excludes_fields() {
		let model = TestModel {
			id: 123,
			uuid: Uuid::new_v4(),
			name: "Test".to_string(),
			version: 1,
			created_at: "2025-01-01T00:00:00Z".to_string(),
		};

		let json = model.to_sync_json().unwrap();

		// Excluded fields should not be present
		assert!(json.get("id").is_none());
		assert!(json.get("created_at").is_none());

		// Other fields should be present
		assert!(json.get("uuid").is_some());
		assert!(json.get("name").is_some());
		assert_eq!(json.get("name").unwrap().as_str().unwrap(), "Test");
		assert!(json.get("version").is_some());
	}

	#[test]
	fn test_validate_unique_sync_ids() {
		let uuid1 = Uuid::new_v4();
		let uuid2 = Uuid::new_v4();

		let models = vec![
			TestModel {
				id: 1,
				uuid: uuid1,
				name: "A".to_string(),
				version: 1,
				created_at: "".to_string(),
			},
			TestModel {
				id: 2,
				uuid: uuid2,
				name: "B".to_string(),
				version: 1,
				created_at: "".to_string(),
			},
		];

		assert!(validate_unique_sync_ids(&models));

		// Test duplicate UUIDs
		let models_with_dup = vec![
			TestModel {
				id: 1,
				uuid: uuid1,
				name: "A".to_string(),
				version: 1,
				created_at: "".to_string(),
			},
			TestModel {
				id: 2,
				uuid: uuid1, // Duplicate!
				name: "B".to_string(),
				version: 1,
				created_at: "".to_string(),
			},
		];

		assert!(!validate_unique_sync_ids(&models_with_dup));
	}
}
