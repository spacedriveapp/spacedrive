use crate::{
	location::file_path_helper::{
		file_path_pub_and_cas_ids, file_path_walker, FilePathMetadata, IsolatedFilePathData,
		MetadataExt,
	},
	prisma::file_path,
	util::{
		db::{device_from_db, inode_from_db},
		error::FileIOError,
	},
};

#[cfg(target_family = "unix")]
use crate::location::file_path_helper::get_inode_and_device;

#[cfg(target_family = "windows")]
use crate::location::file_path_helper::get_inode_and_device_from_path;

use std::{
	collections::{HashMap, HashSet, VecDeque},
	future::Future,
	hash::{Hash, Hasher},
	path::{Path, PathBuf},
};

use chrono::{DateTime, Duration, FixedOffset};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tracing::trace;
use uuid::Uuid;

use super::{
	rules::{IndexerRule, RuleKind},
	IndexerError,
};

const TO_WALK_QUEUE_INITIAL_CAPACITY: usize = 32;
const WALKER_PATHS_BUFFER_INITIAL_CAPACITY: usize = 256;
const WALK_SINGLE_DIR_PATHS_BUFFER_INITIAL_CAPACITY: usize = 32;

/// `WalkEntry` represents a single path in the filesystem, for any comparison purposes, we only
/// consider the path itself, not the metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct WalkedEntry {
	pub pub_id: Uuid,
	pub iso_file_path: IsolatedFilePathData<'static>,
	pub metadata: FilePathMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToWalkEntry {
	path: PathBuf,
	parent_dir_accepted_by_its_children: Option<bool>,
}

#[derive(Debug)]
struct WalkingEntry {
	iso_file_path: IsolatedFilePathData<'static>,
	maybe_metadata: Option<FilePathMetadata>,
}

impl From<WalkingEntry> for WalkedEntry {
	fn from(walking_entry: WalkingEntry) -> Self {
		let WalkingEntry {
			iso_file_path,
			maybe_metadata,
		} = walking_entry;

		Self {
			pub_id: Uuid::new_v4(),
			iso_file_path,
			metadata: maybe_metadata
				.expect("we always use Some in `the inner_walk_single_dir` function"),
		}
	}
}

impl From<(Uuid, WalkingEntry)> for WalkedEntry {
	fn from((pub_id, walking_entry): (Uuid, WalkingEntry)) -> Self {
		let WalkingEntry {
			iso_file_path,
			maybe_metadata,
		} = walking_entry;

		Self {
			pub_id,
			iso_file_path,
			metadata: maybe_metadata
				.expect("we always use Some in `the inner_walk_single_dir` function"),
		}
	}
}

impl PartialEq for WalkingEntry {
	fn eq(&self, other: &Self) -> bool {
		self.iso_file_path == other.iso_file_path
	}
}

impl Eq for WalkingEntry {}

impl Hash for WalkingEntry {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.iso_file_path.hash(state);
	}
}

pub struct WalkResult<Walked, ToUpdate, ToRemove>
where
	Walked: Iterator<Item = WalkedEntry>,
	ToUpdate: Iterator<Item = WalkedEntry>,
	ToRemove: Iterator<Item = file_path_pub_and_cas_ids::Data>,
{
	pub walked: Walked,
	pub to_update: ToUpdate,
	pub to_walk: VecDeque<ToWalkEntry>,
	pub to_remove: ToRemove,
	pub errors: Vec<IndexerError>,
}

/// This function walks through the filesystem, applying the rules to each entry and then returning
/// a list of accepted entries. There are some useful comments in the implementation of this function
/// in case of doubts.
pub(super) async fn walk<FilePathDBFetcherFut, ToRemoveDbFetcherFut>(
	root: impl AsRef<Path>,
	indexer_rules: &[IndexerRule],
	mut update_notifier: impl FnMut(&Path, usize),
	file_paths_db_fetcher: impl Fn(Vec<file_path::WhereParam>) -> FilePathDBFetcherFut,
	to_remove_db_fetcher: impl Fn(
		IsolatedFilePathData<'static>,
		Vec<file_path::WhereParam>,
	) -> ToRemoveDbFetcherFut,
	iso_file_path_factory: impl Fn(&Path, bool) -> Result<IsolatedFilePathData<'static>, IndexerError>,
	limit: u64,
) -> Result<
	WalkResult<
		impl Iterator<Item = WalkedEntry>,
		impl Iterator<Item = WalkedEntry>,
		impl Iterator<Item = file_path_pub_and_cas_ids::Data>,
	>,
	IndexerError,
>
where
	FilePathDBFetcherFut: Future<Output = Result<Vec<file_path_walker::Data>, IndexerError>>,
	ToRemoveDbFetcherFut:
		Future<Output = Result<Vec<file_path_pub_and_cas_ids::Data>, IndexerError>>,
{
	let root = root.as_ref();

	let mut to_walk = VecDeque::with_capacity(TO_WALK_QUEUE_INITIAL_CAPACITY);
	to_walk.push_back(ToWalkEntry {
		path: root.to_path_buf(),
		parent_dir_accepted_by_its_children: None,
	});
	let mut indexed_paths = HashSet::with_capacity(WALKER_PATHS_BUFFER_INITIAL_CAPACITY);
	let mut errors = vec![];
	let mut paths_buffer = Vec::with_capacity(WALKER_PATHS_BUFFER_INITIAL_CAPACITY);
	let mut to_remove = vec![];

	while let Some(ref entry) = to_walk.pop_front() {
		let current_to_remove = inner_walk_single_dir(
			root,
			entry,
			indexer_rules,
			&mut update_notifier,
			&to_remove_db_fetcher,
			&iso_file_path_factory,
			WorkingTable {
				indexed_paths: &mut indexed_paths,
				paths_buffer: &mut paths_buffer,
				maybe_to_walk: Some(&mut to_walk),
				errors: &mut errors,
			},
		)
		.await;
		to_remove.push(current_to_remove);

		if indexed_paths.len() >= limit as usize {
			break;
		}
	}

	let (walked, to_update) = filter_existing_paths(indexed_paths, file_paths_db_fetcher).await?;

	Ok(WalkResult {
		walked,
		to_update,
		to_walk,
		to_remove: to_remove.into_iter().flatten(),
		errors,
	})
}

pub(super) async fn keep_walking<FilePathDBFetcherFut, ToRemoveDbFetcherFut>(
	to_walk_entry: &ToWalkEntry,
	indexer_rules: &[IndexerRule],
	mut update_notifier: impl FnMut(&Path, usize),
	file_paths_db_fetcher: impl Fn(Vec<file_path::WhereParam>) -> FilePathDBFetcherFut,
	to_remove_db_fetcher: impl Fn(
		IsolatedFilePathData<'static>,
		Vec<file_path::WhereParam>,
	) -> ToRemoveDbFetcherFut,
	iso_file_path_factory: impl Fn(&Path, bool) -> Result<IsolatedFilePathData<'static>, IndexerError>,
) -> Result<
	WalkResult<
		impl Iterator<Item = WalkedEntry>,
		impl Iterator<Item = WalkedEntry>,
		impl Iterator<Item = file_path_pub_and_cas_ids::Data>,
	>,
	IndexerError,
>
where
	FilePathDBFetcherFut: Future<Output = Result<Vec<file_path_walker::Data>, IndexerError>>,
	ToRemoveDbFetcherFut:
		Future<Output = Result<Vec<file_path_pub_and_cas_ids::Data>, IndexerError>>,
{
	let mut to_keep_walking = VecDeque::with_capacity(TO_WALK_QUEUE_INITIAL_CAPACITY);
	let mut indexed_paths = HashSet::with_capacity(WALK_SINGLE_DIR_PATHS_BUFFER_INITIAL_CAPACITY);
	let mut paths_buffer = Vec::with_capacity(WALK_SINGLE_DIR_PATHS_BUFFER_INITIAL_CAPACITY);
	let mut errors = vec![];

	let to_remove = inner_walk_single_dir(
		to_walk_entry.path.clone(),
		to_walk_entry,
		indexer_rules,
		&mut update_notifier,
		&to_remove_db_fetcher,
		&iso_file_path_factory,
		WorkingTable {
			indexed_paths: &mut indexed_paths,
			paths_buffer: &mut paths_buffer,
			maybe_to_walk: Some(&mut to_keep_walking),
			errors: &mut errors,
		},
	)
	.await;

	let (walked, to_update) = filter_existing_paths(indexed_paths, file_paths_db_fetcher).await?;

	Ok(WalkResult {
		walked,
		to_update,
		to_walk: to_keep_walking,
		to_remove: to_remove.into_iter(),
		errors,
	})
}

pub(super) async fn walk_single_dir<FilePathDBFetcherFut, ToRemoveDbFetcherFut>(
	root: impl AsRef<Path>,
	indexer_rules: &[IndexerRule],
	mut update_notifier: impl FnMut(&Path, usize) + '_,
	file_paths_db_fetcher: impl Fn(Vec<file_path::WhereParam>) -> FilePathDBFetcherFut,
	to_remove_db_fetcher: impl Fn(
		IsolatedFilePathData<'static>,
		Vec<file_path::WhereParam>,
	) -> ToRemoveDbFetcherFut,
	iso_file_path_factory: impl Fn(&Path, bool) -> Result<IsolatedFilePathData<'static>, IndexerError>,
	add_root: bool,
) -> Result<
	(
		impl Iterator<Item = WalkedEntry>,
		impl Iterator<Item = WalkedEntry>,
		Vec<file_path_pub_and_cas_ids::Data>,
		Vec<IndexerError>,
	),
	IndexerError,
>
where
	FilePathDBFetcherFut: Future<Output = Result<Vec<file_path_walker::Data>, IndexerError>>,
	ToRemoveDbFetcherFut:
		Future<Output = Result<Vec<file_path_pub_and_cas_ids::Data>, IndexerError>>,
{
	let root = root.as_ref();

	let mut indexed_paths = HashSet::with_capacity(WALK_SINGLE_DIR_PATHS_BUFFER_INITIAL_CAPACITY);

	if add_root {
		let metadata = fs::metadata(root)
			.await
			.map_err(|e| FileIOError::from((root, e)))?;

		let (inode, device) = {
			#[cfg(target_family = "unix")]
			{
				get_inode_and_device(&metadata)
			}

			#[cfg(target_family = "windows")]
			{
				get_inode_and_device_from_path(&root).await
			}
		}?;

		indexed_paths.insert(WalkingEntry {
			iso_file_path: iso_file_path_factory(root, true)?,
			maybe_metadata: Some(FilePathMetadata {
				inode,
				device,
				size_in_bytes: metadata.len(),
				created_at: metadata.created_or_now().into(),
				modified_at: metadata.modified_or_now().into(),
			}),
		});
	}

	let mut paths_buffer = Vec::with_capacity(WALK_SINGLE_DIR_PATHS_BUFFER_INITIAL_CAPACITY);
	let mut errors = vec![];

	let to_remove = inner_walk_single_dir(
		root,
		&ToWalkEntry {
			path: root.to_path_buf(),
			parent_dir_accepted_by_its_children: None,
		},
		indexer_rules,
		&mut update_notifier,
		&to_remove_db_fetcher,
		&iso_file_path_factory,
		WorkingTable {
			indexed_paths: &mut indexed_paths,
			paths_buffer: &mut paths_buffer,
			maybe_to_walk: None,
			errors: &mut errors,
		},
	)
	.await;

	let (walked, to_update) = filter_existing_paths(indexed_paths, file_paths_db_fetcher).await?;

	Ok((walked, to_update, to_remove, errors))
}

async fn filter_existing_paths<F>(
	indexed_paths: HashSet<WalkingEntry>,
	file_paths_db_fetcher: impl Fn(Vec<file_path::WhereParam>) -> F,
) -> Result<
	(
		impl Iterator<Item = WalkedEntry>,
		impl Iterator<Item = WalkedEntry>,
	),
	IndexerError,
>
where
	F: Future<Output = Result<Vec<file_path_walker::Data>, IndexerError>>,
{
	if !indexed_paths.is_empty() {
		file_paths_db_fetcher(
			indexed_paths
				.iter()
				.map(|entry| &entry.iso_file_path)
				.map(Into::into)
				.collect(),
		)
		.await
	} else {
		Ok(vec![])
	}
	.map(move |file_paths| {
		let isolated_paths_already_in_db = file_paths
			.into_iter()
			.flat_map(|file_path| {
				IsolatedFilePathData::try_from(file_path.clone())
					.map(|iso_file_path| (iso_file_path, file_path))
			})
			.collect::<HashMap<_, _>>();

		let mut to_update = vec![];

		let to_create = indexed_paths
			.into_iter()
			.filter_map(|entry| {
				if let Some(file_path) = isolated_paths_already_in_db.get(&entry.iso_file_path) {
					if let (Some(metadata), Some(inode), Some(device), Some(date_modified)) = (
						&entry.maybe_metadata,
						&file_path.inode,
						&file_path.device,
						&file_path.date_modified,
					) {
						let (inode, device) =
							(inode_from_db(&inode[0..8]), device_from_db(&device[0..8]));

						// Datetimes stored in DB loses a bit of precision, so we need to check against a delta
						// instead of using != operator
						if inode != metadata.inode
							|| device != metadata.device || DateTime::<FixedOffset>::from(
							metadata.modified_at,
						) - *date_modified
							> Duration::milliseconds(1)
						{
							to_update.push(
								(sd_utils::from_bytes_to_uuid(&file_path.pub_id), entry).into(),
							);
						}
					}

					None
				} else {
					Some(entry.into())
				}
			})
			.collect::<Vec<_>>();

		(to_create.into_iter(), to_update.into_iter())
	})
}

struct WorkingTable<'a> {
	indexed_paths: &'a mut HashSet<WalkingEntry>,
	paths_buffer: &'a mut Vec<WalkingEntry>,
	maybe_to_walk: Option<&'a mut VecDeque<ToWalkEntry>>,
	errors: &'a mut Vec<IndexerError>,
}

async fn inner_walk_single_dir<ToRemoveDbFetcherFut>(
	root: impl AsRef<Path>,
	ToWalkEntry {
		path,
		parent_dir_accepted_by_its_children,
	}: &ToWalkEntry,
	indexer_rules: &[IndexerRule],
	update_notifier: &mut impl FnMut(&Path, usize),
	to_remove_db_fetcher: impl Fn(
		IsolatedFilePathData<'static>,
		Vec<file_path::WhereParam>,
	) -> ToRemoveDbFetcherFut,
	iso_file_path_factory: &impl Fn(&Path, bool) -> Result<IsolatedFilePathData<'static>, IndexerError>,
	WorkingTable {
		indexed_paths,
		paths_buffer,
		mut maybe_to_walk,
		errors,
	}: WorkingTable<'_>,
) -> Vec<file_path_pub_and_cas_ids::Data>
where
	ToRemoveDbFetcherFut:
		Future<Output = Result<Vec<file_path_pub_and_cas_ids::Data>, IndexerError>>,
{
	let Ok(iso_file_path_to_walk) = iso_file_path_factory(path, true).map_err(|e| errors.push(e))
	else {
		return vec![];
	};

	let Ok(mut read_dir) = fs::read_dir(path)
		.await
		.map_err(|e| errors.push(FileIOError::from((path.clone(), e)).into()))
	else {
		return vec![];
	};

	let root = root.as_ref();

	// Just to make sure...
	paths_buffer.clear();

	let mut found_paths_counts = 0;

	// Marking with a loop label here in case of rejection or errors, to continue with next entry
	'entries: loop {
		let entry = match read_dir.next_entry().await {
			Ok(Some(entry)) => entry,
			Ok(None) => break,
			Err(e) => {
				errors.push(FileIOError::from((path.clone(), e)).into());
				continue;
			}
		};

		// Accept by children has three states,
		// None if we don't now yet or if this check doesn't apply
		// Some(true) if this check applies and it passes
		// Some(false) if this check applies and it was rejected
		// and we pass the current parent state to its children
		let mut accept_by_children_dir = *parent_dir_accepted_by_its_children;

		let current_path = entry.path();

		// Just sending updates if we found more paths since the last loop
		let current_found_paths_count = paths_buffer.len();
		if found_paths_counts != current_found_paths_count {
			update_notifier(
				&current_path,
				indexed_paths.len() + current_found_paths_count,
			);
			found_paths_counts = current_found_paths_count;
		}

		trace!(
			"Current filesystem path: {}, accept_by_children_dir: {:#?}",
			current_path.display(),
			accept_by_children_dir
		);

		let Ok(rules_per_kind) = IndexerRule::apply_all(indexer_rules, &current_path)
			.await
			.map_err(|e| errors.push(e.into()))
		else {
			continue 'entries;
		};

		if rules_per_kind
			.get(&RuleKind::RejectFilesByGlob)
			.map_or(false, |reject_results| {
				reject_results.iter().any(|reject| !reject)
			}) {
			trace!(
				"Path {} rejected by `RuleKind::RejectFilesByGlob`",
				current_path.display()
			);
			continue 'entries;
		}

		let Ok(metadata) = entry
			.metadata()
			.await
			.map_err(|e| errors.push(FileIOError::from((entry.path(), e)).into()))
		else {
			continue 'entries;
		};

		// TODO: Hard ignoring symlinks for now, but this should be configurable
		if metadata.is_symlink() {
			continue 'entries;
		}

		let is_dir = metadata.is_dir();

		let Ok((inode, device)) = {
			#[cfg(target_family = "unix")]
			{
				get_inode_and_device(&metadata)
			}

			#[cfg(target_family = "windows")]
			{
				get_inode_and_device_from_path(&current_path).await
			}
		}
		.map_err(|e| errors.push(e.into())) else {
			continue 'entries;
		};

		if is_dir {
			// If it is a directory, first we check if we must reject it and its children entirely
			if rules_per_kind
				.get(&RuleKind::RejectIfChildrenDirectoriesArePresent)
				.map_or(false, |reject_results| {
					reject_results.iter().any(|reject| !reject)
				}) {
				trace!(
					"Path {} rejected by rule `RuleKind::RejectIfChildrenDirectoriesArePresent`",
					current_path.display(),
				);
				continue 'entries;
			}

			// Then we check if we must accept it and its children
			if let Some(accept_by_children_rules) =
				rules_per_kind.get(&RuleKind::AcceptIfChildrenDirectoriesArePresent)
			{
				if accept_by_children_rules.iter().any(|accept| *accept) {
					accept_by_children_dir = Some(true);
				}

				// If it wasn't accepted then we mark as rejected
				if accept_by_children_dir.is_none() {
					trace!(
						"Path {} rejected because it didn't passed in any AcceptIfChildrenDirectoriesArePresent rule",
						current_path.display()
					);
					accept_by_children_dir = Some(false);
				}
			}

			// Then we mark this directory the be walked in too
			if let Some(ref mut to_walk) = maybe_to_walk {
				to_walk.push_back(ToWalkEntry {
					path: entry.path(),
					parent_dir_accepted_by_its_children: accept_by_children_dir,
				});
			}
		}

		if rules_per_kind
			.get(&RuleKind::AcceptFilesByGlob)
			.map_or(false, |accept_rules| {
				accept_rules.iter().all(|accept| !accept)
			}) {
			trace!(
				"Path {} reject because it didn't passed in any AcceptFilesByGlob rules",
				current_path.display()
			);
			continue 'entries;
		}

		if accept_by_children_dir.unwrap_or(true) {
			let Ok(iso_file_path) =
				iso_file_path_factory(&current_path, is_dir).map_err(|e| errors.push(e))
			else {
				continue 'entries;
			};

			paths_buffer.push(WalkingEntry {
				iso_file_path,
				maybe_metadata: Some(FilePathMetadata {
					inode,
					device,
					size_in_bytes: metadata.len(),
					created_at: metadata.created_or_now().into(),
					modified_at: metadata.modified_or_now().into(),
				}),
			});

			// If the ancestors directories wasn't indexed before, now we do
			for ancestor in current_path
				.ancestors()
				.skip(1) // Skip the current directory as it was already indexed
				.take_while(|&ancestor| ancestor != root)
			{
				let Ok(iso_file_path) =
					iso_file_path_factory(ancestor, true).map_err(|e| errors.push(e))
				else {
					// Checking the next ancestor, as this one we got an error
					continue;
				};

				let mut ancestor_iso_walking_entry = WalkingEntry {
					iso_file_path,
					maybe_metadata: None,
				};
				trace!("Indexing ancestor {}", ancestor.display());
				if !indexed_paths.contains(&ancestor_iso_walking_entry) {
					let Ok(metadata) = fs::metadata(ancestor)
						.await
						.map_err(|e| errors.push(FileIOError::from((&ancestor, e)).into()))
					else {
						// Checking the next ancestor, as this one we got an error
						continue;
					};
					let Ok((inode, device)) = {
						#[cfg(target_family = "unix")]
						{
							get_inode_and_device(&metadata)
						}

						#[cfg(target_family = "windows")]
						{
							get_inode_and_device_from_path(ancestor).await
						}
					}
					.map_err(|e| errors.push(e.into())) else {
						// Checking the next ancestor, as this one we got an error
						continue;
					};

					ancestor_iso_walking_entry.maybe_metadata = Some(FilePathMetadata {
						inode,
						device,
						size_in_bytes: metadata.len(),
						created_at: metadata.created_or_now().into(),
						modified_at: metadata.modified_or_now().into(),
					});

					paths_buffer.push(ancestor_iso_walking_entry);
				} else {
					// If indexed_paths contains the current ancestors, then it will contain
					// also all if its ancestors too, so we can stop here
					break;
				}
			}
		}
	}

	// We continue the function even if we fail to fetch `file_path`s to remove,
	// the DB will have old `file_path`s but at least this is better than
	// don't adding the newly indexed paths
	let to_remove = to_remove_db_fetcher(
		iso_file_path_to_walk,
		paths_buffer
			.iter()
			.map(|entry| &entry.iso_file_path)
			.map(Into::into)
			.collect(),
	)
	.await
	.unwrap_or_else(|e| {
		errors.push(e);
		vec![]
	});

	// Just merging the `found_paths` with `indexed_paths` here in the end to avoid possibly
	// multiple rehashes during function execution
	indexed_paths.extend(paths_buffer.drain(..));

	to_remove
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
	use super::super::rules::RulePerKind;
	use super::*;
	use chrono::Utc;
	use globset::{Glob, GlobSetBuilder};
	use tempfile::{tempdir, TempDir};
	use tokio::fs;
	// use tracing_test::traced_test;

	impl PartialEq for WalkedEntry {
		fn eq(&self, other: &Self) -> bool {
			self.iso_file_path == other.iso_file_path
		}
	}

	impl Eq for WalkedEntry {}

	impl Hash for WalkedEntry {
		fn hash<H: Hasher>(&self, state: &mut H) {
			self.iso_file_path.hash(state);
		}
	}

	async fn prepare_location() -> TempDir {
		let root = tempdir().unwrap();
		let root_path = root.path();
		let rust_project = root_path.join("rust_project");
		let inner_project = root_path.join("inner");
		let node_project = inner_project.join("node_project");
		let photos = root_path.join("photos");

		fs::create_dir(&rust_project).await.unwrap();
		fs::create_dir(&inner_project).await.unwrap();
		fs::create_dir(&node_project).await.unwrap();
		fs::create_dir(&photos).await.unwrap();

		// Making rust and node projects a git repository
		fs::create_dir(rust_project.join(".git")).await.unwrap();
		fs::create_dir(node_project.join(".git")).await.unwrap();

		// Populating rust project
		fs::File::create(rust_project.join("Cargo.toml"))
			.await
			.unwrap();
		let rust_src_dir = rust_project.join("src");
		fs::create_dir(&rust_src_dir).await.unwrap();
		fs::File::create(rust_src_dir.join("main.rs"))
			.await
			.unwrap();
		let rust_target_dir = rust_project.join("target");
		fs::create_dir(&rust_target_dir).await.unwrap();
		let rust_build_dir = rust_target_dir.join("debug");
		fs::create_dir(&rust_build_dir).await.unwrap();
		fs::File::create(rust_build_dir.join("main")).await.unwrap();

		// Populating node project
		fs::File::create(node_project.join("package.json"))
			.await
			.unwrap();
		let node_src_dir = node_project.join("src");
		fs::create_dir(&node_src_dir).await.unwrap();
		fs::File::create(node_src_dir.join("App.tsx"))
			.await
			.unwrap();
		let node_modules = node_project.join("node_modules");
		fs::create_dir(&node_modules).await.unwrap();
		let node_modules_dep = node_modules.join("react");
		fs::create_dir(&node_modules_dep).await.unwrap();
		fs::File::create(node_modules_dep.join("package.json"))
			.await
			.unwrap();

		// Photos directory
		for photo in ["photo1.png", "photo2.jpg", "photo3.jpeg", "text.txt"].iter() {
			fs::File::create(photos.join(photo)).await.unwrap();
		}

		root
	}

	#[tokio::test]
	async fn test_walk_without_rules() {
		let root = prepare_location().await;
		let root_path = root.path();

		let metadata = FilePathMetadata {
			inode: 0,
			device: 0,
			size_in_bytes: 0,
			created_at: Utc::now(),
			modified_at: Utc::now(),
		};

		let f = |path, is_dir| IsolatedFilePathData::new(0, root_path, path, is_dir).unwrap();
		let pub_id = Uuid::new_v4();

		#[rustfmt::skip]
		let expected = [
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("rust_project"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("rust_project/.git"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("rust_project/Cargo.toml"), false), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("rust_project/src"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("rust_project/src/main.rs"), false), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("rust_project/target"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("rust_project/target/debug"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("rust_project/target/debug/main"), false), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("inner"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("inner/node_project"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("inner/node_project/.git"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("inner/node_project/package.json"), false), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("inner/node_project/src"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("inner/node_project/src/App.tsx"), false), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("inner/node_project/node_modules"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("inner/node_project/node_modules/react"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("inner/node_project/node_modules/react/package.json"), false), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("photos"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("photos/photo1.png"), false), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("photos/photo2.jpg"), false), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("photos/photo3.jpeg"), false), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("photos/text.txt"), false), metadata },
		]
		.into_iter()
		.collect::<HashSet<_>>();

		let walk_result = walk(
			root_path.to_path_buf(),
			&[],
			|_, _| {},
			|_| async { Ok(vec![]) },
			|_, _| async { Ok(vec![]) },
			|path, is_dir| {
				IsolatedFilePathData::new(0, root_path, path, is_dir).map_err(Into::into)
			},
			420,
		)
		.await
		.unwrap();

		if !walk_result.errors.is_empty() {
			panic!("errors: {:#?}", walk_result.errors);
		}

		let actual = walk_result.walked.collect::<HashSet<_>>();

		if actual != expected {
			panic!("difference: {:#?}", expected.difference(&actual));
		}
	}

	#[tokio::test]
	// #[traced_test]
	async fn test_only_photos() {
		let root = prepare_location().await;
		let root_path = root.path();

		let metadata = FilePathMetadata {
			inode: 0,
			device: 0,
			size_in_bytes: 0,
			created_at: Utc::now(),
			modified_at: Utc::now(),
		};

		let f = |path, is_dir| IsolatedFilePathData::new(0, root_path, path, is_dir).unwrap();
		let pub_id = Uuid::new_v4();

		#[rustfmt::skip]
		let expected = [
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("photos"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("photos/photo1.png"), false), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("photos/photo2.jpg"), false), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("photos/photo3.jpeg"), false), metadata },
		]
		.into_iter()
		.collect::<HashSet<_>>();

		let only_photos_rule = &[IndexerRule::new(
			"only photos".to_string(),
			false,
			vec![RulePerKind::AcceptFilesByGlob(
				vec![],
				GlobSetBuilder::new()
					.add(Glob::new("{*.png,*.jpg,*.jpeg}").unwrap())
					.build()
					.unwrap(),
			)],
		)];

		let walk_result = walk(
			root_path.to_path_buf(),
			only_photos_rule,
			|_, _| {},
			|_| async { Ok(vec![]) },
			|_, _| async { Ok(vec![]) },
			|path, is_dir| {
				IsolatedFilePathData::new(0, root_path, path, is_dir).map_err(Into::into)
			},
			420,
		)
		.await
		.unwrap();

		if !walk_result.errors.is_empty() {
			panic!("errors: {:#?}", walk_result.errors);
		}

		let actual = walk_result.walked.collect::<HashSet<_>>();

		if actual != expected {
			panic!("difference: {:#?}", expected.difference(&actual));
		}
	}

	#[tokio::test]
	// #[traced_test]
	async fn test_git_repos() {
		let root = prepare_location().await;
		let root_path = root.path();

		let metadata = FilePathMetadata {
			inode: 0,
			device: 0,
			size_in_bytes: 0,
			created_at: Utc::now(),
			modified_at: Utc::now(),
		};

		let f = |path, is_dir| IsolatedFilePathData::new(0, root_path, path, is_dir).unwrap();
		let pub_id = Uuid::new_v4();

		#[rustfmt::skip]
		let expected = [
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("rust_project"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("rust_project/.git"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("rust_project/Cargo.toml"), false), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("rust_project/src"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("rust_project/src/main.rs"), false), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("rust_project/target"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("rust_project/target/debug"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("rust_project/target/debug/main"), false), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("inner"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("inner/node_project"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("inner/node_project/.git"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("inner/node_project/package.json"), false), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("inner/node_project/src"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("inner/node_project/src/App.tsx"), false), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("inner/node_project/node_modules"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("inner/node_project/node_modules/react"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("inner/node_project/node_modules/react/package.json"), false), metadata },
		]
		.into_iter()
		.collect::<HashSet<_>>();

		let git_repos = &[IndexerRule::new(
			"git repos".to_string(),
			false,
			vec![RulePerKind::AcceptIfChildrenDirectoriesArePresent(
				[".git".to_string()].into_iter().collect(),
			)],
		)];

		let walk_result = walk(
			root_path.to_path_buf(),
			git_repos,
			|_, _| {},
			|_| async { Ok(vec![]) },
			|_, _| async { Ok(vec![]) },
			|path, is_dir| {
				IsolatedFilePathData::new(0, root_path, path, is_dir).map_err(Into::into)
			},
			420,
		)
		.await
		.unwrap();

		if !walk_result.errors.is_empty() {
			panic!("errors: {:#?}", walk_result.errors);
		}

		let actual = walk_result.walked.collect::<HashSet<_>>();

		if actual != expected {
			panic!("difference: {:#?}", expected.difference(&actual));
		}
	}

	#[tokio::test]
	// #[traced_test]
	async fn git_repos_without_deps_or_build_dirs() {
		let root = prepare_location().await;
		let root_path = root.path();

		let metadata = FilePathMetadata {
			inode: 0,
			device: 0,
			size_in_bytes: 0,
			created_at: Utc::now(),
			modified_at: Utc::now(),
		};

		let f = |path, is_dir| IsolatedFilePathData::new(0, root_path, path, is_dir).unwrap();
		let pub_id = Uuid::new_v4();

		#[rustfmt::skip]
		let expected = [
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("rust_project"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("rust_project/.git"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("rust_project/Cargo.toml"), false), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("rust_project/src"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("rust_project/src/main.rs"), false), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("inner"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("inner/node_project"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("inner/node_project/.git"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("inner/node_project/package.json"), false), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("inner/node_project/src"), true), metadata },
			WalkedEntry { pub_id, iso_file_path: f(root_path.join("inner/node_project/src/App.tsx"), false), metadata },
		]
		.into_iter()
		.collect::<HashSet<_>>();

		let git_repos_no_deps_no_build_dirs = &[
			IndexerRule::new(
				"git repos".to_string(),
				false,
				vec![RulePerKind::AcceptIfChildrenDirectoriesArePresent(
					[".git".to_string()].into_iter().collect(),
				)],
			),
			IndexerRule::new(
				"reject node_modules".to_string(),
				false,
				vec![RulePerKind::RejectFilesByGlob(
					vec![],
					GlobSetBuilder::new()
						.add(Glob::new("{**/node_modules/*,**/node_modules}").unwrap())
						.build()
						.unwrap(),
				)],
			),
			IndexerRule::new(
				"reject rust build dir".to_string(),
				false,
				vec![RulePerKind::RejectFilesByGlob(
					vec![],
					GlobSetBuilder::new()
						.add(Glob::new("{**/target/*,**/target}").unwrap())
						.build()
						.unwrap(),
				)],
			),
		];

		let walk_result = walk(
			root_path.to_path_buf(),
			git_repos_no_deps_no_build_dirs,
			|_, _| {},
			|_| async { Ok(vec![]) },
			|_, _| async { Ok(vec![]) },
			|path, is_dir| {
				IsolatedFilePathData::new(0, root_path, path, is_dir).map_err(Into::into)
			},
			420,
		)
		.await
		.unwrap();

		if !walk_result.errors.is_empty() {
			panic!("errors: {:#?}", walk_result.errors);
		}

		let actual = walk_result.walked.collect::<HashSet<_>>();

		if actual != expected {
			panic!("difference: {:#?}", expected.difference(&actual));
		}
	}
}
