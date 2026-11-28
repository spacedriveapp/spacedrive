//! Requests counted per minute instead of recorded one row each.
//!
//! A flood of unknown tokens, of requests from a locked-out address, or of
//! rate-limited calls lands here as one row per kind, subject and minute, so
//! it cannot fill `grant_audit`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "grant_audit_tally")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,
	/// `unknown_token`, `locked_out` or `rate_limited`.
	pub kind: String,
	/// What the count is per: the client address, or the grant id for
	/// `rate_limited`.
	pub subject: String,
	pub grant_id: Option<Uuid>,
	/// The client address of the latest request counted.
	pub client: Option<String>,
	/// The start of the minute counted.
	pub minute: DateTimeUtc,
	pub count: i64,
	pub last_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
