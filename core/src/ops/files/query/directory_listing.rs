//! Query to list directory contents for file browser
//!
//! This query is optimized for directory browsing in the file explorer UI.
//! It returns direct children of a directory without recursive search.

use crate::infra::query::{QueryError, QueryResult};
use crate::{
	context::CoreContext,
	domain::{addressing::SdPath, content_identity::ContentIdentity, file::File, tag::Tag},
	infra::db::entities::{
		content_identity, directory_paths, entry, sidecar, tag, user_metadata, user_metadata_tag,
		video_media_data,
	},
	infra::query::LibraryQuery,
};
use sea_orm::{
	ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, JoinType, QueryFilter,
	QueryOrder, QuerySelect, RelationTrait,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{collections::HashMap, sync::Arc};
use tracing;
use uuid::Uuid;

/// Input for directory listing
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DirectoryListingInput {
	/// The directory path to list contents for
	pub path: SdPath,
	/// Optional limit on number of results (default: 1000)
	pub limit: Option<u32>,
	/// Whether to include hidden files (default: false)
	pub include_hidden: Option<bool>,
	/// Sort order for results
	pub sort_by: DirectorySortBy,
	/// Whether to show folders before files (default: false)
	pub folders_first: Option<bool>,
}

/// Sort options for directory listing
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum DirectorySortBy {
	/// Sort by name (alphabetical)
	Name,
	/// Sort by modification date (newest first)
	Modified,
	/// Sort by size (largest first)
	Size,
	/// Sort by type (directories first, then files)
	Type,
}

/// Output containing directory contents
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DirectoryListingOutput {
	/// Direct children of the directory as File objects
	pub files: Vec<File>,
	/// Total count of direct children
	pub total_count: u32,
	/// Whether this directory has more children than returned
	pub has_more: bool,
}

/// Query to list directory contents
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DirectoryListingQuery {
	pub input: DirectoryListingInput,
}

impl DirectoryListingQuery {
	pub fn new(path: SdPath) -> Self {
		Self {
			input: DirectoryListingInput {
				path,
				limit: Some(1000),
				include_hidden: Some(false),
				sort_by: DirectorySortBy::Type,
				folders_first: Some(false),
			},
		}
	}

	pub fn with_options(
		path: SdPath,
		limit: Option<u32>,
		include_hidden: Option<bool>,
		sort_by: DirectorySortBy,
	) -> Self {
		Self {
			input: DirectoryListingInput {
				path,
				limit,
				include_hidden,
				sort_by,
				folders_first: Some(false),
			},
		}
	}
}

impl LibraryQuery for DirectoryListingQuery {
	type Input = DirectoryListingInput;
	type Output = DirectoryListingOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		tracing::info!(
			"DirectoryListingQuery::from_input called with input: {:?}",
			input
		);
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		tracing::info!(
			"DirectoryListingQuery::execute called with path: {:?}",
			self.input.path
		);

		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library in session".to_string()))?;
		tracing::info!("Library ID: {}", library_id);

		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| QueryError::Internal("Library not found".to_string()))?;

		let db = library.db();

		// Check if this path's location has IndexMode::None
		if let Some(should_use_ephemeral) = self.check_location_index_mode(db.conn()).await {
			if should_use_ephemeral {
				tracing::info!("Location has IndexMode::None, using ephemeral indexing");
				return self
					.query_ephemeral_directory_impl(context, library_id)
					.await;
			}
		}

		// Try to find the parent directory entry
		match self.find_parent_directory(db.conn()).await {
			Ok(parent_entry) => {
				// Path is indexed - query from database
				let parent_id = parent_entry.id;
				self.query_indexed_directory_impl(parent_id, db.conn())
					.await
			}
			Err(_) => {
				// Path not indexed - trigger ephemeral indexing and return empty
				self.query_ephemeral_directory_impl(context, library_id)
					.await
			}
		}
	}
}

impl DirectoryListingQuery {
	/// Query indexed directory from database
	async fn query_indexed_directory_impl(
		&self,
		parent_id: i32,
		db: &DatabaseConnection,
	) -> QueryResult<DirectoryListingOutput> {
		// Build efficient SQL query to get all data in one go
		let mut sql_query = r#"
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
				e.inode as entry_inode,
				e.parent_id as entry_parent_id,
				ci.id as content_identity_id,
				ci.uuid as content_identity_uuid,
				ci.content_hash as content_hash,
				ci.integrity_hash as integrity_hash,
				ci.mime_type_id as mime_type_id,
				ci.text_content as text_content,
				ci.total_size as total_size,
				ci.entry_count as entry_count,
				ci.first_seen_at as first_seen_at,
				ci.last_verified_at as last_verified_at,
				ck.id as content_kind_id,
				ck.name as content_kind_name,
				vmd.uuid as video_media_uuid,
				vmd.duration_seconds as video_duration_seconds,
				vmd.width as video_width,
				vmd.height as video_height
			FROM entries e
			LEFT JOIN content_identities ci ON e.content_id = ci.id
			LEFT JOIN content_kinds ck ON ci.kind_id = ck.id
			LEFT JOIN video_media_data vmd ON ci.video_media_data_id = vmd.id
			WHERE e.parent_id = ?1
		"#
		.to_string();

		// Apply hidden file filter
		if !self.input.include_hidden.unwrap_or(false) {
			sql_query.push_str(" AND e.name NOT LIKE '.%'");
		}

		// Apply sorting
		let folders_first = self.input.folders_first.unwrap_or(false);
		sql_query.push_str(" ORDER BY ");

		if folders_first {
			// Always show directories before files
			sql_query.push_str("e.kind DESC, ");
		}

		match self.input.sort_by {
			DirectorySortBy::Name => sql_query.push_str("e.name ASC"),
			DirectorySortBy::Modified => sql_query.push_str("e.modified_at DESC"),
			DirectorySortBy::Size => sql_query.push_str("e.size DESC"),
			DirectorySortBy::Type => {
				if !folders_first {
					// Only add kind sorting if folders_first isn't already set
					sql_query.push_str("e.kind DESC, ");
				}
				sql_query.push_str("e.name ASC");
			}
		}

		// Apply limit
		if let Some(limit) = self.input.limit {
			sql_query.push_str(&format!(" LIMIT {}", limit));
		}

		// Execute the query
		let rows = db
			.query_all(sea_orm::Statement::from_sql_and_values(
				sea_orm::DatabaseBackend::Sqlite,
				&sql_query,
				[parent_id.into()],
			))
			.await?;

		tracing::debug!(" Query executed, found {} entries", rows.len());

		if rows.is_empty() {
			tracing::debug!(" Directory is empty");
			return Ok(DirectoryListingOutput {
				files: Vec::new(),
				total_count: 0,
				has_more: false,
			});
		}

		// Get total count for pagination - use the count from the already fetched entries
		let total_count = rows.len() as u32;

		// Collect all content UUIDs for batch sidecar query
		let content_uuids: Vec<Uuid> = rows
			.iter()
			.filter_map(|row| {
				row.try_get::<Option<Uuid>>("", "content_identity_uuid")
					.ok()
					.flatten()
			})
			.collect();

		// Batch fetch all sidecars for these content UUIDs
		let all_sidecars = if !content_uuids.is_empty() {
			sidecar::Entity::find()
				.filter(sidecar::Column::ContentUuid.is_in(content_uuids.clone()))
				.all(db)
				.await?
		} else {
			Vec::new()
		};

		// Group sidecars by content_uuid for fast lookup
		let mut sidecars_by_content: HashMap<Uuid, Vec<crate::domain::file::Sidecar>> =
			HashMap::new();
		for s in all_sidecars {
			sidecars_by_content
				.entry(s.content_uuid)
				.or_insert_with(Vec::new)
				.push(crate::domain::file::Sidecar {
					id: s.id,
					content_uuid: s.content_uuid,
					kind: s.kind,
					variant: s.variant,
					format: s.format,
					status: s.status,
					size: s.size,
					created_at: s.created_at,
					updated_at: s.updated_at,
				});
		}

		// Collect entry UUIDs for tag lookup
		let entry_uuids: Vec<Uuid> = rows
			.iter()
			.filter_map(|row| row.try_get::<Option<Uuid>>("", "entry_uuid").ok().flatten())
			.collect();

		// Batch fetch tags for these entries (both entry-scoped and content-scoped)
		let mut tags_by_entry: HashMap<Uuid, Vec<crate::domain::tag::Tag>> = HashMap::new();

		if !entry_uuids.is_empty() || !content_uuids.is_empty() {
			use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

			tracing::debug!(
				"Loading tags for {} entries and {} content identities",
				entry_uuids.len(),
				content_uuids.len()
			);

			// Load user_metadata for entries and content
			let mut metadata_records = user_metadata::Entity::find()
				.filter(
					user_metadata::Column::EntryUuid
						.is_in(entry_uuids.clone())
						.or(user_metadata::Column::ContentIdentityUuid.is_in(content_uuids.clone())),
				)
				.all(db)
				.await?;

			tracing::debug!("Found {} metadata records", metadata_records.len());

			if !metadata_records.is_empty() {
				let metadata_ids: Vec<i32> = metadata_records.iter().map(|m| m.id).collect();

				// Load user_metadata_tag records
				let metadata_tags = user_metadata_tag::Entity::find()
					.filter(user_metadata_tag::Column::UserMetadataId.is_in(metadata_ids))
					.all(db)
					.await?;

				// Get all unique tag IDs
				let tag_ids: Vec<i32> = metadata_tags.iter().map(|mt| mt.tag_id).collect();
				let unique_tag_ids: std::collections::HashSet<i32> =
					tag_ids.iter().cloned().collect();

				tracing::debug!(
					"Found {} user_metadata_tag records with {} unique tags",
					metadata_tags.len(),
					unique_tag_ids.len()
				);

				// Load tag entities
				let tag_models = tag::Entity::find()
					.filter(tag::Column::Id.is_in(tag_ids))
					.all(db)
					.await?;

				tracing::debug!("Loaded {} tag models", tag_models.len());

				// Build tag_db_id -> Tag mapping
				let tag_map: HashMap<i32, crate::domain::tag::Tag> = tag_models
					.into_iter()
					.filter_map(|t| {
						let db_id = t.id;
						crate::ops::tags::manager::model_to_domain(t)
							.ok()
							.map(|tag| (db_id, tag))
					})
					.collect();

				tracing::debug!("Built tag map with {} entries", tag_map.len());

				// Build metadata_id -> Vec<Tag> mapping
				let mut tags_by_metadata: HashMap<i32, Vec<crate::domain::tag::Tag>> =
					HashMap::new();
				for mt in metadata_tags {
					if let Some(tag) = tag_map.get(&mt.tag_id) {
						tags_by_metadata
							.entry(mt.user_metadata_id)
							.or_insert_with(Vec::new)
							.push(tag.clone());
					}
				}

				// Map tags to entries (prioritize entry-scoped, fall back to content-scoped)
				for metadata in metadata_records {
					if let Some(tags) = tags_by_metadata.get(&metadata.id) {
						// Entry-scoped metadata (higher priority)
						if let Some(entry_uuid) = metadata.entry_uuid {
							tags_by_entry.insert(entry_uuid, tags.clone());
						}
						// Content-scoped metadata (applies to all entries with this content)
						else if let Some(content_uuid) = metadata.content_identity_uuid {
							// Apply to all entries with this content_uuid
							for row in &rows {
								if let Some(ci_uuid) = row
									.try_get::<Option<Uuid>>("", "content_identity_uuid")
									.ok()
									.flatten()
								{
									if ci_uuid == content_uuid {
										if let Some(entry_uuid) = row
											.try_get::<Option<Uuid>>("", "entry_uuid")
											.ok()
											.flatten()
										{
											// Only set if not already set by entry-scoped metadata
											tags_by_entry
												.entry(entry_uuid)
												.or_insert_with(|| tags.clone());
										}
									}
								}
							}
						}
					}
				}
			}
		}

		// Convert to File objects
		let mut files = Vec::new();
		for row in rows {
			// Extract data from SQL row
			let entry_id: i32 = row.try_get("", "entry_id").unwrap_or(0);
			let entry_uuid: Option<Uuid> = row.try_get("", "entry_uuid").ok();
			let entry_name: String = row.try_get("", "entry_name").unwrap_or_default();
			let entry_kind: i32 = row.try_get("", "entry_kind").unwrap_or(0);
			let entry_extension: Option<String> = row.try_get("", "entry_extension").ok();
			let entry_size: i64 = row.try_get("", "entry_size").unwrap_or(0);
			let entry_aggregate_size: i64 = row.try_get("", "entry_aggregate_size").unwrap_or(0);
			let entry_child_count: i32 = row.try_get("", "entry_child_count").unwrap_or(0);
			let entry_file_count: i32 = row.try_get("", "entry_file_count").unwrap_or(0);
			let entry_created_at: chrono::DateTime<chrono::Utc> = row
				.try_get("", "entry_created_at")
				.unwrap_or_else(|_| chrono::Utc::now());
			let entry_modified_at: chrono::DateTime<chrono::Utc> = row
				.try_get("", "entry_modified_at")
				.unwrap_or_else(|_| chrono::Utc::now());
			let entry_accessed_at: Option<chrono::DateTime<chrono::Utc>> =
				row.try_get("", "entry_accessed_at").ok();

			// Content identity data
			let content_identity_uuid: Option<Uuid> = row.try_get("", "content_identity_uuid").ok();
			let content_hash: Option<String> = row.try_get("", "content_hash").ok();
			let integrity_hash: Option<String> = row.try_get("", "integrity_hash").ok();
			let mime_type_id: Option<i32> = row.try_get("", "mime_type_id").ok();
			let text_content: Option<String> = row.try_get("", "text_content").ok();
			let total_size: Option<i64> = row.try_get("", "total_size").ok();
			let entry_count: Option<i32> = row.try_get("", "entry_count").ok();
			let first_seen_at: Option<chrono::DateTime<chrono::Utc>> =
				row.try_get("", "first_seen_at").ok();
			let last_verified_at: Option<chrono::DateTime<chrono::Utc>> =
				row.try_get("", "last_verified_at").ok();

			// Content kind data
			let content_kind_name: Option<String> = row.try_get("", "content_kind_name").ok();

			// Video media data
			let video_media_uuid: Option<Uuid> = row.try_get("", "video_media_uuid").ok();
			let video_duration_seconds: Option<f64> =
				row.try_get("", "video_duration_seconds").ok();
			let video_width: Option<i32> = row.try_get("", "video_width").ok();
			let video_height: Option<i32> = row.try_get("", "video_height").ok();

			// Build SdPath for this entry (child of the parent path)
			// IMPORTANT: Include extension in the path for files
			let full_name = if let Some(ext) = &entry_extension {
				format!("{}.{}", entry_name, ext)
			} else {
				entry_name.clone()
			};

			let entry_sd_path = match &self.input.path {
				SdPath::Physical { device_slug, path } => SdPath::Physical {
					device_slug: device_slug.clone(),
					path: path.join(&full_name).into(),
				},
				SdPath::Cloud {
					service,
					identifier,
					path,
				} => SdPath::Cloud {
					service: *service,
					identifier: identifier.clone(),
					path: format!("{}/{}", path, full_name),
				},
				SdPath::Content { content_id } => {
					// This shouldn't happen since we error on Content paths earlier
					SdPath::Content {
						content_id: *content_id,
					}
				}
				SdPath::Sidecar { .. } => {
					// This shouldn't happen since we error on Sidecar paths earlier
					return Err(QueryError::Internal(
						"Sidecar paths not supported for directory listing".to_string(),
					));
				}
			};

			// Create entity model for conversion
			let entity_model = entry::Model {
				id: entry_id,
				uuid: entry_uuid,
				name: entry_name,
				kind: entry_kind,
				extension: entry_extension,
				metadata_id: None,
				content_id: None,
				size: entry_size,
				aggregate_size: entry_aggregate_size,
				child_count: entry_child_count,
				file_count: entry_file_count,
				created_at: entry_created_at,
				modified_at: entry_modified_at,
				accessed_at: entry_accessed_at,
				indexed_at: None, // Not needed for query result conversion
				permissions: None,
				inode: None,
				parent_id: None,
			};

			// Convert to File using from_entity_model
			let mut file = File::from_entity_model(entity_model, entry_sd_path);

			// Add content identity if available
			if let (Some(ci_uuid), Some(ci_hash), Some(ci_first_seen), Some(ci_last_verified)) = (
				content_identity_uuid,
				content_hash,
				first_seen_at,
				last_verified_at,
			) {
				// Convert content_kind name to ContentKind enum
				let kind = content_kind_name
					.as_ref()
					.map(|name| crate::domain::ContentKind::from(name.as_str()))
					.unwrap_or(crate::domain::ContentKind::Unknown);

				file.content_identity = Some(ContentIdentity {
					uuid: ci_uuid,
					kind,
					content_hash: ci_hash,
					integrity_hash,
					mime_type_id,
					text_content,
					total_size: total_size.unwrap_or(0),
					entry_count: entry_count.unwrap_or(0),
					first_seen_at: ci_first_seen,
					last_verified_at: ci_last_verified,
				});
				file.content_kind = kind;

				// Add sidecars from batch lookup
				if let Some(sidecars) = sidecars_by_content.get(&ci_uuid) {
					file.sidecars = sidecars.clone();
				}
			}

			// Add video media data if available
			if let Some(vmd_uuid) = video_media_uuid {
				file.video_media_data = Some(crate::domain::VideoMediaData {
					uuid: vmd_uuid,
					width: video_width.unwrap_or(0) as u32,
					height: video_height.unwrap_or(0) as u32,
					duration_seconds: video_duration_seconds,
					bit_rate: None,
					codec: None,
					pixel_format: None,
					color_space: None,
					color_range: None,
					color_primaries: None,
					color_transfer: None,
					fps_num: None,
					fps_den: None,
					audio_codec: None,
					audio_channels: None,
					audio_sample_rate: None,
					audio_bit_rate: None,
					title: None,
					artist: None,
					album: None,
					creation_time: None,
					date_captured: None,
					blurhash: None,
				});
			}

			// Add tags from batch lookup
			if let Some(entry_uuid_val) = entry_uuid {
				if let Some(tags) = tags_by_entry.get(&entry_uuid_val) {
					tracing::debug!("Adding {} tags to entry {}", tags.len(), entry_uuid_val);
					file.tags = tags.clone();
				}
			}

			files.push(file);
		}

		let has_more = if let Some(limit) = self.input.limit {
			total_count > limit
		} else {
			false
		};

		Ok(DirectoryListingOutput {
			files,
			total_count,
			has_more,
		})
	}

	/// Query ephemeral directory (not indexed) - check cache first, then trigger on-demand indexing
	async fn query_ephemeral_directory_impl(
		&self,
		context: Arc<CoreContext>,
		library_id: Uuid,
	) -> QueryResult<DirectoryListingOutput> {
		use crate::domain::file::File;
		use crate::ops::indexing::{IndexScope, IndexerJob, IndexerJobConfig};

		// Get the local path for cache lookup
		let local_path = match &self.input.path {
			SdPath::Physical { path, .. } => path.clone(),
			_ => {
				tracing::warn!(
					"Ephemeral indexing only supported for physical paths: {:?}",
					self.input.path
				);
				return Ok(DirectoryListingOutput {
					files: Vec::new(),
					total_count: 0,
					has_more: false,
				});
			}
		};

		let cache = context.ephemeral_cache();

		// Check if we have a cached index that covers this path
		if let Some(index) = cache.get_for_path(&local_path) {
			tracing::info!(
				"Found cached ephemeral index for path: {}",
				local_path.display()
			);

			// Try to get directory listing from cached index
			let index_guard = index.read().await;

			// Check if the index actually has entries for this directory
			if let Some(children) = index_guard.list_directory(&local_path) {
				tracing::debug!(
					"Cached index has {} children for {}",
					children.len(),
					local_path.display()
				);

				// Convert cached entries to File objects
				let mut files = Vec::new();
				for child_path in children {
					if let Some(metadata) = index_guard.get_entry_ref(&child_path) {
						// Apply hidden file filter
						if !self.input.include_hidden.unwrap_or(false) && metadata.is_hidden {
							continue;
						}

						// Get UUID from index
						let entry_uuid = index_guard
							.get_entry_uuid(&child_path)
							.unwrap_or_else(Uuid::new_v4);

						// Build SdPath for this entry
						let entry_sd_path = SdPath::Physical {
							device_slug: match &self.input.path {
								SdPath::Physical { device_slug, .. } => device_slug.clone(),
								_ => String::new(),
							},
							path: child_path.clone(),
						};

						// Get content kind from index (identified by extension)
						let content_kind = index_guard.get_content_kind(&child_path);

						// Convert to File
						let mut file = File::from_ephemeral(entry_uuid, &metadata, entry_sd_path);
						file.content_kind = content_kind;
						files.push(file);
					}
				}

				// Apply sorting
				self.sort_files(&mut files);

				// Apply limit
				let total_count = files.len() as u32;
				let has_more = if let Some(limit) = self.input.limit {
					if files.len() > limit as usize {
						files.truncate(limit as usize);
						true
					} else {
						false
					}
				} else {
					false
				};

				return Ok(DirectoryListingOutput {
					files,
					total_count,
					has_more,
				});
			}

			// Index exists but doesn't have this directory yet
			// Fall through to spawn indexer job
			tracing::debug!(
				"Cached index doesn't contain directory: {}",
				local_path.display()
			);
		}

		// No cached index or index doesn't cover this path
		// Check if indexing is already in progress
		if cache.is_indexing(&local_path) {
			tracing::info!("Indexing already in progress for: {}", local_path.display());
			// Return empty, UI will get updates via events
			return Ok(DirectoryListingOutput {
				files: Vec::new(),
				total_count: 0,
				has_more: false,
			});
		}

		tracing::info!(
			"No cached index, triggering ephemeral indexing for: {:?}",
			self.input.path
		);

		// Get library to dispatch indexer job
		if let Some(library) = context.get_library(library_id).await {
			// Create cache entry and get the index to share with the job
			let ephemeral_index = cache.create_for_indexing(local_path.clone());

			// Clear any stale entries from previous indexing (prevents ghost files)
			let cleared = cache.clear_for_reindex(&local_path).await;
			if cleared > 0 {
				tracing::debug!(
					"Cleared {} stale entries for re-indexing: {}",
					cleared,
					local_path.display()
				);
			}

			// Create ephemeral indexer job for this directory (shallow, current scope only)
			let config = IndexerJobConfig::ephemeral_browse(
				self.input.path.clone(),
				IndexScope::Current, // Only current directory, not recursive
			);

			let mut indexer_job = IndexerJob::new(config);

			// Share the cached index with the job
			indexer_job.set_ephemeral_index(ephemeral_index);

			// Dispatch job asynchronously
			// The job will emit ResourceChanged events as files are discovered
			match library.jobs().dispatch(indexer_job).await {
				Ok(_) => {
					tracing::info!("Dispatched ephemeral indexer for {:?}", self.input.path);
				}
				Err(e) => {
					tracing::warn!(
						"Failed to dispatch ephemeral indexer for {:?}: {}",
						self.input.path,
						e
					);
					// Mark indexing as not in progress since job failed
					cache.mark_indexing_complete(&local_path);
				}
			}
		}

		// Return empty result immediately
		// UI will receive ResourceChanged events and populate incrementally
		Ok(DirectoryListingOutput {
			files: Vec::new(),
			total_count: 0,
			has_more: false,
		})
	}

	/// Sort files according to the input options
	fn sort_files(&self, files: &mut Vec<File>) {
		use crate::domain::file::EntryKind;

		let folders_first = self.input.folders_first.unwrap_or(false);

		files.sort_by(|a, b| {
			// Folders first if enabled
			if folders_first {
				let a_is_dir = matches!(a.kind, EntryKind::Directory);
				let b_is_dir = matches!(b.kind, EntryKind::Directory);
				if a_is_dir != b_is_dir {
					return b_is_dir.cmp(&a_is_dir); // Directories first
				}
			}

			// Then apply sort order
			match self.input.sort_by {
				DirectorySortBy::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
				DirectorySortBy::Modified => b.modified_at.cmp(&a.modified_at),
				DirectorySortBy::Size => b.size.cmp(&a.size),
				DirectorySortBy::Type => {
					// Sort by kind (directories first), then name
					if !folders_first {
						let a_is_dir = matches!(a.kind, EntryKind::Directory);
						let b_is_dir = matches!(b.kind, EntryKind::Directory);
						if a_is_dir != b_is_dir {
							return b_is_dir.cmp(&a_is_dir);
						}
					}
					a.name.to_lowercase().cmp(&b.name.to_lowercase())
				}
			}
		});
	}
}

impl DirectoryListingQuery {
	/// Check if the location containing this path has IndexMode::None
	/// Returns Some(true) if should use ephemeral, Some(false) if indexed, None if location not found
	async fn check_location_index_mode(&self, db: &DatabaseConnection) -> Option<bool> {
		use crate::infra::db::entities::location;

		match &self.input.path {
			SdPath::Physical { device_slug, path } => {
				// Find all locations for this device
				let path_str = path.to_string_lossy().to_string();

				// Get all locations and find the one that is a parent of this path
				if let Ok(locations) = location::Entity::find().all(db).await {
					for loc in locations {
						// Get the location's root path
						if let Some(entry_id) = loc.entry_id {
							if let Ok(Some(dir_path)) =
								directory_paths::Entity::find_by_id(entry_id).one(db).await
							{
								// Check if this location's path is a parent of the requested path
								if path_str.starts_with(&dir_path.path) {
									// Check if index_mode is "none"
									return Some(loc.index_mode == "none");
								}
							}
						}
					}
				}
				None
			}
			SdPath::Cloud { .. } => {
				// Similar logic for cloud paths
				// For now, assume cloud paths should check their location too
				None
			}
			_ => None,
		}
	}

	/// Find the parent directory entry for the given SdPath
	async fn find_parent_directory(&self, db: &DatabaseConnection) -> QueryResult<entry::Model> {
		tracing::debug!(
			" find_parent_directory called with path: {:?}",
			self.input.path
		);

		match &self.input.path {
			SdPath::Physical { device_slug, path } => {
				// For directory browsing, we need to find the directory entry
				// by matching the path in the directory_paths table
				let path_str = path.to_string_lossy().to_string();
				tracing::debug!(" Looking for directory path: '{}'", path_str);

				// Find directory entry by path
				tracing::debug!(" Querying directory_paths table...");
				let directory_path = directory_paths::Entity::find()
					.filter(directory_paths::Column::Path.eq(&path_str))
					.one(db)
					.await?;
				tracing::debug!(" Directory path query result: {:?}", directory_path);

				match directory_path {
					Some(dp) => {
						tracing::debug!(" Found directory path entry: {:?}", dp);
						tracing::debug!(" Looking for entry with ID: {}", dp.entry_id);

						// Get the entry for this directory
						let entry_result = entry::Entity::find_by_id(dp.entry_id).one(db).await?;
						tracing::debug!(" Entry query result: {:?}", entry_result);

						entry_result.ok_or_else(|| {
							QueryError::Internal(format!(
								"Entry not found for directory: {}",
								dp.entry_id
							))
						})
					}
					None => {
						tracing::debug!(" Directory not found in directory_paths table");
						// Directory not found in directory_paths table
						// This means the directory hasn't been indexed yet
						// Return a helpful error message
						Err(QueryError::Internal(
						format!("Directory '{}' has not been indexed yet. Please add this location to Spacedrive and wait for indexing to complete.", path_str)
					))
					}
				}
			}
			SdPath::Cloud {
				service,
				identifier,
				path,
			} => {
				// Cloud storage directory browsing
				tracing::debug!(
					" Looking for cloud directory: service={}, identifier={}, path='{}'",
					service.scheme(),
					identifier,
					path
				);

				// Find directory entry by path in directory_paths table
				// Cloud paths are stored the same way as physical paths
				tracing::debug!(" Querying directory_paths table...");
				let directory_path = directory_paths::Entity::find()
					.filter(directory_paths::Column::Path.eq(path))
					.one(db)
					.await?;
				tracing::debug!(" Directory path query result: {:?}", directory_path);

				match directory_path {
					Some(dp) => {
						tracing::debug!(" Found directory path entry: {:?}", dp);
						tracing::debug!(" Looking for entry with ID: {}", dp.entry_id);

						// Get the entry for this directory
						let entry_result = entry::Entity::find_by_id(dp.entry_id).one(db).await?;
						tracing::debug!(" Entry query result: {:?}", entry_result);

						entry_result.ok_or_else(|| {
							QueryError::Internal(format!(
								"Entry not found for cloud directory: {}",
								dp.entry_id
							))
						})
					}
					None => {
						tracing::debug!(" Cloud directory not found in directory_paths table");
						Err(QueryError::Internal(
							format!("Cloud directory '{}' has not been indexed yet. Please ensure the cloud volume is connected and indexing is complete.", path)
						))
					}
				}
			}
			SdPath::Sidecar { .. } => {
				// Sidecar paths are not supported for directory browsing
				Err(QueryError::Internal(
					"Sidecar paths not supported for directory browsing".to_string(),
				))
			}
			SdPath::Content { .. } => {
				// Content-addressed paths are not supported for directory browsing
				Err(QueryError::Internal(
					"Content-addressed paths not supported for directory browsing".to_string(),
				))
			}
		}
	}
}

// Register the query
crate::register_library_query!(DirectoryListingQuery, "files.directory_listing");
