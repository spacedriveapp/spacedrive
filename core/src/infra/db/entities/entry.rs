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
		Some(&["id", "indexed_at"]) // Exclude PK and indexed_at (set locally)
	}

	fn sync_depends_on() -> &'static [&'static str] {
		// Entries depend on volume, content_identity, and user_metadata to ensure FK references
		// exist before entries arrive. parent_id is self-reference (handled via closure rebuild).
		// This prevents entries arriving with FK references that don't exist yet.
		&["volume", "content_identity", "user_metadata"]
	}

	fn foreign_key_mappings() -> Vec<crate::infra::sync::FKMapping> {
		// All FKs use dependency tracking - NEVER set to NULL on missing reference
		// Source data with NULL values is handled correctly (null UUID → null FK)
		vec![
			crate::infra::sync::FKMapping::new("volume_id", "volumes"),
			crate::infra::sync::FKMapping::new("parent_id", "entries"),
			crate::infra::sync::FKMapping::new("metadata_id", "user_metadata"),
			crate::infra::sync::FKMapping::new("content_id", "content_identities"),
		]
	}

	// FK Lookup Methods (entry is FK target for parent_id self-reference and locations)
	async fn lookup_id_by_uuid(
		uuid: Uuid,
		db: &DatabaseConnection,
	) -> Result<Option<i32>, sea_orm::DbErr> {
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
		Ok(Entity::find()
			.filter(Column::Uuid.eq(Some(uuid)))
			.one(db)
			.await?
			.map(|e| e.id))
	}

	async fn lookup_uuid_by_id(
		id: i32,
		db: &DatabaseConnection,
	) -> Result<Option<Uuid>, sea_orm::DbErr> {
		Ok(Entity::find_by_id(id).one(db).await?.and_then(|e| e.uuid))
	}

	async fn batch_lookup_ids_by_uuids(
		uuids: std::collections::HashSet<Uuid>,
		db: &DatabaseConnection,
	) -> Result<std::collections::HashMap<Uuid, i32>, sea_orm::DbErr> {
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
		if uuids.is_empty() {
			return Ok(std::collections::HashMap::new());
		}
		let uuid_opts: Vec<Option<Uuid>> = uuids.into_iter().map(Some).collect();
		let records = Entity::find()
			.filter(Column::Uuid.is_in(uuid_opts))
			.all(db)
			.await?;
		Ok(records
			.into_iter()
			.filter_map(|r| r.uuid.map(|u| (u, r.id)))
			.collect())
	}

	async fn batch_lookup_uuids_by_ids(
		ids: std::collections::HashSet<i32>,
		db: &DatabaseConnection,
	) -> Result<std::collections::HashMap<i32, Uuid>, sea_orm::DbErr> {
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
		if ids.is_empty() {
			return Ok(std::collections::HashMap::new());
		}
		let records = Entity::find().filter(Column::Id.is_in(ids)).all(db).await?;
		Ok(records
			.into_iter()
			.filter_map(|r| r.uuid.map(|u| (r.id, u)))
			.collect())
	}

	// Post-backfill hook to rebuild entry_closure table
	async fn post_backfill_rebuild(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
		Self::rebuild_all_entry_closures(db).await
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
		device_id: Option<Uuid>,
		since: Option<chrono::DateTime<chrono::Utc>>,
		cursor: Option<(chrono::DateTime<chrono::Utc>, Uuid)>,
		batch_size: usize,
		db: &DatabaseConnection,
	) -> Result<Vec<(Uuid, serde_json::Value, chrono::DateTime<chrono::Utc>)>, sea_orm::DbErr> {
		use crate::infra::sync::Syncable;
		use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

		let mut query = Entity::find();

		// Filter by device ownership if specified (critical for device-owned data sync)
		// Entries reference volumes, which reference devices - join through the chain
		if let Some(owner_device_uuid) = device_id {
			tracing::debug!(
				device_uuid = %owner_device_uuid,
				"Filtering entries by volume ownership"
			);

			// Join through volume to filter by device ownership
			// This allows volume ownership changes to automatically transfer all entries
			use sea_orm::JoinType;
			query = query
				.join(JoinType::InnerJoin, super::entry::Relation::Volume.def())
				.filter(super::volume::Column::DeviceId.eq(owner_device_uuid));

			// Check how many entries match for debugging
			let count_with_device = query.clone().count(db).await?;

			tracing::debug!(
				entries_with_device = count_with_device,
				"Entries matching device ownership through volume"
			);

			if count_with_device == 0 {
				tracing::warn!(
					device_uuid = %owner_device_uuid,
					"No entries found for device, returning empty"
				);
				return Ok(Vec::new());
			}
		}

		// Filter by watermark timestamp if specified
		// Use indexed_at (when we indexed/synced) not modified_at (file modification time)
		if let Some(since_time) = since {
			query = query.filter(Column::IndexedAt.gte(since_time));
		}

		// Cursor-based pagination with tie-breaker
		// WHERE (indexed_at > cursor_ts) OR (indexed_at = cursor_ts AND uuid > cursor_uuid)
		if let Some((cursor_ts, cursor_uuid)) = cursor {
			query = query.filter(
				Condition::any().add(Column::IndexedAt.gt(cursor_ts)).add(
					Condition::all()
						.add(Column::IndexedAt.eq(cursor_ts))
						.add(Column::Uuid.gt(cursor_uuid)),
				),
			);
		}

		// Order by indexed_at + uuid for deterministic pagination
		query = query
			.order_by_asc(Column::IndexedAt)
			.order_by_asc(Column::Uuid);

		// Apply batch limit
		query = query.limit(batch_size as u64);

		let results = query.all(db).await?;

		tracing::debug!(
			result_count = results.len(),
			batch_size = batch_size,
			"Query executed, returning results"
		);

		// Batch lookup directory paths for all directories to avoid N+1 queries
		let directory_ids: Vec<i32> = results
			.iter()
			.filter(|e| e.kind == 1) // Directory
			.map(|e| e.id)
			.collect();

		let directory_paths_map: std::collections::HashMap<i32, String> =
			if !directory_ids.is_empty() {
				super::directory_paths::Entity::find()
					.filter(super::directory_paths::Column::EntryId.is_in(directory_ids))
					.all(db)
					.await?
					.into_iter()
					.map(|dp| (dp.entry_id, dp.path))
					.collect()
			} else {
				std::collections::HashMap::new()
			};

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

			// For directories, include the absolute path from directory_paths
			// This ensures receiving devices get identical paths for universal addressing
			if entry.kind == 1 {
				// Directory
				if let Some(path) = directory_paths_map.get(&entry.id) {
					if let Some(obj) = json.as_object_mut() {
						obj.insert(
							"directory_path".to_string(),
							serde_json::Value::String(path.clone()),
						);
					}
				}
			}

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

			// Use indexed_at for checkpoint/watermark tracking, fallback to modified_at if NULL
			let timestamp = entry.indexed_at.unwrap_or(entry.modified_at);
			sync_results.push((uuid, json, timestamp));
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

	/// Apply deletion by UUID (cascades to entry subtree)
	async fn apply_deletion(uuid: Uuid, db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
		// Find entry by UUID
		let entry = match Entity::find().filter(Column::Uuid.eq(uuid)).one(db).await? {
			Some(e) => e,
			None => return Ok(()), // Already deleted, idempotent
		};

		// Use delete_subtree_internal to cascade delete entire subtree
		// This avoids creating tombstones (we're applying a tombstone)
		crate::ops::indexing::DatabaseStorage::delete_subtree(entry.id, db).await?;

		Ok(())
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
	/// If this is a directory entry, we create or update its entry in the directory_paths table
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

		// Extract UUID (all entries should have UUIDs from indexing)
		let entry_uuid: Option<Uuid> = serde_json::from_value(get_field("uuid")?)
			.map_err(|e| sea_orm::DbErr::Custom(format!("Invalid uuid: {}", e)))?;

		let entry_uuid = entry_uuid.ok_or_else(|| {
			sea_orm::DbErr::Custom(
				"Cannot sync entry without UUID (data consistency error)".to_string(),
			)
		})?;

		// Check if entry was deleted (prevents race condition)
		if Self::is_tombstoned(entry_uuid, db).await? {
			tracing::debug!(uuid = %entry_uuid, "Skipping state change for tombstoned entry");
			return Ok(());
		}

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
		let volume_id: Option<i32> = serde_json::from_value(get_field("volume_id")?).unwrap();

		// Check if parent is tombstoned (prevents orphaned children)
		if let Some(parent) = parent_id {
			// Get parent's UUID to check tombstone
			if let Some(parent_entry) = Entity::find_by_id(parent).one(db).await? {
				if let Some(parent_uuid) = parent_entry.uuid {
					if Self::is_tombstoned(parent_uuid, db).await? {
						tracing::debug!(
							uuid = %entry_uuid,
							parent_uuid = %parent_uuid,
							"Skipping entry - parent is tombstoned"
						);
						return Ok(());
					}
				}
			}
		}

		let now = chrono::Utc::now();

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
				indexed_at: Set(Some(now)), // Record when we synced this entry
				permissions: Set(permissions.clone()),
				inode: Set(inode),
				parent_id: Set(parent_id),
				volume_id: Set(volume_id),
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
				indexed_at: Set(Some(now)), // Record when we synced this entry
				permissions: Set(permissions.clone()),
				inode: Set(inode),
				parent_id: Set(parent_id),
				volume_id: Set(volume_id),
			};
			let inserted = active.insert(db).await?;
			inserted.id
		};

		// Rebuild entry_closure for this synced entry
		// Without this, the entry only has a self-reference and cannot be queried
		// for descendants, breaking subtree operations, location scoping, etc.
		Self::rebuild_entry_closure(entry_id, parent_id, db).await?;

		// If this is a directory, create or update its entry in the directory_paths table
		if EntryKind::from(kind) == EntryKind::Directory {
			// Check if path was included in sync data (preferred - ensures identical paths)
			let path_applied = if let Some(path_value) = data.get("directory_path") {
				if let Some(path_str) = path_value.as_str() {
					// Use the synced absolute path directly from owning device
					let dir_path = super::directory_paths::ActiveModel {
						entry_id: Set(entry_id),
						path: Set(path_str.to_string()),
					};

					// Try insert, fall back to update if exists
					match dir_path.clone().insert(db).await {
						Ok(_) => true,
						Err(_) => {
							// Already exists, update it
							dir_path.update(db).await.is_ok()
						}
					}
				} else {
					false
				}
			} else {
				false
			};

			// Don't rebuild as fallback during sync - this can overwrite correct paths
			// rebuild_directory_path() sets root paths to just the name ("Downloads")
			// which would overwrite the correct absolute path ("/Users/jamespine/Downloads")
			// Path should either:
			// 1. Be included in sync data (location roots, new code)
			// 2. Be created during local indexing (has full filesystem path)
			// If neither, the path will be missing but won't corrupt existing correct paths
		}

		Ok(())
	}

	/// Rebuild entry_closure records for a synced entry
	///
	/// This is critical for maintaining the closure table when entries are synced.
	/// Without this, synced entries only have self-references and cannot be queried
	/// for descendants, which breaks subtree operations and location scoping.
	async fn rebuild_entry_closure(
		entry_id: i32,
		parent_id: Option<i32>,
		db: &DatabaseConnection,
	) -> Result<(), sea_orm::DbErr> {
		use sea_orm::{ConnectionTrait, Set};

		// Delete existing closure records for this entry (as descendant)
		// This ensures we don't have stale relationships if parent changed
		super::entry_closure::Entity::delete_many()
			.filter(super::entry_closure::Column::DescendantId.eq(entry_id))
			.exec(db)
			.await?;

		// Insert self-reference (depth 0)
		let self_closure = super::entry_closure::ActiveModel {
			ancestor_id: Set(entry_id),
			descendant_id: Set(entry_id),
			depth: Set(0),
		};
		self_closure.insert(db).await?;

		// If there's a parent, copy all parent's ancestors
		// This creates the transitive closure relationships
		if let Some(parent_id) = parent_id {
			db.execute(Statement::from_sql_and_values(
				DbBackend::Sqlite,
				r#"
				INSERT INTO entry_closure (ancestor_id, descendant_id, depth)
				SELECT ancestor_id, ?, depth + 1
				FROM entry_closure
				WHERE descendant_id = ?
				"#,
				vec![entry_id.into(), parent_id.into()],
			))
			.await?;

			tracing::debug!(
				entry_id = entry_id,
				parent_id = parent_id,
				"Rebuilt entry_closure for synced entry"
			);
		}

		Ok(())
	}

	/// Creates or updates the entry in the directory_paths table for a directory.
	///
	/// Computes the full path by walking up the parent chain and concatenating
	/// directory names. This is called after syncing a directory entry to ensure
	/// the directory_paths table is correct.
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

	/// Bulk rebuild entire entry_closure table from scratch
	///
	/// This is a safety measure to run after backfill or if the closure table
	/// becomes corrupted. Rebuilds all relationships from the parent_id links.
	pub async fn rebuild_all_entry_closures(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
		tracing::info!("Starting bulk entry_closure rebuild...");

		// Clear existing closure table
		super::entry_closure::Entity::delete_many().exec(db).await?;

		// 1. Insert all self-references (depth 0)
		db.execute(Statement::from_sql_and_values(
			DbBackend::Sqlite,
			r#"
			INSERT INTO entry_closure (ancestor_id, descendant_id, depth)
			SELECT id, id, 0 FROM entries
			"#,
			vec![],
		))
		.await?;

		// 2. Recursively build parent-child relationships
		// Keep inserting until no new relationships found
		let mut iteration = 0;
		loop {
			let result = db
				.execute(Statement::from_sql_and_values(
					DbBackend::Sqlite,
					r#"
					INSERT OR IGNORE INTO entry_closure (ancestor_id, descendant_id, depth)
					SELECT ec.ancestor_id, e.id, ec.depth + 1
					FROM entries e
					INNER JOIN entry_closure ec ON ec.descendant_id = e.parent_id
					WHERE e.parent_id IS NOT NULL
					  AND NOT EXISTS (
						SELECT 1 FROM entry_closure
						WHERE ancestor_id = ec.ancestor_id
						  AND descendant_id = e.id
					  )
					"#,
					vec![],
				))
				.await?;

			iteration += 1;
			let rows_affected = result.rows_affected();

			tracing::debug!(
				iteration = iteration,
				rows_inserted = rows_affected,
				"entry_closure rebuild iteration"
			);

			if rows_affected == 0 {
				break; // No more relationships to add
			}

			if iteration > 100 {
				return Err(sea_orm::DbErr::Custom(
					"entry_closure rebuild exceeded max iterations - possible cycle".to_string(),
				));
			}
		}

		// Count final relationships
		let total = super::entry_closure::Entity::find().count(db).await?;

		tracing::info!(
			iterations = iteration,
			total_relationships = total,
			"Bulk entry_closure rebuild complete"
		);

		Ok(())
	}
}

// Register with sync system via inventory
crate::register_syncable_device_owned!(Model, "entry", "entries", with_deletion, with_rebuild);
