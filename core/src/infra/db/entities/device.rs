//! Device entity

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "devices")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,
	pub uuid: Uuid,
	pub name: String,
	pub os: String,
	pub os_version: Option<String>,
	pub hardware_model: Option<String>,
	pub network_addresses: Json, // Vec<String> as JSON
	pub is_online: bool,
	pub last_seen_at: DateTimeUtc,
	pub capabilities: Json, // DeviceCapabilities as JSON
	pub created_at: DateTimeUtc,
	pub updated_at: DateTimeUtc,
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

	/// Query devices for sync backfill
	async fn query_for_sync(
		device_id: Option<Uuid>,
		since: Option<chrono::DateTime<chrono::Utc>>,
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
		// Deserialize incoming data
		let device: Model = serde_json::from_value(data)
			.map_err(|e| sea_orm::DbErr::Custom(format!("Device deserialization failed: {}", e)))?;

		// Build ActiveModel for upsert
		use sea_orm::{ActiveValue::NotSet, Set};

		let active = ActiveModel {
			id: NotSet,
			uuid: Set(device.uuid),
			name: Set(device.name),
			os: Set(device.os),
			os_version: Set(device.os_version),
			hardware_model: Set(device.hardware_model),
			network_addresses: Set(device.network_addresses),
			is_online: Set(device.is_online),
			last_seen_at: Set(device.last_seen_at),
			capabilities: Set(device.capabilities),
			created_at: Set(chrono::Utc::now().into()),
			updated_at: Set(chrono::Utc::now().into()),
		};

		// Idempotent upsert by UUID
		Entity::insert(active)
			.on_conflict(
				sea_orm::sea_query::OnConflict::column(Column::Uuid)
					.update_columns([
						Column::Name,
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
