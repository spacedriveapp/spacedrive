//! An actor: whoever made a change. A person, a device, or an agent.
//!
//! `crate::ops::actors` owns every write of this table.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "actor")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,
	#[sea_orm(unique)]
	pub uuid: Uuid,
	/// `person`, `device` or `agent`.
	pub kind: String,
	pub label: String,
	/// The agent harness, such as `claude-code` or `codex`.
	pub harness: Option<String>,
	/// The actor that created this one.
	pub created_by: Option<Uuid>,
	pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
