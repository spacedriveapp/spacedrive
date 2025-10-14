//! Query to list directory contents for file browser
//!
//! This query is optimized for directory browsing in the file explorer UI.
//! It returns direct children of a directory without recursive search.

use crate::infra::query::{QueryError, QueryResult};
use crate::{
	context::CoreContext,
	domain::{
		addressing::SdPath,
		content_identity::ContentIdentity,
		file::{File, FileConstructionData},
		tag::Tag,
	},
	infra::db::entities::{
		content_identity, directory_paths, entry, sidecar, tag, user_metadata, user_metadata_tag,
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

		// First, find the parent directory entry
		let parent_entry = self.find_parent_directory(db.conn()).await?;
		let parent_id = parent_entry.id;

		// Build efficient SQL query to get all data in one go
		let mut sql_query = r#"
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
				ck.name as content_kind_name
			FROM entries e
			LEFT JOIN content_identities ci ON e.content_id = ci.id
			LEFT JOIN content_kinds ck ON ci.kind_id = ck.id
			WHERE e.parent_id = ?1
		"#
		.to_string();

		// Apply hidden file filter
		if !self.input.include_hidden.unwrap_or(false) {
			sql_query.push_str(" AND e.name NOT LIKE '.%'");
		}

		// Apply sorting
		match self.input.sort_by {
			DirectorySortBy::Name => sql_query.push_str(" ORDER BY e.name ASC"),
			DirectorySortBy::Modified => sql_query.push_str(" ORDER BY e.modified_at DESC"),
			DirectorySortBy::Size => sql_query.push_str(" ORDER BY e.size DESC"),
			DirectorySortBy::Type => {
				sql_query.push_str(" ORDER BY e.kind DESC, e.name ASC"); // Directories first, then files
			}
		}

		// Apply limit
		if let Some(limit) = self.input.limit {
			sql_query.push_str(&format!(" LIMIT {}", limit));
		}

		// Execute the query
		let rows = db
			.conn()
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
			let entry_inode: Option<i64> = row.try_get("", "entry_inode").ok();

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

			// Use entry ID as UUID if uuid is None
			let entry_uuid = entry_uuid.unwrap_or_else(|| {
				// Generate a UUID from the entry ID for entries without UUIDs
				Uuid::parse_str(&format!(
					"{:08x}-0000-0000-0000-{:012x}",
					entry_id, entry_id
				))
				.unwrap_or_else(|_| Uuid::new_v4())
			});

			// Create domain Entry
			let entry = crate::domain::Entry {
				id: entry_uuid,
				sd_path: crate::domain::entry::SdPathSerialized {
					device_id: Uuid::new_v4(),
					path: entry_name.clone(),
				},
				name: entry_name.clone(),
				kind: match entry_kind {
					0 => crate::domain::entry::EntryKind::File {
						extension: entry_extension,
					},
					1 => crate::domain::entry::EntryKind::Directory,
					2 => crate::domain::entry::EntryKind::Symlink {
						target: String::new(),
					},
					_ => crate::domain::entry::EntryKind::File {
						extension: entry_extension,
					},
				},
				size: Some(entry_size as u64),
				created_at: Some(entry_created_at),
				modified_at: Some(entry_modified_at),
				accessed_at: entry_accessed_at,
				inode: entry_inode.map(|i| i as u64),
				file_id: None,
				parent_id: None,
				location_id: None,
				metadata_id: Uuid::new_v4(),
				content_id: None,
				first_seen_at: entry_created_at,
				last_indexed_at: Some(entry_created_at),
			};

			// Create content identity if available
			let content_identity =
				if let (Some(ci_uuid), Some(ci_hash), Some(ci_first_seen), Some(ci_last_verified)) =
					(content_identity_uuid, content_hash, first_seen_at, last_verified_at)
				{
					// Convert content_kind name to ContentKind enum
					let kind = content_kind_name
						.as_ref()
						.map(|name| crate::domain::ContentKind::from(name.as_str()))
						.unwrap_or(crate::domain::ContentKind::Unknown);

					Some(crate::domain::ContentIdentity {
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
					})
				} else {
					None
				};

			// Create file construction data
			let file_data = FileConstructionData {
				entry,
				content_identity,
				tags: Vec::new(),            // TODO: Load tags
				sidecars: Vec::new(),        // TODO: Load sidecars
				alternate_paths: Vec::new(), // TODO: Load alternate paths
			};

			let file = File::from_data(file_data);
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
}

impl DirectoryListingQuery {
	/// Find the parent directory entry for the given SdPath
	async fn find_parent_directory(&self, db: &DatabaseConnection) -> QueryResult<entry::Model> {
		tracing::debug!(
			" find_parent_directory called with path: {:?}",
			self.input.path
		);

		match &self.input.path {
			SdPath::Physical { device_id, path } => {
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
			SdPath::Cloud { volume_id, path } => {
				// Cloud storage directory browsing
				tracing::debug!(" Looking for cloud directory: volume={}, path='{}'", volume_id, path);

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
			SdPath::Content { .. } => {
				// Content-addressed paths are not supported for directory browsing
				Err(QueryError::Internal(
					"Content-addressed paths not supported for directory browsing".to_string(),
				))
			}
		}
	}

	/// Convert database model to Entry domain object using proper From implementation
	fn convert_to_entry(&self, entry_model: entry::Model) -> QueryResult<crate::domain::Entry> {
		Ok(crate::domain::Entry::try_from((
			entry_model,
			self.input.path.clone(),
		))?)
	}
}

// Register the query
crate::register_library_query!(DirectoryListingQuery, "files.directory_listing");
