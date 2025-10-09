//! Entry entity

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "entries")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,
	pub uuid: Option<Uuid>, // None until content identification phase complete (sync readiness indicator)
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
	pub permissions: Option<String>, // Unix permissions as string
	pub inode: Option<i64>,          // Platform-specific file identifier for change detection
	pub parent_id: Option<i32>,      // Reference to parent entry for hierarchical relationships
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

impl ActiveModelBehavior for ActiveModel {}

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

	/// UUID Assignment Rules:
	/// - Directories: Assign UUID immediately (no content to identify)
	/// - Empty files: Assign UUID immediately (size = 0, no content to hash)
	/// - Regular files: Assign UUID after content identification completes
	pub fn should_assign_uuid_immediately(&self) -> bool {
		self.entry_kind() == EntryKind::Directory || self.size == 0
	}

	/// Check if this entry is ready for sync (has UUID assigned)
	pub fn is_sync_ready(&self) -> bool {
		self.uuid.is_some()
	}

	/// Apply device-owned state change (idempotent upsert)
	///
	/// Entries are device-owned, so we use state-based replication:
	/// - No HLC ordering needed (only owner modifies)
	/// - Idempotent upsert by UUID
	/// - Last state wins (no conflict resolution needed)
	///
	/// # Errors
	///
	/// Returns error if:
	/// - JSON deserialization fails
	/// - Database upsert fails
	/// - Foreign key constraints violated (metadata_id, content_id, or parent_id not found)
	pub async fn apply_state_change(
		data: serde_json::Value,
		db: &DatabaseConnection,
	) -> Result<(), sea_orm::DbErr> {
		// Deserialize incoming data
		let entry: Self = serde_json::from_value(data)
			.map_err(|e| sea_orm::DbErr::Custom(format!("Entry deserialization failed: {}", e)))?;

		// Only sync entries that have UUIDs (sync-ready entries)
		let entry_uuid = entry
			.uuid
			.ok_or_else(|| sea_orm::DbErr::Custom("Cannot sync entry without UUID".to_string()))?;

		// Build ActiveModel for upsert
		use sea_orm::{ActiveValue::NotSet, Set};

		let active = ActiveModel {
			id: NotSet, // Database PK, not synced
			uuid: Set(Some(entry_uuid)),
			name: Set(entry.name),
			kind: Set(entry.kind),
			extension: Set(entry.extension),
			metadata_id: Set(entry.metadata_id),
			content_id: Set(entry.content_id),
			size: Set(entry.size),
			aggregate_size: Set(entry.aggregate_size),
			child_count: Set(entry.child_count),
			file_count: Set(entry.file_count),
			created_at: Set(entry.created_at),
			modified_at: Set(entry.modified_at),
			accessed_at: Set(entry.accessed_at),
			permissions: Set(entry.permissions),
			inode: Set(entry.inode),
			parent_id: Set(entry.parent_id),
		};

		// Idempotent upsert: insert or update based on UUID
		Entity::insert(active)
			.on_conflict(
				sea_orm::sea_query::OnConflict::column(Column::Uuid)
					.update_columns([
						Column::Name,
						Column::Kind,
						Column::Extension,
						Column::MetadataId,
						Column::ContentId,
						Column::Size,
						Column::AggregateSize,
						Column::ChildCount,
						Column::FileCount,
						Column::CreatedAt,
						Column::ModifiedAt,
						Column::AccessedAt,
						Column::Permissions,
						Column::Inode,
						Column::ParentId,
					])
					.to_owned(),
			)
			.exec(db)
			.await?;

		Ok(())
	}
}
