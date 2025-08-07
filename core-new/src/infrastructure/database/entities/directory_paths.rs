//! DirectoryPaths entity for caching directory paths

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "directory_paths")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = false)]
	pub entry_id: i32,
	pub path: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(
		belongs_to = "super::entry::Entity",
		from = "Column::EntryId",
		to = "super::entry::Column::Id",
		on_update = "Cascade",
		on_delete = "Cascade"
	)]
	Entry,
}

impl Related<super::entry::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::Entry.def()
	}
}

impl ActiveModelBehavior for ActiveModel {}