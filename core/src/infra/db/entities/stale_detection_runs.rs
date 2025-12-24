//! Stale Detection Runs entity
//!
//! Tracks history and metrics of stale detection runs for each location.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "stale_detection_runs")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,
	pub location_id: i32,
	pub job_id: String,
	pub triggered_by: String, // "startup", "periodic", "manual", "offline_threshold"
	pub started_at: DateTimeUtc,
	pub completed_at: Option<DateTimeUtc>,
	pub status: String, // "running", "completed", "failed"
	pub directories_pruned: i32,
	pub directories_scanned: i32,
	pub changes_detected: i32,
	pub error_message: Option<String>,
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
