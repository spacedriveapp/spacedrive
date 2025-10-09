//! Location entity

use crate::infra::sync::Syncable;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "locations")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,
	pub uuid: Uuid,
	pub device_id: i32,
	pub entry_id: i32,
	pub name: Option<String>,
	pub index_mode: String, // "shallow", "content", "deep"
	pub scan_state: String, // "pending", "scanning", "completed", "error"
	pub last_scan_at: Option<DateTimeUtc>,
	pub error_message: Option<String>,
	pub total_file_count: i64,
	pub total_byte_size: i64,
	pub created_at: DateTimeUtc,
	pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(
		belongs_to = "super::device::Entity",
		from = "Column::DeviceId",
		to = "super::device::Column::Id"
	)]
	Device,
	#[sea_orm(
		belongs_to = "super::entry::Entity",
		from = "Column::EntryId",
		to = "super::entry::Column::Id"
	)]
	Entry,
}

impl Related<super::device::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::Device.def()
	}
}

impl Related<super::entry::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::Entry.def()
	}
}

impl ActiveModelBehavior for ActiveModel {}

// ============================================================================
// Syncable Implementation
// ============================================================================
//
// **Location Ownership Model**:
// Each location is OWNED by a single device (the device with the physical filesystem).
// Locations + their entries sync to all paired devices for read-only remote access.
//
// **Sync Domain**: Index (full replication - location + entries)
//
// **What Syncs**:
// - Location identity: uuid
// - Location metadata: name, index_mode
// - Device ownership: device_id (which device owns this location)
// - Entry reference: entry_id (root entry UUID, resolved via sync)
// - Scan statistics: total_file_count, total_byte_size, last_scan_at
// - All entries under this location (synced separately via entry sync)
//
// **What Doesn't Sync**:
// - id: Database primary key (device-specific auto-increment)
// - scan_state: Local state (owner device may be scanning, others just see it)
// - error_message: Local error state (only relevant on owning device)
// - created_at, updated_at: Platform-specific timestamps
//
// **Example Scenario**:
// ```
// Device A (Alice's Laptop):
//   - Creates location "Photos" for /Users/alice/Photos
//   - Indexes 10,000 files → creates Entry records
//   - Location syncs with: uuid, name, device_id (A), entry_id, file counts
//   - All 10,000 entries sync with their location_id reference
//
// Device B (Bob's Desktop):
//   - Receives synced location (owned by Device A)
//   - Receives all 10,000 synced entries
//   - Can browse Alice's Photos remotely (read-only)
//   - Cannot modify files (location is owned by Device A)
//   - May trigger file transfer if accessing content
// ```
//
// **Important**: Only the owning device can modify a location's entries.
// Other devices have read-only access and see the remote filesystem.
//
impl Syncable for Model {
	const SYNC_MODEL: &'static str = "location";

	fn sync_id(&self) -> Uuid {
		self.uuid
	}

	fn version(&self) -> i64 {
		// TODO: Add version field to locations table via migration
		// Migration SQL:
		//   ALTER TABLE locations ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
		// For now, return a default value
		1
	}

	fn exclude_fields() -> Option<&'static [&'static str]> {
		// Only exclude database-specific fields and local state
		Some(&[
			"id",            // Database primary key (device-specific)
			"scan_state",    // Local state (not relevant for remote devices)
			"error_message", // Local error state
			"created_at",    // Platform-specific timestamp
			"updated_at",    // Platform-specific timestamp
		])
		// Note: device_id DOES sync - it indicates which device owns this location
		// Note: entry_id DOES sync - it's the UUID of the root entry (resolved via sync)
		// Note: Statistics (total_file_count, etc.) DO sync - they reflect the owner's data
	}

	/// Query locations for sync backfill
	async fn query_for_sync(
		_device_id: Option<Uuid>,
		since: Option<chrono::DateTime<chrono::Utc>>,
		batch_size: usize,
		db: &DatabaseConnection,
	) -> Result<Vec<(Uuid, serde_json::Value, chrono::DateTime<chrono::Utc>)>, sea_orm::DbErr> {
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

		let mut query = Entity::find();

		// Filter by timestamp if specified
		if let Some(since_time) = since {
			query = query.filter(Column::UpdatedAt.gte(since_time));
		}

		// Apply batch limit
		query = query.limit(batch_size as u64);

		let results = query.all(db).await?;

		// Convert to sync format using the Syncable trait's to_sync_json
		Ok(results
			.into_iter()
			.filter_map(|loc| match loc.to_sync_json() {
				Ok(json) => Some((loc.uuid, json, loc.updated_at)),
				Err(e) => {
					tracing::warn!(error = %e, "Failed to serialize location for sync");
					None
				}
			})
			.collect())
	}

	/// Apply device-owned state change (idempotent upsert)
	///
	/// Locations are device-owned, so we use state-based replication:
	/// - No HLC ordering needed (only owner modifies)
	/// - Idempotent upsert by UUID
	/// - Last state wins (no conflict resolution needed)
	///
	/// # Errors
	///
	/// Returns error if:
	/// - JSON deserialization fails
	/// - Database upsert fails
	/// - Foreign key constraints violated (device_id or entry_id not found)
	async fn apply_state_change(
		data: serde_json::Value,
		db: &DatabaseConnection,
	) -> Result<(), sea_orm::DbErr> {
		// Deserialize incoming data
		let location: Model = serde_json::from_value(data).map_err(|e| {
			sea_orm::DbErr::Custom(format!("Location deserialization failed: {}", e))
		})?;

		// Build ActiveModel for upsert
		// Note: We use Set() for all synced fields, NotSet for id (auto-generated)
		use sea_orm::{ActiveValue::NotSet, Set};

		let active = ActiveModel {
			id: NotSet, // Database PK, not synced
			uuid: Set(location.uuid),
			device_id: Set(location.device_id),
			entry_id: Set(location.entry_id),
			name: Set(location.name),
			index_mode: Set(location.index_mode),
			scan_state: Set("pending".to_string()), // Reset local state
			last_scan_at: Set(location.last_scan_at),
			error_message: Set(None), // Reset local error
			total_file_count: Set(location.total_file_count),
			total_byte_size: Set(location.total_byte_size),
			created_at: Set(chrono::Utc::now().into()), // Local timestamp
			updated_at: Set(chrono::Utc::now().into()), // Local timestamp
		};

		// Idempotent upsert: insert or update based on UUID
		Entity::insert(active)
			.on_conflict(
				sea_orm::sea_query::OnConflict::column(Column::Uuid)
					.update_columns([
						Column::DeviceId,
						Column::EntryId,
						Column::Name,
						Column::IndexMode,
						Column::LastScanAt,
						Column::TotalFileCount,
						Column::TotalByteSize,
						Column::UpdatedAt,
					])
					.to_owned(),
			)
			.exec(db)
			.await?;

		Ok(())
	}
}

// Register location model for automatic sync handling
// TODO: Re-enable when register_syncable_model macro is implemented for leaderless
// crate::register_syncable_model!(Model);

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_location_syncable() {
		let location = Model {
			id: 1,
			uuid: Uuid::new_v4(),
			device_id: 1,
			entry_id: 1,
			name: Some("Photos".to_string()),
			index_mode: "deep".to_string(),
			scan_state: "completed".to_string(),
			last_scan_at: Some(chrono::Utc::now().into()),
			error_message: None,
			total_file_count: 100,
			total_byte_size: 1000000,
			created_at: chrono::Utc::now().into(),
			updated_at: chrono::Utc::now().into(),
		};

		// Test sync methods
		assert_eq!(Model::SYNC_MODEL, "location");
		assert_eq!(location.sync_id(), location.uuid);
		assert_eq!(location.version(), 1);

		// Test JSON serialization
		let json = location.to_sync_json().unwrap();

		// Excluded fields (local state only)
		assert!(json.get("id").is_none());
		assert!(json.get("scan_state").is_none());
		assert!(json.get("error_message").is_none());
		assert!(json.get("created_at").is_none());
		assert!(json.get("updated_at").is_none());

		// Fields that SHOULD sync (ownership + data)
		assert!(json.get("uuid").is_some());
		assert!(json.get("device_id").is_some()); // Owner device
		assert!(json.get("entry_id").is_some()); // Root entry
		assert!(json.get("name").is_some());
		assert!(json.get("index_mode").is_some());
		assert!(json.get("last_scan_at").is_some());
		assert!(json.get("total_file_count").is_some());
		assert!(json.get("total_byte_size").is_some());

		assert_eq!(json.get("name").unwrap().as_str().unwrap(), "Photos");
		assert_eq!(json.get("total_file_count").unwrap().as_i64().unwrap(), 100);
	}

	// Note: apply_state_change requires database setup, tested in integration tests
	// See core/tests/sync/location_sync_test.rs
}
