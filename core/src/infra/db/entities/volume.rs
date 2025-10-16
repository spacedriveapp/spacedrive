//! Volume entity

use crate::infra::sync::Syncable;
use crate::volume::types::TrackedVolume;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "volumes")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,
	pub uuid: Uuid,
	pub device_id: Uuid, // Foreign key to devices table
	pub fingerprint: String,
	pub display_name: Option<String>,
	pub tracked_at: DateTimeUtc,
	pub last_seen_at: DateTimeUtc,
	pub is_online: bool,
	pub total_capacity: Option<i64>,
	pub available_capacity: Option<i64>,
	pub read_speed_mbps: Option<i32>,
	pub write_speed_mbps: Option<i32>,
	pub last_speed_test_at: Option<DateTimeUtc>,
	pub file_system: Option<String>,
	pub mount_point: Option<String>,
	pub is_removable: Option<bool>,
	pub is_network_drive: Option<bool>,
	pub device_model: Option<String>,
	/// Volume type classification
	pub volume_type: Option<String>,
	/// Whether volume is visible in default UI
	pub is_user_visible: Option<bool>,
	/// Whether volume is eligible for auto-tracking
	pub auto_track_eligible: Option<bool>,
	/// Cloud identifier (bucket/drive/container name) for cloud volumes
	pub cloud_identifier: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(
		belongs_to = "super::device::Entity",
		from = "Column::DeviceId",
		to = "super::device::Column::Uuid"
	)]
	Device,
}

impl Related<super::device::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::Device.def()
	}
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
	/// Convert database model to tracked volume
	pub fn to_tracked_volume(&self) -> crate::volume::types::TrackedVolume {
		crate::volume::types::TrackedVolume {
			id: self.id,
			uuid: self.uuid,
			device_id: self.device_id,
			fingerprint: crate::volume::VolumeFingerprint(self.fingerprint.clone()),
			display_name: self.display_name.clone(),
			tracked_at: self.tracked_at,
			last_seen_at: self.last_seen_at,
			is_online: self.is_online,
			total_capacity: self.total_capacity.map(|c| c as u64),
			available_capacity: self.available_capacity.map(|c| c as u64),
			read_speed_mbps: self.read_speed_mbps.map(|s| s as u32),
			write_speed_mbps: self.write_speed_mbps.map(|s| s as u32),
			last_speed_test_at: self.last_speed_test_at,
			file_system: self.file_system.clone(),
			mount_point: self.mount_point.clone(),
			is_removable: self.is_removable,
			is_network_drive: self.is_network_drive,
			device_model: self.device_model.clone(),
			volume_type: self.volume_type.as_deref().unwrap_or("Unknown").to_string(),
			is_user_visible: self.is_user_visible,
			auto_track_eligible: self.auto_track_eligible,
		}
	}
}

// Syncable Implementation
//
// Volumes are DEVICE-OWNED using state-based replication. Each volume is owned by
// a single device and syncs to all paired devices for read-only remote access.
// Only the owning device can modify the volume state.
impl Syncable for Model {
	const SYNC_MODEL: &'static str = "volume";

	fn sync_id(&self) -> Uuid {
		self.uuid
	}

	fn version(&self) -> i64 {
		1
	}

	fn exclude_fields() -> Option<&'static [&'static str]> {
		Some(&[
			"id",
			"is_online",
			"last_seen_at",
			"last_speed_test_at",
			"tracked_at",
		])
	}

	fn sync_depends_on() -> &'static [&'static str] {
		&["device"]
	}

	fn foreign_key_mappings() -> Vec<crate::infra::sync::FKMapping> {
		vec![]
	}

	async fn query_for_sync(
		_device_id: Option<Uuid>,
		since: Option<chrono::DateTime<chrono::Utc>>,
		batch_size: usize,
		db: &DatabaseConnection,
	) -> Result<Vec<(Uuid, serde_json::Value, chrono::DateTime<chrono::Utc>)>, sea_orm::DbErr> {
		use crate::infra::sync::Syncable;
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

		let mut query = Entity::find();

		if let Some(since_time) = since {
			query = query.filter(Column::LastSeenAt.gte(since_time));
		}

		query = query.limit(batch_size as u64);

		let results = query.all(db).await?;

		let mut sync_results = Vec::new();

		for volume in results {
			let json = match volume.to_sync_json() {
				Ok(j) => j,
				Err(e) => {
					tracing::warn!(error = %e, uuid = %volume.uuid, "Failed to serialize volume for sync");
					continue;
				}
			};

			sync_results.push((volume.uuid, json, volume.last_seen_at));
		}

		Ok(sync_results)
	}

	async fn apply_state_change(
		data: serde_json::Value,
		db: &DatabaseConnection,
	) -> Result<(), sea_orm::DbErr> {
		let volume_uuid: Uuid = serde_json::from_value(
			data.get("uuid")
				.ok_or_else(|| sea_orm::DbErr::Custom("Missing uuid".to_string()))?
				.clone(),
		)
		.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid uuid: {}", e)))?;

		let device_uuid: Uuid = serde_json::from_value(
			data.get("device_id")
				.ok_or_else(|| sea_orm::DbErr::Custom("Missing device_id".to_string()))?
				.clone(),
		)
		.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid device_id: {}", e)))?;

		use sea_orm::{ActiveValue::NotSet, Set};

		let active = ActiveModel {
			id: NotSet,
			uuid: Set(volume_uuid),
			device_id: Set(device_uuid),
			fingerprint: Set(data
				.get("fingerprint")
				.and_then(|v| v.as_str())
				.unwrap_or("")
				.to_string()),
			display_name: Set(data
				.get("display_name")
				.and_then(|v| v.as_str())
				.map(String::from)),
			tracked_at: Set(chrono::Utc::now().into()),
			last_seen_at: Set(chrono::Utc::now().into()),
			is_online: Set(false),
			total_capacity: Set(data.get("total_capacity").and_then(|v| v.as_i64())),
			available_capacity: Set(data.get("available_capacity").and_then(|v| v.as_i64())),
			read_speed_mbps: Set(data.get("read_speed_mbps").and_then(|v| v.as_i64()).map(|v| v as i32)),
			write_speed_mbps: Set(data.get("write_speed_mbps").and_then(|v| v.as_i64()).map(|v| v as i32)),
			last_speed_test_at: Set(None),
			file_system: Set(data.get("file_system").and_then(|v| v.as_str()).map(String::from)),
			mount_point: Set(data.get("mount_point").and_then(|v| v.as_str()).map(String::from)),
			is_removable: Set(data.get("is_removable").and_then(|v| v.as_bool())),
			is_network_drive: Set(data.get("is_network_drive").and_then(|v| v.as_bool())),
			device_model: Set(data.get("device_model").and_then(|v| v.as_str()).map(String::from)),
			volume_type: Set(data.get("volume_type").and_then(|v| v.as_str()).map(String::from)),
			is_user_visible: Set(data.get("is_user_visible").and_then(|v| v.as_bool())),
			auto_track_eligible: Set(data.get("auto_track_eligible").and_then(|v| v.as_bool())),
			cloud_identifier: Set(data.get("cloud_identifier").and_then(|v| v.as_str()).map(String::from)),
		};

		Entity::insert(active)
			.on_conflict(
				sea_orm::sea_query::OnConflict::column(Column::Uuid)
					.update_columns([
						Column::DeviceId,
						Column::Fingerprint,
						Column::DisplayName,
						Column::TotalCapacity,
						Column::AvailableCapacity,
						Column::ReadSpeedMbps,
						Column::WriteSpeedMbps,
						Column::FileSystem,
						Column::MountPoint,
						Column::IsRemovable,
						Column::IsNetworkDrive,
						Column::DeviceModel,
						Column::VolumeType,
						Column::IsUserVisible,
						Column::AutoTrackEligible,
						Column::CloudIdentifier,
						Column::LastSeenAt,
					])
					.to_owned(),
			)
			.exec(db)
			.await?;

		Ok(())
	}
}
