//! Entry entity

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use crate::infra::sync::Syncable;

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

// Syncable Implementation
impl crate::infra::sync::Syncable for Model {
	const SYNC_MODEL: &'static str = "entry";

	fn sync_id(&self) -> Uuid {
		self.uuid.unwrap_or_else(|| Uuid::from_bytes([0; 16]))
	}

	fn version(&self) -> i64 {
		// Entry sync is state-based, version not needed
		1
	}

	fn exclude_fields() -> Option<&'static [&'static str]> {
		Some(&["id"]) // Only exclude database PK
	}

	fn sync_depends_on() -> &'static [&'static str] {
		&["location"] // Entry belongs to a location
	}

	fn foreign_key_mappings() -> Vec<crate::infra::sync::FKMapping> {
		vec![
			crate::infra::sync::FKMapping::new("parent_id", "entries"),
			crate::infra::sync::FKMapping::new("metadata_id", "user_metadata"),
			crate::infra::sync::FKMapping::new("content_id", "content_identities"),
		]
	}

	fn to_sync_json(&self) -> Result<serde_json::Value, serde_json::Error> {
		// Serialize to JSON with field exclusions
		let mut value = serde_json::to_value(self)?;

		// Apply field exclusions
		if let Some(excluded) = Self::exclude_fields() {
			if let Some(obj) = value.as_object_mut() {
				for field in excluded {
					obj.remove(*field);
				}
			}
		}

		// Note: FK mapping to UUIDs will be done by the sync system
		// when broadcasting, not here. This is because we need database
		// access to look up UUIDs, which isn't available in this trait method.

		Ok(value)
	}

	/// Query entries for sync backfill
	///
	/// Note: This method handles FK to UUID conversion internally before returning.
	async fn query_for_sync(
		_device_id: Option<Uuid>,
		since: Option<chrono::DateTime<chrono::Utc>>,
		batch_size: usize,
		db: &DatabaseConnection,
	) -> Result<Vec<(Uuid, serde_json::Value, chrono::DateTime<chrono::Utc>)>, sea_orm::DbErr> {
		use crate::infra::sync::Syncable;
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

		let mut query = Entity::find();

		// Only sync entries that have UUIDs (are sync-ready)
		query = query.filter(Column::Uuid.is_not_null());

		// Filter by timestamp if specified
		if let Some(since_time) = since {
			query = query.filter(Column::ModifiedAt.gte(since_time));
		}

		// Apply batch limit
		query = query.limit(batch_size as u64);

		let results = query.all(db).await?;

		// Convert to sync format with FK mapping
		let mut sync_results = Vec::new();

		for entry in results {
			let uuid = match entry.uuid {
				Some(u) => u,
				None => continue, // Skip entries without UUIDs
			};

			// Serialize to JSON
			let mut json = match entry.to_sync_json() {
				Ok(j) => j,
				Err(e) => {
					tracing::warn!(error = %e, uuid = %uuid, "Failed to serialize entry for sync");
					continue;
				}
			};

			// Convert FK integer IDs to UUIDs
			for fk in <Model as Syncable>::foreign_key_mappings() {
				if let Err(e) =
					crate::infra::sync::fk_mapper::convert_fk_to_uuid(&mut json, &fk, db).await
				{
					tracing::warn!(
						error = %e,
						uuid = %uuid,
						fk_field = fk.local_field,
						"Failed to convert FK to UUID, skipping entry"
					);
					continue;
				}
			}

			sync_results.push((uuid, json, entry.modified_at));
		}

		Ok(sync_results)
	}

	/// Apply state change - already implemented in Model impl block below
	async fn apply_state_change(
		data: serde_json::Value,
		db: &DatabaseConnection,
	) -> Result<(), sea_orm::DbErr> {
		Model::apply_state_change(data, db).await
	}
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
	/// # Foreign Key Mapping
	///
	/// The incoming JSON contains UUIDs for FKs (parent_uuid, metadata_uuid, content_uuid).
	/// These must be mapped to local integer IDs before deserialization.
	///
	/// # Directory Paths
	///
	/// If this is a directory entry, we rebuild its directory_paths cache entry
	/// after upsert using the local filesystem paths.
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
		// Map UUID FKs to local integer IDs
		let data = crate::infra::sync::fk_mapper::map_sync_json_to_local(
			data,
			<Model as crate::infra::sync::Syncable>::foreign_key_mappings(),
			db,
		)
		.await
		.map_err(|e| sea_orm::DbErr::Custom(format!("FK mapping failed: {}", e)))?;

		// Extract fields from JSON (can't deserialize to Model because id is excluded)
		let obj = data
			.as_object()
			.ok_or_else(|| sea_orm::DbErr::Custom("Entry data is not an object".to_string()))?;

		// Helper to extract field from JSON
		let get_field = |name: &str| -> Result<serde_json::Value, sea_orm::DbErr> {
			obj.get(name)
				.cloned()
				.ok_or_else(|| sea_orm::DbErr::Custom(format!("Missing field: {}", name)))
		};

		// Only sync entries that have UUIDs (sync-ready entries)
		let entry_uuid: Option<Uuid> = serde_json::from_value(get_field("uuid")?)
			.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid uuid: {}", e)))?;

		let entry_uuid = entry_uuid
			.ok_or_else(|| sea_orm::DbErr::Custom("Cannot sync entry without UUID".to_string()))?;

		// Check if entry already exists by UUID
		use sea_orm::{ActiveValue::NotSet, ColumnTrait, EntityTrait, QueryFilter, Set};

		let existing = Entity::find()
			.filter(Column::Uuid.eq(Some(entry_uuid)))
			.one(db)
			.await?;

		// Extract all fields needed for upsert
		let name: String = serde_json::from_value(get_field("name")?).unwrap();
		let kind: i32 = serde_json::from_value(get_field("kind")?).unwrap();
		let extension: Option<String> = serde_json::from_value(get_field("extension")?).unwrap();
		let metadata_id: Option<i32> = serde_json::from_value(get_field("metadata_id")?).unwrap();
		let content_id: Option<i32> = serde_json::from_value(get_field("content_id")?).unwrap();
		let size: i64 = serde_json::from_value(get_field("size")?).unwrap();
		let aggregate_size: i64 = serde_json::from_value(get_field("aggregate_size")?).unwrap();
		let child_count: i32 = serde_json::from_value(get_field("child_count")?).unwrap();
		let file_count: i32 = serde_json::from_value(get_field("file_count")?).unwrap();
		let created_at: DateTimeUtc = serde_json::from_value(get_field("created_at")?).unwrap();
		let modified_at: DateTimeUtc = serde_json::from_value(get_field("modified_at")?).unwrap();
		let accessed_at: Option<DateTimeUtc> =
			serde_json::from_value(get_field("accessed_at")?).unwrap();
		let permissions: Option<String> =
			serde_json::from_value(get_field("permissions")?).unwrap();
		let inode: Option<i64> = serde_json::from_value(get_field("inode")?).unwrap();
		let parent_id: Option<i32> = serde_json::from_value(get_field("parent_id")?).unwrap();

		let entry_id = if let Some(existing_entry) = existing {
			// Update existing entry
			let active = ActiveModel {
				id: Set(existing_entry.id),
				uuid: Set(Some(entry_uuid)),
				name: Set(name.clone()),
				kind: Set(kind),
				extension: Set(extension.clone()),
				metadata_id: Set(metadata_id),
				content_id: Set(content_id),
				size: Set(size),
				aggregate_size: Set(aggregate_size),
				child_count: Set(child_count),
				file_count: Set(file_count),
				created_at: Set(created_at),
				modified_at: Set(modified_at),
				accessed_at: Set(accessed_at),
				permissions: Set(permissions.clone()),
				inode: Set(inode),
				parent_id: Set(parent_id),
			};
			active.update(db).await?;
			existing_entry.id
		} else {
			// Insert new entry
			let active = ActiveModel {
				id: NotSet,
				uuid: Set(Some(entry_uuid)),
				name: Set(name.clone()),
				kind: Set(kind),
				extension: Set(extension.clone()),
				metadata_id: Set(metadata_id),
				content_id: Set(content_id),
				size: Set(size),
				aggregate_size: Set(aggregate_size),
				child_count: Set(child_count),
				file_count: Set(file_count),
				created_at: Set(created_at),
				modified_at: Set(modified_at),
				accessed_at: Set(accessed_at),
				permissions: Set(permissions.clone()),
				inode: Set(inode),
				parent_id: Set(parent_id),
			};
			let inserted = active.insert(db).await?;
			inserted.id
		};

		// If this is a directory, rebuild its directory_paths cache entry
		if EntryKind::from(kind) == EntryKind::Directory {
			// Rebuild directory path from parent chain
			Self::rebuild_directory_path(entry_id, parent_id, &name, db).await?;
		}

		Ok(())
	}

	/// Rebuild directory_paths cache entry for a directory
	///
	/// Computes the full path by walking up the parent chain and concatenating
	/// directory names. This is called after syncing a directory entry to ensure
	/// the local directory_paths cache is correct.
	async fn rebuild_directory_path(
		entry_id: i32,
		parent_id: Option<i32>,
		name: &str,
		db: &DatabaseConnection,
	) -> Result<(), sea_orm::DbErr> {
		use sea_orm::Set;

		// Compute path from parent
		let path = if let Some(parent_id) = parent_id {
			// Get parent's directory path
			match super::directory_paths::Entity::find_by_id(parent_id)
				.one(db)
				.await?
			{
				Some(parent_path) => format!("{}/{}", parent_path.path, name),
				None => {
					// Parent path not found yet - might be syncing out of order
					// This will be fixed by bulk rebuild after backfill completes
					tracing::warn!(
						entry_id = entry_id,
						parent_id = parent_id,
						"Parent directory path not found during sync, deferring rebuild"
					);
					return Ok(());
				}
			}
		} else {
			// Root directory - just use the name
			name.to_string()
		};

		// Upsert directory_paths entry
		let dir_path = super::directory_paths::ActiveModel {
			entry_id: Set(entry_id),
			path: Set(path),
		};

		super::directory_paths::Entity::insert(dir_path)
			.on_conflict(
				sea_orm::sea_query::OnConflict::column(super::directory_paths::Column::EntryId)
					.update_column(super::directory_paths::Column::Path)
					.to_owned(),
			)
			.exec(db)
			.await?;

		Ok(())
	}
}
