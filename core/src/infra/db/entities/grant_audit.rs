//! One request made with a grant, allowed or refused.
//!
//! `crate::ops::grants::audit` bounds the table by age and by row count.

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
	/// The path as the agent sent it, relative to the granted directory.
	pub path: String,
	pub allowed: bool,
	/// Why a request was refused. Null when it was allowed.
	pub reason: Option<String>,
	pub at: DateTimeUtc,
	/// The grant's label when the request was made.
	pub grant_label: Option<String>,
	/// The client's IP address, for surfaces reached over the network.
	pub client: Option<String>,
	/// The MCP session the request was sent in.
	pub session: Option<String>,
	/// The grep or glob pattern, cut to its first 256 bytes.
	pub pattern: Option<String>,
	/// Bytes of the response body.
	pub bytes: Option<i64>,
	/// `ok`, `denied`, `limited` or `error`.
	pub result: Option<String>,
	pub duration_ms: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
