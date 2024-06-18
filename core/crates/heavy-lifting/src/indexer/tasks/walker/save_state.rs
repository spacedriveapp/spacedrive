use crate::{Error, NonCriticalError};

use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_indexer_rules::{IndexerRuler, RuleKind};
use sd_core_prisma_helpers::file_path_pub_and_cas_ids;

use std::{
	collections::{HashMap, HashSet},
	path::PathBuf,
	sync::Arc,
	time::Duration,
};

use sd_task_system::{SerializableTask, TaskId};
use serde::{Deserialize, Serialize};

use super::{
	entry::{ToWalkEntry, WalkingEntry},
	metadata::InnerMetadata,
	IsoFilePathFactory, WalkedEntry, Walker, WalkerDBProxy, WalkerStage,
};

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct WalkDirSaveState {
	id: TaskId,
	is_shallow: bool,

	entry: ToWalkEntry,
	root: Arc<PathBuf>,
	entry_iso_file_path: IsolatedFilePathData<'static>,

	stage: WalkerStageSaveState,

	errors: Vec<NonCriticalError>,
	scan_time: Duration,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) enum WalkerStageSaveState {
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
				non_indexed_paths,
			} => Self::GatheringFilePathsToRemove {
				accepted_paths,
				maybe_to_keep_walking,
				accepted_ancestors,
				non_indexed_paths,
			},
			WalkerStage::Finalize {
				walking_entries,
				accepted_ancestors,
				to_remove_entries,
				maybe_to_keep_walking,
				non_indexed_paths,
			} => Self::Finalize {
				walking_entries,
				accepted_ancestors,
				to_remove_entries,
				maybe_to_keep_walking,
				non_indexed_paths,
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
				non_indexed_paths,
			} => Self::GatheringFilePathsToRemove {
				accepted_paths,
				maybe_to_keep_walking,
				accepted_ancestors,
				non_indexed_paths,
			},
			WalkerStageSaveState::Finalize {
				walking_entries,
				accepted_ancestors,
				to_remove_entries,
				maybe_to_keep_walking,
				non_indexed_paths,
			} => Self::Finalize {
				walking_entries,
				accepted_ancestors,
				to_remove_entries,
				maybe_to_keep_walking,
				non_indexed_paths,
			},
		}
	}
}

impl<DBProxy, IsoPathFactory> SerializableTask<Error> for Walker<DBProxy, IsoPathFactory>
where
	DBProxy: WalkerDBProxy,
	IsoPathFactory: IsoFilePathFactory,
{
	type SerializeError = rmp_serde::encode::Error;
	type DeserializeError = rmp_serde::decode::Error;
	type DeserializeCtx = (IndexerRuler, DBProxy, IsoPathFactory);

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
			is_shallow,
			entry,
			root,
			entry_iso_file_path,
			stage: stage.into(),
			errors,
			scan_time,
		})
	}

	async fn deserialize(
		data: &[u8],
		(indexer_ruler, db_proxy, iso_file_path_factory): Self::DeserializeCtx,
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
				errors,
				scan_time,
				is_shallow,
			},
		)
	}
}
