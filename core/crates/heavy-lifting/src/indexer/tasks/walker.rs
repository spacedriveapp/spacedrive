use crate::{
	indexer::{IndexerError, NonCriticalIndexerError},
	Error, NonCriticalJobError,
};

use sd_core_file_path_helper::{FilePathError, FilePathMetadata, IsolatedFilePathData};
use sd_core_indexer_rules::{IndexerRuler, MetadataForIndexerRules, RuleKind};
use sd_core_prisma_helpers::{file_path_pub_and_cas_ids, file_path_walker};

use sd_prisma::prisma::file_path;
use sd_task_system::{
	check_interruption, BaseTaskDispatcher, ExecStatus, Interrupter, IntoAnyTaskOutput,
	SerializableTask, Task, TaskDispatcher, TaskHandle, TaskId,
};
use sd_utils::{db::inode_from_db, error::FileIOError};

use std::{
	collections::{hash_map::Entry, HashMap, HashSet},
	fmt,
	fs::Metadata,
	future::Future,
	hash::{Hash, Hasher},
	mem,
	path::{Path, PathBuf},
	sync::Arc,
	time::Duration,
};

use chrono::{DateTime, Duration as ChronoDuration, FixedOffset, Utc};
use futures_concurrency::future::Join;
use serde::{Deserialize, Serialize};
use tokio::{fs, time::Instant};
use tokio_stream::{wrappers::ReadDirStream, StreamExt};
use tracing::trace;
use uuid::Uuid;

/// `WalkedEntry` represents a single path in the filesystem
#[derive(Debug, Serialize, Deserialize)]
pub struct WalkedEntry {
	pub pub_id: Uuid,
	pub maybe_object_id: file_path::object_id::Type,
	pub iso_file_path: IsolatedFilePathData<'static>,
	pub metadata: FilePathMetadata,
}

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

#[derive(Debug, Serialize, Deserialize)]
struct WalkingEntry {
	iso_file_path: IsolatedFilePathData<'static>,
	metadata: FilePathMetadata,
}

impl From<WalkingEntry> for WalkedEntry {
	fn from(
		WalkingEntry {
			iso_file_path,
			metadata,
		}: WalkingEntry,
	) -> Self {
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
		(
			pub_id,
			maybe_object_id,
			WalkingEntry {
				iso_file_path,
				metadata,
			},
		): (Uuid, file_path::object_id::Type, WalkingEntry),
	) -> Self {
		Self {
			pub_id,
			maybe_object_id,
			iso_file_path,
			metadata,
		}
	}
}

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
	) -> impl Future<Output = Result<Vec<file_path_walker::Data>, IndexerError>> + Send;

	fn fetch_file_paths_to_remove(
		&self,
		parent_iso_file_path: &IsolatedFilePathData<'_>,
		unique_location_id_materialized_path_name_extension_params: Vec<file_path::WhereParam>,
	) -> impl Future<Output = Result<Vec<file_path_pub_and_cas_ids::Data>, NonCriticalIndexerError>> + Send;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToWalkEntry {
	path: PathBuf,
	parent_dir_accepted_by_its_children: Option<bool>,
}

impl<P: AsRef<Path>> From<P> for ToWalkEntry {
	fn from(path: P) -> Self {
		Self {
			path: path.as_ref().into(),
			parent_dir_accepted_by_its_children: None,
		}
	}
}

#[derive(Debug)]
pub struct WalkTaskOutput {
	pub to_create: Vec<WalkedEntry>,
	pub to_update: Vec<WalkedEntry>,
	pub to_remove: Vec<file_path_pub_and_cas_ids::Data>,
	pub accepted_ancestors: HashSet<WalkedEntry>,
	pub errors: Vec<NonCriticalJobError>,
	pub directory_iso_file_path: IsolatedFilePathData<'static>,
	pub total_size: u64,
	pub handles: Vec<TaskHandle<Error>>,
	pub scan_time: Duration,
}

#[derive(Debug, Serialize, Deserialize)]
struct InnerMetadata {
	pub is_dir: bool,
	pub is_symlink: bool,
	pub inode: u64,
	pub size_in_bytes: u64,
	pub hidden: bool,
	pub created_at: DateTime<Utc>,
	pub modified_at: DateTime<Utc>,
}

impl InnerMetadata {
	fn new(path: impl AsRef<Path>, metadata: &Metadata) -> Result<Self, NonCriticalIndexerError> {
		let FilePathMetadata {
			inode,
			size_in_bytes,
			created_at,
			modified_at,
			hidden,
		} = FilePathMetadata::from_path(path, metadata)
			.map_err(|e| NonCriticalIndexerError::FilePathMetadata(e.to_string()))?;

		Ok(Self {
			is_dir: metadata.is_dir(),
			is_symlink: metadata.is_symlink(),
			inode,
			size_in_bytes,
			hidden,
			created_at,
			modified_at,
		})
	}
}

impl MetadataForIndexerRules for InnerMetadata {
	fn is_dir(&self) -> bool {
		self.is_dir
	}
}

impl From<InnerMetadata> for FilePathMetadata {
	fn from(metadata: InnerMetadata) -> Self {
		Self {
			inode: metadata.inode,
			size_in_bytes: metadata.size_in_bytes,
			hidden: metadata.hidden,
			created_at: metadata.created_at,
			modified_at: metadata.modified_at,
		}
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
	},
	Finalize {
		walking_entries: Vec<WalkingEntry>,
		accepted_ancestors: HashSet<WalkedEntry>,
		to_remove_entries: Vec<file_path_pub_and_cas_ids::Data>,
		maybe_to_keep_walking: Option<Vec<ToWalkEntry>>,
	},
}

#[derive(Debug, Serialize, Deserialize)]
struct WalkDirSaveState {
	id: TaskId,
	entry: ToWalkEntry,
	root: Arc<PathBuf>,
	entry_iso_file_path: IsolatedFilePathData<'static>,
	stage: WalkerStageSaveState,
	errors: Vec<NonCriticalJobError>,
	scan_time: Duration,
	is_shallow: bool,
}

#[derive(Debug, Serialize, Deserialize)]
enum WalkerStageSaveState {
	Start,
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
	},
	Finalize {
		walking_entries: Vec<WalkingEntry>,
		accepted_ancestors: HashSet<WalkedEntry>,
		to_remove_entries: Vec<file_path_pub_and_cas_ids::Data>,
		maybe_to_keep_walking: Option<Vec<ToWalkEntry>>,
	},
}

impl From<WalkerStage> for WalkerStageSaveState {
	fn from(stage: WalkerStage) -> Self {
		match stage {
			// We can't store the current state of `ReadDirStream` so we start again from the beginning
			WalkerStage::Start | WalkerStage::Walking { .. } => Self::Start,
			WalkerStage::CollectingMetadata { found_paths } => {
				Self::CollectingMetadata { found_paths }
			}
			WalkerStage::CheckingIndexerRules {
				paths_and_metadatas,
			} => Self::CheckingIndexerRules {
				paths_and_metadatas,
			},
			WalkerStage::ProcessingRulesResults {
				paths_metadatas_and_acceptance,
			} => Self::ProcessingRulesResults {
				paths_metadatas_and_acceptance,
			},
			WalkerStage::GatheringFilePathsToRemove {
				accepted_paths,
				maybe_to_keep_walking,
				accepted_ancestors,
			} => Self::GatheringFilePathsToRemove {
				accepted_paths,
				maybe_to_keep_walking,
				accepted_ancestors,
			},
			WalkerStage::Finalize {
				walking_entries,
				accepted_ancestors,
				to_remove_entries,
				maybe_to_keep_walking,
			} => Self::Finalize {
				walking_entries,
				accepted_ancestors,
				to_remove_entries,
				maybe_to_keep_walking,
			},
		}
	}
}

impl From<WalkerStageSaveState> for WalkerStage {
	fn from(value: WalkerStageSaveState) -> Self {
		match value {
			WalkerStageSaveState::Start => Self::Start,
			WalkerStageSaveState::CollectingMetadata { found_paths } => {
				Self::CollectingMetadata { found_paths }
			}
			WalkerStageSaveState::CheckingIndexerRules {
				paths_and_metadatas,
			} => Self::CheckingIndexerRules {
				paths_and_metadatas,
			},
			WalkerStageSaveState::ProcessingRulesResults {
				paths_metadatas_and_acceptance,
			} => Self::ProcessingRulesResults {
				paths_metadatas_and_acceptance,
			},
			WalkerStageSaveState::GatheringFilePathsToRemove {
				accepted_paths,
				maybe_to_keep_walking,
				accepted_ancestors,
			} => Self::GatheringFilePathsToRemove {
				accepted_paths,
				maybe_to_keep_walking,
				accepted_ancestors,
			},
			WalkerStageSaveState::Finalize {
				walking_entries,
				accepted_ancestors,
				to_remove_entries,
				maybe_to_keep_walking,
			} => Self::Finalize {
				walking_entries,
				accepted_ancestors,
				to_remove_entries,
				maybe_to_keep_walking,
			},
		}
	}
}

#[derive(Debug)]
pub struct WalkDirTask<DBProxy, IsoPathFactory, Dispatcher = BaseTaskDispatcher<Error>>
where
	DBProxy: WalkerDBProxy,
	IsoPathFactory: IsoFilePathFactory,
	Dispatcher: TaskDispatcher<Error>,
{
	id: TaskId,
	entry: ToWalkEntry,
	root: Arc<PathBuf>,
	entry_iso_file_path: IsolatedFilePathData<'static>,
	indexer_ruler: IndexerRuler,
	iso_file_path_factory: IsoPathFactory,
	db_proxy: DBProxy,
	stage: WalkerStage,
	maybe_dispatcher: Option<Dispatcher>,
	errors: Vec<NonCriticalJobError>,
	scan_time: Duration,
	is_shallow: bool,
}

impl<DBProxy, IsoPathFactory, Dispatcher> WalkDirTask<DBProxy, IsoPathFactory, Dispatcher>
where
	DBProxy: WalkerDBProxy,
	IsoPathFactory: IsoFilePathFactory,
	Dispatcher: TaskDispatcher<Error>,
{
	pub fn new_deep(
		entry: impl Into<ToWalkEntry> + Send,
		root: Arc<PathBuf>,
		indexer_ruler: IndexerRuler,
		iso_file_path_factory: IsoPathFactory,
		db_proxy: DBProxy,
		dispatcher: Dispatcher,
	) -> Result<Self, IndexerError> {
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
			maybe_dispatcher: Some(dispatcher),
			is_shallow: false,
			errors: Vec::new(),
			scan_time: Duration::ZERO,
		})
	}
}

impl<DBProxy, IsoPathFactory> WalkDirTask<DBProxy, IsoPathFactory, BaseTaskDispatcher<Error>>
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
	) -> Result<Self, IndexerError> {
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
			maybe_dispatcher: None,
			is_shallow: true,
			errors: Vec::new(),
			scan_time: Duration::ZERO,
		})
	}
}

impl<DBProxy, IsoPathFactory, Dispatcher> SerializableTask<Error>
	for WalkDirTask<DBProxy, IsoPathFactory, Dispatcher>
where
	DBProxy: WalkerDBProxy,
	IsoPathFactory: IsoFilePathFactory,
	Dispatcher: TaskDispatcher<Error>,
{
	type SerializeError = rmp_serde::encode::Error;
	type DeserializeError = rmp_serde::decode::Error;
	type DeserializeCtx = (IndexerRuler, DBProxy, IsoPathFactory, Dispatcher);

	async fn serialize(self) -> Result<Vec<u8>, Self::SerializeError> {
		let Self {
			id,
			entry,
			root,
			entry_iso_file_path,
			stage,
			errors,
			scan_time,
			is_shallow,
			..
		} = self;
		rmp_serde::to_vec_named(&WalkDirSaveState {
			id,
			entry,
			root,
			entry_iso_file_path,
			stage: stage.into(),
			errors,
			scan_time,
			is_shallow,
		})
	}

	async fn deserialize(
		data: &[u8],
		(indexer_ruler, db_proxy, iso_file_path_factory, dispatcher): Self::DeserializeCtx,
	) -> Result<Self, Self::DeserializeError> {
		rmp_serde::from_slice(data).map(
			|WalkDirSaveState {
			     id,
			     entry,
			     root,
			     entry_iso_file_path,
			     stage,
			     errors,
			     scan_time,
			     is_shallow,
			 }| Self {
				id,
				entry,
				root,
				entry_iso_file_path,
				indexer_ruler,
				iso_file_path_factory,
				db_proxy,
				stage: stage.into(),
				maybe_dispatcher: is_shallow.then_some(dispatcher),
				errors,
				scan_time,
				is_shallow,
			},
		)
	}
}

#[async_trait::async_trait]
impl<DBProxy, IsoPathFactory, Dispatcher> Task<Error>
	for WalkDirTask<DBProxy, IsoPathFactory, Dispatcher>
where
	DBProxy: WalkerDBProxy,
	IsoPathFactory: IsoFilePathFactory,
	Dispatcher: TaskDispatcher<Error>,
{
	fn id(&self) -> TaskId {
		self.id
	}

	fn with_priority(&self) -> bool {
		// If we're running in shallow mode, then we want priority
		self.is_shallow
	}

	#[allow(clippy::too_many_lines)]
	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, Error> {
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
			maybe_dispatcher,
			errors,
			scan_time,
			..
		} = self;

		let start_time = Instant::now();

		let (to_create, to_update, total_size, to_remove, accepted_ancestors, handles) = loop {
			match stage {
				WalkerStage::Start => {
					*stage = WalkerStage::Walking {
						read_dir_stream: ReadDirStream::new(fs::read_dir(&path).await.map_err(
							|e| {
								IndexerError::FileIO(
									(&path, e, "Failed to open directory to read its entries")
										.into(),
								)
							},
						)?),
						found_paths: Vec::new(),
					};
				}

				WalkerStage::Walking {
					read_dir_stream,
					found_paths,
				} => {
					while let Some(res) = read_dir_stream.next().await {
						match res {
							Ok(dir_entry) => {
								found_paths.push(dir_entry.path());
							}
							Err(e) => {
								errors.push(NonCriticalJobError::Indexer(
									NonCriticalIndexerError::FailedDirectoryEntry(
										FileIOError::from((&path, e)).to_string(),
									),
								));
							}
						}

						check_interruption!(interrupter, start_time, scan_time);
					}

					*stage = WalkerStage::CollectingMetadata {
						found_paths: mem::take(found_paths),
					};

					check_interruption!(interrupter, start_time, scan_time);
				}

				WalkerStage::CollectingMetadata { found_paths } => {
					*stage = WalkerStage::CheckingIndexerRules {
						paths_and_metadatas: collect_metadata(found_paths, errors).await,
					};

					check_interruption!(interrupter, start_time, scan_time);
				}

				WalkerStage::CheckingIndexerRules {
					paths_and_metadatas,
				} => {
					*stage = WalkerStage::ProcessingRulesResults {
						paths_metadatas_and_acceptance: apply_indexer_rules(
							paths_and_metadatas,
							indexer_ruler,
							errors,
						)
						.await,
					};

					check_interruption!(interrupter, start_time, scan_time);
				}

				WalkerStage::ProcessingRulesResults {
					paths_metadatas_and_acceptance,
				} => {
					let mut maybe_to_keep_walking = maybe_dispatcher.is_some().then(Vec::new);
					let (accepted_paths, accepted_ancestors) = process_rules_results(
						root,
						iso_file_path_factory,
						*parent_dir_accepted_by_its_children,
						paths_metadatas_and_acceptance,
						&mut maybe_to_keep_walking,
						errors,
					)
					.await;

					*stage = WalkerStage::GatheringFilePathsToRemove {
						accepted_paths,
						maybe_to_keep_walking,
						accepted_ancestors,
					};

					check_interruption!(interrupter, start_time, scan_time);
				}

				WalkerStage::GatheringFilePathsToRemove {
					accepted_paths,
					maybe_to_keep_walking,
					accepted_ancestors,
				} => {
					let (walking_entries, to_remove_entries) = gather_file_paths_to_remove(
						accepted_paths,
						entry_iso_file_path,
						iso_file_path_factory,
						db_proxy,
						errors,
					)
					.await;

					*stage = WalkerStage::Finalize {
						walking_entries,
						to_remove_entries,
						maybe_to_keep_walking: mem::take(maybe_to_keep_walking),
						accepted_ancestors: mem::take(accepted_ancestors),
					};

					check_interruption!(interrupter, start_time, scan_time);
				}

				// From this points onwards, we will not allow to be interrupted anymore
				WalkerStage::Finalize {
					walking_entries,
					to_remove_entries,
					maybe_to_keep_walking,
					accepted_ancestors,
				} => {
					let (to_create, to_update, total_size) =
						segregate_creates_and_updates(walking_entries, db_proxy).await?;

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

					break (
						to_create,
						to_update,
						total_size,
						mem::take(to_remove_entries),
						mem::take(accepted_ancestors),
						handles,
					);
				}
			}
		};

		*scan_time += start_time.elapsed();

		// Taking out some data as the task is finally complete
		Ok(ExecStatus::Done(
			WalkTaskOutput {
				to_create,
				to_update,
				to_remove,
				accepted_ancestors,
				errors: mem::take(errors),
				directory_iso_file_path: mem::take(entry_iso_file_path),
				total_size,
				handles,
				scan_time: *scan_time,
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
								inode_from_db(&inode[0..8]) != metadata.inode
								// Datetimes stored in DB loses a bit of precision, so we need to check against a delta
								// instead of using != operator
								|| DateTime::<FixedOffset>::from(metadata.modified_at) - *date_modified
									> ChronoDuration::milliseconds(1) || file_path.hidden.is_none() || metadata.hidden != file_path.hidden.unwrap_or_default()
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
	iso_file_path_factory: &impl IsoFilePathFactory,
	db_proxy: &impl WalkerDBProxy,
	maybe_to_keep_walking: &mut Option<Vec<ToWalkEntry>>,
	dispatcher: &Option<impl TaskDispatcher<Error>>,
	errors: &mut Vec<NonCriticalJobError>,
) -> Vec<TaskHandle<Error>> {
	if let (Some(dispatcher), Some(to_keep_walking)) = (dispatcher, maybe_to_keep_walking) {
		dispatcher
			.dispatch_many(
				to_keep_walking
					.drain(..)
					.map(|entry| {
						WalkDirTask::new_deep(
							entry,
							Arc::clone(root),
							indexer_ruler.clone(),
							iso_file_path_factory.clone(),
							db_proxy.clone(),
							dispatcher.clone(),
						)
						.map_err(|e| NonCriticalIndexerError::DispatchKeepWalking(e.to_string()))
					})
					.filter_map(|res| res.map_err(|e| errors.push(e.into())).ok()),
			)
			.await
	} else {
		Vec::new()
	}
}

async fn collect_metadata(
	found_paths: &mut Vec<PathBuf>,
	errors: &mut Vec<NonCriticalJobError>,
) -> HashMap<PathBuf, InnerMetadata> {
	found_paths
		.drain(..)
		.map(|current_path| async move {
			fs::metadata(&current_path)
				.await
				.map_err(|e| {
					NonCriticalIndexerError::Metadata(
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

async fn apply_indexer_rules(
	paths_and_metadatas: &mut HashMap<PathBuf, InnerMetadata>,
	indexer_ruler: &IndexerRuler,
	errors: &mut Vec<NonCriticalJobError>,
) -> HashMap<PathBuf, (InnerMetadata, HashMap<RuleKind, Vec<bool>>)> {
	paths_and_metadatas
		.drain()
		// TODO: Hard ignoring symlinks for now, but this should be configurable
		.filter(|(_, metadata)| !metadata.is_symlink)
		.map(|(current_path, metadata)| async {
			indexer_ruler
				.apply_all(&current_path, &metadata)
				.await
				.map(|acceptance_per_rule_kind| {
					(current_path, (metadata, acceptance_per_rule_kind))
				})
				.map_err(|e| NonCriticalIndexerError::IndexerRule(e.to_string()))
		})
		.collect::<Vec<_>>()
		.join()
		.await
		.into_iter()
		.filter_map(|res| res.map_err(|e| errors.push(e.into())).ok())
		.collect()
}

async fn process_rules_results(
	root: &Arc<PathBuf>,
	iso_file_path_factory: &impl IsoFilePathFactory,
	parent_dir_accepted_by_its_children: Option<bool>,
	paths_metadatas_and_acceptance: &mut HashMap<
		PathBuf,
		(InnerMetadata, HashMap<RuleKind, Vec<bool>>),
	>,
	maybe_to_keep_walking: &mut Option<Vec<ToWalkEntry>>,
	errors: &mut Vec<NonCriticalJobError>,
) -> (HashMap<PathBuf, InnerMetadata>, HashSet<WalkedEntry>) {
	let root = root.as_ref();

	let (accepted, accepted_ancestors) = paths_metadatas_and_acceptance.drain().fold(
		(HashMap::new(), HashMap::new()),
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
					iso_file_path_factory,
					&mut accepted_ancestors,
					errors,
				);
			}

			(accepted, accepted_ancestors)
		},
	);

	(
		accepted,
		accepted_ancestors
			.into_iter()
			.map(|(ancestor_iso_file_path, ancestor_path)| async move {
				fs::metadata(&ancestor_path)
					.await
					.map_err(|e| {
						NonCriticalIndexerError::Metadata(
							FileIOError::from((&ancestor_path, e)).to_string(),
						)
					})
					.and_then(|metadata| {
						FilePathMetadata::from_path(&ancestor_path, &metadata)
							.map(|metadata| {
								WalkingEntry {
									iso_file_path: ancestor_iso_file_path,
									metadata,
								}
								.into()
							})
							.map_err(|e| NonCriticalIndexerError::FilePathMetadata(e.to_string()))
					})
			})
			.collect::<Vec<_>>()
			.join()
			.await
			.into_iter()
			.filter_map(|res| res.map_err(|e| errors.push(e.into())).ok())
			.collect(),
	)
}

fn process_and_maybe_reject_by_directory_rules(
	current_path: &Path,
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
		});
	}

	false
}

fn accept_ancestors(
	current_path: PathBuf,
	metadata: InnerMetadata,
	root: &Path,
	accepted: &mut HashMap<PathBuf, InnerMetadata>,
	iso_file_path_factory: &impl IsoFilePathFactory,
	accepted_ancestors: &mut HashMap<IsolatedFilePathData<'static>, PathBuf>,
	errors: &mut Vec<NonCriticalJobError>,
) {
	// If the ancestors directories wasn't indexed before, now we do
	for ancestor in current_path
		.ancestors()
		.skip(1) // Skip the current directory as it was already indexed
		.take_while(|&ancestor| ancestor != root)
	{
		if let Ok(iso_file_path) = iso_file_path_factory
			.build(ancestor, true)
			.map_err(|e| errors.push(NonCriticalIndexerError::IsoFilePath(e.to_string()).into()))
		{
			match accepted_ancestors.entry(iso_file_path) {
				Entry::Occupied(_) => {
					// If we already accepted this ancestor, then it will contain
					// also all if its ancestors too, so we can stop here
					break;
				}
				Entry::Vacant(entry) => {
					trace!("Accepted ancestor {}", ancestor.display());
					entry.insert(ancestor.to_path_buf());
				}
			}
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
	accepted_paths: &mut HashMap<PathBuf, InnerMetadata>,
	entry_iso_file_path: &IsolatedFilePathData<'_>,
	iso_file_path_factory: &impl IsoFilePathFactory,
	db_proxy: &impl WalkerDBProxy,
	errors: &mut Vec<NonCriticalJobError>,
) -> (Vec<WalkingEntry>, Vec<file_path_pub_and_cas_ids::Data>) {
	let (walking, to_delete_params) = accepted_paths
		.drain()
		.filter_map(|(path, metadata)| {
			iso_file_path_factory
				.build(&path, metadata.is_dir())
				.map(|iso_file_path| {
					let params = file_path::WhereParam::from(&iso_file_path);

					(
						WalkingEntry {
							iso_file_path,
							metadata: FilePathMetadata::from(metadata),
						},
						params,
					)
				})
				.map_err(|e| {
					errors.push(NonCriticalIndexerError::IsoFilePath(e.to_string()).into());
				})
				.ok()
		})
		.unzip::<_, _, Vec<_>, Vec<_>>();

	// We continue the function even if we fail to fetch `file_path`s to remove,
	// the DB will have old `file_path`s but at least this is better than
	// don't adding the newly indexed paths
	let to_remove_entries = db_proxy
		.fetch_file_paths_to_remove(entry_iso_file_path, to_delete_params)
		.await
		.map_err(|e| errors.push(e.into()))
		.unwrap_or_default();

	(walking, to_remove_entries)
}

#[cfg(test)]
mod tests {
	use super::*;

	use sd_core_indexer_rules::{IndexerRule, RulePerKind};
	use sd_task_system::{TaskOutput, TaskStatus, TaskSystem};

	use chrono::Utc;
	use futures_concurrency::future::FutureGroup;
	use globset::{Glob, GlobSetBuilder};
	use lending_stream::{LendingStream, StreamExt};
	use tempfile::{tempdir, TempDir};
	use tokio::fs;
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
		) -> Result<Vec<file_path_walker::Data>, IndexerError> {
			Ok(vec![])
		}

		async fn fetch_file_paths_to_remove(
			&self,
			_: &IsolatedFilePathData<'_>,
			_: Vec<file_path::WhereParam>,
		) -> Result<Vec<file_path_pub_and_cas_ids::Data>, NonCriticalIndexerError> {
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
				WalkDirTask::new_deep(
					root_path.to_path_buf(),
					Arc::new(root_path.to_path_buf()),
					indexer_ruler,
					DummyIsoPathFactory {
						root_path: Arc::new(root_path.to_path_buf()),
					},
					DummyDBProxy,
					system.get_dispatcher(),
				)
				.unwrap(),
			)
			.await;

		let mut group = FutureGroup::new();

		group.insert(handle);

		let mut group = group.lend_mut();

		let mut actual_set = HashSet::new();

		let mut ancestors = HashSet::new();

		while let Some((group, task_result)) = group.next().await {
			let TaskStatus::Done((_task_id, TaskOutput::Out(output))) = task_result.unwrap() else {
				panic!("unexpected task output")
			};

			let WalkTaskOutput {
				to_create,
				accepted_ancestors,
				errors,
				handles,
				..
			} = *output.downcast::<WalkTaskOutput>().unwrap();

			assert!(errors.is_empty(), "errors: {errors:#?}");

			actual_set.extend(to_create);
			ancestors.extend(accepted_ancestors);

			for handle in handles {
				group.insert(handle);
			}
		}

		for actual in &actual_set {
			ancestors.remove(actual);
		}

		if !ancestors.is_empty() {
			debug!("Adding ancestors to actual: {:#?}", ancestors);
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
