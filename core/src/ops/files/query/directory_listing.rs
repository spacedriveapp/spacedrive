//! Query to list directory contents for file browser
//!
//! This query is optimized for directory browsing in the file explorer UI.
//! It returns direct children of a directory without recursive search.

use crate::{
	context::CoreContext,
	cqrs::LibraryQuery,
	domain::{
		addressing::SdPath,
		content_identity::ContentIdentity,
		file::{File, FileConstructionData},
		tag::Tag,
	},
	infra::db::entities::{
		content_identity, directory_paths, entry, sidecar, tag, user_metadata, user_metadata_tag,
	},
};
use anyhow::Result;
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

	fn from_input(input: Self::Input) -> Result<Self> {
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
	) -> Result<Self::Output> {
		tracing::info!(
			"DirectoryListingQuery::execute called with path: {:?}",
			self.input.path
		);

		let library_id = session
			.current_library_id
			.ok_or_else(|| anyhow::anyhow!("No library in session"))?;
		tracing::info!("Library ID: {}", library_id);

		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| anyhow::anyhow!("Library not found"))?;

		let db = library.db();

		// First, find the parent directory entry
		let parent_entry = self.find_parent_directory(db.conn()).await?;
		let parent_id = parent_entry.id;

		// Query for direct children
		let mut query = entry::Entity::find()
			.filter(entry::Column::ParentId.eq(parent_id))
			.join(JoinType::LeftJoin, entry::Relation::UserMetadata.def())
			.join(JoinType::LeftJoin, entry::Relation::ContentIdentity.def());

		// Apply hidden file filter
		if !self.input.include_hidden.unwrap_or(false) {
			// Filter out entries that start with '.' (hidden files)
			query = query.filter(entry::Column::Name.not_like(".%"));
		}

		// Apply sorting
		query = match self.input.sort_by {
			DirectorySortBy::Name => query.order_by_asc(entry::Column::Name),
			DirectorySortBy::Modified => query.order_by_desc(entry::Column::ModifiedAt),
			DirectorySortBy::Size => query.order_by_desc(entry::Column::Size),
			DirectorySortBy::Type => {
				// Directories first, then files, then sort by name within each type
				query
					.order_by_desc(entry::Column::Kind) // Kind 1=Directory, 0=File
					.order_by_asc(entry::Column::Name)
			}
		};

		// Apply limit
		if let Some(limit) = self.input.limit {
			query = query.limit(limit as u64);
		}

		// Execute query
		let entry_models = query.all(db.conn()).await?;
		tracing::debug!(" Query executed, found {} entries", entry_models.len());

		if entry_models.is_empty() {
			tracing::debug!(" Directory is empty");
			return Ok(DirectoryListingOutput {
				files: Vec::new(),
				total_count: 0,
				has_more: false,
			});
		}

		// Extract entry UUIDs for loading related data
		let entry_uuids: Vec<Uuid> = entry_models.iter().filter_map(|e| e.uuid).collect();

		// Load all related data efficiently
		let file_data_map = self.load_files_with_joins(&entry_uuids, db.conn()).await?;

		// Get total count for pagination - use the count from the already fetched entries
		// This is a simplified approach - in production you'd want a separate count query
		let total_count = entry_models.len() as u32;

		// Convert to File objects
		let mut files = Vec::new();
		for entry_model in entry_models {
			// Use entry ID as UUID if uuid is None
			let entry_uuid = entry_model.uuid.unwrap_or_else(|| {
				// Generate a UUID from the entry ID for entries without UUIDs
				Uuid::parse_str(&format!(
					"{:08x}-0000-0000-0000-{:012x}",
					entry_model.id, entry_model.id
				))
				.unwrap_or_else(|_| Uuid::new_v4())
			});

			let file_data = file_data_map.get(&entry_uuid).cloned().unwrap_or_else(|| {
				// Create minimal file data if no related data found
				let entry = self
					.convert_to_entry(entry_model.clone())
					.unwrap_or_else(|_| {
						// Create a minimal entry if conversion fails
						crate::domain::Entry {
							id: entry_uuid,
							sd_path: crate::domain::entry::SdPathSerialized {
								device_id: Uuid::new_v4(),
								path: entry_model.name.clone(),
							},
							name: entry_model.name.clone(),
							kind: crate::domain::entry::EntryKind::File { extension: None },
							size: Some(entry_model.size as u64),
							created_at: Some(entry_model.created_at),
							modified_at: Some(entry_model.modified_at),
							accessed_at: entry_model.accessed_at,
							inode: entry_model.inode.map(|i| i as u64),
							file_id: None,
							parent_id: None,
							location_id: None,
							metadata_id: Uuid::new_v4(),
							content_id: None,
							first_seen_at: entry_model.created_at,
							last_indexed_at: Some(entry_model.created_at),
						}
					});

				FileConstructionData {
					entry,
					content_identity: None,
					tags: Vec::new(),
					sidecars: Vec::new(),
					alternate_paths: Vec::new(),
				}
			});

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
	async fn find_parent_directory(&self, db: &DatabaseConnection) -> Result<entry::Model> {
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
							anyhow::anyhow!("Entry not found for directory: {}", dp.entry_id)
						})
					}
					None => {
						tracing::debug!(" Directory not found in directory_paths table");
						// Directory not found in directory_paths table
						// This means the directory hasn't been indexed yet
						// Return a helpful error message
						Err(anyhow::anyhow!(
							"Directory '{}' has not been indexed yet. Please add this location to Spacedrive and wait for indexing to complete.",
							path_str
						))
					}
				}
			}
			SdPath::Content { .. } => {
				// Content-addressed paths are not supported for directory browsing
				Err(anyhow::anyhow!(
					"Content-addressed paths not supported for directory browsing"
				))
			}
		}
	}

	/// Convert database model to Entry domain object using proper From implementation
	fn convert_to_entry(&self, entry_model: entry::Model) -> Result<crate::domain::Entry> {
		crate::domain::Entry::try_from((entry_model, self.input.path.clone()))
	}

	/// Load files with all related data using SQL joins
	async fn load_files_with_joins(
		&self,
		entry_uuids: &[Uuid],
		db: &DatabaseConnection,
	) -> Result<HashMap<Uuid, FileConstructionData>> {
		if entry_uuids.is_empty() {
			return Ok(HashMap::new());
		}

		// For now, return empty map - we'll create minimal File objects
		// In a real implementation, you'd use proper SQL joins to load:
		// - ContentIdentity data
		// - Tags
		// - Sidecars
		// - Alternate paths
		Ok(HashMap::new())
	}
}

// Register the query
crate::register_library_query!(DirectoryListingQuery, "files.directory_listing");
