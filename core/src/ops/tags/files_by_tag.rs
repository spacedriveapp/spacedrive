//! Get files by tag query

use crate::{
	context::CoreContext,
	domain::addressing::SdPath,
	infra::{
		db::entities::{
			content_identity, entry, tag, user_metadata, user_metadata_tag,
		},
		query::{LibraryQuery, QueryError, QueryResult},
	},
};
use sea_orm::{ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, Statement};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetFilesByTagInput {
	pub tag_id: Uuid,
	pub include_children: bool,
	pub min_confidence: f32,
}

/// File summary for tag view — includes sd_path for navigation
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TaggedFileSummary {
	pub id: Uuid,
	pub name: String,
	pub extension: Option<String>,
	pub size: u64,
	pub kind: i32,
	pub modified_at: String,
	pub sd_path: SdPath,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetFilesByTagOutput {
	pub files: Vec<TaggedFileSummary>,
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
		let entry_ids = find_entry_ids_for_tag(db.conn(), self.input.tag_id).await?;

		if entry_ids.is_empty() {
			return Ok(GetFilesByTagOutput { files: vec![] });
		}

		// Build SQL with joins to get parent_path and device_slug for SdPath
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
				e.modified_at as entry_modified_at,
				dp.path as parent_path,
				d.slug as device_slug
			FROM entries e
			LEFT JOIN directory_paths dp ON dp.entry_id = e.parent_id
			LEFT JOIN volumes v ON e.volume_id = v.id
			LEFT JOIN devices d ON v.device_id = d.uuid
			WHERE e.id IN ({})
			"#,
			entry_ids_str
		);

		let rows = db
			.conn()
			.query_all(Statement::from_string(
				sea_orm::DatabaseBackend::Sqlite,
				sql_query,
			))
			.await
			.map_err(QueryError::SeaOrm)?;

		let mut files = Vec::new();
		for row in rows {
			let entry_uuid: Option<Uuid> = row.try_get("", "entry_uuid").ok();
			let Some(id) = entry_uuid else { continue };

			let entry_name: String = row.try_get("", "entry_name").unwrap_or_default();
			let entry_kind: i32 = row.try_get("", "entry_kind").unwrap_or(0);
			let entry_extension: Option<String> = row.try_get("", "entry_extension").ok();
			let entry_size: i64 = row.try_get("", "entry_size").unwrap_or(0);
			let entry_modified_at: chrono::DateTime<chrono::Utc> = row
				.try_get("", "entry_modified_at")
				.unwrap_or_else(|_| chrono::Utc::now());

			let parent_path: Option<String> = row.try_get("", "parent_path").ok();
			let device_slug: Option<String> = row.try_get("", "device_slug").ok();

			// Build full path: parent directory path + filename
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
				format!("/{}", entry_name)
			};

			let sd_path = SdPath::Physical {
				device_slug: device_slug.unwrap_or_else(|| "unknown-device".to_string()),
				path: PathBuf::from(file_path),
			};

			files.push(TaggedFileSummary {
				id,
				name: entry_name,
				extension: entry_extension,
				size: entry_size as u64,
				kind: entry_kind,
				modified_at: entry_modified_at.to_rfc3339(),
				sd_path,
			});
		}

		Ok(GetFilesByTagOutput { files })
	}
}

/// Find all entry IDs tagged with the given tag UUID.
/// Handles both entry-scoped and content-scoped user_metadata.
async fn find_entry_ids_for_tag(
	db: &DatabaseConnection,
	tag_uuid: Uuid,
) -> QueryResult<Vec<i32>> {
	// 1. Find tag by UUID
	let Some(tag_model) = tag::Entity::find()
		.filter(tag::Column::Uuid.eq(tag_uuid))
		.one(db)
		.await
		.map_err(QueryError::SeaOrm)?
	else {
		return Ok(vec![]);
	};

	// 2. Find all user_metadata_tag records for this tag
	let umt_records = user_metadata_tag::Entity::find()
		.filter(user_metadata_tag::Column::TagId.eq(tag_model.id))
		.all(db)
		.await
		.map_err(QueryError::SeaOrm)?;

	if umt_records.is_empty() {
		return Ok(vec![]);
	}

	let um_ids: Vec<i32> = umt_records.iter().map(|r| r.user_metadata_id).collect();

	// 3. Fetch all user_metadata records in batch
	let um_records = user_metadata::Entity::find()
		.filter(user_metadata::Column::Id.is_in(um_ids))
		.all(db)
		.await
		.map_err(QueryError::SeaOrm)?;

	let entry_uuids: Vec<Uuid> = um_records.iter().filter_map(|um| um.entry_uuid).collect();
	let ci_uuids: Vec<Uuid> =
		um_records.iter().filter_map(|um| um.content_identity_uuid).collect();

	let mut entry_ids: HashSet<i32> = HashSet::new();

	// 4a. Entries directly linked via entry_uuid
	if !entry_uuids.is_empty() {
		let entries = entry::Entity::find()
			.filter(entry::Column::Uuid.is_in(entry_uuids))
			.all(db)
			.await
			.map_err(QueryError::SeaOrm)?;
		entry_ids.extend(entries.iter().map(|e| e.id));
	}

	// 4b. Entries linked via content_identity_uuid
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
