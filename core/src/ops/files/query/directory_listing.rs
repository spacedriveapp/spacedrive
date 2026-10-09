//! Query to list directory contents for file browser
//!
//! This query is optimized for directory browsing in the file explorer UI.
//! It returns direct children of a directory without recursive search.

use crate::infra::query::{QueryError, QueryResult};
use crate::{
	context::CoreContext,
	domain::{addressing::SdPath, content_identity::ContentIdentity, file::File, tag::Tag},
	infra::query::LibraryQuery,
	ops::files::plan::{ChangeKind, FsPlan, SkipReason},
};
use sea_orm::{
	ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, JoinType, QueryFilter,
	QueryOrder, QuerySelect, RelationTrait,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{
	collections::{BTreeMap, BTreeSet, HashMap, HashSet},
	path::{Path, PathBuf},
	sync::Arc,
};
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
	/// The handle of a previewed plan. Through it the listing is the
	/// directory as it would look after that operation
	#[serde(default)]
	pub overlay: Option<Uuid>,
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
	/// Sort by capture time (newest first), from the image facet. Files
	/// without one come last.
	DateTaken,
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
	/// What the overlaid plan does to rows of this directory, by the row's
	/// full name. Empty without an overlay
	#[serde(default)]
	pub overlay: Vec<OverlaidRow>,
}

/// One row of a listing as the overlaid plan changes it.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct OverlaidRow {
	pub name: String,
	pub change: ChangeKind,
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
				overlay: None,
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
				overlay: None,
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

		let mut listing = self
			.query_arena_directory_impl(context.clone(), library_id)
			.await?;
		if let Some(handle) = self.input.overlay {
			let plan = context.plans.get(handle).ok_or_else(|| {
				QueryError::InvalidInput(
					"the plan under that handle has lapsed; preview again".to_string(),
				)
			})?;
			self.overlay(&context, &plan, &mut listing).await;
		}
		Ok(listing)
	}
}

impl DirectoryListingQuery {
	/// List a directory from the volume index, walking it on demand when no partition covers it yet
	async fn query_arena_directory_impl(
		&self,
		context: Arc<CoreContext>,
		library_id: Uuid,
	) -> QueryResult<DirectoryListingOutput> {
		use crate::domain::file::File;
		use crate::ops::indexing::{IndexScope, IndexerJob, IndexerJobConfig};

		// A path on another device is served from its replica, kept current
		// by the peer-mount plane. Indexing happens on the owner, so there is
		// no dispatch fall-through: what the replica holds is the answer.
		if let SdPath::Physical { device_slug, path } = &self.input.path {
			if *device_slug != crate::device::get_current_device_slug() {
				if let Some(output) = self
					.list_remote_replica(&context, library_id, device_slug, path)
					.await
				{
					return Ok(output);
				}
				tracing::debug!(
					"no replica covers {} on device '{device_slug}'",
					path.display()
				);
				return Ok(DirectoryListingOutput {
					files: Vec::new(),
					total_count: 0,
					has_more: false,
					overlay: Vec::new(),
				});
			}
		}

		// Get the local path for cache lookup
		let local_path = match &self.input.path {
			SdPath::Physical { path, .. } => path.clone(),
			_ => {
				tracing::warn!(
					"Browsing is only supported for physical paths: {:?}",
					self.input.path
				);
				return Ok(DirectoryListingOutput {
					files: Vec::new(),
					total_count: 0,
					has_more: false,
					overlay: Vec::new(),
				});
			}
		};

		// The volume decides how the path is written. A folder browsed as
		// /Users/me is the same directory as /System/Volumes/Data/Users/me,
		// and only the volume's spelling lands in the partition the volume
		// map owns; taking the caller's spelling would grow a second tree of
		// files the map already holds.
		let local_path = match context.volume_manager.locate_path(&local_path).await {
			Some((_, spelled)) => spelled,
			None => local_path,
		};

		let cache = context.volume_index();

		// A registered source that hasn't been touched this session restores
		// from its snapshot here — including detached drives, whose indexes
		// serve read-only.
		cache.ensure_restored(&local_path).await;

		// Check if we have a cached index that covers this path (or a parent path)
		if let Some(index) = cache.get_for_search(&local_path) {
			tracing::debug!("Found an arena covering path: {}", local_path.display());

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

					let device_slug = match &self.input.path {
						SdPath::Physical { device_slug, .. } => device_slug.clone(),
						_ => String::new(),
					};
					let mut files = self.files_from_index(&index, children, &device_slug).await;
					// Decorate after the cap: assertions and facets live
					// only in SQLite, so the arena's page still reads the
					// store. Only a capture order needs the facet ahead of
					// the sort, at the directory's cost rather than the page's.
					let by_capture = matches!(self.input.sort_by, DirectorySortBy::DateTaken);
					if by_capture {
						crate::ops::search::capture::decorate_files(&cache, &mut files).await;
					}
					let mut listing = self.finalize_listing(files);
					if !by_capture {
						crate::ops::search::capture::decorate_files(&cache, &mut listing.files)
							.await;
					}
					crate::ops::tags::decorate::decorate_files(&cache, &mut listing.files).await;
					crate::ops::indexing::kinds::decorate_kinds(&cache, &mut listing.files).await;
					return Ok(listing);
				}
			} else {
				// Index exists but doesn't have this directory yet
				tracing::debug!(
					"Cached index doesn't contain directory: {}",
					local_path.display()
				);
			}
		}

		// A detached source has no filesystem underneath it, and dispatching
		// an indexer at a missing mount would only produce errors or, worse,
		// an empty rescan. Its records are still true, though: when the
		// snapshot did not cover this directory, the store answers.
		if cache.is_detached(&local_path) {
			if let Some(listing) = self.list_from_store(&context, &local_path).await {
				tracing::debug!(
					"Source for {} is detached; serving its store",
					local_path.display()
				);
				return Ok(listing);
			}
			tracing::debug!(
				"Source for {} is detached; serving snapshot contents only",
				local_path.display()
			);
			return Ok(DirectoryListingOutput {
				files: Vec::new(),
				total_count: 0,
				has_more: false,
				overlay: Vec::new(),
			});
		}

		// A source whose map is being filled from its store is in that state
		// for the length of the fill, and its records are already true, so
		// the store answers for anything beneath the root until the arena
		// does; a browse dispatched into that tree would clear the subtree
		// the fill has already placed.
		if cache.is_filling_from_store(&local_path) {
			if let Some(listing) = self.list_from_store(&context, &local_path).await {
				tracing::debug!(
					"Map of {} is being filled from its store; serving the store",
					local_path.display()
				);
				return Ok(listing);
			}
		}

		// No cached index or index doesn't cover this path
		// Check if indexing is already in progress
		if cache.is_indexing(&local_path) {
			tracing::debug!("Indexing already in progress for {}", local_path.display());
			return Ok(DirectoryListingOutput {
				files: Vec::new(),
				total_count: 0,
				has_more: false,
				overlay: Vec::new(),
			});
		}

		tracing::debug!("DirectoryListingQuery path={:?}", self.input.path);

		// Get library to dispatch indexer job
		if let Some(library) = context.get_library(library_id).await {
			// Create cache entry and get the index to share with the job
			let arena = cache.create_for_indexing(local_path.clone());

			// Clear any stale entries from previous indexing (prevents ghost files)
			let cleared = cache.clear_for_reindex(&local_path).await;
			if cleared > 0 {
				tracing::debug!(
					"Cleared {} stale entries for re-indexing: {}",
					cleared,
					local_path.display()
				);
			}

			// Walk this directory (current scope only). The job walks the
			// volume's spelling of the path so every entry it inserts lands in
			// the partition resolved above.
			let mut config = IndexerJobConfig::new(
				SdPath::Physical {
					device_slug: crate::device::get_current_device_slug(),
					path: local_path.clone(),
				},
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
			indexer_job.set_arena(arena);
			if let Some(store) = cache.store_for(&local_path).await {
				indexer_job.set_source_store(store);
			}

			// Dispatch job asynchronously
			// The job will emit ResourceChanged events as files are discovered
			match library.jobs().dispatch(indexer_job).await {
				Ok(_) => {
					tracing::info!("Dispatched a browse walk for {:?}", self.input.path);
				}
				Err(e) => {
					tracing::warn!(
						"Failed to dispatch a browse walk for {:?}: {}",
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
			overlay: Vec::new(),
		})
	}

	/// Serve a listing from the source's store when no arena can answer: a
	/// detached drive whose snapshot did not cover this directory still lists
	/// what it retains. `None` when no registered source covers the path or
	/// the store holds no row for it, which falls back to the empty listing.
	async fn list_from_store(
		&self,
		context: &Arc<CoreContext>,
		local_path: &std::path::Path,
	) -> Option<DirectoryListingOutput> {
		let cache = context.volume_index();
		let source = cache
			.sources()
			.into_iter()
			.filter(|source| local_path.starts_with(&source.root))
			.max_by_key(|source| source.root.as_os_str().len())?;
		let db = cache.read_store(source.id).await?;

		// Store paths are source-relative with forward slashes whatever the
		// host writes; the strip below has to speak the same dialect.
		let relative = local_path
			.strip_prefix(&source.root)
			.ok()?
			.to_str()?
			.replace(std::path::MAIN_SEPARATOR, "/");
		let parent = if relative.is_empty() {
			None
		} else {
			Some(
				sd_store::read::resolve_path(db.pool(), &relative)
					.await
					.ok()??,
			)
		};

		let include_hidden = self.input.include_hidden.unwrap_or(false);
		let children = sd_store::read::children_of(db.pool(), parent, include_hidden)
			.await
			.ok()?;

		let device_slug = crate::device::get_current_device_slug();
		let files = children
			.into_iter()
			.filter_map(|entry| {
				let absolute = source.root.join(&entry.relative_path);
				// Bundle internals are lensed out with hidden files, exactly
				// as the arena path lenses them.
				if !include_hidden && crate::ops::indexing::lens::is_bundle_internal(&absolute) {
					return None;
				}
				let sd_path = SdPath::Physical {
					device_slug: device_slug.clone(),
					path: absolute,
				};
				Some(File::from_store_entry(&entry, sd_path))
			})
			.collect();
		let mut listing = self.finalize_listing(files);
		crate::ops::tags::decorate::decorate_from_store(&db, &mut listing.files).await;
		Some(listing)
	}

	/// Sort files according to the input options
	/// Serve a listing from a paired device's replicated index. `None` when
	/// no replica covers the path, which a caller reports as an empty
	/// directory rather than an error: the owner may simply not share it.
	async fn list_remote_replica(
		&self,
		context: &Arc<CoreContext>,
		library_id: Uuid,
		device_slug: &str,
		path: &std::path::Path,
	) -> Option<DirectoryListingOutput> {
		let share = crate::service::mounts::peer::share_for(context, device_slug, path).await?;

		let children = { share.index.read().await.list_directory(path) }?;
		let files = self
			.files_from_index(&share.index, children, device_slug)
			.await;
		let mut listing = self.finalize_listing(files);
		crate::ops::tags::decorate::decorate_replica(
			context,
			library_id,
			&share,
			&mut listing.files,
		)
		.await;
		Some(listing)
	}

	/// Convert one directory's children in an in-memory index to `File`s,
	/// applying the hidden/bundle lens. Shared by the local cache path and
	/// peer replicas.
	async fn files_from_index(
		&self,
		index: &Arc<tokio::sync::RwLock<crate::ops::indexing::Arena>>,
		children: Vec<std::path::PathBuf>,
		device_slug: &str,
	) -> Vec<File> {
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
					device_slug: device_slug.to_string(),
					path: child_path.clone(),
				};
				let content_kind = index_write.get_content_kind(&child_path);

				let mut file = File::from_arena(entry_uuid, &metadata, entry_sd_path);
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

		files
	}

	/// Sort and cap a listing the way the input asked for.
	fn finalize_listing(&self, mut files: Vec<File>) -> DirectoryListingOutput {
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

		DirectoryListingOutput {
			files,
			total_count,
			has_more,
			overlay: Vec::new(),
		}
	}

	/// The listing as it would look after `plan`: what the plan creates
	/// appears as a row, every row it touches is named with its change, and
	/// a folder it changes inside is marked as merged into. Beneath a source
	/// the plan consumes, what the operation settles is marked as removed and
	/// what it does not stays with its reason. A plan carries paths as its
	/// caller spelled them, so the listed directory matches under both of
	/// its spellings.
	async fn overlay(
		&self,
		context: &CoreContext,
		plan: &FsPlan,
		listing: &mut DirectoryListingOutput,
	) {
		let Some(here) = self.input.path.as_local_path() else {
			return;
		};
		let mut spellings = vec![here.to_path_buf()];
		if let Some((_, spelled)) = context.volume_manager.locate_path(here).await {
			if spelled != here {
				spellings.push(spelled);
			}
		}
		let device_slug = match &self.input.path {
			SdPath::Physical { device_slug, .. } => device_slug.clone(),
			_ => String::new(),
		};

		let mut rows: BTreeMap<String, ChangeKind> = BTreeMap::new();
		let mut inside: BTreeSet<String> = BTreeSet::new();
		for planned in &plan.changes {
			if let Some((name, direct)) = planned
				.path
				.as_local_path()
				.and_then(|path| beneath(&spellings, path))
			{
				if direct {
					rows.insert(name, planned.change.clone());
				} else {
					inside.insert(name);
				}
			}
			if let ChangeKind::Move { from } = &planned.change {
				if let Some((name, direct)) = from
					.as_local_path()
					.and_then(|path| beneath(&spellings, path))
				{
					if direct {
						rows.insert(name, ChangeKind::Delete { last_copy: false });
					} else {
						inside.insert(name);
					}
				}
			}
		}
		for root in plan.roots.iter().filter(|root| root.consumes) {
			let (Some(source), Some(destination)) = (
				root.source.as_local_path(),
				root.destination.as_local_path(),
			) else {
				continue;
			};
			let Some(relative) = spellings
				.iter()
				.find_map(|dir| dir.strip_prefix(source).ok())
			else {
				continue;
			};
			let counterpart = [destination.join(relative)];
			for planned in &plan.changes {
				let Some((name, direct)) = planned
					.path
					.as_local_path()
					.and_then(|path| beneath(&counterpart, path))
				else {
					continue;
				};
				if !direct {
					inside.insert(name);
					continue;
				}
				let after = match &planned.change {
					ChangeKind::Create { .. }
					| ChangeKind::Replace { .. }
					| ChangeKind::Move { .. }
					| ChangeKind::Skip {
						reason:
							SkipReason::DuplicateCandidate
							| SkipReason::DuplicateConfirmed
							| SkipReason::Junk,
					} => ChangeKind::Delete { last_copy: false },
					ChangeKind::CreateDirectory | ChangeKind::MergeInto => ChangeKind::MergeInto,
					stays => stays.clone(),
				};
				rows.insert(name, after);
			}
		}
		for name in inside {
			rows.entry(name).or_insert(ChangeKind::MergeInto);
		}

		let present: HashSet<String> = listing
			.files
			.iter()
			.map(|file| match &file.extension {
				Some(extension) => format!("{}.{extension}", file.name).to_lowercase(),
				None => file.name.to_lowercase(),
			})
			.collect();
		let ghosts: Vec<File> = rows
			.iter()
			.filter_map(|(name, change)| {
				let (size, directory) = match change {
					ChangeKind::Create { size } => (*size, false),
					ChangeKind::CreateDirectory => (0, true),
					ChangeKind::Move { .. } => (0, false),
					_ => return None,
				};
				if present.contains(&name.to_lowercase()) {
					return None;
				}
				let sd_path = SdPath::Physical {
					device_slug: device_slug.clone(),
					path: here.join(name),
				};
				Some(File::planned(sd_path, size, directory))
			})
			.collect();
		if !ghosts.is_empty() {
			listing.total_count += ghosts.len() as u32;
			listing.files.extend(ghosts);
			self.sort_files(&mut listing.files);
		}
		listing.overlay = rows
			.into_iter()
			.map(|(name, change)| OverlaidRow { name, change })
			.collect();
	}

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
				// Dated files newest first, then the undated ones by name.
				DirectorySortBy::DateTaken => match (a.captured_at(), b.captured_at()) {
					(Some(a), Some(b)) => b.cmp(&a),
					(Some(_), None) => std::cmp::Ordering::Less,
					(None, Some(_)) => std::cmp::Ordering::Greater,
					(None, None) => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
				},
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

/// The first name of `path` beneath any of `dirs`, and whether that name is
/// all that remains of it.
fn beneath(dirs: &[PathBuf], path: &Path) -> Option<(String, bool)> {
	dirs.iter().find_map(|dir| {
		let mut parts = path.strip_prefix(dir).ok()?.components();
		let first = parts.next()?;
		Some((
			first.as_os_str().to_string_lossy().into_owned(),
			parts.next().is_none(),
		))
	})
}

#[cfg(test)]
mod tests {
	use sd_store::file::FileKind;

	use super::*;
	use crate::{
		domain::SdPathBatch,
		infra::action::preflight::PreviewableAction,
		ops::files::{
			fixture::{Fixture, T},
			merge::{action::FileMergeAction, FileMergeInput, MergeConflictPolicy},
			plan::SkipReason,
		},
	};

	async fn list(
		fixture: &Fixture,
		path: &std::path::Path,
		overlay: Uuid,
	) -> DirectoryListingOutput {
		let mut query = DirectoryListingQuery::new(SdPath::local(path));
		query.input.overlay = Some(overlay);
		query
			.execute(fixture.core.context.clone(), fixture.session())
			.await
			.expect("listed")
	}

	fn row<'a>(listing: &'a DirectoryListingOutput, name: &str) -> Option<&'a ChangeKind> {
		listing
			.overlay
			.iter()
			.find(|row| row.name == name)
			.map(|row| &row.change)
	}

	/// Through a plan's handle a listing is the directory after the
	/// operation: the destination grows what the plan creates, a folder the
	/// plan continues into is marked, and a consumed source shows what
	/// leaves it and what stays with its reason.
	#[tokio::test]
	async fn an_overlaid_listing_is_the_directory_after_the_plan() {
		let fixture = Fixture::new().await;
		let file = FileKind::File;
		let source = [
			("new.txt", file, 7, T, Some("n"), None, None),
			("diff.txt", file, 3, T, Some("d1"), None, None),
			("sub/inner.txt", file, 2, T, Some("i"), None, None),
		];
		let destination = [
			("diff.txt", file, 5, T, Some("d2"), None, None),
			("sub", FileKind::Directory, 0, T, None, None, None),
		];
		fixture.materialize(&fixture.source, &source);
		fixture.index(&fixture.source, &source).await;
		fixture.materialize(&fixture.destination, &destination);
		fixture.index(&fixture.destination, &destination).await;

		let input = FileMergeInput {
			sources: SdPathBatch {
				paths: vec![SdPath::local(&fixture.source)],
			},
			destination: SdPath::local(&fixture.destination),
			on_conflict: MergeConflictPolicy::Skip,
			consume_sources: true,
			remove_extras: false,
		};
		let plan = FileMergeAction::preview(input, &fixture.preview())
			.await
			.expect("planned");
		let handle = plan.handle.expect("retained");

		let after = list(&fixture, &fixture.destination, handle).await;
		assert_eq!(
			row(&after, "new.txt"),
			Some(&ChangeKind::Create { size: 7 })
		);
		assert_eq!(
			row(&after, "diff.txt"),
			Some(&ChangeKind::Skip {
				reason: SkipReason::Policy
			})
		);
		assert_eq!(row(&after, "sub"), Some(&ChangeKind::MergeInto));
		let ghost = after
			.files
			.iter()
			.find(|file| file.name == "new" && file.extension.as_deref() == Some("txt"))
			.expect("a planned create is a row");
		assert_eq!(ghost.size, 7);
		assert_eq!(
			ghost.sd_path,
			SdPath::local(fixture.destination.join("new.txt"))
		);

		let source_after = list(&fixture, &fixture.source, handle).await;
		assert_eq!(
			row(&source_after, "new.txt"),
			Some(&ChangeKind::Delete { last_copy: false })
		);
		assert_eq!(
			row(&source_after, "diff.txt"),
			Some(&ChangeKind::Skip {
				reason: SkipReason::Policy
			})
		);
		assert_eq!(row(&source_after, "sub"), Some(&ChangeKind::MergeInto));

		let mut lapsed = DirectoryListingQuery::new(SdPath::local(&fixture.destination));
		lapsed.input.overlay = Some(Uuid::new_v4());
		let lapsed = lapsed
			.execute(fixture.core.context.clone(), fixture.session())
			.await;
		assert!(matches!(lapsed, Err(QueryError::InvalidInput(_))));
	}
}
