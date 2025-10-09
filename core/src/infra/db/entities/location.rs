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

// Syncable Implementation
//
// Locations are DEVICE-OWNED using state-based replication. Each location is owned by
// a single device and syncs to all paired devices for read-only remote access.
// Only the owning device can modify the location and its entries.
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
		Some(&[
			"id",
			"scan_state",
			"error_message",
			"created_at",
			"updated_at",
		])
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

	/// Apply state change with idempotent upsert by UUID.
	/// No conflict resolution needed (device-owned).
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
