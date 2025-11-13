//! Query to list media content (images/videos) for gallery views
//!
//! This query is optimized for gallery/camera roll UI patterns.
//! It returns media files (images and videos) from a directory and optionally
//! includes all descendants, making any directory browsable as a media gallery.

use crate::infra::query::{QueryError, QueryResult};
use crate::{
	context::CoreContext,
	domain::{
		addressing::SdPath,
		content_identity::ContentIdentity,
		file::File,
		ContentKind,
	},
	infra::db::entities::{
		content_identity, directory_paths, entry, image_media_data, sidecar, video_media_data,
	},
	infra::query::LibraryQuery,
};
use sea_orm::{ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

/// Input for media listing
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MediaListingInput {
	/// The directory path to list media for
	pub path: SdPath,
	/// Whether to include media from descendant directories (default: false)
	pub include_descendants: Option<bool>,
	/// Which media types to include (default: both Image and Video)
	pub media_types: Option<Vec<ContentKind>>,
	/// Optional limit on number of results (default: 1000)
	pub limit: Option<u32>,
	/// Sort order for results
	pub sort_by: MediaSortBy,
}

/// Sort options for media listing
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum MediaSortBy {
	/// Sort by modification date (newest first)
	Modified,
	/// Sort by creation date (newest first)
	Created,
	/// Sort by date taken/captured (newest first)
	DateTaken,
	/// Sort by name (alphabetical)
	Name,
	/// Sort by size (largest first)
	Size,
}

/// Output containing media files
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MediaListingOutput {
	/// Media files (images/videos)
	pub files: Vec<File>,
	/// Total count of media files found
	pub total_count: u32,
	/// Whether there are more results than returned
	pub has_more: bool,
}

/// Query to list media content
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MediaListingQuery {
	pub input: MediaListingInput,
}

impl MediaListingQuery {
	pub fn new(path: SdPath) -> Self {
		Self {
			input: MediaListingInput {
				path,
				include_descendants: Some(false),
				media_types: Some(vec![ContentKind::Image, ContentKind::Video]),
				limit: Some(1000),
				sort_by: MediaSortBy::DateTaken,
			},
		}
	}

	pub fn with_options(
		path: SdPath,
		include_descendants: Option<bool>,
		media_types: Option<Vec<ContentKind>>,
		limit: Option<u32>,
		sort_by: MediaSortBy,
	) -> Self {
		Self {
			input: MediaListingInput {
				path,
				include_descendants,
				media_types,
				limit,
				sort_by,
			},
		}
	}
}

impl LibraryQuery for MediaListingQuery {
	type Input = MediaListingInput;
	type Output = MediaListingOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		tracing::info!(
			"MediaListingQuery::from_input called with input: {:?}",
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
			"MediaListingQuery::execute called with path: {:?}",
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

		// First, find the parent directory entry
		let parent_entry = self.find_parent_directory(db.conn()).await?;
		let parent_id = parent_entry.id;

		// Get media types to filter (default to Image and Video)
		let media_types = self
			.input
			.media_types
			.as_ref()
			.cloned()
			.unwrap_or_else(|| vec![ContentKind::Image, ContentKind::Video]);

		// Convert ContentKind enum to database IDs
		let media_type_ids: Vec<i32> = media_types.iter().map(|k| *k as i32).collect();
		let media_type_ids_str = media_type_ids
			.iter()
			.map(|id| id.to_string())
			.collect::<Vec<_>>()
			.join(", ");

		// Build efficient SQL query
		let mut sql_query = format!(
			r#"
			SELECT
				e.id as entry_id,
				e.uuid as entry_uuid,
				e.name as entry_name,
				e.kind as entry_kind,
				e.extension as entry_extension,
				e.size as entry_size,
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
				dp.path as directory_path,
				vmd.uuid as video_media_uuid,
				vmd.duration_seconds as video_duration_seconds,
				vmd.date_captured as video_date_captured,
				imd.date_taken as image_date_taken
			FROM entries e
			INNER JOIN content_identities ci ON e.content_id = ci.id
			LEFT JOIN content_kinds ck ON ci.kind_id = ck.id
			LEFT JOIN directory_paths dp ON e.parent_id = dp.entry_id
			LEFT JOIN video_media_data vmd ON ci.video_media_data_id = vmd.id
			LEFT JOIN image_media_data imd ON ci.image_media_data_id = imd.id
			WHERE ci.kind_id IN ({})
		"#,
			media_type_ids_str
		);

		// Apply directory scope filter
		if self.input.include_descendants.unwrap_or(false) {
			// Include all descendants: match entries where directory path starts with parent path
			sql_query.push_str(" AND (e.parent_id = ?1 OR dp.path LIKE ?2)");
		} else {
			// Only direct children
			sql_query.push_str(" AND e.parent_id = ?1");
		}

		// Apply sorting
		match self.input.sort_by {
			MediaSortBy::Modified => sql_query.push_str(" ORDER BY e.modified_at DESC"),
			MediaSortBy::Created => sql_query.push_str(" ORDER BY e.created_at DESC"),
			MediaSortBy::DateTaken => {
				// Use date_taken for images, date_captured for videos, fall back to modified_at
				sql_query.push_str(" ORDER BY COALESCE(imd.date_taken, vmd.date_captured, e.modified_at) DESC")
			}
			MediaSortBy::Name => sql_query.push_str(" ORDER BY e.name ASC"),
			MediaSortBy::Size => sql_query.push_str(" ORDER BY e.size DESC"),
		}

		// Apply limit
		if let Some(limit) = self.input.limit {
			sql_query.push_str(&format!(" LIMIT {}", limit));
		}

		// Build query parameters
		let parent_path_pattern = if self.input.include_descendants.unwrap_or(false) {
			// Get the directory path for the parent to construct LIKE pattern
			let parent_path = self.get_parent_directory_path(db.conn(), parent_id).await?;
			Some(format!("{}/%", parent_path))
		} else {
			None
		};

		// Execute the query
		let rows = if let Some(path_pattern) = parent_path_pattern {
			db.conn()
				.query_all(sea_orm::Statement::from_sql_and_values(
					sea_orm::DatabaseBackend::Sqlite,
					&sql_query,
					[parent_id.into(), path_pattern.into()],
				))
				.await?
		} else {
			db.conn()
				.query_all(sea_orm::Statement::from_sql_and_values(
					sea_orm::DatabaseBackend::Sqlite,
					&sql_query,
					[parent_id.into()],
				))
				.await?
		};

		tracing::debug!("Query executed, found {} media files", rows.len());

		if rows.is_empty() {
			tracing::debug!("No media files found");
			return Ok(MediaListingOutput {
				files: Vec::new(),
				total_count: 0,
				has_more: false,
			});
		}

		let total_count = rows.len() as u32;

		// Collect all content UUIDs for batch sidecar query
		let content_uuids: Vec<Uuid> = rows
			.iter()
			.filter_map(|row| row.try_get::<Option<Uuid>>("", "content_identity_uuid").ok().flatten())
			.collect();

		// Batch fetch all sidecars for these content UUIDs
		let all_sidecars = if !content_uuids.is_empty() {
			sidecar::Entity::find()
				.filter(sidecar::Column::ContentUuid.is_in(content_uuids.clone()))
				.all(db.conn())
				.await?
		} else {
			Vec::new()
		};

		// Group sidecars by content_uuid for fast lookup
		let mut sidecars_by_content: HashMap<Uuid, Vec<crate::domain::file::Sidecar>> = HashMap::new();
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

			// Directory path
			let directory_path: Option<String> = row.try_get("", "directory_path").ok();

			// Video media data (just duration for grid display)
			let video_media_uuid: Option<Uuid> = row.try_get("", "video_media_uuid").ok();
			let video_duration_seconds: Option<f64> = row.try_get("", "video_duration_seconds").ok();

			// Build full path with extension
			let full_name = if let Some(ext) = &entry_extension {
				format!("{}.{}", entry_name, ext)
			} else {
				entry_name.clone()
			};

			// Construct full file path
			let entry_sd_path = if let Some(dir_path) = directory_path {
				let full_path = if dir_path.ends_with('/') {
					format!("{}{}", dir_path, full_name)
				} else {
					format!("{}/{}", dir_path, full_name)
				};

				match &self.input.path {
					SdPath::Physical { device_slug, .. } => SdPath::Physical {
						device_slug: device_slug.clone(),
						path: full_path.into(),
					},
					SdPath::Cloud {
						service,
						identifier,
						..
					} => SdPath::Cloud {
						service: *service,
						identifier: identifier.clone(),
						path: full_path,
					},
					SdPath::Content { content_id } => SdPath::Content {
						content_id: *content_id,
					},
					SdPath::Sidecar { .. } => {
						return Err(QueryError::Internal(
							"Sidecar paths not supported for media listing".to_string(),
						));
					}
				}
			} else {
				// Fallback to constructing path from parent
				match &self.input.path {
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
					SdPath::Content { content_id } => SdPath::Content {
						content_id: *content_id,
					},
					SdPath::Sidecar { .. } => {
						return Err(QueryError::Internal(
							"Sidecar paths not supported for media listing".to_string(),
						));
					}
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
				aggregate_size: 0,
				child_count: 0,
				file_count: 0,
				created_at: entry_created_at,
				modified_at: entry_modified_at,
				accessed_at: entry_accessed_at,
				indexed_at: None,
				permissions: None,
				inode: None,
				parent_id: None,
			};

			// Convert to File using from_entity_model
			let mut file = File::from_entity_model(entity_model, entry_sd_path);

			// Add content identity if available
			if let (
				Some(ci_uuid),
				Some(ci_hash),
				Some(ci_first_seen),
				Some(ci_last_verified),
			) = (
				content_identity_uuid,
				content_hash,
				first_seen_at,
				last_verified_at,
			) {
				// Convert content_kind name to ContentKind enum
				let kind = content_kind_name
					.as_ref()
					.map(|name| ContentKind::from(name.as_str()))
					.unwrap_or(ContentKind::Unknown);

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

			// Add video duration if available (minimal VideoMediaData for normalized cache)
			if let (Some(vmd_uuid), Some(duration)) = (video_media_uuid, video_duration_seconds) {
				file.video_media_data = Some(crate::domain::VideoMediaData {
					uuid: vmd_uuid,
					width: 0,
					height: 0,
					duration_seconds: Some(duration),
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
				});
			}

			files.push(file);
		}

		let has_more = if let Some(limit) = self.input.limit {
			total_count > limit
		} else {
			false
		};

		Ok(MediaListingOutput {
			files,
			total_count,
			has_more,
		})
	}
}

impl MediaListingQuery {
	/// Find the parent directory entry for the given SdPath
	async fn find_parent_directory(&self, db: &DatabaseConnection) -> QueryResult<entry::Model> {
		tracing::debug!(
			"find_parent_directory called with path: {:?}",
			self.input.path
		);

		match &self.input.path {
			SdPath::Physical { device_slug, path } => {
				let path_str = path.to_string_lossy().to_string();
				tracing::debug!("Looking for directory path: '{}'", path_str);

				let directory_path = directory_paths::Entity::find()
					.filter(directory_paths::Column::Path.eq(&path_str))
					.one(db)
					.await?;
				tracing::debug!("Directory path query result: {:?}", directory_path);

				match directory_path {
					Some(dp) => {
						tracing::debug!("Found directory path entry: {:?}", dp);
						tracing::debug!("Looking for entry with ID: {}", dp.entry_id);

						let entry_result = entry::Entity::find_by_id(dp.entry_id).one(db).await?;
						tracing::debug!("Entry query result: {:?}", entry_result);

						entry_result.ok_or_else(|| {
							QueryError::Internal(format!(
								"Entry not found for directory: {}",
								dp.entry_id
							))
						})
					}
					None => {
						tracing::debug!("Directory not found in directory_paths table");
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
				tracing::debug!(
					"Looking for cloud directory: service={}, identifier={}, path='{}'",
					service.scheme(),
					identifier,
					path
				);

				let directory_path = directory_paths::Entity::find()
					.filter(directory_paths::Column::Path.eq(path))
					.one(db)
					.await?;
				tracing::debug!("Directory path query result: {:?}", directory_path);

				match directory_path {
					Some(dp) => {
						tracing::debug!("Found directory path entry: {:?}", dp);
						tracing::debug!("Looking for entry with ID: {}", dp.entry_id);

						let entry_result = entry::Entity::find_by_id(dp.entry_id).one(db).await?;
						tracing::debug!("Entry query result: {:?}", entry_result);

						entry_result.ok_or_else(|| {
							QueryError::Internal(format!(
								"Entry not found for cloud directory: {}",
								dp.entry_id
							))
						})
					}
					None => {
						tracing::debug!("Cloud directory not found in directory_paths table");
						Err(QueryError::Internal(
							format!("Cloud directory '{}' has not been indexed yet. Please ensure the cloud volume is connected and indexing is complete.", path)
						))
					}
				}
			}
			SdPath::Sidecar { .. } => Err(QueryError::Internal(
				"Sidecar paths not supported for media listing".to_string(),
			)),
			SdPath::Content { .. } => Err(QueryError::Internal(
				"Content-addressed paths not supported for media listing".to_string(),
			)),
		}
	}

	/// Get the directory path for a given entry ID
	async fn get_parent_directory_path(
		&self,
		db: &DatabaseConnection,
		entry_id: i32,
	) -> QueryResult<String> {
		let directory_path = directory_paths::Entity::find()
			.filter(directory_paths::Column::EntryId.eq(entry_id))
			.one(db)
			.await?
			.ok_or_else(|| {
				QueryError::Internal(format!(
					"Directory path not found for entry_id: {}",
					entry_id
				))
			})?;

		Ok(directory_path.path)
	}
}

// Register the query
crate::register_library_query!(MediaListingQuery, "files.media_listing");
