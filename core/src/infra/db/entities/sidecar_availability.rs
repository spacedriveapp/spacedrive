use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sidecar_availability")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,

	pub content_uuid: Uuid,

	pub kind: String,

	pub variant: String,

	pub device_uuid: Uuid,

	pub has: bool,

	pub size: Option<i64>,

	pub checksum: Option<String>,

	pub last_seen_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(
		belongs_to = "super::content_identity::Entity",
		from = "Column::ContentUuid",
		to = "super::content_identity::Column::Uuid"
	)]
	ContentIdentity,

	#[sea_orm(
		belongs_to = "super::device::Entity",
		from = "Column::DeviceUuid",
		to = "super::device::Column::Uuid"
	)]
	Device,
}

impl Related<super::content_identity::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::ContentIdentity.def()
	}
}

impl Related<super::device::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::Device.def()
	}
}

impl ActiveModelBehavior for ActiveModel {}
