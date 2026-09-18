//! Assertions waiting to reach the device that owns their source. See the
//! migration for why this is a queue of finished rows rather than commands.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "assertion_outbox")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,

	/// The owner to deliver to.
	pub device_uuid: Uuid,
	pub source_uuid: Uuid,

	/// `tag` today; overlay writes ride the same table later.
	pub kind: String,

	/// One whole `sources.assertions.merge` input, JSON.
	pub payload: String,

	pub created_at: DateTimeUtc,
	pub attempts: i32,
	pub next_attempt_at: Option<DateTimeUtc>,
	pub last_error: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
