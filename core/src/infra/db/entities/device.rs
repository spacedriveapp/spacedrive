//! Device entity

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "devices")]
pub struct Model {
	#[sea_orm(primary_key)]
	#[serde(default)]
	pub id: i32,
	pub uuid: Uuid,
	pub name: String,
	pub slug: String,
	pub os: String,
	pub os_version: Option<String>,
	pub hardware_model: Option<String>,
	pub network_addresses: Json, // Vec<String> as JSON
	pub is_online: bool,
	pub last_seen_at: DateTimeUtc,
	pub capabilities: Json, // DeviceCapabilities as JSON
	#[serde(default)]
	pub created_at: DateTimeUtc,
	#[serde(default)]
	pub updated_at: DateTimeUtc,

	// Sync coordination fields (added in m20251009_000001_add_sync_to_devices)
	// Watermarks moved to sync.db per-resource tracking (m20251115_000001)
	pub sync_enabled: bool,
	pub last_sync_at: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(has_many = "super::location::Entity")]
	Locations,
}

impl Related<super::location::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::Locations.def()
	}
}

impl ActiveModelBehavior for ActiveModel {}

// Syncable Implementation
impl crate::infra::sync::Syncable for Model {
	const SYNC_MODEL: &'static str = "device";

	fn sync_id(&self) -> Uuid {
		self.uuid
	}

	fn version(&self) -> i64 {
		// Device sync is state-based, version not needed
		1
	}

	fn exclude_fields() -> Option<&'static [&'static str]> {
		Some(&["id", "created_at", "updated_at"])
	}

	fn sync_depends_on() -> &'static [&'static str] {
		&[] // Device has no dependencies (root of dependency graph)
	}

	// FK Lookup Methods (device is FK target for locations, volumes)
	async fn lookup_id_by_uuid(
		uuid: Uuid,
		db: &DatabaseConnection,
	) -> Result<Option<i32>, sea_orm::DbErr> {
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
		Ok(Entity::find()
			.filter(Column::Uuid.eq(uuid))
			.one(db)
			.await?
			.map(|d| d.id))
	}

	async fn lookup_uuid_by_id(
		id: i32,
		db: &DatabaseConnection,
	) -> Result<Option<Uuid>, sea_orm::DbErr> {
		Ok(Entity::find_by_id(id).one(db).await?.map(|d| d.uuid))
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
		let records = Entity::find()
			.filter(Column::Id.is_in(ids))
			.all(db)
			.await?;
		Ok(records.into_iter().map(|r| (r.id, r.uuid)).collect())
	}

	/// Query devices for sync backfill
	async fn query_for_sync(
		device_id: Option<Uuid>,
		since: Option<chrono::DateTime<chrono::Utc>>,
		_cursor: Option<(chrono::DateTime<chrono::Utc>, Uuid)>,
		batch_size: usize,
		db: &DatabaseConnection,
	) -> Result<Vec<(Uuid, serde_json::Value, chrono::DateTime<chrono::Utc>)>, sea_orm::DbErr> {
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

		let mut query = Entity::find();

		// Filter by device UUID if specified
		if let Some(dev_id) = device_id {
			query = query.filter(Column::Uuid.eq(dev_id));
		}

		// Filter by timestamp if specified
		if let Some(since_time) = since {
			query = query.filter(Column::UpdatedAt.gte(since_time));
		}

		// Apply batch limit
		query = query.limit(batch_size as u64);

		let results = query.all(db).await?;

		// Convert to sync format
		Ok(results
			.into_iter()
			.filter_map(|device| match device.to_sync_json() {
				Ok(json) => Some((device.uuid, json, device.updated_at)),
				Err(e) => {
					tracing::warn!(error = %e, "Failed to serialize device for sync");
					None
				}
			})
			.collect())
	}

	/// Apply device state change (idempotent upsert)
	async fn apply_state_change(
		data: serde_json::Value,
		db: &DatabaseConnection,
	) -> Result<(), sea_orm::DbErr> {
		tracing::info!("[DEVICE_SYNC] apply_state_change called");

		// Deserialize incoming data
		let device: Model = serde_json::from_value(data)
			.map_err(|e| sea_orm::DbErr::Custom(format!("Device deserialization failed: {}", e)))?;

		tracing::info!(
			"[DEVICE_SYNC] Processing device: uuid={}, slug={}",
			device.uuid,
			device.slug
		);

		// Check if this device already exists (by UUID)
		use sea_orm::{ActiveValue::NotSet, ColumnTrait, EntityTrait, QueryFilter, Set};

		let existing_device = Entity::find()
			.filter(Column::Uuid.eq(device.uuid))
			.one(db)
			.await?;

		// Determine the slug to use
		let slug_to_use = if let Some(existing) = existing_device {
			// Device exists - keep its existing slug to avoid breaking references
			tracing::info!(
				"[DEVICE_SYNC] Device exists, keeping existing slug: {}",
				existing.slug
			);
			existing.slug
		} else {
			// New device - check for slug collisions
			tracing::info!("[DEVICE_SYNC] New device, checking for slug collisions");
			let existing_slugs: Vec<String> = Entity::find()
				.all(db)
				.await?
				.iter()
				.map(|d| d.slug.clone())
				.collect();

			tracing::info!(
				"[DEVICE_SYNC] Existing slugs in database: {:?}",
				existing_slugs
			);

			let unique_slug =
				crate::library::Library::ensure_unique_slug(&device.slug, &existing_slugs);

			if unique_slug != device.slug {
				tracing::info!(
					"[DEVICE_SYNC] Slug collision! Using '{}' instead of '{}'",
					unique_slug,
					device.slug
				);
			} else {
				tracing::info!("[DEVICE_SYNC] No collision, using slug: {}", unique_slug);
			}

			unique_slug
		};

		// Build ActiveModel for upsert
		let active = ActiveModel {
			id: NotSet,
			uuid: Set(device.uuid),
			name: Set(device.name),
			slug: Set(slug_to_use),
			os: Set(device.os),
			os_version: Set(device.os_version),
			hardware_model: Set(device.hardware_model),
			network_addresses: Set(device.network_addresses),
			is_online: Set(device.is_online),
			last_seen_at: Set(device.last_seen_at),
			capabilities: Set(device.capabilities),
			created_at: Set(chrono::Utc::now().into()),
			updated_at: Set(chrono::Utc::now().into()),
			sync_enabled: Set(true),
			last_sync_at: Set(None),
		};

		// Idempotent upsert by UUID
		Entity::insert(active)
			.on_conflict(
				sea_orm::sea_query::OnConflict::column(Column::Uuid)
					.update_columns([
						Column::Name,
						// Note: slug is NOT updated on conflict to preserve local slug overrides
						Column::Os,
						Column::OsVersion,
						Column::HardwareModel,
						Column::NetworkAddresses,
						Column::IsOnline,
						Column::LastSeenAt,
						Column::Capabilities,
						Column::UpdatedAt,
					])
					.to_owned(),
			)
			.exec(db)
			.await?;

		Ok(())
	}
}

// Register with sync system via inventory
crate::register_syncable_device_owned!(Model, "device", "devices");
