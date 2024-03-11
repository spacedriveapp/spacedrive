use crate::Error;

use sd_core_file_path_helper::{FilePathMetadata, IsolatedFilePathData};
use sd_core_indexer_rules::{IndexerRuler, RuleKind};
use sd_core_prisma_helpers::{file_path_pub_and_cas_ids, file_path_walker};

use sd_prisma::prisma::file_path;
use sd_task_system::{
	check_interruption, ExecStatus, Interrupter, IntoAnyTaskOutput, Task, TaskDispatcher,
	TaskHandle, TaskId,
};
use sd_utils::{db::inode_from_db, error::FileIOError};

use std::{
	collections::{HashMap, HashSet},
	fmt,
	fs::Metadata,
	future::Future,
	mem,
	path::{Path, PathBuf},
	sync::Arc,
};

use chrono::{DateTime, Duration, FixedOffset};
use futures_concurrency::future::Join;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio_stream::{wrappers::ReadDirStream, StreamExt};
use tracing::trace;
use uuid::Uuid;

use super::IndexerError;

/// `WalkedEntry` represents a single path in the filesystem
#[derive(Debug, Serialize, Deserialize)]
pub struct WalkedEntry {
	pub pub_id: Uuid,
	pub maybe_object_id: file_path::object_id::Type,
	pub iso_file_path: IsolatedFilePathData<'static>,
	pub metadata: FilePathMetadata,
}

#[derive(Debug)]
struct WalkingEntry {
	iso_file_path: IsolatedFilePathData<'static>,
	metadata: FilePathMetadata,
}

impl From<WalkingEntry> for WalkedEntry {
	fn from(walking_entry: WalkingEntry) -> Self {
		let WalkingEntry {
			iso_file_path,
			metadata,
		} = walking_entry;

		Self {
			pub_id: Uuid::new_v4(),
			maybe_object_id: None,
			iso_file_path,
			metadata,
		}
	}
}

impl From<(Uuid, file_path::object_id::Type, WalkingEntry)> for WalkedEntry {
	fn from(
		(pub_id, maybe_object_id, walking_entry): (Uuid, file_path::object_id::Type, WalkingEntry),
	) -> Self {
		let WalkingEntry {
			iso_file_path,
			metadata,
		} = walking_entry;

		Self {
			pub_id,
			maybe_object_id,
			iso_file_path,
			metadata,
		}
	}
}

pub enum IndexerRulerAcceptKind {
	Accept,
	Reject,
	AcceptAncestors,
}

pub trait IsoFilePathFactory: Send + Sync + fmt::Debug + 'static {
	fn build(
		&self,
		path: impl AsRef<Path>,
		is_dir: bool,
	) -> Result<IsolatedFilePathData<'static>, IndexerError>;
}

pub trait WalkerDBProxy: Send + Sync + fmt::Debug + 'static {
	fn fetch_file_paths(
		&self,
		params: Vec<file_path::WhereParam>,
	) -> impl Future<Output = Result<Vec<file_path_walker::Data>, IndexerError>> + Send;

	fn fetch_file_paths_to_remove(
		&self,
		iso_file_path: &IsolatedFilePathData<'_>,
		params: Vec<file_path::WhereParam>,
	) -> impl Future<Output = Result<Vec<file_path_pub_and_cas_ids::Data>, IndexerError>> + Send;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToWalkEntry {
	path: PathBuf,
	parent_dir_accepted_by_its_children: Option<bool>,
	maybe_parent: Option<PathBuf>,
}

impl From<PathBuf> for ToWalkEntry {
	fn from(path: PathBuf) -> Self {
		Self {
			path,
			parent_dir_accepted_by_its_children: None,
			maybe_parent: None,
		}
	}
}

#[derive(Debug)]
struct WalkDirTask<DBProxy, IsoPathFactory>
where
	DBProxy: WalkerDBProxy,
	IsoPathFactory: IsoFilePathFactory,
{
	id: TaskId,
	entry: ToWalkEntry,
	root: Arc<PathBuf>,
	entry_iso_file_path: IsolatedFilePathData<'static>,
	indexer_ruler: IndexerRuler,
	iso_file_path_factory: Arc<IsoPathFactory>,
	db_proxy: Arc<DBProxy>,
	read_dir_stream: ReadDirStream,
	found_paths: Vec<PathBuf>,
	paths_and_metadatas: HashMap<PathBuf, Metadata>,
	paths_metadatas_and_acceptance: HashMap<PathBuf, (Metadata, HashMap<RuleKind, Vec<bool>>)>,
	accepted_paths: HashMap<PathBuf, Metadata>,
	accepted_ancestors: HashSet<PathBuf>,
	walking_entries: Vec<WalkingEntry>,
	to_remove_entries: Vec<file_path_pub_and_cas_ids::Data>,
	maybe_to_keep_walking: Option<Vec<ToWalkEntry>>,
	maybe_dispatcher: Option<TaskDispatcher<Error>>,
	errors: Vec<IndexerError>,
}

impl<DBProxy, IsoPathFactory> WalkDirTask<DBProxy, IsoPathFactory>
where
	DBProxy: WalkerDBProxy,
	IsoPathFactory: IsoFilePathFactory,
{
	pub async fn new(
		entry: impl Into<ToWalkEntry> + Send,
		root: Arc<PathBuf>,
		indexer_ruler: IndexerRuler,
		iso_file_path_factory: Arc<IsoPathFactory>,
		db_proxy: Arc<DBProxy>,
		maybe_dispatcher: Option<TaskDispatcher<Error>>,
	) -> Result<Self, IndexerError> {
		let entry = entry.into();
		Ok(Self {
			id: TaskId::new_v4(),
			root,
			indexer_ruler,
			entry_iso_file_path: iso_file_path_factory.build(&entry.path, true)?,
			iso_file_path_factory,
			db_proxy,
			read_dir_stream: ReadDirStream::new(
				fs::read_dir(&entry.path)
					.await
					.map_err(|e| FileIOError::from((&entry.path, e)))?,
			),
			entry,
			found_paths: Vec::new(),
			paths_and_metadatas: HashMap::new(),
			paths_metadatas_and_acceptance: HashMap::new(),
			accepted_paths: HashMap::new(),
			accepted_ancestors: HashSet::new(),
			walking_entries: Vec::new(),
			to_remove_entries: Vec::new(),
			maybe_to_keep_walking: maybe_dispatcher.is_some().then(Vec::new),
			maybe_dispatcher,
			errors: Vec::new(),
		})
	}
}

#[async_trait::async_trait]
impl<DBProxy, IsoPathFactory> Task<Error> for WalkDirTask<DBProxy, IsoPathFactory>
where
	DBProxy: WalkerDBProxy,
	IsoPathFactory: IsoFilePathFactory,
{
	fn id(&self) -> TaskId {
		self.id
	}

	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, Error> {
		let Self {
			root,
			entry:
				ToWalkEntry {
					path,
					parent_dir_accepted_by_its_children,
					maybe_parent,
				},
			entry_iso_file_path,
			iso_file_path_factory,
			indexer_ruler,
			db_proxy,
			found_paths,
			paths_and_metadatas,
			paths_metadatas_and_acceptance,
			accepted_paths,
			accepted_ancestors,
			walking_entries,
			to_remove_entries,
			maybe_to_keep_walking,
			maybe_dispatcher,
			errors,
			..
		} = self;

		while let Some(res) = self.read_dir_stream.next().await {
			match res {
				Ok(dir_entry) => {
					found_paths.push(dir_entry.path());
				}
				Err(e) => {
					errors.push(IndexerError::FileIO(FileIOError::from((&path, e))));
				}
			}

			check_interruption!(interrupter);
		}

		check_interruption!(interrupter);

		collect_metadata(found_paths, paths_and_metadatas, errors).await;

		check_interruption!(interrupter);

		apply_indexer_rules(
			paths_and_metadatas,
			indexer_ruler,
			paths_metadatas_and_acceptance,
			errors,
		)
		.await;

		check_interruption!(interrupter);

		process_rules_results(
			&path,
			root,
			*parent_dir_accepted_by_its_children,
			paths_metadatas_and_acceptance,
			accepted_paths,
			accepted_ancestors,
			maybe_to_keep_walking,
		);

		check_interruption!(interrupter);

		gather_file_paths_to_remove(
			accepted_paths,
			walking_entries,
			to_remove_entries,
			entry_iso_file_path,
			iso_file_path_factory.as_ref(),
			db_proxy.as_ref(),
			errors,
		)
		.await;

		check_interruption!(interrupter);

		// From this points onwards, we will not allow to be interrupted anymore

		let (to_create, to_update, total_size) =
			segregate_creates_and_updates(walking_entries, db_proxy.as_ref()).await?;

		let handles = keep_walking(
			root,
			indexer_ruler,
			iso_file_path_factory,
			db_proxy,
			maybe_to_keep_walking,
			maybe_dispatcher,
			errors,
		)
		.await;

		// Taking out some data as the task is finally complete
		Ok(ExecStatus::Done(
			WalkOutput {
				to_create,
				to_update,
				to_remove: mem::take(to_remove_entries),
				accepted_ancestors: mem::take(accepted_ancestors),
				errors: mem::take(errors),
				directory: mem::take(path),
				total_size,
				maybe_parent: mem::take(maybe_parent),
				handles,
			}
			.into_output(),
		))
	}
}

async fn segregate_creates_and_updates(
	walking_entries: &mut Vec<WalkingEntry>,
	db_proxy: &impl WalkerDBProxy,
) -> Result<(Vec<WalkedEntry>, Vec<WalkedEntry>, u64), IndexerError> {
	if walking_entries.is_empty() {
		Ok((vec![], vec![], 0))
	} else {
		let iso_paths_already_in_db = db_proxy
			.fetch_file_paths(
				walking_entries
					.iter()
					.map(|entry| file_path::WhereParam::from(&entry.iso_file_path))
					.collect(),
			)
			.await?
			.into_iter()
			.flat_map(|file_path| {
				IsolatedFilePathData::try_from(file_path.clone())
					.map(|iso_file_path| (iso_file_path, file_path))
			})
			.collect::<HashMap<_, _>>();

		Ok(walking_entries.drain(..).fold(
				(Vec::new(), Vec::new(), 0),
				|(mut to_create, mut to_update, mut total_size), entry| {
					let WalkingEntry{iso_file_path, metadata} = &entry;

					total_size += metadata.size_in_bytes;

					if let Some(file_path) = iso_paths_already_in_db.get(iso_file_path) {
						if let (Some(inode), Some(date_modified)) = (
						&file_path.inode,
						&file_path.date_modified,
					) {
						if (
								inode_from_db(&inode[0..8]) != entry.metadata.inode
								// Datetimes stored in DB loses a bit of precision, so we need to check against a delta
								// instead of using != operator
								|| DateTime::<FixedOffset>::from(entry.metadata.modified_at) - *date_modified
									> Duration::milliseconds(1) || file_path.hidden.is_none() || metadata.hidden != file_path.hidden.unwrap_or_default()
							)
							// We ignore the size of directories because it is not reliable, we need to
							// calculate it ourselves later
							&& !(
								entry.iso_file_path.to_parts().is_dir
								&& metadata.size_in_bytes
									!= file_path
										.size_in_bytes_bytes
										.as_ref()
										.map(|size_in_bytes_bytes| {
											u64::from_be_bytes([
												size_in_bytes_bytes[0],
												size_in_bytes_bytes[1],
												size_in_bytes_bytes[2],
												size_in_bytes_bytes[3],
												size_in_bytes_bytes[4],
												size_in_bytes_bytes[5],
												size_in_bytes_bytes[6],
												size_in_bytes_bytes[7],
											])
										})
										.unwrap_or_default()
								) {
							to_update.push(
								WalkedEntry::from((sd_utils::from_bytes_to_uuid(&file_path.pub_id), file_path.object_id, entry)),
							);
						}
					}
					} else {
						to_create.push(WalkedEntry::from(entry));
					}

					(to_create, to_update, total_size)
				}
			))
	}
}

async fn keep_walking(
	root: &Arc<PathBuf>,
	indexer_ruler: &IndexerRuler,
	iso_file_path_factory: &Arc<impl IsoFilePathFactory>,
	db_proxy: &Arc<impl WalkerDBProxy>,
	maybe_to_keep_walking: &mut Option<Vec<ToWalkEntry>>,
	dispatcher: &Option<TaskDispatcher<Error>>,
	errors: &mut Vec<IndexerError>,
) -> Vec<TaskHandle<Error>> {
	if let (Some(dispatcher), Some(to_keep_walking)) = (dispatcher, maybe_to_keep_walking) {
		dispatcher
			.dispatch_many(
				to_keep_walking
					.drain(..)
					.map(|entry| async {
						WalkDirTask::new(
							entry,
							Arc::clone(root),
							indexer_ruler.clone(),
							Arc::clone(iso_file_path_factory),
							Arc::clone(db_proxy),
							Some(dispatcher.clone()),
						)
						.await
					})
					.collect::<Vec<_>>()
					.join()
					.await
					.into_iter()
					.filter_map(|res| res.map_err(|e| errors.push(e)).ok()),
			)
			.await
	} else {
		Vec::new()
	}
}

#[derive(Debug)]
pub(crate) struct WalkOutput {
	to_create: Vec<WalkedEntry>,
	to_update: Vec<WalkedEntry>,
	to_remove: Vec<file_path_pub_and_cas_ids::Data>,
	accepted_ancestors: HashSet<PathBuf>,
	errors: Vec<IndexerError>,
	directory: PathBuf,
	total_size: u64,
	maybe_parent: Option<PathBuf>,
	handles: Vec<TaskHandle<Error>>,
}

async fn collect_metadata(
	found_paths: &mut Vec<PathBuf>,
	paths_and_metadatas: &mut HashMap<PathBuf, Metadata>,
	errors: &mut Vec<IndexerError>,
) {
	if !found_paths.is_empty() && paths_and_metadatas.is_empty() {
		*paths_and_metadatas = found_paths
			.drain(..)
			.map(|current_path| async move {
				fs::metadata(&current_path)
					.await
					.map_err(|e| FileIOError::from((&current_path, e)))
					.map(|metadata| (current_path, metadata))
			})
			.collect::<Vec<_>>()
			.join()
			.await
			.into_iter()
			.filter_map(|res| res.map_err(|e| errors.push(e.into())).ok())
			.collect::<HashMap<_, _>>();
	}
}

async fn apply_indexer_rules(
	paths_and_metadatas: &mut HashMap<PathBuf, Metadata>,
	indexer_ruler: &IndexerRuler,
	paths_metadatas_and_acceptance: &mut HashMap<PathBuf, (Metadata, HashMap<RuleKind, Vec<bool>>)>,
	errors: &mut Vec<IndexerError>,
) {
	if !paths_and_metadatas.is_empty() && paths_metadatas_and_acceptance.is_empty() {
		*paths_metadatas_and_acceptance = paths_and_metadatas
			.drain()
			// TODO: Hard ignoring symlinks for now, but this should be configurable
			.filter(|(_, metadata)| !metadata.is_symlink())
			.map(|(current_path, metadata)| async {
				indexer_ruler.apply_all(&current_path, &metadata).await.map(
					|acceptance_per_rule_kind| (current_path, (metadata, acceptance_per_rule_kind)),
				)
			})
			.collect::<Vec<_>>()
			.join()
			.await
			.into_iter()
			.filter_map(|res| res.map_err(|e| errors.push(e.into())).ok())
			.collect();
	}
}

fn process_rules_results(
	source_directory: impl AsRef<Path>,
	root: &Arc<PathBuf>,
	parent_dir_accepted_by_its_children: Option<bool>,
	paths_metadatas_and_acceptance: &mut HashMap<PathBuf, (Metadata, HashMap<RuleKind, Vec<bool>>)>,
	accepted_paths: &mut HashMap<PathBuf, Metadata>,
	accepted_ancestors: &mut HashSet<PathBuf>,
	maybe_to_keep_walking: &mut Option<Vec<ToWalkEntry>>,
) {
	let source_directory = source_directory.as_ref();
	let root = root.as_ref();

	if !paths_metadatas_and_acceptance.is_empty()
		&& accepted_paths.is_empty()
		&& accepted_ancestors.is_empty()
	{
		(*accepted_paths, *accepted_ancestors) = paths_metadatas_and_acceptance.drain().fold(
			(HashMap::new(), HashSet::new()),
			|(mut accepted, mut accepted_ancestors),
			 (current_path, (metadata, acceptance_per_rule_kind))| {
				// Accept by children has three states,
				// None if we don't now yet or if this check doesn't apply
				// Some(true) if this check applies and it passes
				// Some(false) if this check applies and it was rejected
				// and we pass the current parent state to its children
				let mut accept_by_children_dir = parent_dir_accepted_by_its_children;

				if rejected_by_reject_glob(&acceptance_per_rule_kind) {
					trace!(
						"Path {} rejected by `RuleKind::RejectFilesByGlob`",
						current_path.display()
					);

					return (accepted, accepted_ancestors);
				}

				let is_dir = metadata.is_dir();

				if is_dir
					&& process_and_maybe_reject_by_directory_rules(
						&current_path,
						source_directory,
						&acceptance_per_rule_kind,
						&mut accept_by_children_dir,
						maybe_to_keep_walking,
					) {
					trace!(
						"Path {} rejected by rule `RuleKind::RejectIfChildrenDirectoriesArePresent`",
						current_path.display(),
					);
					return (accepted, accepted_ancestors);
				}

				if rejected_by_accept_glob(&acceptance_per_rule_kind) {
					trace!(
						"Path {} reject because it didn't passed in any AcceptFilesByGlob rules",
						current_path.display()
					);
					return (accepted, accepted_ancestors);
				}

				if accept_by_children_dir.unwrap_or(true) {
					accept_ancestors(
						current_path,
						metadata,
						root,
						&mut accepted,
						&mut accepted_ancestors,
					);
				}

				(accepted, accepted_ancestors)
			},
		);
	}
}

fn process_and_maybe_reject_by_directory_rules(
	current_path: &Path,
	parent: &Path,
	acceptance_per_rule_kind: &HashMap<RuleKind, Vec<bool>>,
	accept_by_children_dir: &mut Option<bool>,
	maybe_to_keep_walking: &mut Option<Vec<ToWalkEntry>>,
) -> bool {
	// If it is a directory, first we check if we must reject it and its children entirely
	if rejected_by_children_directories(acceptance_per_rule_kind) {
		return true;
	}

	// Then we check if we must accept it and its children
	if let Some(accepted_by_children_rules) =
		acceptance_per_rule_kind.get(&RuleKind::AcceptIfChildrenDirectoriesArePresent)
	{
		if accepted_by_children_rules.iter().any(|accept| *accept) {
			*accept_by_children_dir = Some(true);
		}

		// If it wasn't accepted then we mark as rejected
		if accept_by_children_dir.is_none() {
			trace!(
								"Path {} rejected because it didn't passed in any AcceptIfChildrenDirectoriesArePresent rule",
								current_path.display()
							);
			*accept_by_children_dir = Some(false);
		}
	}

	// Then we mark this directory to maybe be walked in too
	if let Some(ref mut to_keep_walking) = maybe_to_keep_walking {
		to_keep_walking.push(ToWalkEntry {
			path: current_path.to_path_buf(),
			parent_dir_accepted_by_its_children: *accept_by_children_dir,
			maybe_parent: Some(parent.to_path_buf()),
		});
	}

	false
}

fn accept_ancestors(
	current_path: PathBuf,
	metadata: Metadata,
	root: &Path,
	accepted: &mut HashMap<PathBuf, Metadata>,
	accepted_ancestors: &mut HashSet<PathBuf>,
) {
	// If the ancestors directories wasn't indexed before, now we do
	for ancestor in current_path
		.ancestors()
		.skip(1) // Skip the current directory as it was already indexed
		.take_while(|&ancestor| ancestor != root)
	{
		if accepted_ancestors.insert(ancestor.to_path_buf()) {
			trace!("Accepted ancestor {}", ancestor.display());
		} else {
			// If we already accepted this ancestor, then it will contain
			// also all if its ancestors too, so we can stop here
			break;
		}
	}

	accepted.insert(current_path, metadata);
}

fn rejected_by_accept_glob(acceptance_per_rule_kind: &HashMap<RuleKind, Vec<bool>>) -> bool {
	acceptance_per_rule_kind
		.get(&RuleKind::AcceptFilesByGlob)
		.map_or(false, |accept_rules| {
			accept_rules.iter().all(|accept| !accept)
		})
}

fn rejected_by_children_directories(
	acceptance_per_rule_kind: &HashMap<RuleKind, Vec<bool>>,
) -> bool {
	acceptance_per_rule_kind
		.get(&RuleKind::RejectIfChildrenDirectoriesArePresent)
		.map_or(false, |reject_results| {
			reject_results.iter().any(|reject| !reject)
		})
}

fn rejected_by_reject_glob(acceptance_per_rule_kind: &HashMap<RuleKind, Vec<bool>>) -> bool {
	acceptance_per_rule_kind
		.get(&RuleKind::RejectFilesByGlob)
		.map_or(false, |reject_results| {
			reject_results.iter().any(|reject| !reject)
		})
}

async fn gather_file_paths_to_remove(
	accepted_paths: &mut HashMap<PathBuf, Metadata>,
	walking_entries: &mut Vec<WalkingEntry>,
	to_remove_entries: &mut Vec<file_path_pub_and_cas_ids::Data>,
	entry_iso_file_path: &IsolatedFilePathData<'_>,
	iso_file_path_factory: &impl IsoFilePathFactory,
	db_proxy: &impl WalkerDBProxy,
	errors: &mut Vec<IndexerError>,
) {
	if !accepted_paths.is_empty() && walking_entries.is_empty() && to_remove_entries.is_empty() {
		let (walking, to_delete_params) = accepted_paths
			.drain()
			.filter_map(|(path, metadata)| {
				iso_file_path_factory
					.build(&path, metadata.is_dir())
					.and_then(|iso_file_path| {
						FilePathMetadata::from_path(path, &metadata)
							.map(|metadata| {
								let params = file_path::WhereParam::from(&iso_file_path);

								(
									WalkingEntry {
										iso_file_path,
										metadata,
									},
									params,
								)
							})
							.map_err(Into::into)
					})
					.map_err(|e| errors.push(e))
					.ok()
			})
			.unzip::<_, _, Vec<_>, Vec<_>>();

		*walking_entries = walking;

		// We continue the function even if we fail to fetch `file_path`s to remove,
		// the DB will have old `file_path`s but at least this is better than
		// don't adding the newly indexed paths
		if let Ok(entries) = db_proxy
			.fetch_file_paths_to_remove(entry_iso_file_path, to_delete_params)
			.await
			.map_err(|e| errors.push(e))
		{
			*to_remove_entries = entries;
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	use sd_core_indexer_rules::{IndexerRule, RulePerKind};
	use sd_task_system::{TaskOutput, TaskStatus, TaskSystem};

	use std::hash::{Hash, Hasher};

	use chrono::Utc;
	use futures_concurrency::future::FutureGroup;
	use globset::{Glob, GlobSetBuilder};
	use lending_stream::{LendingStream, StreamExt};
	use tempfile::{tempdir, TempDir};
	use tokio::fs;
	use tracing::debug;
	use tracing_test::traced_test;

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

	#[derive(Debug)]
	struct DummyIsoPathFactory {
		root_path: PathBuf,
	}

	impl IsoFilePathFactory for DummyIsoPathFactory {
		fn build(
			&self,
			path: impl AsRef<Path>,
			is_dir: bool,
		) -> Result<IsolatedFilePathData<'static>, IndexerError> {
			IsolatedFilePathData::new(0, &self.root_path, path, is_dir).map_err(Into::into)
		}
	}

	#[derive(Debug)]
	struct DummyDBProxy;

	impl WalkerDBProxy for DummyDBProxy {
		async fn fetch_file_paths(
			&self,
			_: Vec<file_path::WhereParam>,
		) -> Result<Vec<file_path_walker::Data>, IndexerError> {
			Ok(vec![])
		}

		async fn fetch_file_paths_to_remove(
			&self,
			_: &IsolatedFilePathData<'_>,
			_: Vec<file_path::WhereParam>,
		) -> Result<Vec<file_path_pub_and_cas_ids::Data>, IndexerError> {
			Ok(vec![])
		}
	}

	fn new_indexer_rule(
		name: impl Into<String>,
		default: bool,
		rules: Vec<RulePerKind>,
	) -> IndexerRule {
		IndexerRule {
			id: None,
			name: name.into(),
			default,
			rules,
			date_created: Utc::now(),
			date_modified: Utc::now(),
		}
	}

	async fn prepare_location() -> TempDir {
		// root
		// |__ rust_project
		// |   |__ .git
		// |        |__ <empty>
		// |   |__ Cargo.toml
		// |   |__ src
		// |   |   |__ main.rs
		// |   |__ target
		// |       |__ debug
		// |           |__ main
		// |__ inner
		// |   |__ node_project
		// |       |__ .git
		// |            |__ <empty>
		// |       |__ package.json
		// |       |__ src
		// |       |   |__ App.tsx
		// |       |__ node_modules
		// |           |__ react
		// |               |__ package.json
		// |__ photos
		//     |__ photo1.png
		//     |__ photo2.jpg
		//     |__ photo3.jpeg
		//     |__ text.txt

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
		for photo in ["photo1.png", "photo2.jpg", "photo3.jpeg", "text.txt"] {
			fs::File::create(photos.join(photo)).await.unwrap();
		}

		root
	}

	async fn run_test(
		root_path: &Path,
		indexer_ruler: IndexerRuler,
		expected: HashSet<WalkedEntry>,
	) {
		let system = TaskSystem::new();

		let handle = system
			.dispatch(
				WalkDirTask::new(
					root_path.to_path_buf(),
					Arc::new(root_path.to_path_buf()),
					indexer_ruler,
					Arc::new(DummyIsoPathFactory {
						root_path: root_path.to_path_buf(),
					}),
					Arc::new(DummyDBProxy),
					Some(system.get_dispatcher()),
				)
				.await
				.unwrap(),
			)
			.await;

		let mut group = FutureGroup::new();

		group.insert(handle);

		let mut group = group.lend_mut();

		let mut actual = HashSet::new();

		let mut ancestors = HashSet::new();

		while let Some((group, task_result)) = group.next().await {
			let TaskStatus::Done(TaskOutput::Out(output)) = task_result.unwrap() else {
				panic!("unexpected task output")
			};

			let walk_result = output.downcast::<WalkOutput>().unwrap();

			debug!("{walk_result:#?}");

			assert!(
				walk_result.errors.is_empty(),
				"errors: {:#?}",
				walk_result.errors
			);

			actual.extend(walk_result.to_create);
			ancestors.extend(walk_result.accepted_ancestors);

			for handle in walk_result.handles {
				group.insert(handle);
			}
		}

		for WalkedEntry { iso_file_path, .. } in &actual {
			ancestors.remove(&root_path.join(iso_file_path));
		}

		if !ancestors.is_empty() {
			debug!("Adding ancestors to actual: {:#?}", ancestors);
			actual.extend(ancestors.into_iter().map(|path| WalkedEntry {
				pub_id: Uuid::new_v4(),
				maybe_object_id: None,
				iso_file_path: IsolatedFilePathData::new(0, root_path, path, true).unwrap(),
				metadata: FilePathMetadata {
					inode: 0,
					size_in_bytes: 0,
					created_at: Utc::now(),
					modified_at: Utc::now(),
					hidden: false,
				},
			}));
		}

		assert_eq!(
			actual,
			expected,
			"Expected \\ Actual: {:#?};\n Actual \\ Expected: {:#?}",
			expected.difference(&actual),
			actual.difference(&expected)
		);
	}

	#[tokio::test]
	#[traced_test]
	async fn test_walk_without_rules() {
		let root = prepare_location().await;
		let root_path = root.path();

		let metadata = FilePathMetadata {
			inode: 0,
			size_in_bytes: 0,
			created_at: Utc::now(),
			modified_at: Utc::now(),
			hidden: false,
		};

		let f = |path, is_dir| IsolatedFilePathData::new(0, root_path, path, is_dir).unwrap();
		let pub_id = Uuid::new_v4();
		let maybe_object_id = None;

		#[rustfmt::skip]
		let expected = [
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("rust_project"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("rust_project/.git"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("rust_project/Cargo.toml"), false), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("rust_project/src"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("rust_project/src/main.rs"), false), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("rust_project/target"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("rust_project/target/debug"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("rust_project/target/debug/main"), false), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("inner"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("inner/node_project"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/.git"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/package.json"), false), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/src"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/src/App.tsx"), false), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/node_modules"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/node_modules/react"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/node_modules/react/package.json"), false), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("photos"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("photos/photo1.png"), false), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("photos/photo2.jpg"), false), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("photos/photo3.jpeg"), false), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("photos/text.txt"), false), metadata },
		]
		.into_iter()
		.collect::<HashSet<_>>();

		run_test(root_path, IndexerRuler::default(), expected).await;
	}

	#[tokio::test]
	#[traced_test]
	async fn test_only_photos() {
		let root = prepare_location().await;
		let root_path = root.path();

		let metadata = FilePathMetadata {
			inode: 0,
			size_in_bytes: 0,
			created_at: Utc::now(),
			modified_at: Utc::now(),
			hidden: false,
		};

		let f = |path, is_dir| IsolatedFilePathData::new(0, root_path, path, is_dir).unwrap();
		let pub_id = Uuid::new_v4();
		let maybe_object_id = None;

		#[rustfmt::skip]
		let expected = [
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("photos"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("photos/photo1.png"), false), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("photos/photo2.jpg"), false), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("photos/photo3.jpeg"), false), metadata },
		]
		.into_iter()
		.collect::<HashSet<_>>();

		run_test(
			root_path,
			IndexerRuler::new(vec![new_indexer_rule(
				"only photos",
				false,
				vec![RulePerKind::AcceptFilesByGlob(
					vec![],
					GlobSetBuilder::new()
						.add(Glob::new("{*.png,*.jpg,*.jpeg}").unwrap())
						.build()
						.unwrap(),
				)],
			)]),
			expected,
		)
		.await;
	}

	#[tokio::test]
	#[traced_test]
	async fn test_git_repos() {
		let root = prepare_location().await;
		let root_path = root.path();

		let metadata = FilePathMetadata {
			inode: 0,
			size_in_bytes: 0,
			created_at: Utc::now(),
			modified_at: Utc::now(),
			hidden: false,
		};

		let f = |path, is_dir| IsolatedFilePathData::new(0, root_path, path, is_dir).unwrap();
		let pub_id = Uuid::new_v4();
		let maybe_object_id = None;

		#[rustfmt::skip]
		let expected = [
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("rust_project"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("rust_project/.git"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("rust_project/Cargo.toml"), false), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("rust_project/src"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("rust_project/src/main.rs"), false), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("rust_project/target"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("rust_project/target/debug"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("rust_project/target/debug/main"), false), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("inner"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("inner/node_project"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/.git"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/package.json"), false), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/src"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/src/App.tsx"), false), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/node_modules"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/node_modules/react"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/node_modules/react/package.json"), false), metadata },
		]
		.into_iter()
		.collect::<HashSet<_>>();

		run_test(
			root_path,
			IndexerRuler::new(vec![new_indexer_rule(
				"git repos",
				false,
				vec![RulePerKind::AcceptIfChildrenDirectoriesArePresent(
					HashSet::from([".git".to_string()]),
				)],
			)]),
			expected,
		)
		.await;
	}

	#[tokio::test]
	#[traced_test]
	async fn git_repos_without_deps_or_build_dirs() {
		let root = prepare_location().await;
		let root_path = root.path();

		let metadata = FilePathMetadata {
			inode: 0,
			size_in_bytes: 0,
			created_at: Utc::now(),
			modified_at: Utc::now(),
			hidden: false,
		};

		let f = |path, is_dir| IsolatedFilePathData::new(0, root_path, path, is_dir).unwrap();
		let pub_id = Uuid::new_v4();
		let maybe_object_id = None;

		#[rustfmt::skip]
		let expected = [
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("rust_project"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("rust_project/.git"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("rust_project/Cargo.toml"), false), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("rust_project/src"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("rust_project/src/main.rs"), false), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("inner"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("inner/node_project"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/.git"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/package.json"), false), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/src"), true), metadata },
			WalkedEntry { pub_id, maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/src/App.tsx"), false), metadata },
		]
		.into_iter()
		.collect::<HashSet<_>>();

		run_test(
			root_path,
			IndexerRuler::new(vec![
				new_indexer_rule(
					"git repos",
					false,
					vec![RulePerKind::AcceptIfChildrenDirectoriesArePresent(
						HashSet::from([".git".into()]),
					)],
				),
				new_indexer_rule(
					"reject node_modules",
					false,
					vec![RulePerKind::RejectFilesByGlob(
						vec![],
						GlobSetBuilder::new()
							.add(Glob::new("{**/node_modules/*,**/node_modules}").unwrap())
							.build()
							.unwrap(),
					)],
				),
				new_indexer_rule(
					"reject rust build dir",
					false,
					vec![RulePerKind::RejectFilesByGlob(
						vec![],
						GlobSetBuilder::new()
							.add(Glob::new("{**/target/*,**/target}").unwrap())
							.build()
							.unwrap(),
					)],
				),
			]),
			expected,
		)
		.await;
	}
}
