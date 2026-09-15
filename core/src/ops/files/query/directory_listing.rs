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
		tracing::debug!("DirectoryListingQuery path={:?}", self.input.path);

		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library in session".to_string()))?;

		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| QueryError::Internal("Library not found".to_string()))?;

		self.query_ephemeral_directory_impl(context, library_id)
			.await
	}
}

impl DirectoryListingQuery {
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

		// A registered source that hasn't been touched this session restores
		// from its snapshot here — including detached drives, whose indexes
		// serve read-only.
		cache.ensure_restored(&local_path).await;

		// Check if we have a cached index that covers this path (or a parent path)
		if let Some(index) = cache.get_for_search(&local_path) {
			tracing::debug!(
				"Found cached ephemeral index for path: {}",
				local_path.display()
			);

			// Try to get directory listing from cached index
			let children = {
				let index_guard = index.read().await;
				index_guard.list_directory(&local_path)
			};

			// Check if the index actually has entries for this directory
			if let Some(children) = children {
				// A parent directory's shallow index may contain this path as an entry
				// but have no children indexed under it. In that case, fall through
				// to trigger a new indexer job for this specific directory.
				if children.is_empty() && !cache.is_indexed(&local_path) {
					tracing::debug!(
						"Subdirectory has no indexed children, will trigger indexing: {}",
						local_path.display()
					);
					// Fall through to indexer dispatch below
				} else {
					tracing::debug!(
						"Cached index has {} children for {}",
						children.len(),
						local_path.display()
					);

					// Convert cached entries to File objects with lazy UUID assignment
					let mut index_write = index.write().await;
					let mut files = Vec::new();

					for child_path in children {
						if let Some(metadata) = index_write.get_entry_ref(&child_path) {
							// Bundle internals are lensed out with hidden files:
							// the package browses as one opaque item.
							if !self.input.include_hidden.unwrap_or(false)
								&& (metadata.is_hidden
									|| crate::ops::indexing::lens::is_bundle_internal(&child_path))
							{
								continue;
							}

							let entry_uuid = index_write.get_or_assign_uuid(&child_path);

							let entry_sd_path = SdPath::Physical {
								device_slug: match &self.input.path {
									SdPath::Physical { device_slug, .. } => device_slug.clone(),
									_ => String::new(),
								},
								path: child_path.clone(),
							};

							let content_kind = index_write.get_content_kind(&child_path);

							let mut file =
								File::from_ephemeral(entry_uuid, &metadata, entry_sd_path);
							file.content_kind = content_kind;
							// Directories report their subtree rollup rather
							// than the directory entry's own on-disk size.
							if metadata.kind == crate::ops::indexing::state::EntryKind::Directory {
								if let Some(bytes) = index_write.subtree_size(&child_path) {
									file.size = bytes;
								}
							}
							files.push(file);
						}
					}
					drop(index_write);

					self.sort_files(&mut files);

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
			} else {
				// Index exists but doesn't have this directory yet
				tracing::debug!(
					"Cached index doesn't contain directory: {}",
					local_path.display()
				);
			}
		}

		// A detached source has no filesystem underneath it; what the restored
		// snapshot holds is all there is. Dispatching an indexer at a missing
		// mount would only produce errors or, worse, an empty rescan.
		if cache.is_detached(&local_path) {
			tracing::debug!(
				"Source for {} is detached; serving snapshot contents only",
				local_path.display()
			);
			return Ok(DirectoryListingOutput {
				files: Vec::new(),
				total_count: 0,
				has_more: false,
			});
		}

		// No cached index or index doesn't cover this path
		// Check if indexing is already in progress
		if cache.is_indexing(&local_path) {
			tracing::debug!(
				"Ephemeral indexing already in progress for {}",
				local_path.display()
			);
			return Ok(DirectoryListingOutput {
				files: Vec::new(),
				total_count: 0,
				has_more: false,
			});
		}

		tracing::debug!("DirectoryListingQuery path={:?}", self.input.path);

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
			let mut config = IndexerJobConfig::ephemeral_browse(
				self.input.path.clone(),
				IndexScope::Current, // Only current directory, not recursive
				false,               // Directory browsing, not volume indexing
			);
			// The listing reaches the UI through ResourceChanged events as
			// files are discovered; the walk itself is background work. A
			// persisted job row and a completion event per directory visited
			// would announce every navigation as a finished job.
			config.run_in_background = true;

			let mut indexer_job = IndexerJob::new(config);

			// Share the cached index with the job
			indexer_job.set_ephemeral_index(ephemeral_index);
			if let Some(store) = cache.store_for(&local_path).await {
				indexer_job.set_source_store(store);
			}

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
					// Dispatch failed, so nothing was indexed — clear the flag
					// without claiming a result.
					cache.mark_indexing_failed(&local_path);
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

crate::register_library_query!(DirectoryListingQuery, "files.directory_listing");
