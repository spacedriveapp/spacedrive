use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "collection_entries")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = false)]
	pub collection_id: i32,

	#[sea_orm(primary_key, auto_increment = false)]
	pub entry_id: i32,

	pub added_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(
		belongs_to = "super::collection::Entity",
		from = "Column::CollectionId",
		to = "super::collection::Column::Id",
		on_delete = "Cascade"
	)]
	Collection,

	#[sea_orm(
		belongs_to = "super::entry::Entity",
		from = "Column::EntryId",
		to = "super::entry::Column::Id",
		on_delete = "Cascade"
	)]
	Entry,
}

impl Related<super::collection::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::Collection.def()
	}
}

impl Related<super::entry::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::Entry.def()
	}
}

impl ActiveModelBehavior for ActiveModel {}
