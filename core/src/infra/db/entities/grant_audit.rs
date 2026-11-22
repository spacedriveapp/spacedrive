//! One request made with a grant, allowed or refused.
//!
//! The table is a bounded ring: `crate::ops::grants::audit` trims the oldest
//! rows on every write.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "grant_audit")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,
	pub grant_id: Uuid,
	/// The surface's name for what was attempted, such as `read` or `list_dir`.
	pub op: String,
	/// The path as the agent sent it.
	pub path: String,
	pub allowed: bool,
	/// Why a request was refused. Null when it was allowed.
	pub reason: Option<String>,
	pub at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
