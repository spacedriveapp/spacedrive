//! Syncable trait for database models that participate in synchronization
//!
//! Models that implement `Syncable` can be automatically logged in the sync log
//! when they are created, updated, or deleted via the TransactionManager.

use sea_orm::DatabaseConnection;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
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

	/// Declare sync dependencies on other models
	///
	/// Models listed here must be synced before this model to prevent foreign key violations.
	/// This establishes the dependency graph used for topological ordering during backfill.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// fn sync_depends_on() -> &'static [&'static str] {
	///     &["device", "location"]  // Entry depends on device and location
	/// }
	/// ```
	fn sync_depends_on() -> &'static [&'static str] {
		&[]
	}

	/// Declare foreign key mappings for automatic UUID conversion
	///
	/// Models with FK relationships override this to enable UUID mapping.
	/// Models without FKs use the default (empty vec).
	///
	/// # Example
	///
	/// ```rust,ignore
	/// fn foreign_key_mappings() -> Vec<crate::infra::sync::FKMapping> {
	///     vec![
	///         crate::infra::sync::FKMapping::new("device_id", "devices"),
	///         crate::infra::sync::FKMapping::new("entry_id", "entries"),
	///     ]
	/// }
	/// ```
	fn foreign_key_mappings() -> Vec<super::FKMapping> {
		vec![]
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

	/// Query instances of this model for sync backfill (device-owned models only)
	///
	/// This is an associated function that queries the database for all instances
	/// of this model that match the given criteria. Used for backfill operations.
	///
	/// # Parameters
	/// - `device_id`: Optional device filter (for multi-device filtering)
	/// - `since`: Optional timestamp to only get records modified after this time
	/// - `batch_size`: Maximum number of records to return
	/// - `db`: Database connection
	///
	/// # Returns
	/// Vector of (uuid, json_data, timestamp) tuples
	fn query_for_sync(
		device_id: Option<Uuid>,
		since: Option<chrono::DateTime<chrono::Utc>>,
		cursor: Option<(chrono::DateTime<chrono::Utc>, Uuid)>,
		batch_size: usize,
		db: &DatabaseConnection,
	) -> impl std::future::Future<
		Output = Result<
			Vec<(Uuid, serde_json::Value, chrono::DateTime<chrono::Utc>)>,
			sea_orm::DbErr,
		>,
	> + Send
	where
		Self: Sized,
	{
		async move {
			// Default implementation returns empty - models must override
			let _ = (device_id, since, cursor, batch_size, db);
			Ok(Vec::new())
		}
	}

	/// Apply a state change from sync (device-owned models only)
	///
	/// This is an associated function that applies a state change received
	/// from another device. It should deserialize the data and upsert it
	/// into the database using "last write wins" semantics.
	///
	/// # Parameters
	/// - `data`: The JSON data for this model
	/// - `db`: Database connection
	fn apply_state_change(
		data: serde_json::Value,
		db: &DatabaseConnection,
	) -> impl std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send
	where
		Self: Sized,
	{
		async move {
			// Default implementation does nothing - models must override
			Ok(())
		}
	}

	/// Apply a shared change from sync log (shared models only)
	///
	/// This is an associated function that applies a log-based change with
	/// HLC-based conflict resolution. It should compare timestamps and
	/// only apply if the incoming change is newer.
	///
	/// # Parameters
	/// - `entry`: The SharedChangeEntry containing HLC and data
	/// - `db`: Database connection
	fn apply_shared_change(
		entry: super::SharedChangeEntry,
		db: &DatabaseConnection,
	) -> impl std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send
	where
		Self: Sized,
	{
		async move {
			// Default implementation does nothing - models must override
			Ok(())
		}
	}

	/// Apply a deletion by UUID (device-owned models only)
	///
	/// This is called when a deletion tombstone is received during sync.
	/// The implementation should:
	/// 1. Find the record by UUID
	/// 2. Delete it (with any necessary cascades)
	/// 3. Be idempotent (no-op if already deleted)
	///
	/// # Parameters
	/// - `uuid`: The UUID of the record to delete
	/// - `db`: Database connection
	///
	/// # Note
	/// For entries, this triggers cascading deletion via delete_subtree_internal.
	/// For locations, this deletes the location and its root entry tree.
	/// For volumes, this simply deletes the volume record.
	fn apply_deletion(
		uuid: Uuid,
		db: &DatabaseConnection,
	) -> impl std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send
	where
		Self: Sized,
	{
		async move {
			// Default implementation is no-op (models must override if they support deletion sync)
			let _ = (uuid, db);
			Ok(())
		}
	}

	/// Check if a record is tombstoned (deleted)
	///
	/// Used during apply_state_change to prevent re-creating deleted records.
	/// This handles the race condition where a deletion tombstone arrives
	/// before or during backfill of the record itself.
	///
	/// # Parameters
	/// - `uuid`: The UUID of the record to check
	/// - `db`: Database connection
	///
	/// # Returns
	/// True if the record has been tombstoned (deleted), false otherwise
	async fn is_tombstoned(uuid: Uuid, db: &DatabaseConnection) -> Result<bool, sea_orm::DbErr> {
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let exists = crate::infra::db::entities::device_state_tombstone::Entity::find()
			.filter(
				crate::infra::db::entities::device_state_tombstone::Column::ModelType
					.eq(Self::SYNC_MODEL),
			)
			.filter(crate::infra::db::entities::device_state_tombstone::Column::RecordUuid.eq(uuid))
			.one(db)
			.await?
			.is_some();

		Ok(exists)
	}

	// ============================================
	// FK Lookup Methods (for generic FK mapping)
	// ============================================

	/// Lookup local integer ID by UUID
	///
	/// Used by FK mapper to convert UUIDs back to local IDs during sync receive.
	/// Returns None if not found (caller handles missing records).
	///
	/// Models that are used as FK targets must implement this.
	fn lookup_id_by_uuid(
		uuid: Uuid,
		db: &DatabaseConnection,
	) -> impl std::future::Future<Output = Result<Option<i32>, sea_orm::DbErr>> + Send
	where
		Self: Sized,
	{
		async move {
			// Default: not implemented (model is not an FK target)
			let _ = (uuid, db);
			Ok(None)
		}
	}

	/// Lookup UUID by local integer ID
	///
	/// Used by FK mapper to convert local IDs to UUIDs during sync send.
	/// Returns None if not found or if UUID is null.
	///
	/// Models that are used as FK targets must implement this.
	fn lookup_uuid_by_id(
		id: i32,
		db: &DatabaseConnection,
	) -> impl std::future::Future<Output = Result<Option<Uuid>, sea_orm::DbErr>> + Send
	where
		Self: Sized,
	{
		async move {
			// Default: not implemented (model is not an FK target)
			let _ = (id, db);
			Ok(None)
		}
	}

	/// Batch lookup local IDs by UUIDs (single query optimization)
	///
	/// Returns HashMap mapping UUID -> local_id for found records.
	/// Records not found are omitted from the result.
	///
	/// This reduces database queries from N to 1 for batch FK resolution.
	fn batch_lookup_ids_by_uuids(
		uuids: HashSet<Uuid>,
		db: &DatabaseConnection,
	) -> impl std::future::Future<Output = Result<HashMap<Uuid, i32>, sea_orm::DbErr>> + Send
	where
		Self: Sized,
	{
		async move {
			// Default: not implemented (model is not an FK target)
			let _ = (uuids, db);
			Ok(HashMap::new())
		}
	}

	/// Batch lookup UUIDs by local IDs (single query optimization)
	///
	/// Returns HashMap mapping local_id -> UUID for found records.
	/// Records not found or with null UUIDs are omitted from the result.
	///
	/// This reduces database queries from N to 1 for batch FK resolution.
	fn batch_lookup_uuids_by_ids(
		ids: HashSet<i32>,
		db: &DatabaseConnection,
	) -> impl std::future::Future<Output = Result<HashMap<i32, Uuid>, sea_orm::DbErr>> + Send
	where
		Self: Sized,
	{
		async move {
			// Default: not implemented (model is not an FK target)
			let _ = (ids, db);
			Ok(HashMap::new())
		}
	}

	// ============================================
	// Post-Backfill Hook
	// ============================================

	/// Called after backfill completes to rebuild any derived data
	///
	/// Override this for models with derived tables (e.g., closure tables).
	/// Default is no-op.
	fn post_backfill_rebuild(
		db: &DatabaseConnection,
	) -> impl std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send
	where
		Self: Sized,
	{
		async move {
			let _ = db;
			Ok(())
		}
	}
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
