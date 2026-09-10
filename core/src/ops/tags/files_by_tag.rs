//! Files carrying a tag.
//!
//! A tag reaches a file two ways: applied to that copy, or applied to the bytes
//! and therefore to every copy of them. Both live on `user_metadata`, one keyed
//! by the record the volume index minted and one by content identity, so this
//! resolves both and merges the answer.

use crate::{
	context::CoreContext,
	domain::{addressing::SdPath, File},
	infra::{
		db::entities::{tag, user_metadata, user_metadata_tag},
		query::{LibraryQuery, QueryError, QueryResult},
	},
	ops::tags::manager::TagManager,
};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::{HashMap, HashSet};
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

		let conn = library.db().conn();

		// The requested tag, plus its children when asked: tagging something
		// "Camera" should find what was tagged "Camera / Leica".
		let mut tag_uuids = vec![self.input.tag_id];
		if self.input.include_children {
			let manager = TagManager::new(Arc::new(conn.clone()));
			let descendants = manager
				.get_descendants(self.input.tag_id)
				.await
				.map_err(|e| QueryError::Internal(format!("Failed to get descendants: {}", e)))?;
			tag_uuids.extend(descendants.iter().map(|t| t.id));
		}

		let scopes = tagged_scopes(conn, &tag_uuids, self.input.min_confidence).await?;
		if scopes.records.is_empty() && scopes.contents.is_empty() {
			return Ok(GetFilesByTagOutput { files: vec![] });
		}

		let cache = context.ephemeral_cache();

		// A record can arrive by both routes: tagged directly, and holding
		// bytes that were tagged. It is one file either way.
		let mut paths: HashMap<Uuid, std::path::PathBuf> = HashMap::new();
		for record in scopes.records {
			if let Some(path) = cache.path_of_record(record).await {
				paths.insert(record, path);
			}
		}
		for content in scopes.contents {
			for copy in cache.copies_of_content(content).await {
				paths.entry(copy.record_uuid).or_insert(copy.path);
			}
		}

		let record_uuids: Vec<Uuid> = paths.keys().copied().collect();
		let tags_by_record = tags_for(conn, &cache, &record_uuids).await?;

		let mut files = Vec::with_capacity(paths.len());
		for (record_uuid, path) in paths {
			let index = cache.resolve_index(&path);
			let mut index = index.write().await;
			let Some(metadata) = index.get_entry_ref(&path) else {
				continue;
			};
			let content_kind = index.get_content_kind(&path);
			drop(index);

			let mut file = File::from_ephemeral(record_uuid, &metadata, SdPath::local(path));
			file.content_kind = content_kind;
			file.tags = tags_by_record
				.get(&record_uuid)
				.cloned()
				.unwrap_or_default();
			files.push(file);
		}

		files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

		Ok(GetFilesByTagOutput { files })
	}
}

/// What a set of tags was applied to, before any of it is resolved to files.
#[derive(Default)]
struct TaggedScopes {
	records: Vec<Uuid>,
	contents: Vec<Uuid>,
}

/// The scopes carrying any of these tags, at or above the confidence floor.
async fn tagged_scopes(
	db: &impl ConnectionTrait,
	tag_uuids: &[Uuid],
	min_confidence: f32,
) -> QueryResult<TaggedScopes> {
	let tag_models = tag::Entity::find()
		.filter(tag::Column::Uuid.is_in(tag_uuids.to_vec()))
		.all(db)
		.await
		.map_err(QueryError::SeaOrm)?;

	if tag_models.is_empty() {
		return Ok(TaggedScopes::default());
	}

	let mut umt_query = user_metadata_tag::Entity::find()
		.filter(user_metadata_tag::Column::TagId.is_in(tag_models.iter().map(|t| t.id)));

	if min_confidence > 0.0 {
		umt_query = umt_query.filter(user_metadata_tag::Column::Confidence.gte(min_confidence));
	}

	let um_ids: Vec<i32> = umt_query
		.all(db)
		.await
		.map_err(QueryError::SeaOrm)?
		.iter()
		.map(|r| r.user_metadata_id)
		.collect();

	if um_ids.is_empty() {
		return Ok(TaggedScopes::default());
	}

	let um_records = user_metadata::Entity::find()
		.filter(user_metadata::Column::Id.is_in(um_ids))
		.all(db)
		.await
		.map_err(QueryError::SeaOrm)?;

	Ok(TaggedScopes {
		records: um_records.iter().filter_map(|um| um.entry_uuid).collect(),
		contents: um_records
			.iter()
			.filter_map(|um| um.content_identity_uuid)
			.collect(),
	})
}

/// Every tag on each of these records, from both scopes, deduplicated.
///
/// A tag can reach one record twice, applied to the copy and to the bytes.
/// Showing it twice would be a bug, so the merge is by tag id.
async fn tags_for(
	conn: &impl ConnectionTrait,
	cache: &crate::ops::indexing::ephemeral::EphemeralIndexCache,
	record_uuids: &[Uuid],
) -> QueryResult<HashMap<Uuid, Vec<crate::domain::tag::Tag>>> {
	let mut by_record: HashMap<Uuid, Vec<crate::domain::tag::Tag>> = HashMap::new();
	if record_uuids.is_empty() {
		return Ok(by_record);
	}

	let mut content_of: HashMap<Uuid, Uuid> = HashMap::new();
	for &record in record_uuids {
		if let Some(content) = cache.content_of(record).await {
			content_of.insert(record, content);
		}
	}

	let metadata_records = user_metadata::Entity::find()
		.filter(
			user_metadata::Column::EntryUuid
				.is_in(record_uuids.to_vec())
				.or(user_metadata::Column::ContentIdentityUuid
					.is_in(content_of.values().copied().collect::<Vec<_>>())),
		)
		.all(conn)
		.await
		.map_err(QueryError::SeaOrm)?;

	if metadata_records.is_empty() {
		return Ok(by_record);
	}

	let metadata_tags = user_metadata_tag::Entity::find()
		.filter(
			user_metadata_tag::Column::UserMetadataId
				.is_in(metadata_records.iter().map(|m| m.id).collect::<Vec<_>>()),
		)
		.all(conn)
		.await
		.map_err(QueryError::SeaOrm)?;

	let tag_models = tag::Entity::find()
		.filter(tag::Column::Id.is_in(metadata_tags.iter().map(|mt| mt.tag_id).collect::<Vec<_>>()))
		.all(conn)
		.await
		.map_err(QueryError::SeaOrm)?;

	let tag_map: HashMap<i32, crate::domain::tag::Tag> = tag_models
		.into_iter()
		.filter_map(|t| {
			let db_id = t.id;
			crate::ops::tags::manager::model_to_domain(t)
				.ok()
				.map(|domain| (db_id, domain))
		})
		.collect();

	let mut tags_by_metadata: HashMap<i32, Vec<crate::domain::tag::Tag>> = HashMap::new();
	for mt in metadata_tags {
		if let Some(domain) = tag_map.get(&mt.tag_id) {
			tags_by_metadata
				.entry(mt.user_metadata_id)
				.or_default()
				.push(domain.clone());
		}
	}

	for metadata in &metadata_records {
		let Some(tags) = tags_by_metadata.get(&metadata.id) else {
			continue;
		};

		if let Some(record) = metadata.entry_uuid {
			by_record.entry(record).or_default().extend(tags.clone());
		} else if let Some(content) = metadata.content_identity_uuid {
			for (&record, &record_content) in &content_of {
				if record_content == content {
					by_record.entry(record).or_default().extend(tags.clone());
				}
			}
		}
	}

	for tags in by_record.values_mut() {
		let mut seen = HashSet::new();
		tags.retain(|t| seen.insert(t.id));
	}

	Ok(by_record)
}

crate::register_library_query!(GetFilesByTagQuery, "files.by_tag");
