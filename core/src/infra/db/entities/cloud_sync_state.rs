//! SeaORM model for the `cloud_sync_state` table.
//!
//! Keeps the persisted row definition close to the other cloud entities. The
//! domain-level wrapper (`CloudSyncState` in
//! `crate::ops::cloud::change_detection::repository`) does the mapping to
//! `DateTime<Utc>` and hides the SeaORM types from higher layers.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "cloud_sync_state")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = false)]
	pub volume_id: i32,

	pub provider: String,

	pub change_token: Option<String>,

	pub last_full_sync_at: Option<DateTimeUtc>,
	pub last_incremental_at: Option<DateTimeUtc>,

	pub consecutive_failures: i32,

	pub created_at: DateTimeUtc,
	pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(
		belongs_to = "super::volume::Entity",
		from = "Column::VolumeId",
		to = "super::volume::Column::Id",
		on_delete = "Cascade"
	)]
	Volume,
}

impl Related<super::volume::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::Volume.def()
	}
}

impl ActiveModelBehavior for ActiveModel {}
