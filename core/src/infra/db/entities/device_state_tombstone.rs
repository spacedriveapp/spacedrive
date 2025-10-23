//! Device state tombstone entity
//!
//! Tracks deletions of device-owned data (locations, entries, volumes) for sync.
//! Uses cascading tombstones - only root UUIDs are stored, receivers cascade using entry_closure.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "device_state_tombstones")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,
	pub model_type: String, // "location", "entry", "volume"
	pub record_uuid: Uuid,  // UUID of deleted record (root only for cascading)
	pub device_id: i32,
	pub deleted_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(
		belongs_to = "super::device::Entity",
		from = "Column::DeviceId",
		to = "super::device::Column::Id",
		on_delete = "Cascade"
	)]
	Device,
}

impl Related<super::device::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::Device.def()
	}
}

impl ActiveModelBehavior for ActiveModel {}
