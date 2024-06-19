use crate::{
	indexer::{
		self,
		tasks::walker::rules::{apply_indexer_rules, process_rules_results},
	},
	Error, NonCriticalError,
};

use sd_core_file_path_helper::{FilePathError, FilePathMetadata, IsolatedFilePathData};
use sd_core_indexer_rules::{
	seed::{GitIgnoreRules, GITIGNORE},
	IndexerRuler, MetadataForIndexerRules, RuleKind,
};
use sd_core_prisma_helpers::{file_path_pub_and_cas_ids, file_path_walker};

use sd_prisma::prisma::file_path;
use sd_task_system::{
	check_interruption, ExecStatus, Interrupter, IntoAnyTaskOutput, Task, TaskId,
};
use sd_utils::{
	db::{inode_from_db, inode_to_db},
	error::FileIOError,
};

use std::{
	collections::{HashMap, HashSet},
	fmt,
	future::Future,
	mem,
	path::{Path, PathBuf},
	sync::Arc,
	time::Duration,
};

use chrono::{DateTime, Duration as ChronoDuration, FixedOffset};
use futures_concurrency::future::Join;
use tokio::{fs, time::Instant};
use tokio_stream::{wrappers::ReadDirStream, StreamExt};
use tracing::{instrument, trace, Level};

mod entry;
mod metadata;
mod rules;
mod save_state;

pub use entry::{ToWalkEntry, WalkedEntry};

use entry::WalkingEntry;
use metadata::InnerMetadata;

pub trait IsoFilePathFactory: Clone + Send + Sync + fmt::Debug + 'static {
	fn build(
		&self,
		path: impl AsRef<Path>,
		is_dir: bool,
	) -> Result<IsolatedFilePathData<'static>, FilePathError>;
}

pub trait WalkerDBProxy: Clone + Send + Sync + fmt::Debug + 'static {
	fn fetch_file_paths(
		&self,
		found_paths: Vec<file_path::WhereParam>,
	) -> impl Future<Output = Result<Vec<file_path_walker::Data>, indexer::Error>> + Send;

	fn fetch_file_paths_to_remove(
		&self,
		parent_iso_file_path: &IsolatedFilePathData<'_>,
		existing_inodes: HashSet<Vec<u8>>,
		unique_location_id_materialized_path_name_extension_params: Vec<file_path::WhereParam>,
	) -> impl Future<
		Output = Result<Vec<file_path_pub_and_cas_ids::Data>, indexer::NonCriticalIndexerError>,
	> + Send;
}

#[derive(Debug)]
pub struct Walker<DBProxy, IsoPathFactory>
where
	DBProxy: WalkerDBProxy,
	IsoPathFactory: IsoFilePathFactory,
{
	// Task control
	id: TaskId,
	is_shallow: bool,

	// Received input args
	entry: ToWalkEntry,
	root: Arc<PathBuf>,
	entry_iso_file_path: IsolatedFilePathData<'static>,
	indexer_ruler: IndexerRuler,

	// Inner state
	stage: WalkerStage,

	// Dependencies
	iso_file_path_factory: IsoPathFactory,
	db_proxy: DBProxy,

	// Non critical errors that happened during the task execution
	errors: Vec<NonCriticalError>,

	// Time spent walking through the received directory
	scan_time: Duration,
}

/// [`Walker`] Task output
#[derive(Debug)]
pub struct Output<DBProxy, IsoPathFactory>
where
	DBProxy: WalkerDBProxy,
	IsoPathFactory: IsoFilePathFactory,
{
	/// Entries found in the file system that need to be created in database
	pub to_create: Vec<WalkedEntry>,
	/// Entries found in the file system that need to be updated in database
	pub to_update: Vec<WalkedEntry>,
	/// Entries found in the file system that need to be removed from database
	pub to_remove: Vec<file_path_pub_and_cas_ids::Data>,
	/// Entries found in the file system that will not be indexed
	pub non_indexed_paths: Vec<PathBuf>,
	/// Ancestors of entries that were indexed
	pub accepted_ancestors: HashSet<WalkedEntry>,
	/// Errors that happened during the task execution
	pub errors: Vec<NonCriticalError>,
	/// Directory that was indexed
	pub directory_iso_file_path: IsolatedFilePathData<'static>,
	/// Total size of the directory that was indexed
	pub total_size: u64,
	/// Task handles that were dispatched to run `WalkDir` tasks for inner directories
	pub keep_walking_tasks: Vec<Walker<DBProxy, IsoPathFactory>>,
	/// Time spent walking through the received directory
	pub scan_time: Duration,
}

#[async_trait::async_trait]
impl<DBProxy, IsoPathFactory> Task<Error> for Walker<DBProxy, IsoPathFactory>
where
	DBProxy: WalkerDBProxy,
	IsoPathFactory: IsoFilePathFactory,
{
	fn id(&self) -> TaskId {
		self.id
	}

	fn with_priority(&self) -> bool {
		// If we're running in shallow mode, then we want priority
		self.is_shallow
	}

	#[instrument(
		skip_all,
		fields(
			task_id = %self.id,
			walked_entry = %self.entry.path.display(),
			is_shallow = self.is_shallow,
		),
		ret(level = Level::TRACE),
		err,
	)]
	#[allow(clippy::blocks_in_conditions)] // Due to `err` on `instrument` macro above
	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, Error> {
		let is_shallow = self.is_shallow;
		let Self {
			root,
			entry: ToWalkEntry {
				path,
				parent_dir_accepted_by_its_children,
			},
			entry_iso_file_path,
			iso_file_path_factory,
			indexer_ruler,
			db_proxy,
			stage,
			errors,
			scan_time,
			..
		} = self;

		let start_time = Instant::now();

		let (
			to_create,
			to_update,
			to_remove,
			non_indexed_paths,
			accepted_ancestors,
			total_size,
			keep_walking_tasks,
		) = loop {
			match stage {
				WalkerStage::Start => {
					trace!("Preparing git indexer rules for walking root");
					if indexer_ruler.has_system(&GITIGNORE) {
						if let Some(rules) =
							GitIgnoreRules::get_rules_if_in_git_repo(root.as_ref(), path).await
						{
							trace!("Found gitignore rules to follow");
							indexer_ruler.extend(rules.map(Into::into));
						}
					}

					*stage = WalkerStage::Walking {
						read_dir_stream: ReadDirStream::new(fs::read_dir(&path).await.map_err(
							|e| {
								indexer::Error::FileIO(
									(&path, e, "Failed to open directory to read its entries")
										.into(),
								)
							},
						)?),
						found_paths: Vec::new(),
					};
					trace!("Starting to walk!");
				}

				WalkerStage::Walking {
					read_dir_stream,
					found_paths,
				} => {
					trace!("Walking...");
					while let Some(res) = read_dir_stream.next().await {
						match res {
							Ok(dir_entry) => {
								found_paths.push(dir_entry.path());
								trace!(
									new_path = %dir_entry.path().display(),
									total_paths = found_paths.len(),
									"Found path;"
								);
							}
							Err(e) => {
								errors.push(NonCriticalError::Indexer(
									indexer::NonCriticalIndexerError::FailedDirectoryEntry(
										FileIOError::from((&path, e)).to_string(),
									),
								));
							}
						}

						check_interruption!(interrupter, start_time, scan_time);
					}

					trace!(total_paths = found_paths.len(), "Finished walking!;");

					*stage = WalkerStage::CollectingMetadata {
						found_paths: mem::take(found_paths),
					};

					check_interruption!(interrupter, start_time, scan_time);
				}

				WalkerStage::CollectingMetadata { found_paths } => {
					trace!("Collecting metadata for found paths");
					*stage = WalkerStage::CheckingIndexerRules {
						paths_and_metadatas: collect_metadata(found_paths, errors).await,
					};
					trace!("Finished collecting metadata!");

					check_interruption!(interrupter, start_time, scan_time);
				}

				WalkerStage::CheckingIndexerRules {
					paths_and_metadatas,
				} => {
					trace!("Checking indexer rules for found paths");
					*stage = WalkerStage::ProcessingRulesResults {
						paths_metadatas_and_acceptance: apply_indexer_rules(
							paths_and_metadatas,
							indexer_ruler,
							errors,
						)
						.await,
					};
					trace!("Finished checking indexer rules!");

					check_interruption!(interrupter, start_time, scan_time);
				}

				WalkerStage::ProcessingRulesResults {
					paths_metadatas_and_acceptance,
				} => {
					trace!("Processing rules results");
					let mut maybe_to_keep_walking = (!is_shallow).then(Vec::new);
					let (accepted_paths, accepted_ancestors, rejected_paths) =
						process_rules_results(
							root,
							iso_file_path_factory,
							*parent_dir_accepted_by_its_children,
							paths_metadatas_and_acceptance,
							&mut maybe_to_keep_walking,
							is_shallow,
							errors,
						)
						.await;

					trace!(
						total_accepted_paths = accepted_paths.len(),
						total_accepted_ancestors = accepted_ancestors.len(),
						collect_rejected_paths = self.is_shallow,
						total_rejected_paths = rejected_paths.len(),
						"Finished processing rules results!;"
					);

					*stage = WalkerStage::GatheringFilePathsToRemove {
						accepted_paths,
						maybe_to_keep_walking,
						accepted_ancestors,
						non_indexed_paths: rejected_paths,
					};

					check_interruption!(interrupter, start_time, scan_time);
				}

				WalkerStage::GatheringFilePathsToRemove {
					accepted_paths,
					maybe_to_keep_walking,
					accepted_ancestors,
					non_indexed_paths,
				} => {
					trace!("Gathering file paths to remove");
					let (walking_entries, to_remove_entries) = gather_file_paths_to_remove(
						accepted_paths,
						entry_iso_file_path,
						iso_file_path_factory,
						db_proxy,
						errors,
					)
					.await;
					trace!("Finished gathering file paths to remove!");

					*stage = WalkerStage::Finalize {
						walking_entries,
						to_remove_entries,
						maybe_to_keep_walking: mem::take(maybe_to_keep_walking),
						accepted_ancestors: mem::take(accepted_ancestors),
						non_indexed_paths: mem::take(non_indexed_paths),
					};

					check_interruption!(interrupter, start_time, scan_time);
				}

				// From this points onwards, we will not allow to be interrupted anymore
				WalkerStage::Finalize {
					walking_entries,
					to_remove_entries,
					maybe_to_keep_walking,
					accepted_ancestors,
					non_indexed_paths,
				} => {
					trace!("Segregating creates and updates");
					let (to_create, to_update, total_size) =
						segregate_creates_and_updates(walking_entries, db_proxy).await?;
					trace!(
						total_to_create = to_create.len(),
						total_to_update = to_update.len(),
						total_to_remove = to_remove_entries.len(),
						total_non_indexed_paths = non_indexed_paths.len(),
						total_size,
						"Finished segregating creates and updates!;"
					);

					let keep_walking_tasks = keep_walking(
						root,
						indexer_ruler,
						iso_file_path_factory,
						db_proxy,
						maybe_to_keep_walking.as_mut(),
						errors,
					);

					break (
						to_create,
						to_update,
						mem::take(to_remove_entries),
						mem::take(non_indexed_paths),
						mem::take(accepted_ancestors),
						total_size,
						keep_walking_tasks,
					);
				}
			}
		};

		*scan_time += start_time.elapsed();

		// Taking out some data as the task is finally complete
		Ok(ExecStatus::Done(
			Output {
				to_create,
				to_update,
				to_remove,
				non_indexed_paths,
				accepted_ancestors,
				errors: mem::take(errors),
				directory_iso_file_path: mem::take(entry_iso_file_path),
				total_size,
				keep_walking_tasks,
				scan_time: *scan_time,
			}
			.into_output(),
		))
	}
}

#[derive(Debug)]
enum WalkerStage {
	Start,
	Walking {
		read_dir_stream: ReadDirStream,
		found_paths: Vec<PathBuf>,
	},
	CollectingMetadata {
		found_paths: Vec<PathBuf>,
	},
	CheckingIndexerRules {
		paths_and_metadatas: HashMap<PathBuf, InnerMetadata>,
	},
	ProcessingRulesResults {
		paths_metadatas_and_acceptance:
			HashMap<PathBuf, (InnerMetadata, HashMap<RuleKind, Vec<bool>>)>,
	},
	GatheringFilePathsToRemove {
		accepted_paths: HashMap<PathBuf, InnerMetadata>,
		maybe_to_keep_walking: Option<Vec<ToWalkEntry>>,
		accepted_ancestors: HashSet<WalkedEntry>,
		non_indexed_paths: Vec<PathBuf>,
	},
	Finalize {
		walking_entries: Vec<WalkingEntry>,
		accepted_ancestors: HashSet<WalkedEntry>,
		to_remove_entries: Vec<file_path_pub_and_cas_ids::Data>,
		maybe_to_keep_walking: Option<Vec<ToWalkEntry>>,
		non_indexed_paths: Vec<PathBuf>,
	},
}

impl<DBProxy, IsoPathFactory> Walker<DBProxy, IsoPathFactory>
where
	DBProxy: WalkerDBProxy,
	IsoPathFactory: IsoFilePathFactory,
{
	pub fn new_deep(
		entry: impl Into<ToWalkEntry> + Send,
		root: Arc<PathBuf>,
		indexer_ruler: IndexerRuler,
		iso_file_path_factory: IsoPathFactory,
		db_proxy: DBProxy,
	) -> Result<Self, indexer::Error> {
		let entry = entry.into();
		Ok(Self {
			id: TaskId::new_v4(),
			root,
			indexer_ruler,
			entry_iso_file_path: iso_file_path_factory.build(&entry.path, true)?,
			iso_file_path_factory,
			db_proxy,
			stage: WalkerStage::Start,
			entry,
			is_shallow: false,
			errors: Vec::new(),
			scan_time: Duration::ZERO,
		})
	}
}

impl<DBProxy, IsoPathFactory> Walker<DBProxy, IsoPathFactory>
where
	DBProxy: WalkerDBProxy,
	IsoPathFactory: IsoFilePathFactory,
{
	pub fn new_shallow(
		entry: impl Into<ToWalkEntry> + Send,
		root: Arc<PathBuf>,
		indexer_ruler: IndexerRuler,
		iso_file_path_factory: IsoPathFactory,
		db_proxy: DBProxy,
	) -> Result<Self, indexer::Error> {
		let entry = entry.into();
		Ok(Self {
			id: TaskId::new_v4(),
			root,
			indexer_ruler,
			entry_iso_file_path: iso_file_path_factory.build(&entry.path, true)?,
			iso_file_path_factory,
			db_proxy,
			stage: WalkerStage::Start,
			entry,
			is_shallow: true,
			errors: Vec::new(),
			scan_time: Duration::ZERO,
		})
	}
}

#[instrument(
	skip_all,
	fields(entries_count = walking_entries.len()),
	err,
)]
async fn segregate_creates_and_updates(
	walking_entries: &mut Vec<WalkingEntry>,
	db_proxy: &impl WalkerDBProxy,
) -> Result<(Vec<WalkedEntry>, Vec<WalkedEntry>, u64), Error> {
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
				let WalkingEntry {
					iso_file_path,
					metadata,
				} = &entry;

				total_size += metadata.size_in_bytes;

				if let Some(file_path) = iso_paths_already_in_db.get(iso_file_path) {
					if let (Some(inode), Some(date_modified)) =
						(&file_path.inode, &file_path.date_modified)
					{
						if (
									inode_from_db(&inode[0..8]) != metadata.inode
									// Datetimes stored in DB loses a bit of precision,
									// so we need to check against a delta
									// instead of using != operator
									|| (
										DateTime::<FixedOffset>::from(metadata.modified_at) - *date_modified
											> ChronoDuration::milliseconds(1)
										)
									|| file_path.hidden.is_none()
									|| metadata.hidden != file_path.hidden.unwrap_or_default()
								)
								// We ignore the size of directories because it is not reliable, we need to
								// calculate it ourselves later
								&& !(
									iso_file_path.to_parts().is_dir
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
							to_update.push(WalkedEntry::from((
								&file_path.pub_id,
								file_path.object_id,
								entry,
							)));
						}
					}
				} else {
					to_create.push(WalkedEntry::from(entry));
				}

				(to_create, to_update, total_size)
			},
		))
	}
}

fn keep_walking<DBProxy, IsoPathFactory>(
	root: &Arc<PathBuf>,
	indexer_ruler: &IndexerRuler,
	iso_file_path_factory: &IsoPathFactory,
	db_proxy: &DBProxy,
	maybe_to_keep_walking: Option<&mut Vec<ToWalkEntry>>,
	errors: &mut Vec<NonCriticalError>,
) -> Vec<Walker<DBProxy, IsoPathFactory>>
where
	DBProxy: WalkerDBProxy,
	IsoPathFactory: IsoFilePathFactory,
{
	maybe_to_keep_walking
		.map(|to_keep_walking| {
			to_keep_walking
				.drain(..)
				.map(|entry| {
					Walker::new_deep(
						entry,
						Arc::clone(root),
						indexer_ruler.clone(),
						iso_file_path_factory.clone(),
						db_proxy.clone(),
					)
					.map_err(|e| {
						indexer::NonCriticalIndexerError::DispatchKeepWalking(e.to_string())
					})
				})
				.filter_map(|res| res.map_err(|e| errors.push(e.into())).ok())
				.collect()
		})
		.unwrap_or_default()
}

async fn collect_metadata(
	found_paths: &mut Vec<PathBuf>,
	errors: &mut Vec<NonCriticalError>,
) -> HashMap<PathBuf, InnerMetadata> {
	found_paths
		.drain(..)
		.map(|current_path| async move {
			fs::metadata(&current_path)
				.await
				.map_err(|e| {
					indexer::NonCriticalIndexerError::Metadata(
						FileIOError::from((&current_path, e)).to_string(),
					)
				})
				.and_then(|metadata| {
					InnerMetadata::new(&current_path, &metadata)
						.map(|metadata| (current_path, metadata))
				})
		})
		.collect::<Vec<_>>()
		.join()
		.await
		.into_iter()
		.filter_map(|res| res.map_err(|e| errors.push(e.into())).ok())
		.collect()
}

async fn gather_file_paths_to_remove(
	accepted_paths: &mut HashMap<PathBuf, InnerMetadata>,
	entry_iso_file_path: &IsolatedFilePathData<'_>,
	iso_file_path_factory: &impl IsoFilePathFactory,
	db_proxy: &impl WalkerDBProxy,
	errors: &mut Vec<NonCriticalError>,
) -> (Vec<WalkingEntry>, Vec<file_path_pub_and_cas_ids::Data>) {
	let mut existing_inodes = HashSet::new();

	let (walking, to_delete_params) = accepted_paths
		.drain()
		.filter_map(|(path, metadata)| {
			iso_file_path_factory
				.build(&path, metadata.is_dir())
				.map(|iso_file_path| {
					let params = file_path::WhereParam::from(&iso_file_path);
					existing_inodes.insert(inode_to_db(metadata.inode));

					(
						WalkingEntry {
							iso_file_path,
							metadata: FilePathMetadata::from(metadata),
						},
						params,
					)
				})
				.map_err(|e| {
					errors
						.push(indexer::NonCriticalIndexerError::IsoFilePath(e.to_string()).into());
				})
				.ok()
		})
		.unzip::<_, _, Vec<_>, Vec<_>>();

	// We continue the function even if we fail to fetch `file_path`s to remove,
	// the DB will have old `file_path`s but at least this is better than
	// don't adding the newly indexed paths
	let to_remove_entries = db_proxy
		.fetch_file_paths_to_remove(entry_iso_file_path, existing_inodes, to_delete_params)
		.await
		.map_err(|e| errors.push(e.into()))
		.unwrap_or_default();

	(walking, to_remove_entries)
}

#[cfg(test)]
mod tests {
	use super::*;

	use sd_core_indexer_rules::{IndexerRule, RulePerKind};
	use sd_core_prisma_helpers::FilePathPubId;
	use sd_task_system::{TaskOutput, TaskStatus, TaskSystem};

	use chrono::Utc;
	use futures::stream::FuturesUnordered;
	use globset::{Glob, GlobSetBuilder};
	use lending_stream::{LendingStream, StreamExt};
	use tempfile::{tempdir, TempDir};
	use tokio::{fs, io::AsyncWriteExt};
	use tracing::debug;
	use tracing_test::traced_test;

	#[derive(Debug, Clone)]
	struct DummyIsoPathFactory {
		root_path: Arc<PathBuf>,
	}

	impl IsoFilePathFactory for DummyIsoPathFactory {
		fn build(
			&self,
			path: impl AsRef<Path>,
			is_dir: bool,
		) -> Result<IsolatedFilePathData<'static>, FilePathError> {
			IsolatedFilePathData::new(0, self.root_path.as_ref(), path, is_dir).map_err(Into::into)
		}
	}

	#[derive(Debug, Clone)]
	struct DummyDBProxy;

	impl WalkerDBProxy for DummyDBProxy {
		async fn fetch_file_paths(
			&self,
			_: Vec<file_path::WhereParam>,
		) -> Result<Vec<file_path_walker::Data>, indexer::Error> {
			Ok(vec![])
		}

		async fn fetch_file_paths_to_remove(
			&self,
			_: &IsolatedFilePathData<'_>,
			_: HashSet<Vec<u8>>,
			_: Vec<file_path::WhereParam>,
		) -> Result<Vec<file_path_pub_and_cas_ids::Data>, indexer::NonCriticalIndexerError> {
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

	#[allow(clippy::cognitive_complexity)]
	async fn prepare_location() -> TempDir {
		// root
		// |__ rust_project
		// |   |__ .git
		// |   |   |__ <empty>
		// |   |__ .gitignore
		// |   |__ ignorable.file
		// |   |__ Cargo.toml
		// |   |__ src
		// |   |   |__ main.rs
		// |   |__ target
		// |   |   |__ debug
		// |   |        |__ main
		// |   |__ partial
		// |   |   |__ ignoreme
		// |   |   |__ readme
		// |   |__ inner
		// |       |__ node_project
		// |           |__ .git
		// |           |   |__ <empty>
		// |           |__ .gitignore
		// |           |__ ignorable.file
		// |           |__ package.json
		// |           |__ src
		// |           |   |__ App.tsx
		// |           |__ node_modules
		// |               |__ react
		// |                   |__ package.json
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

		// Inner directory partially ignored by git
		let partial_dir = rust_project.join("partial");
		fs::create_dir(&partial_dir).await.unwrap();
		fs::File::create(partial_dir.join("ignoreme"))
			.await
			.unwrap();
		fs::File::create(partial_dir.join("readme")).await.unwrap();

		// Making rust and node projects a git repository
		fs::create_dir(rust_project.join(".git")).await.unwrap();
		let gitignore = rust_project.join(".gitignore");
		let mut file = fs::File::create(gitignore).await.unwrap();
		file.write_all(b"*.file\n/target\npartial/ignoreme")
			.await
			.unwrap();
		fs::create_dir(node_project.join(".git")).await.unwrap();
		let gitignore = node_project.join(".gitignore");
		let mut file = fs::File::create(gitignore).await.unwrap();
		file.write_all(b"ignorable.file").await.unwrap();

		// Populating rust project
		fs::File::create(rust_project.join("Cargo.toml"))
			.await
			.unwrap();
		fs::File::create(rust_project.join("ignorable.file"))
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
		fs::File::create(node_project.join("ignorable.file"))
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
				Walker::new_deep(
					root_path.to_path_buf(),
					Arc::new(root_path.to_path_buf()),
					indexer_ruler,
					DummyIsoPathFactory {
						root_path: Arc::new(root_path.to_path_buf()),
					},
					DummyDBProxy,
				)
				.unwrap(),
			)
			.await
			.unwrap();

		let group = FuturesUnordered::new();

		group.push(handle);

		let mut group = group.lend_mut();

		let mut actual_set = HashSet::new();

		let mut ancestors = HashSet::new();

		while let Some((group, task_result)) = group.next().await {
			let TaskStatus::Done((_task_id, TaskOutput::Out(output))) = task_result.unwrap() else {
				panic!("unexpected task output")
			};

			let Output {
				to_create,
				accepted_ancestors,
				errors,
				keep_walking_tasks,
				..
			} = *output
				.downcast::<Output<DummyDBProxy, DummyIsoPathFactory>>()
				.unwrap();

			assert!(errors.is_empty(), "errors: {errors:#?}");

			actual_set.extend(to_create);
			ancestors.extend(accepted_ancestors);

			group.extend(system.dispatch_many(keep_walking_tasks).await.unwrap());
		}

		for actual in &actual_set {
			ancestors.remove(actual);
		}

		if !ancestors.is_empty() {
			debug!(?ancestors, "Adding ancestors to actual");
			actual_set.extend(ancestors);
		}

		assert_eq!(
			actual_set,
			expected,
			"Expected \\ Actual: {:#?};\n Actual \\ Expected: {:#?}",
			expected.difference(&actual_set),
			actual_set.difference(&expected)
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
		let pub_id = FilePathPubId::new();
		let maybe_object_id = None;

		#[rustfmt::skip]
		let expected = [
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("rust_project"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("rust_project/.git"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("rust_project/.gitignore"), false), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("rust_project/Cargo.toml"), false), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("rust_project/partial"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("rust_project/partial/readme"), false), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("rust_project/src"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("rust_project/src/main.rs"), false), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("inner"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("inner/node_project"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/.git"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/.gitignore"), false), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/package.json"), false), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/src"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/src/App.tsx"), false), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/node_modules"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/node_modules/react"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/node_modules/react/package.json"), false), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("photos"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("photos/photo1.png"), false), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("photos/photo2.jpg"), false), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("photos/photo3.jpeg"), false), metadata },
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
		let pub_id = FilePathPubId::new();
		let maybe_object_id = None;

		#[rustfmt::skip]
		let expected = [
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("photos"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("photos/photo1.png"), false), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("photos/photo2.jpg"), false), metadata },
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
		let pub_id = FilePathPubId::new();
		let maybe_object_id = None;

		#[rustfmt::skip]
		let expected = [
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("rust_project"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("rust_project/.git"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("rust_project/.gitignore"), false), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("rust_project/Cargo.toml"), false), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("rust_project/src"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("rust_project/src/main.rs"), false), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("rust_project/partial"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("rust_project/partial/readme"), false), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("inner"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("inner/node_project"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/.git"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/package.json"), false), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/.gitignore"), false), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/src"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/src/App.tsx"), false), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/node_modules"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/node_modules/react"), true), metadata },
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
		let pub_id = FilePathPubId::new();
		let maybe_object_id = None;

		#[rustfmt::skip]
		let expected = [
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("rust_project"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("rust_project/.git"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("rust_project/.gitignore"), false), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("rust_project/Cargo.toml"), false), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("rust_project/partial"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("rust_project/partial/readme"), false), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("rust_project/src"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("rust_project/src/main.rs"), false), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("inner"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("inner/node_project"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/.git"), true), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/.gitignore"), false), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/package.json"), false), metadata },
			WalkedEntry { pub_id: pub_id.clone(), maybe_object_id, iso_file_path: f(root_path.join("inner/node_project/src"), true), metadata },
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
