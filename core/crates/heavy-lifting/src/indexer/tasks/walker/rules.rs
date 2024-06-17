use crate::{indexer, NonCriticalError};

use sd_core_file_path_helper::{FilePathMetadata, IsolatedFilePathData};
use sd_core_indexer_rules::{IndexerRuler, MetadataForIndexerRules, RuleKind};

use sd_utils::error::FileIOError;

use std::{
	collections::{hash_map::Entry, HashMap, HashSet},
	path::{Path, PathBuf},
	sync::Arc,
};

use futures_concurrency::future::Join;
use tokio::fs;
use tracing::{instrument, trace};

use super::{
	entry::{ToWalkEntry, WalkingEntry},
	InnerMetadata, IsoFilePathFactory, WalkedEntry,
};

pub(super) async fn apply_indexer_rules(
	paths_and_metadatas: &mut HashMap<PathBuf, InnerMetadata>,
	indexer_ruler: &IndexerRuler,
	errors: &mut Vec<NonCriticalError>,
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
				.map_err(|e| indexer::NonCriticalIndexerError::IndexerRule(e.to_string()))
		})
		.collect::<Vec<_>>()
		.join()
		.await
		.into_iter()
		.filter_map(|res| res.map_err(|e| errors.push(e.into())).ok())
		.collect()
}

pub(super) async fn process_rules_results(
	root: &Arc<PathBuf>,
	iso_file_path_factory: &impl IsoFilePathFactory,
	parent_dir_accepted_by_its_children: Option<bool>,
	paths_metadatas_and_acceptance: &mut HashMap<
		PathBuf,
		(InnerMetadata, HashMap<RuleKind, Vec<bool>>),
	>,
	maybe_to_keep_walking: &mut Option<Vec<ToWalkEntry>>,
	collect_rejected_paths: bool,
	errors: &mut Vec<NonCriticalError>,
) -> (
	HashMap<PathBuf, InnerMetadata>,
	HashSet<WalkedEntry>,
	Vec<PathBuf>,
) {
	let (accepted, accepted_ancestors, rejected) = segregate_paths(
		root,
		iso_file_path_factory,
		paths_metadatas_and_acceptance.drain(),
		parent_dir_accepted_by_its_children,
		maybe_to_keep_walking,
		collect_rejected_paths,
		errors,
	);

	(
		accepted,
		accepted_ancestors
			.into_iter()
			.map(|(ancestor_iso_file_path, ancestor_path)| async move {
				fs::metadata(&ancestor_path)
					.await
					.map_err(|e| {
						indexer::NonCriticalIndexerError::Metadata(
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
							.map_err(|e| {
								indexer::NonCriticalIndexerError::FilePathMetadata(e.to_string())
							})
					})
			})
			.collect::<Vec<_>>()
			.join()
			.await
			.into_iter()
			.filter_map(|res| res.map_err(|e| errors.push(e.into())).ok())
			.collect(),
		rejected,
	)
}

fn segregate_paths(
	root: &Arc<PathBuf>,
	iso_file_path_factory: &impl IsoFilePathFactory,
	paths_metadatas_and_acceptance: impl IntoIterator<
		Item = (PathBuf, (InnerMetadata, HashMap<RuleKind, Vec<bool>>)),
	>,
	parent_dir_accepted_by_its_children: Option<bool>,
	maybe_to_keep_walking: &mut Option<Vec<ToWalkEntry>>,
	collect_rejected_paths: bool,
	errors: &mut Vec<NonCriticalError>,
) -> (
	HashMap<PathBuf, InnerMetadata>,
	HashMap<IsolatedFilePathData<'static>, PathBuf>,
	Vec<PathBuf>,
) {
	let root = root.as_ref();

	let mut accepted = HashMap::new();
	let mut accepted_ancestors = HashMap::new();
	let mut rejected = Vec::new();

	for (current_path, (metadata, acceptance_per_rule_kind)) in paths_metadatas_and_acceptance {
		// Accept by children has three states,
		// None if we don't now yet or if this check doesn't apply
		// Some(true) if this check applies and it passes
		// Some(false) if this check applies and it was rejected
		// and we pass the current parent state to its children
		let mut accept_by_children_dir = parent_dir_accepted_by_its_children;

		if !reject_path(
			&current_path,
			&metadata,
			&acceptance_per_rule_kind,
			&mut accept_by_children_dir,
			maybe_to_keep_walking,
		) && accept_by_children_dir.unwrap_or(true)
		{
			accept_path_and_ancestors(
				current_path,
				metadata,
				root,
				&mut accepted,
				iso_file_path_factory,
				&mut accepted_ancestors,
				errors,
			);

			continue;
		}

		if collect_rejected_paths {
			rejected.push(current_path);
		}
	}

	(accepted, accepted_ancestors, rejected)
}

#[instrument(skip_all, fields(current_path = %current_path.display()))]
fn reject_path(
	current_path: &Path,
	metadata: &InnerMetadata,
	acceptance_per_rule_kind: &HashMap<RuleKind, Vec<bool>>,
	accept_by_children_dir: &mut Option<bool>,
	maybe_to_keep_walking: &mut Option<Vec<ToWalkEntry>>,
) -> bool {
	IndexerRuler::rejected_by_reject_glob(acceptance_per_rule_kind)
		|| IndexerRuler::rejected_by_git_ignore(acceptance_per_rule_kind)
		|| (metadata.is_dir()
			&& process_and_maybe_reject_by_directory_rules(
				current_path,
				acceptance_per_rule_kind,
				accept_by_children_dir,
				maybe_to_keep_walking,
			)) || IndexerRuler::rejected_by_accept_glob(acceptance_per_rule_kind)
}

fn process_and_maybe_reject_by_directory_rules(
	current_path: &Path,
	acceptance_per_rule_kind: &HashMap<RuleKind, Vec<bool>>,
	accept_by_children_dir: &mut Option<bool>,
	maybe_to_keep_walking: &mut Option<Vec<ToWalkEntry>>,
) -> bool {
	// If it is a directory, first we check if we must reject it and its children entirely
	if IndexerRuler::rejected_by_children_directories(acceptance_per_rule_kind) {
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
				"Rejected because it didn't passed in any \
				`RuleKind::AcceptIfChildrenDirectoriesArePresent` rule",
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

fn accept_path_and_ancestors(
	current_path: PathBuf,
	metadata: InnerMetadata,
	root: &Path,
	accepted: &mut HashMap<PathBuf, InnerMetadata>,
	iso_file_path_factory: &impl IsoFilePathFactory,
	accepted_ancestors: &mut HashMap<IsolatedFilePathData<'static>, PathBuf>,
	errors: &mut Vec<NonCriticalError>,
) {
	// If the ancestors directories wasn't indexed before, now we do
	for ancestor in current_path
		.ancestors()
		.skip(1) // Skip the current directory as it was already indexed
		.take_while(|&ancestor| ancestor != root)
	{
		if let Ok(iso_file_path) = iso_file_path_factory.build(ancestor, true).map_err(|e| {
			errors.push(indexer::NonCriticalIndexerError::IsoFilePath(e.to_string()).into());
		}) {
			match accepted_ancestors.entry(iso_file_path) {
				Entry::Occupied(_) => {
					// If we already accepted this ancestor, then it will contain
					// also all if its ancestors too, so we can stop here
					break;
				}
				Entry::Vacant(entry) => {
					trace!(ancestor = %ancestor.display(), "Accepted ancestor");
					entry.insert(ancestor.to_path_buf());
				}
			}
		}
	}

	accepted.insert(current_path, metadata);
}
