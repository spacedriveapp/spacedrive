//! Get files by tag query

use crate::{
	context::CoreContext,
	infra::{
		db::entities::{
			content_identity, entry, tag, user_metadata, user_metadata_tag,
		},
		query::{LibraryQuery, QueryError, QueryResult},
	},
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashSet;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetFilesByTagInput {
	pub tag_id: Uuid,
	pub include_children: bool,
	pub min_confidence: f32,
}

/// Lightweight file summary for tag view (count + basic display)
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TaggedFileSummary {
	pub id: Uuid,
	pub name: String,
	pub extension: Option<String>,
	pub size: u64,
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

		let entries = entry::Entity::find()
			.filter(entry::Column::Id.is_in(entry_ids))
			.all(db.conn())
			.await
			.map_err(QueryError::SeaOrm)?;

		let files = entries
			.into_iter()
			.filter_map(|e| {
				Some(TaggedFileSummary {
					id: e.uuid?,
					name: e.name,
					extension: e.extension,
					size: e.size as u64,
				})
			})
			.collect();

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
