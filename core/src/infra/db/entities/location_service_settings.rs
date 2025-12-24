//! LocationServiceSettings entity

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "location_service_settings")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub location_id: i32,
	pub watcher_enabled: bool,
	pub watcher_config: Option<String>, // JSON
	pub stale_detector_enabled: bool,
	pub stale_detector_config: Option<String>, // JSON
	pub sync_enabled: bool,
	pub sync_config: Option<String>, // JSON
	pub created_at: DateTimeUtc,
	pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(
		belongs_to = "super::location::Entity",
		from = "Column::LocationId",
		to = "super::location::Column::Id",
		on_update = "Cascade",
		on_delete = "Cascade"
	)]
	Location,
}

impl Related<super::location::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::Location.def()
	}
}

impl ActiveModelBehavior for ActiveModel {}
