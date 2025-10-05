use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "collections")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,

	#[sea_orm(unique)]
	pub uuid: Uuid,

	pub name: String,

	pub description: Option<String>,

	pub created_at: DateTime<Utc>,

	pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(has_many = "super::collection_entry::Entity")]
	CollectionEntries,
}

impl Related<super::collection_entry::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::CollectionEntries.def()
	}
}

impl Related<super::entry::Entity> for Entity {
	fn to() -> RelationDef {
		super::collection_entry::Relation::Entry.def()
	}

	fn via() -> Option<RelationDef> {
		Some(super::collection_entry::Relation::Collection.def().rev())
	}
}

impl ActiveModelBehavior for ActiveModel {}
