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
	pub volume_id: Option<i32>, // Resolved lazily; NULL for locations created before this field
	pub entry_id: Option<i32>,  // Nullable to handle circular FK with entries during sync
	pub name: Option<String>,
	pub index_mode: String, // "shallow", "content", "deep"
	pub scan_state: String, // "pending", "scanning", "completed", "error"
	pub last_scan_at: Option<DateTimeUtc>,
	pub error_message: Option<String>,
	pub total_file_count: i64,
	pub total_byte_size: i64,
	pub job_policies: Option<String>, // JSON-serialized JobPolicies
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
		belongs_to = "super::volume::Entity",
		from = "Column::VolumeId",
		to = "super::volume::Column::Id"
	)]
	Volume,
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

impl Related<super::volume::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::Volume.def()
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
			"job_policies", // Local configuration, not synced
			"created_at",
			"updated_at",
		])
	}

	fn sync_depends_on() -> &'static [&'static str] {
		// Location belongs to a device and references a volume
		// Note: entry_id references the root entry of this location's tree, but this creates
		// a circular dependency (location → entry, entry → location). We handle this by making
		// entry_id nullable during sync and fixing it up after both are synced.
		&["device", "volume"]
	}

	fn foreign_key_mappings() -> Vec<crate::infra::sync::FKMapping> {
		// entry_id and volume_id may be NULL in source
		// If source has UUID=null → FK is null (handled by FK mapper)
		// If source has UUID=xxx but missing → fail for dependency tracking
		vec![
			crate::infra::sync::FKMapping::new("device_id", "devices"),
			crate::infra::sync::FKMapping::new("volume_id", "volumes"),
			crate::infra::sync::FKMapping::new("entry_id", "entries"),
		]
	}

	// FK Lookup Methods (location is not typically an FK target, but consistent pattern)
	async fn lookup_id_by_uuid(
		uuid: Uuid,
		db: &DatabaseConnection,
	) -> Result<Option<i32>, sea_orm::DbErr> {
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
		Ok(Entity::find()
			.filter(Column::Uuid.eq(uuid))
			.one(db)
			.await?
			.map(|l| l.id))
	}

	async fn lookup_uuid_by_id(
		id: i32,
		db: &DatabaseConnection,
	) -> Result<Option<Uuid>, sea_orm::DbErr> {
		Ok(Entity::find_by_id(id).one(db).await?.map(|l| l.uuid))
	}

	async fn batch_lookup_ids_by_uuids(
		uuids: std::collections::HashSet<Uuid>,
		db: &DatabaseConnection,
	) -> Result<std::collections::HashMap<Uuid, i32>, sea_orm::DbErr> {
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
		if uuids.is_empty() {
			return Ok(std::collections::HashMap::new());
		}
		let records = Entity::find()
			.filter(Column::Uuid.is_in(uuids))
			.all(db)
			.await?;
		Ok(records.into_iter().map(|r| (r.uuid, r.id)).collect())
	}

	async fn batch_lookup_uuids_by_ids(
		ids: std::collections::HashSet<i32>,
		db: &DatabaseConnection,
	) -> Result<std::collections::HashMap<i32, Uuid>, sea_orm::DbErr> {
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
		if ids.is_empty() {
			return Ok(std::collections::HashMap::new());
		}
		let records = Entity::find().filter(Column::Id.is_in(ids)).all(db).await?;
		Ok(records.into_iter().map(|r| (r.id, r.uuid)).collect())
	}

	/// Query locations for sync backfill
	///
	/// Note: This method handles FK to UUID conversion internally before returning.
	async fn query_for_sync(
		device_id: Option<Uuid>,
		since: Option<chrono::DateTime<chrono::Utc>>,
		cursor: Option<(chrono::DateTime<chrono::Utc>, Uuid)>,
		batch_size: usize,
		db: &DatabaseConnection,
	) -> Result<Vec<(Uuid, serde_json::Value, chrono::DateTime<chrono::Utc>)>, sea_orm::DbErr> {
		use crate::infra::sync::Syncable;
		use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

		let mut query = Entity::find();

		// Filter by device ownership - need to join through devices table
		// since location.device_id is an integer FK to devices.id
		if let Some(device_uuid) = device_id {
			use super::device;
			query = query
				.inner_join(device::Entity)
				.filter(device::Column::Uuid.eq(device_uuid));
		}

		// Filter by watermark timestamp if specified
		if let Some(since_time) = since {
			query = query.filter(Column::UpdatedAt.gte(since_time));
		}

		// Cursor-based pagination with tie-breaker
		if let Some((cursor_ts, cursor_uuid)) = cursor {
			query = query.filter(
				Condition::any().add(Column::UpdatedAt.gt(cursor_ts)).add(
					Condition::all()
						.add(Column::UpdatedAt.eq(cursor_ts))
						.add(Column::Uuid.gt(cursor_uuid)),
				),
			);
		}

		// Order by updated_at + uuid for deterministic pagination
		query = query
			.order_by_asc(Column::UpdatedAt)
			.order_by_asc(Column::Uuid);

		// Apply batch limit
		query = query.limit(batch_size as u64);

		let results = query.all(db).await?;

		// Convert to sync format with FK mapping
		let mut sync_results = Vec::new();

		for location in results {
			// Serialize to JSON
			let mut json = match location.to_sync_json() {
				Ok(j) => j,
				Err(e) => {
					tracing::warn!(error = %e, uuid = %location.uuid, "Failed to serialize location for sync");
					continue;
				}
			};

			// Convert FK integer IDs to UUIDs
			for fk in <Model as Syncable>::foreign_key_mappings() {
				if let Err(e) =
					crate::infra::sync::fk_mapper::convert_fk_to_uuid(&mut json, &fk, db).await
				{
					tracing::warn!(
						error = %e,
						uuid = %location.uuid,
						fk_field = fk.local_field,
						"Failed to convert FK to UUID, skipping location"
					);
					break;
				}
			}

			sync_results.push((location.uuid, json, location.updated_at));
		}

		Ok(sync_results)
	}

	/// Apply state change with idempotent upsert by UUID.
	/// No conflict resolution needed (device-owned).
	async fn apply_state_change(
		data: serde_json::Value,
		db: &DatabaseConnection,
	) -> Result<(), sea_orm::DbErr> {
		// Map UUIDs to local IDs for FK fields
		let data =
			crate::infra::sync::map_sync_json_to_local(data, Self::foreign_key_mappings(), db)
				.await
				.map_err(|e| sea_orm::DbErr::Custom(format!("FK mapping failed: {}", e)))?;

		// Extract fields from JSON (can't deserialize to Model because id is missing)
		let location_uuid: Uuid = serde_json::from_value(
			data.get("uuid")
				.ok_or_else(|| sea_orm::DbErr::Custom("Missing uuid".to_string()))?
				.clone(),
		)
		.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid uuid: {}", e)))?;

		// Check if location was deleted (prevents race condition)
		if Self::is_tombstoned(location_uuid, db).await? {
			tracing::debug!(uuid = %location_uuid, "Skipping state change for tombstoned location");
			return Ok(());
		}

		let device_id: i32 = serde_json::from_value(
			data.get("device_id")
				.ok_or_else(|| sea_orm::DbErr::Custom("Missing device_id".to_string()))?
				.clone(),
		)
		.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid device_id: {}", e)))?;

		// entry_id and volume_id may be null
		let entry_id: Option<i32> = data.get("entry_id").and_then(|v| {
			if v.is_null() {
				None
			} else {
				serde_json::from_value(v.clone()).ok()
			}
		});

		let volume_id: Option<i32> = data.get("volume_id").and_then(|v| {
			if v.is_null() {
				None
			} else {
				serde_json::from_value(v.clone()).ok()
			}
		});

		// Build ActiveModel for upsert
		use sea_orm::{ActiveValue::NotSet, Set};

		let active = ActiveModel {
			id: NotSet, // Database PK, not synced
			uuid: Set(location_uuid),
			device_id: Set(device_id),
			volume_id: Set(volume_id),
			entry_id: Set(entry_id),
			name: Set(data.get("name").and_then(|v| v.as_str()).map(String::from)),
			index_mode: Set(data
				.get("index_mode")
				.and_then(|v| v.as_str())
				.unwrap_or("shallow")
				.to_string()),
			scan_state: Set("pending".to_string()), // Reset local state
			last_scan_at: Set(None),                // Reset local state
			error_message: Set(None),               // Reset local error
			total_file_count: Set(data
				.get("total_file_count")
				.and_then(|v| v.as_i64())
				.unwrap_or(0)),
			total_byte_size: Set(data
				.get("total_byte_size")
				.and_then(|v| v.as_i64())
				.unwrap_or(0)),
			job_policies: NotSet,                       // Local config, not synced
			created_at: Set(chrono::Utc::now().into()), // Local timestamp
			updated_at: Set(chrono::Utc::now().into()), // Local timestamp
		};

		// Idempotent upsert: insert or update based on UUID
		Entity::insert(active)
			.on_conflict(
				sea_orm::sea_query::OnConflict::column(Column::Uuid)
					.update_columns([
						Column::DeviceId,
						Column::VolumeId,
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

	/// Apply deletion by UUID (cascades to entry tree)
	async fn apply_deletion(uuid: Uuid, db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
		// Find location by UUID
		let location = match Entity::find().filter(Column::Uuid.eq(uuid)).one(db).await? {
			Some(loc) => loc,
			None => return Ok(()), // Already deleted, idempotent
		};

		// Delete root entry tree first if it exists
		// Use delete_subtree_internal to avoid creating tombstones (we're applying a tombstone)
		if let Some(entry_id) = location.entry_id {
			crate::ops::indexing::DatabaseStorage::delete_subtree(entry_id, db).await?;
		}

		// Delete location record
		Entity::delete_by_id(location.id).exec(db).await?;

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
			volume_id: Some(1),
			entry_id: Some(1),
			name: Some("Photos".to_string()),
			index_mode: "deep".to_string(),
			scan_state: "completed".to_string(),
			last_scan_at: Some(chrono::Utc::now().into()),
			error_message: None,
			total_file_count: 100,
			total_byte_size: 1000000,
			job_policies: None,
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

// Register with sync system via inventory
crate::register_syncable_device_owned!(Model, "location", "locations", with_deletion);
