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

	// TODO: Reimplement with new leaderless architecture
	// Old apply_sync_entry removed - will use state-based sync
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
}
