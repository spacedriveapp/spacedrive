//! A refresh token hash that a rotation has already replaced.
//!
//! Only a copy of a spent refresh token can still be presented, so
//! `crate::ops::grants::oauth` revokes the grant when one comes back. Rows of
//! grants that have ended are pruned on the next rotation.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "agent_grant_spent_refresh")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,
	pub grant_id: Uuid,
	/// Lowercase hex SHA-256 of the spent refresh token.
	#[sea_orm(unique)]
	pub hash: String,
	pub spent_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
