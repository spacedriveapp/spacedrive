//! Get files by tag query

use crate::{
	context::CoreContext,
	domain::{addressing::SdPath, File},
	infra::{
		db::entities::{
			content_identity, entry, tag, user_metadata, user_metadata_tag,
		},
		query::{LibraryQuery, QueryError, QueryResult},
	},
	ops::tags::manager::TagManager,
};
use sea_orm::{ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, Statement};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetFilesByTagInput {
	pub tag_id: Uuid,
	pub include_children: bool,
	pub min_confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetFilesByTagOutput {
	pub files: Vec<File>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetFilesByTagQuery {
	pub input: GetFilesByTagInput,
}

impl LibraryQuery for GetFilesByTagQuery {
	type Input = GetFilesByTagInput;
	type Output = GetFilesByTagOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library in session".to_string()))?;
		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| QueryError::Internal("Library not found".to_string()))?;

		let db = library.db();
		let conn = db.conn();

		// Collect tag IDs: the requested tag + its children if include_children is set
		let mut tag_uuids = vec![self.input.tag_id];
		if self.input.include_children {
			let manager = TagManager::new(Arc::new(conn.clone()));
			let descendants = manager
				.get_descendants(self.input.tag_id)
				.await
				.map_err(|e| QueryError::Internal(format!("Failed to get descendants: {}", e)))?;
			tag_uuids.extend(descendants.iter().map(|t| t.id));
		}

		let entry_ids = find_entry_ids_for_tags(conn, &tag_uuids, self.input.min_confidence).await?;

		if entry_ids.is_empty() {
			return Ok(GetFilesByTagOutput { files: vec![] });
		}

		// Same SQL join pattern as directory_listing
		let entry_ids_str = entry_ids
			.iter()
			.map(|id| id.to_string())
			.collect::<Vec<_>>()
			.join(",");

		let sql_query = format!(
			r#"
			SELECT
				e.id as entry_id,
				e.uuid as entry_uuid,
				e.name as entry_name,
				e.kind as entry_kind,
				e.extension as entry_extension,
				e.size as entry_size,
				e.aggregate_size as entry_aggregate_size,
				e.child_count as entry_child_count,
				e.file_count as entry_file_count,
				e.created_at as entry_created_at,
				e.modified_at as entry_modified_at,
				e.accessed_at as entry_accessed_at,
				e.content_id as entry_content_id,
				dp.path as parent_path,
				d.slug as device_slug,
				ck.name as content_kind_name,
				ci.uuid as content_identity_uuid
			FROM entries e
			LEFT JOIN directory_paths dp ON dp.entry_id = e.parent_id
			LEFT JOIN volumes v ON e.volume_id = v.id
			LEFT JOIN devices d ON v.device_id = d.uuid
			LEFT JOIN content_identities ci ON e.content_id = ci.id
			LEFT JOIN content_kinds ck ON ci.kind_id = ck.id
			WHERE e.id IN ({})
			"#,
			entry_ids_str
		);

		let rows = conn
			.query_all(Statement::from_string(
				sea_orm::DatabaseBackend::Sqlite,
				sql_query,
			))
			.await
			.map_err(QueryError::SeaOrm)?;

		// Collect UUIDs for batch tag loading
		let entry_uuids: Vec<Uuid> = rows
			.iter()
			.filter_map(|row| row.try_get::<Option<Uuid>>("", "entry_uuid").ok().flatten())
			.collect();
		let content_uuids: Vec<Uuid> = rows
			.iter()
			.filter_map(|row| row.try_get::<Option<Uuid>>("", "content_identity_uuid").ok().flatten())
			.collect();

		// Batch load tags (same logic as directory_listing)
		let tags_by_entry = load_tags_for_entries(conn, &entry_uuids, &content_uuids, &rows).await?;

		// Build File objects — skip rows where required fields can't be decoded
		let mut files = Vec::new();
		for row in &rows {
			let Ok(entry_id) = row.try_get::<i32>("", "entry_id") else {
				tracing::warn!("Skipping row: failed to decode entry_id");
				continue;
			};
			let Ok(entry_name) = row.try_get::<String>("", "entry_name") else {
				tracing::warn!("Skipping row: failed to decode entry_name");
				continue;
			};
			let Ok(entry_created_at) =
				row.try_get::<chrono::DateTime<chrono::Utc>>("", "entry_created_at")
			else {
				tracing::warn!("Skipping row {}: failed to decode entry_created_at", entry_id);
				continue;
			};
			let Ok(entry_modified_at) =
				row.try_get::<chrono::DateTime<chrono::Utc>>("", "entry_modified_at")
			else {
				tracing::warn!("Skipping row {}: failed to decode entry_modified_at", entry_id);
				continue;
			};

			let entry_uuid: Option<Uuid> = row.try_get("", "entry_uuid").ok();
			let Ok(entry_kind) = row.try_get::<i32>("", "entry_kind") else {
				tracing::warn!("Skipping row {}: failed to decode entry_kind", entry_id);
				continue;
			};
			let entry_extension: Option<String> = row.try_get("", "entry_extension").ok();
			let entry_size: i64 = row.try_get("", "entry_size").unwrap_or(0);
			let entry_aggregate_size: i64 = row.try_get("", "entry_aggregate_size").unwrap_or(0);
			let entry_child_count: i32 = row.try_get("", "entry_child_count").unwrap_or(0);
			let entry_file_count: i32 = row.try_get("", "entry_file_count").unwrap_or(0);
			let entry_accessed_at: Option<chrono::DateTime<chrono::Utc>> =
				row.try_get("", "entry_accessed_at").ok();
			let content_kind_name: Option<String> = row.try_get("", "content_kind_name").ok();
			let parent_path: Option<String> = row.try_get("", "parent_path").ok();
			let device_slug: Option<String> = row.try_get("", "device_slug").ok();

			let file_path = if let Some(dir_path) = parent_path {
				let file_name = if let Some(ext) = &entry_extension {
					format!("{}.{}", entry_name, ext)
				} else {
					entry_name.clone()
				};
				if dir_path.ends_with('/') {
					format!("{}{}", dir_path, file_name)
				} else {
					format!("{}/{}", dir_path, file_name)
				}
			} else {
				let file_name = if let Some(ext) = &entry_extension {
					format!("{}.{}", entry_name, ext)
				} else {
					entry_name.clone()
				};
				format!("/{}", file_name)
			};

			let sd_path = SdPath::Physical {
				device_slug: device_slug
					.unwrap_or_else(|| crate::device::get_current_device_slug()),
				path: PathBuf::from(file_path),
			};

			let entry_content_id: Option<i32> = row.try_get("", "entry_content_id").ok();

			let entity_model = entry::Model {
				id: entry_id,
				uuid: entry_uuid,
				name: entry_name,
				kind: entry_kind,
				extension: entry_extension,
				metadata_id: None,
				content_id: entry_content_id,
				size: entry_size,
				aggregate_size: entry_aggregate_size,
				child_count: entry_child_count,
				file_count: entry_file_count,
				created_at: entry_created_at,
				modified_at: entry_modified_at,
				accessed_at: entry_accessed_at,
				indexed_at: None,
				permissions: None,
				inode: None,
				parent_id: None,
				volume_id: None,
			};

			let mut file = File::from_entity_model(entity_model, sd_path);

			if let Some(kind_name) = content_kind_name {
				file.content_kind = crate::domain::ContentKind::from(kind_name.as_str());
			}

			// Attach tags
			if let Some(uuid) = entry_uuid {
				if let Some(tags) = tags_by_entry.get(&uuid) {
					file.tags = tags.clone();
				}
			}

			files.push(file);
		}

		Ok(GetFilesByTagOutput { files })
	}
}

/// Batch load tags for entries, checking both entry_uuid and content_identity_uuid paths.
/// Same logic as directory_listing's tag loading.
async fn load_tags_for_entries(
	conn: &impl ConnectionTrait,
	entry_uuids: &[Uuid],
	content_uuids: &[Uuid],
	rows: &[sea_orm::QueryResult],
) -> QueryResult<HashMap<Uuid, Vec<crate::domain::tag::Tag>>> {
	let mut tags_by_entry: HashMap<Uuid, Vec<crate::domain::tag::Tag>> = HashMap::new();

	if entry_uuids.is_empty() && content_uuids.is_empty() {
		return Ok(tags_by_entry);
	}

	// Load user_metadata for entries and content
	let metadata_records = user_metadata::Entity::find()
		.filter(
			user_metadata::Column::EntryUuid
				.is_in(entry_uuids.to_vec())
				.or(user_metadata::Column::ContentIdentityUuid.is_in(content_uuids.to_vec())),
		)
		.all(conn)
		.await
		.map_err(QueryError::SeaOrm)?;

	if metadata_records.is_empty() {
		return Ok(tags_by_entry);
	}

	let metadata_ids: Vec<i32> = metadata_records.iter().map(|m| m.id).collect();

	// Load user_metadata_tag records
	let metadata_tags = user_metadata_tag::Entity::find()
		.filter(user_metadata_tag::Column::UserMetadataId.is_in(metadata_ids))
		.all(conn)
		.await
		.map_err(QueryError::SeaOrm)?;

	let tag_ids: Vec<i32> = metadata_tags.iter().map(|mt| mt.tag_id).collect();

	// Load tag entities
	let tag_models = tag::Entity::find()
		.filter(tag::Column::Id.is_in(tag_ids))
		.all(conn)
		.await
		.map_err(QueryError::SeaOrm)?;

	// Build tag_db_id -> Tag mapping
	let tag_map: HashMap<i32, crate::domain::tag::Tag> = tag_models
		.into_iter()
		.filter_map(|t| {
			let db_id = t.id;
			crate::ops::tags::manager::model_to_domain(t)
				.ok()
				.map(|tag_domain| (db_id, tag_domain))
		})
		.collect();

	// Build metadata_id -> Vec<Tag> mapping
	let mut tags_by_metadata: HashMap<i32, Vec<crate::domain::tag::Tag>> = HashMap::new();
	for mt in metadata_tags {
		if let Some(tag_domain) = tag_map.get(&mt.tag_id) {
			tags_by_metadata
				.entry(mt.user_metadata_id)
				.or_default()
				.push(tag_domain.clone());
		}
	}

	// Pre-index rows: content_identity_uuid -> Vec<entry_uuid>
	// This avoids O(metadata × rows) rescanning in the content-scoped branch
	let mut entries_by_content: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
	for row in rows {
		if let Some(ci_uuid) = row
			.try_get::<Option<Uuid>>("", "content_identity_uuid")
			.ok()
			.flatten()
		{
			if let Some(eu) = row
				.try_get::<Option<Uuid>>("", "entry_uuid")
				.ok()
				.flatten()
			{
				entries_by_content.entry(ci_uuid).or_default().push(eu);
			}
		}
	}

	// Map tags to entries — merge both entry-scoped and content-scoped tags
	for metadata in &metadata_records {
		if let Some(tags) = tags_by_metadata.get(&metadata.id) {
			if let Some(entry_uuid) = metadata.entry_uuid {
				tags_by_entry
					.entry(entry_uuid)
					.or_default()
					.extend(tags.iter().cloned());
			} else if let Some(content_uuid) = metadata.content_identity_uuid {
				// Content-scoped: lookup pre-indexed entries sharing this content
				if let Some(entry_uuids) = entries_by_content.get(&content_uuid) {
					for eu in entry_uuids {
						tags_by_entry
							.entry(*eu)
							.or_default()
							.extend(tags.iter().cloned());
					}
				}
			}
		}
	}

	// Deduplicate tags per entry (same tag can appear via both entry and content metadata)
	for tags in tags_by_entry.values_mut() {
		let mut seen = HashSet::new();
		tags.retain(|t| seen.insert(t.id));
	}

	Ok(tags_by_entry)
}

/// Find all entry IDs tagged with any of the given tag UUIDs, with optional confidence filter.
async fn find_entry_ids_for_tags(
	db: &impl ConnectionTrait,
	tag_uuids: &[Uuid],
	min_confidence: f32,
) -> QueryResult<Vec<i32>> {
	let tag_models = tag::Entity::find()
		.filter(tag::Column::Uuid.is_in(tag_uuids.to_vec()))
		.all(db)
		.await
		.map_err(QueryError::SeaOrm)?;

	if tag_models.is_empty() {
		return Ok(vec![]);
	}

	let tag_db_ids: Vec<i32> = tag_models.iter().map(|t| t.id).collect();

	let mut umt_query = user_metadata_tag::Entity::find()
		.filter(user_metadata_tag::Column::TagId.is_in(tag_db_ids));

	if min_confidence > 0.0 {
		umt_query = umt_query.filter(user_metadata_tag::Column::Confidence.gte(min_confidence));
	}

	let umt_records = umt_query
		.all(db)
		.await
		.map_err(QueryError::SeaOrm)?;

	if umt_records.is_empty() {
		return Ok(vec![]);
	}

	let um_ids: Vec<i32> = umt_records.iter().map(|r| r.user_metadata_id).collect();

	let um_records = user_metadata::Entity::find()
		.filter(user_metadata::Column::Id.is_in(um_ids))
		.all(db)
		.await
		.map_err(QueryError::SeaOrm)?;

	let entry_uuids: Vec<Uuid> = um_records.iter().filter_map(|um| um.entry_uuid).collect();
	let ci_uuids: Vec<Uuid> =
		um_records.iter().filter_map(|um| um.content_identity_uuid).collect();

	let mut entry_ids: HashSet<i32> = HashSet::new();

	if !entry_uuids.is_empty() {
		let entries = entry::Entity::find()
			.filter(entry::Column::Uuid.is_in(entry_uuids))
			.all(db)
			.await
			.map_err(QueryError::SeaOrm)?;
		entry_ids.extend(entries.iter().map(|e| e.id));
	}

	if !ci_uuids.is_empty() {
		let cis = content_identity::Entity::find()
			.filter(content_identity::Column::Uuid.is_in(ci_uuids.into_iter().map(Some)))
			.all(db)
			.await
			.map_err(QueryError::SeaOrm)?;
		if !cis.is_empty() {
			let ci_ids: Vec<i32> = cis.iter().map(|ci| ci.id).collect();
			let entries = entry::Entity::find()
				.filter(entry::Column::ContentId.is_in(ci_ids.into_iter().map(Some)))
				.all(db)
				.await
				.map_err(QueryError::SeaOrm)?;
			entry_ids.extend(entries.iter().map(|e| e.id));
		}
	}

	Ok(entry_ids.into_iter().collect())
}

crate::register_library_query!(GetFilesByTagQuery, "files.by_tag");
