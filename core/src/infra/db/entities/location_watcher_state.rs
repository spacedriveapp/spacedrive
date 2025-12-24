//! Location Watcher State entity
//!
//! Tracks the lifecycle and health of the filesystem watcher for each location.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "location_watcher_state")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = false)]
	pub location_id: i32,
	pub last_watch_start: Option<DateTimeUtc>,
	pub last_watch_stop: Option<DateTimeUtc>,
	pub last_successful_event: Option<DateTimeUtc>,
	pub watch_interrupted: bool,
	pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(
		belongs_to = "super::location::Entity",
		from = "Column::LocationId",
		to = "super::location::Column::Id",
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
