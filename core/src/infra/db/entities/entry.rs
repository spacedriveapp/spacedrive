//! Entry entity

use sea_orm::{entity::prelude::*, ConnectionTrait, DbBackend, Statement};
use serde::{Deserialize, Serialize};

use crate::infra::sync::Syncable;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "entries")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,
	pub uuid: Option<Uuid>, // Always present (assigned during indexing for UI caching compatibility)
	pub name: String,
	pub kind: i32,                 // Entry type: 0=File, 1=Directory, 2=Symlink
	pub extension: Option<String>, // File extension (without dot), None for directories
	pub metadata_id: Option<i32>,  // Optional - only when user adds metadata
	pub content_id: Option<i32>,   // Optional - for deduplication
	pub size: i64,
	pub aggregate_size: i64, // Total size including all children (for directories)
	pub child_count: i32,    // Total number of direct children
	pub file_count: i32,     // Total number of files in this directory and subdirectories
	pub created_at: DateTimeUtc,
	pub modified_at: DateTimeUtc,
	pub accessed_at: Option<DateTimeUtc>,
	pub indexed_at: Option<DateTimeUtc>, // When this entry was indexed/synced (for watermark tracking)
	pub permissions: Option<String>,     // Unix permissions as string
	pub inode: Option<i64>,              // Platform-specific file identifier for change detection
	pub parent_id: Option<i32>,          // Reference to parent entry for hierarchical relationships
	pub volume_id: Option<i32>, // Volume this entry is on (ownership inherited from volume's device)
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(
		belongs_to = "super::user_metadata::Entity",
		from = "Column::MetadataId",
		to = "super::user_metadata::Column::Id"
	)]
	UserMetadata,
	#[sea_orm(
		belongs_to = "super::content_identity::Entity",
		from = "Column::ContentId",
		to = "super::content_identity::Column::Id"
	)]
	ContentIdentity,
	#[sea_orm(belongs_to = "Entity", from = "Column::ParentId", to = "Column::Id")]
	Parent,
	#[sea_orm(
		belongs_to = "super::volume::Entity",
		from = "Column::VolumeId",
		to = "super::volume::Column::Id"
	)]
	Volume,
}

impl Related<super::user_metadata::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::UserMetadata.def()
	}
}

impl Related<super::content_identity::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::ContentIdentity.def()
	}
}

impl Related<super::volume::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::Volume.def()
	}
}

impl ActiveModelBehavior for ActiveModel {}

// Syncable Implementation

/// Delete an entry and everything beneath it.
///
/// Descendants come from `entry_closure`, so the traversal is one query
/// regardless of depth. Closure links go first, then the paths, then the
/// entries, which is the order the foreign keys allow.
pub async fn delete_subtree(
	entry_id: i32,
	db: &sea_orm::DatabaseConnection,
) -> Result<(), sea_orm::DbErr> {
	use sea_orm::TransactionTrait;

	let txn = db.begin().await?;
	delete_subtree_in_txn(entry_id, &txn).await?;
	txn.commit().await?;
	Ok(())
}

/// [`delete_subtree`] within a transaction the caller already owns.
pub async fn delete_subtree_in_txn<C>(entry_id: i32, db: &C) -> Result<(), sea_orm::DbErr>
where
	C: sea_orm::ConnectionTrait,
{
	use crate::infra::db::entities::{directory_paths, entry_closure};
	use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

	let mut to_delete: Vec<i32> = vec![entry_id];
	if let Ok(rows) = entry_closure::Entity::find()
		.filter(entry_closure::Column::AncestorId.eq(entry_id))
		.all(db)
		.await
	{
		to_delete.extend(rows.into_iter().map(|r| r.descendant_id));
	}
	to_delete.sort_unstable();
	to_delete.dedup();

	if to_delete.is_empty() {
		return Ok(());
	}

	entry_closure::Entity::delete_many()
		.filter(entry_closure::Column::DescendantId.is_in(to_delete.clone()))
		.exec(db)
		.await?;
	entry_closure::Entity::delete_many()
		.filter(entry_closure::Column::AncestorId.is_in(to_delete.clone()))
		.exec(db)
		.await?;
	directory_paths::Entity::delete_many()
		.filter(directory_paths::Column::EntryId.is_in(to_delete.clone()))
		.exec(db)
		.await?;
	Entity::delete_many()
		.filter(Column::Id.is_in(to_delete))
		.exec(db)
		.await?;

	Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryKind {
	File = 0,
	Directory = 1,
	Symlink = 2,
}

impl From<i32> for EntryKind {
	fn from(value: i32) -> Self {
		match value {
			0 => EntryKind::File,
			1 => EntryKind::Directory,
			2 => EntryKind::Symlink,
			_ => EntryKind::File, // Default fallback
		}
	}
}

impl From<EntryKind> for i32 {
	fn from(kind: EntryKind) -> Self {
		kind as i32
	}
}

impl Model {
	/// Get the entry kind as enum
	pub fn entry_kind(&self) -> EntryKind {
		EntryKind::from(self.kind)
	}
}
