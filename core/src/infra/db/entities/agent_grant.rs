//! A grant: one subtree of one source, lent to an agent for a bounded time.
//!
//! The row never holds the token, only its SHA-256, so the library database
//! can be copied or backed up without leaking a credential.
//! `crate::ops::grants` owns every read and write of this table.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "agent_grant")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,
	#[sea_orm(unique)]
	pub uuid: Uuid,
	/// Lowercase hex SHA-256 of the full token, prefix included.
	#[sea_orm(unique)]
	pub token_hash: String,
	/// The registry uuid of the source the subtree belongs to.
	pub source_id: Uuid,
	/// Canonical absolute path of the granted directory at creation.
	pub subtree: String,
	/// `read` or `read_write`.
	pub caps: String,
	pub label: Option<String>,
	/// The device that minted the grant.
	pub created_by: String,
	pub created_at: DateTimeUtc,
	pub expires_at: DateTimeUtc,
	pub revoked_at: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
