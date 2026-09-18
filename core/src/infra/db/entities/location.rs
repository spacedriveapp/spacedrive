//! A place someone cares about, inside a source.
//!
//! A location owns nothing. It is a name, a path relative to its source root,
//! and whether the user put it there. It preserves navigation intent without
//! changing what the source captures or watches.
//!
//! The path is relative so the row survives the drive being mounted somewhere
//! else, which is the whole reason a source has a root and a location does not.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::infra::sync::Syncable;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "location")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,

	#[sea_orm(unique)]
	pub uuid: Uuid,

	/// The source whose store holds this subtree's records. A location under
	/// nested sources belongs to the innermost one containing it.
	pub source_uuid: Uuid,

	/// Relative to the source root. Empty means the source root itself.
	pub relative_path: String,

	pub name: String,

	/// `default` for legacy seeded folders, `user` for an explicit pin.
	pub origin: String,

	pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(
		belongs_to = "super::source::Entity",
		from = "Column::SourceUuid",
		to = "super::source::Column::Uuid"
	)]
	Source,
}

impl Related<super::source::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::Source.def()
	}
}

impl ActiveModelBehavior for ActiveModel {}

/// Where a location row came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
	/// Written by an older library initializer from the platform's known folders.
	Default,
	/// Pinned by someone.
	User,
}

impl Origin {
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Default => "default",
			Self::User => "user",
		}
	}
}

impl From<&str> for Origin {
	fn from(value: &str) -> Self {
		match value {
			"default" => Self::Default,
			_ => Self::User,
		}
	}
}
