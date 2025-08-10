use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "indexer_rules")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,

	pub name: String,
	pub default: bool,

	// Serialized rules blob (rmp-serde of Vec<RulePerKind>), stored as bytes
	pub rules_blob: Vec<u8>,

	pub created_at: DateTimeUtc,
	pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
