//! Foreign Key UUID Mapping for Sync
//!
//! Handles automatic conversion between local integer FKs and global UUIDs for sync.
//!
//! ## The Problem
//!
//! Auto-incrementing integer PKs are local to each database:
//! - Device A has device_id=1 for Device A, device_id=2 for Device B
//! - Device B has device_id=1 for Device B, device_id=2 for Device A
//!
//! We cannot sync integer IDs directly - they mean different things on each device.
//!
//! ## The Solution
//!
//! Sync protocol uses UUIDs exclusively. This module provides:
//! 1. `to_sync_json()` - Converts local integer FKs → UUIDs before sending
//! 2. `map_uuids_to_local_ids()` - Converts UUIDs → local integer FKs on receive
//!
//! ## Usage
//!
//! Models just declare their FKs, the rest is automatic:
//!
//! ```rust
//! impl Syncable for location::Model {
//!     fn foreign_key_mappings() -> Vec<FKMapping> {
//!         vec![
//!             FKMapping::new("device_id", "devices"),
//!             FKMapping::new("entry_id", "entries"),
//!         ]
//!     }
//!
//!     // to_sync_json() - uses default implementation (automatic!)
//!
//!     async fn apply_state_change(data: Value, db: &DatabaseConnection) -> Result<()> {
//!         let data = map_sync_json_to_local(data, Self::foreign_key_mappings(), db).await?;
//!         let model: Model = serde_json::from_value(data)?;
//!         upsert_by_uuid(model).await?;
//!         Ok(())
//!     }
//! }
//! ```

use crate::infra::db::entities;
use anyhow::{anyhow, Result};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::{json, Value};
use uuid::Uuid;

/// Foreign key mapping declaration
#[derive(Debug, Clone)]
pub struct FKMapping {
	/// Field name in the model (e.g., "device_id")
	pub local_field: &'static str,

	/// Target table name (e.g., "devices")
	pub target_table: &'static str,
}

impl FKMapping {
	pub fn new(local_field: &'static str, target_table: &'static str) -> Self {
		Self {
			local_field,
			target_table,
		}
	}

	/// Get the UUID field name for sync JSON
	/// Example: "device_id" → "device_uuid"
	pub fn uuid_field_name(&self) -> String {
		format!("{}_uuid", self.local_field.trim_end_matches("_id"))
	}
}

/// Convert local integer FK to UUID for sync
///
/// Modifies JSON in place:
/// - Looks up the UUID for the integer FK
/// - Adds a new field with UUID (e.g., "device_uuid")
/// - Removes the original integer field (e.g., "device_id")
pub async fn convert_fk_to_uuid(
	json: &mut Value,
	fk: &FKMapping,
	db: &DatabaseConnection,
) -> Result<()> {
	// Extract the local integer ID
	let local_id = match json.get(fk.local_field).and_then(|v| v.as_i64()) {
		Some(id) => id as i32,
		None => {
			// Field might be null (e.g., parent_id for root entries)
			if json
				.get(fk.local_field)
				.map(|v| v.is_null())
				.unwrap_or(false)
			{
				// Add null UUID field and return
				json[fk.uuid_field_name()] = Value::Null;
				return Ok(());
			}
			return Err(anyhow!(
				"Missing or invalid FK field '{}' in sync data",
				fk.local_field
			));
		}
	};

	// Look up UUID from target table
	let uuid = lookup_uuid_for_local_id(fk.target_table, local_id, db).await?;

	// Add UUID field to JSON
	json[fk.uuid_field_name()] = json!(uuid.to_string());

	// Remove integer ID field (we only sync UUIDs)
	if let Some(obj) = json.as_object_mut() {
		obj.remove(fk.local_field);
	}

	Ok(())
}

/// Look up UUID for a local integer ID in any table
async fn lookup_uuid_for_local_id(
	table: &str,
	local_id: i32,
	db: &DatabaseConnection,
) -> Result<Uuid> {
	match table {
		"devices" => {
			let device = entities::device::Entity::find_by_id(local_id)
				.one(db)
				.await?
				.ok_or_else(|| anyhow!("Device with id={} not found", local_id))?;
			Ok(device.uuid)
		}
		"entries" => {
			let entry = entities::entry::Entity::find_by_id(local_id)
				.one(db)
				.await?
				.ok_or_else(|| anyhow!("Entry with id={} not found", local_id))?;
			entry
				.uuid
				.ok_or_else(|| anyhow!("Entry id={} has no UUID (not sync-ready)", local_id))
		}
		"locations" => {
			let location = entities::location::Entity::find_by_id(local_id)
				.one(db)
				.await?
				.ok_or_else(|| anyhow!("Location with id={} not found", local_id))?;
			Ok(location.uuid)
		}
		"user_metadata" => {
			let metadata = entities::user_metadata::Entity::find_by_id(local_id)
				.one(db)
				.await?
				.ok_or_else(|| anyhow!("UserMetadata with id={} not found", local_id))?;
			Ok(metadata.uuid)
		}
		"content_identities" => {
			let content = entities::content_identity::Entity::find_by_id(local_id)
				.one(db)
				.await?
				.ok_or_else(|| anyhow!("ContentIdentity with id={} not found", local_id))?;
			content
				.uuid
				.ok_or_else(|| anyhow!("ContentIdentity id={} has no UUID", local_id))
		}
		_ => Err(anyhow!("Unknown table for FK mapping: {}", table)),
	}
}

/// Convert UUIDs back to local integer IDs for database insertion
///
/// Modifies JSON in place:
/// - Looks up local ID for each UUID FK
/// - Replaces UUID field with integer ID field
/// - Removes UUID field
pub async fn map_sync_json_to_local(
	mut data: Value,
	mappings: Vec<FKMapping>,
	db: &DatabaseConnection,
) -> Result<Value> {
	for fk in mappings {
		let uuid_field = fk.uuid_field_name();

		// Extract UUID
		let uuid_value = data.get(&uuid_field);

		if uuid_value.is_none() || uuid_value.unwrap().is_null() {
			// Null UUID means null FK (e.g., parent_id for root entries)
			data[fk.local_field] = Value::Null;
			continue;
		}

		let uuid: Uuid = uuid_value
			.and_then(|v| v.as_str())
			.ok_or_else(|| anyhow!("Missing UUID field '{}'", uuid_field))?
			.parse()?;

		// Map UUID to local ID
		let local_id = lookup_local_id_for_uuid(fk.target_table, uuid, db).await?;

		// Replace UUID with local ID
		data[fk.local_field] = json!(local_id);

		// Remove UUID field
		if let Some(obj) = data.as_object_mut() {
			obj.remove(&uuid_field);
		}
	}

	Ok(data)
}

/// Look up local integer ID for a UUID in any table
async fn lookup_local_id_for_uuid(table: &str, uuid: Uuid, db: &DatabaseConnection) -> Result<i32> {
	match table {
		"devices" => {
			let device = entities::device::Entity::find()
				.filter(entities::device::Column::Uuid.eq(uuid))
				.one(db)
				.await?
				.ok_or_else(|| {
					anyhow!(
						"Device with uuid={} not found (sync dependency missing)",
						uuid
					)
				})?;
			Ok(device.id)
		}
		"entries" => {
			let entry = entities::entry::Entity::find()
				.filter(entities::entry::Column::Uuid.eq(Some(uuid)))
				.one(db)
				.await?
				.ok_or_else(|| {
					anyhow!(
						"Entry with uuid={} not found (sync dependency missing)",
						uuid
					)
				})?;
			Ok(entry.id)
		}
		"locations" => {
			let location = entities::location::Entity::find()
				.filter(entities::location::Column::Uuid.eq(uuid))
				.one(db)
				.await?
				.ok_or_else(|| anyhow!("Location with uuid={} not found", uuid))?;
			Ok(location.id)
		}
		"user_metadata" => {
			let metadata = entities::user_metadata::Entity::find()
				.filter(entities::user_metadata::Column::Uuid.eq(uuid))
				.one(db)
				.await?
				.ok_or_else(|| {
					anyhow!(
						"UserMetadata with uuid={} not found (sync dependency missing)",
						uuid
					)
				})?;
			Ok(metadata.id)
		}
		"content_identities" => {
			let content = entities::content_identity::Entity::find()
				.filter(entities::content_identity::Column::Uuid.eq(Some(uuid)))
				.one(db)
				.await?
				.ok_or_else(|| {
					anyhow!(
						"ContentIdentity with uuid={} not found (sync dependency missing)",
						uuid
					)
				})?;
			Ok(content.id)
		}
		_ => Err(anyhow!("Unknown table for FK mapping: {}", table)),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_uuid_field_name() {
		let fk = FKMapping::new("device_id", "devices");
		assert_eq!(fk.uuid_field_name(), "device_uuid");

		let fk = FKMapping::new("parent_id", "entries");
		assert_eq!(fk.uuid_field_name(), "parent_uuid");

		let fk = FKMapping::new("entry_id", "entries");
		assert_eq!(fk.uuid_field_name(), "entry_uuid");
	}
}
