//! File-operation rows of the entries drop and file operations acceptance
//! matrix (`docs/core/acceptance/entries-drop-and-file-operations.md`), on
//! the three-folder fixture: preflight writes nothing, previews match what
//! the jobs then do, dedupe keeps a copy whose bytes differ, a dated batch
//! rename runs and undoes, a mirror's deletes are the reversed comparison,
//! the trash reports where an item went, and every file a job touched is in
//! its journal.

use std::{
	collections::BTreeMap,
	path::{Path, PathBuf},
	time::{Duration, Instant},
};

use sd_store::file::FileKind;

use super::{
	copy::{
		action::FileCopyAction,
		input::{CopyMethod, FileCopyInput},
		job::FileCopyJob,
	},
	delete::{
		DeleteJob, DeleteMode, DeleteTargets, Duplicates, FileDeleteAction, FileDeleteInput, Keep,
	},
	fixture::{Entry, Fixture, T},
	merge::{FileMergeAction, FileMergeInput, FolderMergeJob, MergeConflictPolicy},
	plan::{ChangeKind, FsPlan},
	rename::{FileRenameBatchAction, FileRenameBatchInput, RenameJob, RenameRule},
	trash_view::{TrashListInput, TrashListQuery},
	undo::{FileUndoAction, FileUndoInput, UndoJob},
};
use crate::{
	domain::{SdPath, SdPathBatch},
	infra::{
		action::{
			preflight::{PreviewableAction, ValidatedAction},
			LibraryAction,
		},
		job::{
			journal::Effect,
			output::JobOutput,
			traits::{Job, JobHandler},
			types::JobId,
		},
		query::LibraryQuery,
	},
	ops::paths::compare::{CompareBy, CompareSet, Comparison, PathCompareInput, PathCompareQuery},
};

async fn run<J>(fixture: &Fixture, job: J) -> (JobId, JobOutput)
where
	J: Job + JobHandler + crate::infra::job::types::ErasedJob + crate::infra::job::traits::DynJob,
{
	let handle = fixture
		.library
		.jobs()
		.dispatch(job)
		.await
		.expect("dispatched");
	let id = handle.id();
	let output = tokio::time::timeout(Duration::from_secs(60), handle.wait())
		.await
		.expect("in time")
		.expect("completed");
	(id, output)
}

async fn journal(fixture: &Fixture, job: JobId) -> Vec<Effect> {
	fixture
		.library
		.jobs()
		.database()
		.journal(job)
		.await
		.expect("journal")
		.into_iter()
		.map(|recorded| recorded.effect)
		.collect()
}

async fn undo(fixture: &Fixture, job: JobId) -> serde_json::Value {
	let input = FileUndoInput {
		job: job.0,
		effects: None,
	};
	let validation = FileUndoAction::validate(&input, &fixture.preview())
		.await
		.expect("validated");
	assert!(!validation.refuses(), "{:?}", validation.findings);
	let (_, output) = run(fixture, UndoJob::new(job.0, None)).await;
	let JobOutput::Custom(value) = output else {
		panic!("not an undo output: {output:?}");
	};
	value
}

/// Every file beneath `root`, by relative path, with its bytes.
fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
	fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
		let Ok(entries) = std::fs::read_dir(dir) else {
			return;
		};
		for entry in entries.flatten() {
			let path = entry.path();
			let relative = path
				.strip_prefix(root)
				.expect("beneath root")
				.to_string_lossy()
				.replace(std::path::MAIN_SEPARATOR, "/");
			let meta = std::fs::symlink_metadata(&path).expect("metadata");
			if meta.is_dir() {
				out.insert(format!("{relative}/"), Vec::new());
				walk(root, &path, out);
			} else if meta.is_symlink() {
				out.insert(relative, b"<link>".to_vec());
			} else {
				out.insert(relative, std::fs::read(&path).expect("bytes"));
			}
		}
	}
	let mut out = BTreeMap::new();
	walk(root, root, &mut out);
	out
}

fn change_at(plan: &FsPlan, path: &Path) -> Option<ChangeKind> {
	plan.changes
		.iter()
		.find(|change| change.path.path().map(PathBuf::as_path) == Some(path))
		.map(|change| change.change.clone())
}

fn copy_input(sources: &[&Path], destination: &Path) -> FileCopyInput {
	FileCopyInput {
		sources: SdPathBatch {
			paths: sources.iter().map(|path| SdPath::local(path)).collect(),
		},
		destination: SdPath::local(destination),
		overwrite: false,
		verify_checksum: false,
		preserve_timestamps: true,
		move_files: false,
		copy_method: CopyMethod::Auto,
		on_conflict: None,
	}
}

/// The job `files.copy` dispatches for `input`, built the way
/// `FileCopyAction::execute` builds it, so the preview and the job see the
/// same options.
fn copy_job(input: FileCopyInput) -> FileCopyJob {
	let action = <FileCopyAction as LibraryAction>::from_input(input).expect("a valid copy input");
	let options = action.options.clone();
	FileCopyJob::new(action.sources, action.destination).with_options(options)
}

fn merge_input(fixture: &Fixture, policy: MergeConflictPolicy) -> FileMergeInput {
	FileMergeInput {
		sources: SdPathBatch {
			paths: vec![SdPath::local(&fixture.source)],
		},
		destination: SdPath::local(&fixture.destination),
		on_conflict: policy,
		consume_sources: false,
		remove_extras: false,
	}
}

/// The source and destination trees most rows here start from: creates, a
/// shared file, a differing file, a nested folder, and an extra only the
/// destination holds.
fn source_tree() -> [Entry<'static>; 4] {
	let file = FileKind::File;
	[
		("same.txt", file, 4, T, Some("s"), None, None),
		("diff.txt", file, 3, T + 10, Some("d1"), None, None),
		("new.txt", file, 7, T, Some("n"), None, None),
		("sub/inner.txt", file, 2, T, Some("i"), None, None),
	]
}

fn destination_tree() -> [Entry<'static>; 3] {
	let file = FileKind::File;
	[
		("same.txt", file, 4, T, Some("s"), None, None),
		("diff.txt", file, 5, T, Some("d2"), None, None),
		("extra.txt", file, 8, T, Some("e"), None, None),
	]
}

async fn populated() -> Fixture {
	let fixture = Fixture::new().await;
	fixture.materialize(&fixture.source, &source_tree());
	fixture.index(&fixture.source, &source_tree()).await;
	fixture.materialize(&fixture.destination, &destination_tree());
	fixture
		.index(&fixture.destination, &destination_tree())
		.await;
	fixture
}

/// Each fixture store's revision after its queued writes are committed, so
/// an observation a preflight path enqueued would show up here.
async fn revisions(fixture: &Fixture) -> Vec<sd_store::revision::Revision> {
	let mut out = Vec::new();
	for root in [&fixture.source, &fixture.other, &fixture.destination] {
		let store = fixture
			.core
			.context
			.volume_index()
			.store_for(root)
			.await
			.expect("store");
		store.flush().await.expect("flushed");
		out.push(store.db().revision().await.expect("revision"));
	}
	out
}

/// Action previews acceptance "confirm neither method made a filesystem
/// write" and V1's "preflight calls perform no writes": validate and preview
/// for copy, move, merge, delete, dedupe and batch rename over the fixture
/// leave every byte on disk and every store revision where it was.
#[tokio::test]
async fn preflight_makes_no_filesystem_write_and_moves_no_revision() {
	let fixture = populated().await;
	let disk_before = (
		tree(&fixture.source),
		tree(&fixture.other),
		tree(&fixture.destination),
	);
	let revisions_before = revisions(&fixture).await;

	let ctx = fixture.preview();
	let copy = copy_input(&[&fixture.source], &fixture.destination);
	FileCopyAction::validate(&copy, &ctx)
		.await
		.expect("validated");
	FileCopyAction::preview(copy.clone(), &ctx)
		.await
		.expect("planned");
	let mut moving = copy_input(&[&fixture.source.join("new.txt")], &fixture.other);
	moving.move_files = true;
	FileCopyAction::validate(&moving, &ctx)
		.await
		.expect("validated");
	FileCopyAction::preview(moving, &ctx)
		.await
		.expect("planned");
	for policy in [
		MergeConflictPolicy::Skip,
		MergeConflictPolicy::Overwrite,
		MergeConflictPolicy::KeepBoth,
	] {
		let mut merge = merge_input(&fixture, policy);
		merge.remove_extras = true;
		FileMergeAction::validate(&merge, &ctx)
			.await
			.expect("validated");
		FileMergeAction::preview(merge, &ctx)
			.await
			.expect("planned");
	}
	let delete = FileDeleteInput {
		targets: DeleteTargets::Paths {
			paths: vec![SdPath::local(fixture.destination.join("extra.txt"))],
		},
		permanent: false,
		recursive: true,
	};
	FileDeleteAction::validate(&delete, &ctx)
		.await
		.expect("validated");
	FileDeleteAction::preview(delete, &ctx)
		.await
		.expect("planned");
	let dedupe = FileDeleteInput {
		targets: DeleteTargets::Duplicates {
			duplicates: Duplicates {
				scope: Some(SdPath::local(&fixture.destination)),
				keep: Keep::First,
				min_size: None,
			},
		},
		permanent: false,
		recursive: true,
	};
	FileDeleteAction::validate(&dedupe, &ctx)
		.await
		.expect("validated");
	FileDeleteAction::preview(dedupe, &ctx)
		.await
		.expect("planned");
	let rename = FileRenameBatchInput {
		targets: vec![
			SdPath::local(fixture.source.join("same.txt")),
			SdPath::local(fixture.source.join("diff.txt")),
		],
		rules: vec![RenameRule::Sequence {
			pattern: "file_{n:03}".into(),
			start: 1,
			step: 1,
		}],
	};
	FileRenameBatchAction::validate(&rename, &ctx)
		.await
		.expect("validated");
	FileRenameBatchAction::preview(rename, &ctx)
		.await
		.expect("planned");

	assert_eq!(
		(
			tree(&fixture.source),
			tree(&fixture.other),
			tree(&fixture.destination)
		),
		disk_before,
		"no byte on disk changed"
	);
	assert_eq!(
		revisions(&fixture).await,
		revisions_before,
		"no store revision moved"
	);
}

/// The projection a plan makes, applied to the tree a root held before the
/// job: what exists afterwards, by relative path.
fn apply_plan(plan: &FsPlan, root: &Path, before: &BTreeMap<String, Vec<u8>>) -> Vec<String> {
	let mut after: Vec<String> = before
		.keys()
		.filter(|k| !k.ends_with('/'))
		.cloned()
		.collect();
	for change in &plan.changes {
		let Some(path) = change.path.path() else {
			continue;
		};
		let Ok(relative) = path.strip_prefix(root) else {
			continue;
		};
		let relative = relative
			.to_string_lossy()
			.replace(std::path::MAIN_SEPARATOR, "/");
		match &change.change {
			ChangeKind::Create { .. } | ChangeKind::Replace { .. } => {
				if !after.contains(&relative) {
					after.push(relative);
				}
			}
			ChangeKind::Move { .. } => {
				if !after.contains(&relative) {
					after.push(relative);
				}
			}
			ChangeKind::Delete { .. } => after.retain(|p| p != &relative),
			ChangeKind::CreateDirectory
			| ChangeKind::MergeInto
			| ChangeKind::Skip { .. }
			| ChangeKind::Conflict { .. }
			| ChangeKind::SetAttributes { .. } => {}
		}
	}
	after.sort();
	after
}

fn files_in(root: &Path) -> Vec<String> {
	let mut files: Vec<String> = tree(root)
		.into_keys()
		.filter(|k| !k.ends_with('/'))
		.collect();
	files.sort();
	files
}

/// Brief row "copy/move/merge/delete preflight validate: and preview: match
/// execution outcomes on the fixture": for each operation the preview's
/// projection of the destination, and of a consumed source, is exactly
/// the set of files the job leaves there.
#[tokio::test]
async fn preview_rows_match_execution_for_copy_move_merge_and_delete() {
	// Copy a folder into the destination.
	let fixture = populated().await;
	let ctx = fixture.preview();
	let copy = copy_input(&[&fixture.source.join("sub")], &fixture.destination);
	let validation = FileCopyAction::validate(&copy, &ctx)
		.await
		.expect("validated");
	assert!(!validation.refuses(), "{:?}", validation.findings);
	let plan = FileCopyAction::preview(copy.clone(), &ctx)
		.await
		.expect("planned");
	let projected = apply_plan(&plan, &fixture.destination, &tree(&fixture.destination));
	run(&fixture, copy_job(copy)).await;
	assert_eq!(files_in(&fixture.destination), projected, "copy");
	assert!(projected.contains(&"sub/inner.txt".to_string()));

	// Move a file into another folder.
	let fixture = populated().await;
	let ctx = fixture.preview();
	let mut moving = copy_input(&[&fixture.source.join("new.txt")], &fixture.other);
	moving.move_files = true;
	let plan = FileCopyAction::preview(moving.clone(), &ctx)
		.await
		.expect("planned");
	assert_eq!(
		change_at(&plan, &fixture.other.join("new.txt")),
		Some(ChangeKind::Move {
			from: SdPath::local(fixture.source.join("new.txt"))
		})
	);
	let other_projected = apply_plan(&plan, &fixture.other, &tree(&fixture.other));
	let source_before = files_in(&fixture.source);
	run(&fixture, copy_job(moving)).await;
	assert_eq!(
		files_in(&fixture.other),
		other_projected,
		"move: destination"
	);
	assert_eq!(
		files_in(&fixture.source),
		source_before
			.into_iter()
			.filter(|f| f != "new.txt")
			.collect::<Vec<_>>(),
		"move: source"
	);

	// Merge with every leaf kind, skipping what differs.
	let fixture = populated().await;
	let ctx = fixture.preview();
	let merge = merge_input(&fixture, MergeConflictPolicy::Skip);
	let plan = FileMergeAction::preview(merge.clone(), &ctx)
		.await
		.expect("planned");
	let dst = &fixture.destination;
	assert_eq!(
		change_at(&plan, &dst.join("new.txt")),
		Some(ChangeKind::Create { size: 7 })
	);
	assert!(matches!(
		change_at(&plan, &dst.join("diff.txt")),
		Some(ChangeKind::Skip { .. })
	));
	let projected = apply_plan(&plan, dst, &tree(dst));
	let diff_before = std::fs::read(dst.join("diff.txt")).expect("bytes");
	run(&fixture, FolderMergeJob::new(merge)).await;
	assert_eq!(files_in(dst), projected, "merge");
	assert_eq!(
		std::fs::read(dst.join("diff.txt")).expect("bytes"),
		diff_before,
		"a skipped leaf is untouched"
	);

	// Delete named paths.
	let fixture = populated().await;
	let ctx = fixture.preview();
	let targets = DeleteTargets::Paths {
		paths: vec![
			SdPath::local(fixture.destination.join("extra.txt")),
			SdPath::local(fixture.destination.join("diff.txt")),
		],
	};
	let delete = FileDeleteInput {
		targets: targets.clone(),
		permanent: true,
		recursive: true,
	};
	let plan = FileDeleteAction::preview(delete, &ctx)
		.await
		.expect("planned");
	assert_eq!(plan.summary.deletes.files, 2);
	let projected = apply_plan(&plan, &fixture.destination, &tree(&fixture.destination));
	run(&fixture, DeleteJob::new(targets, DeleteMode::Permanent)).await;
	assert_eq!(files_in(&fixture.destination), projected, "delete");
	assert_eq!(projected, vec!["same.txt".to_string()]);
}

/// Release gate "Candidate content does not authorize deletion" and the
/// brief row "dedupe refuses to delete when integrity hashes are absent or
/// mismatched": two files the index pairs on a sampled hash alone, whose
/// bytes on disk differ, are both kept, and the job says which one differed.
/// SPAC-19's `dedupe_own_hash_test` covers the store-side half, a sampled
/// write landing on a confirmed row.
#[tokio::test]
async fn dedupe_keeps_a_copy_whose_bytes_differ_from_its_keeper() {
	let fixture = Fixture::new().await;
	let file = FileKind::File;
	let tree = [
		("a/x.bin", file, 4, T, Some("h1"), None, None),
		("b/x.bin", file, 4, T, Some("h1"), None, None),
		("c/y.bin", file, 4, T, Some("h2"), None, None),
		("d/y.bin", file, 4, T, Some("h2"), None, None),
	];
	fixture.materialize(&fixture.source, &tree);
	fixture.index(&fixture.source, &tree).await;
	// The index is wrong about b: its bytes are not a's. c and d are real
	// copies.
	std::fs::write(fixture.source.join("b/x.bin"), b"yyyy").expect("write");

	let targets = DeleteTargets::Duplicates {
		duplicates: Duplicates {
			scope: Some(SdPath::local(&fixture.source)),
			keep: Keep::First,
			min_size: None,
		},
	};
	let input = FileDeleteInput {
		targets: targets.clone(),
		permanent: true,
		recursive: true,
	};
	let ctx = fixture.preview();
	let validation = FileDeleteAction::validate(&input, &ctx)
		.await
		.expect("validated");
	assert!(!validation.refuses(), "{:?}", validation.findings);
	let plan = FileDeleteAction::preview(input, &ctx)
		.await
		.expect("planned");
	assert_eq!(
		plan.summary.deletes.files, 2,
		"the index, trusted alone, would remove both second copies"
	);

	let (_, output) = run(&fixture, DeleteJob::new(targets, DeleteMode::Permanent)).await;
	let JobOutput::FileDelete {
		deleted_count,
		skipped_count,
		..
	} = output
	else {
		panic!("not a delete output: {output:?}");
	};
	assert_eq!(deleted_count, 1, "only the proven copy went");
	assert_eq!(
		skipped_count, 1,
		"the differing copy was skipped, not failed"
	);
	assert!(fixture.source.join("a/x.bin").exists());
	assert!(
		fixture.source.join("b/x.bin").exists(),
		"the copy whose bytes differ stays"
	);
	assert!(fixture.source.join("c/y.bin").exists());
	assert!(!fixture.source.join("d/y.bin").exists());
}

/// File operations acceptance "batch rename one day's photos to a dated
/// sequence and undo it": the preview names every new name, the job
/// produces exactly those, and undo puts the originals back.
#[tokio::test]
async fn a_dated_batch_rename_runs_as_previewed_and_undoes() {
	let fixture = Fixture::new().await;
	let file = FileKind::File;
	let tree = [
		("IMG_0007.JPG", file, 4, T, Some("7"), None, None),
		("IMG_0008.JPG", file, 3, T, Some("8"), None, None),
		("IMG_0009.JPG", file, 2, T, Some("9"), None, None),
	];
	fixture.materialize(&fixture.source, &tree);
	fixture.index(&fixture.source, &tree).await;
	let originals = files_in(&fixture.source);

	let input = FileRenameBatchInput {
		targets: tree
			.iter()
			.map(|(name, ..)| SdPath::local(fixture.source.join(name)))
			.collect(),
		rules: vec![
			RenameRule::Sequence {
				pattern: "{date:%Y-%m-%d}_{n:03}".into(),
				start: 1,
				step: 1,
			},
			RenameRule::Case {
				stem: super::rename::CaseRule::Keep,
				extension: super::rename::ExtensionCase::Lower,
			},
		],
	};
	let ctx = fixture.preview();
	let validation = FileRenameBatchAction::validate(&input, &ctx)
		.await
		.expect("validated");
	assert!(!validation.refuses(), "{:?}", validation.findings);
	let plan = FileRenameBatchAction::preview(input.clone(), &ctx)
		.await
		.expect("planned");
	assert_eq!(plan.summary.moves.files, 3);
	let mut previewed: Vec<String> = plan
		.changes
		.iter()
		.filter(|change| matches!(change.change, ChangeKind::Move { .. }))
		.filter_map(|change| change.path.path())
		.map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
		.collect();
	previewed.sort();
	for (name, n) in previewed.iter().zip(1..) {
		let (date, rest) = name.split_at(10);
		assert!(
			date.len() == 10 && date.as_bytes()[4] == b'-' && date.as_bytes()[7] == b'-',
			"{name}"
		);
		assert_eq!(rest, format!("_{n:03}.jpg"), "{name}");
	}

	let (job, output) = run(&fixture, RenameJob::ruled(input.targets, input.rules)).await;
	let JobOutput::Custom(value) = output else {
		panic!("not a rename output: {output:?}");
	};
	assert_eq!(value["renamed"], 3);
	assert_eq!(
		files_in(&fixture.source),
		previewed,
		"the job did what the preview said"
	);
	assert_eq!(journal(&fixture, job).await.len(), 3);

	let undone = undo(&fixture, job).await;
	assert_eq!(undone["reversed"], 3, "{undone}");
	assert_eq!(files_in(&fixture.source), originals);
}

/// File operations acceptance "mirror the folder with extras removed,
/// confirm the plan's deletes match `file compare` reversed": every delete
/// the mirror plans is a file the path comparison lists as only in the
/// destination, and nothing else; after the job those files are gone and
/// the rest of the destination matches the source.
#[tokio::test]
async fn a_mirrors_deletes_are_the_reversed_comparison() {
	let fixture = populated().await;
	let ctx = fixture.preview();
	let mut mirror = merge_input(&fixture, MergeConflictPolicy::Overwrite);
	mirror.remove_extras = true;
	let plan = FileMergeAction::preview(mirror.clone(), &ctx)
		.await
		.expect("planned");
	let mut planned_deletes: Vec<String> = plan
		.changes
		.iter()
		.filter(|change| matches!(change.change, ChangeKind::Delete { .. }))
		.filter_map(|change| change.path.path())
		.map(|path| {
			path.strip_prefix(&fixture.destination)
				.expect("in the destination")
				.to_string_lossy()
				.replace(std::path::MAIN_SEPARATOR, "/")
		})
		.collect();
	planned_deletes.sort();

	let compared = PathCompareQuery::from_input(PathCompareInput {
		comparison: Comparison {
			a: SdPath::local(&fixture.destination),
			b: SdPath::local(&fixture.source),
			by: CompareBy::Path,
			show: CompareSet::OnlyA,
			include_hidden: false,
		},
		after: None,
		limit: 100,
	})
	.expect("input")
	.execute(fixture.core.context.clone(), fixture.session())
	.await
	.expect("compared");
	let mut only_destination: Vec<String> = compared
		.entries
		.iter()
		.map(|entry| entry.path.clone())
		.collect();
	only_destination.sort();
	assert_eq!(planned_deletes, only_destination);
	assert_eq!(planned_deletes, vec!["extra.txt".to_string()]);

	run(&fixture, FolderMergeJob::new(mirror)).await;
	assert_eq!(files_in(&fixture.destination), files_in(&fixture.source));
	assert_eq!(
		std::fs::read(fixture.destination.join("diff.txt")).expect("bytes"),
		std::fs::read(fixture.source.join("diff.txt")).expect("bytes"),
		"overwrite replaced the differing file"
	);
}

/// Action previews acceptance "no destination file changed without a
/// `Replace` or `KeepBoth` entry saying so": after a merge, every
/// destination file whose bytes changed has a `Replace` row, and every
/// file the plan did not name holds its previous bytes.
#[tokio::test]
async fn a_merge_changes_no_destination_file_the_plan_did_not_name() {
	let fixture = populated().await;
	let ctx = fixture.preview();
	let merge = merge_input(&fixture, MergeConflictPolicy::Overwrite);
	let plan = FileMergeAction::preview(merge.clone(), &ctx)
		.await
		.expect("planned");
	let before = tree(&fixture.destination);
	run(&fixture, FolderMergeJob::new(merge)).await;
	let after = tree(&fixture.destination);

	for (relative, bytes) in &before {
		if relative.ends_with('/') {
			continue;
		}
		let Some(now) = after.get(relative) else {
			panic!("{relative} vanished from a non-consuming merge");
		};
		if now != bytes {
			let path = fixture.destination.join(relative);
			assert!(
				matches!(change_at(&plan, &path), Some(ChangeKind::Replace { .. })),
				"{relative} changed without a Replace row: {:?}",
				change_at(&plan, &path)
			);
		}
	}
	assert_ne!(
		before["diff.txt"], after["diff.txt"],
		"the overwrite happened"
	);
	assert_eq!(before["same.txt"], after["same.txt"]);
	assert_eq!(before["extra.txt"], after["extra.txt"]);
}

/// File operations acceptance "trash a file ... and restore each from the
/// trash view", on this platform's trash: the delete journals where the
/// item went, the trash view lists it there as present, and undo brings
/// the bytes back.
///
/// Passes on macOS, where `NSFileManager` answers with the item's location.
/// On Linux and Windows the `trash` crate's item id is the `.trashinfo`
/// path, which `trash::restore` treats as the item itself and renames onto
/// the original path, so the restored file holds the trashinfo text.
#[cfg_attr(
	not(target_os = "macos"),
	ignore = "FDA: trash restore on Linux and Windows: the journaled location is the .trashinfo file and undo renames it over the original, so the restored bytes are the trashinfo text"
)]
#[tokio::test]
async fn a_trashed_file_is_listed_with_its_location_and_comes_back() {
	let fixture = Fixture::new().await;
	let tree = [("gone.txt", FileKind::File, 4, T, Some("g"), None, None)];
	fixture.materialize(&fixture.source, &tree);
	fixture.index(&fixture.source, &tree).await;
	let gone = fixture.source.join("gone.txt");

	let (job, _) = run(
		&fixture,
		DeleteJob::new(
			DeleteTargets::Paths {
				paths: vec![SdPath::local(&gone)],
			},
			DeleteMode::Trash,
		),
	)
	.await;
	let effects = journal(&fixture, job).await;
	let [Effect::Trashed {
		to: Some(location), ..
	}] = effects.as_slice()
	else {
		panic!("not a trashing with a location: {effects:?}");
	};
	assert!(!gone.exists());

	let listed = TrashListQuery::from_input(TrashListInput { limit: None })
		.expect("input")
		.execute(fixture.core.context.clone(), fixture.session())
		.await
		.expect("listed");
	let item = listed
		.items
		.iter()
		.find(|item| item.from == gone)
		.expect("the trash view lists the file");
	assert_eq!(&item.location, location);
	assert!(item.present, "the item is at its recorded location");
	assert_eq!(item.job, job.0);

	let output = undo(&fixture, job).await;
	assert_eq!(output["reversed"], 1, "{output}");
	assert_eq!(
		std::fs::read(&gone).expect("restored"),
		b"xxxx",
		"the restored file holds its bytes"
	);
}

/// File operations acceptance "confirm no operation left a file the journal
/// does not account for": after a copy, a rename and a permanent delete,
/// every path that appeared is, or sits under, a journaled creation or move
/// target, and every path that vanished is, or sat under, a journaled move
/// source or removal. A folder copy journals the folder it created, which
/// accounts for the files inside it the way undo removes them.
#[tokio::test]
async fn every_file_a_job_touched_is_in_its_journal() {
	let fixture = populated().await;
	let before = (tree(&fixture.source), tree(&fixture.destination));

	let (copy, _) = run(
		&fixture,
		FileCopyJob::new(
			SdPathBatch::new(vec![SdPath::local(fixture.source.join("sub"))]),
			SdPath::local(&fixture.destination),
		),
	)
	.await;
	let (rename, _) = run(
		&fixture,
		RenameJob::named(
			SdPath::local(fixture.source.join("new.txt")),
			"renamed.txt".to_string(),
		),
	)
	.await;
	let (delete, _) = run(
		&fixture,
		DeleteJob::new(
			DeleteTargets::Paths {
				paths: vec![SdPath::local(fixture.destination.join("extra.txt"))],
			},
			DeleteMode::Permanent,
		),
	)
	.await;

	let mut created = Vec::new();
	let mut removed = Vec::new();
	for job in [copy, rename, delete] {
		for effect in journal(&fixture, job).await {
			match effect {
				Effect::Created { path, .. } => created.push(path),
				Effect::Moved { from, to, .. } => {
					removed.push(from);
					created.push(to);
				}
				Effect::Trashed { from, .. } => removed.push(from),
				Effect::Replaced { path, .. } => created.push(path),
				Effect::Removed { path } => removed.push(path),
				Effect::Attributes { .. } => {}
			}
		}
	}

	let accounted = |path: &Path, effects: &[PathBuf]| {
		effects
			.iter()
			.any(|effect| path == effect || path.starts_with(effect))
	};
	let after = (tree(&fixture.source), tree(&fixture.destination));
	for (root, before, after) in [
		(&fixture.source, &before.0, &after.0),
		(&fixture.destination, &before.1, &after.1),
	] {
		for relative in after.keys().filter(|k| !before.contains_key(*k)) {
			let path = root.join(relative.trim_end_matches('/'));
			assert!(
				accounted(&path, &created),
				"{} appeared without a journal entry: {created:?}",
				path.display()
			);
		}
		for relative in before.keys().filter(|k| !after.contains_key(*k)) {
			let path = root.join(relative.trim_end_matches('/'));
			assert!(
				accounted(&path, &removed),
				"{} vanished without a journal entry: {removed:?}",
				path.display()
			);
		}
	}
	assert!(
		created.len() >= 2 && removed.len() >= 2,
		"{created:?} {removed:?}"
	);
}

/// File operations acceptance "measure preview time for a batch rename over
/// the largest folder available", CI-sized: a thousand files preview in
/// one call. The measurement is printed; the ceiling is loose enough that
/// only a pathological preview, not a busy runner, trips it (locally the
/// debug build answers in about 100 ms).
#[tokio::test]
async fn a_batch_rename_preview_over_a_thousand_files_answers_within_the_ceiling() {
	let fixture = Fixture::new().await;
	let names: Vec<String> = (0..1000).map(|n| format!("IMG_{n:04}.JPG")).collect();
	let entries: Vec<Entry<'_>> = names
		.iter()
		.map(|name| (name.as_str(), FileKind::File, 1, T, None, None, None))
		.collect();
	fixture.materialize(&fixture.source, &entries);
	fixture.index(&fixture.source, &entries).await;

	let input = FileRenameBatchInput {
		targets: names
			.iter()
			.map(|name| SdPath::local(fixture.source.join(name)))
			.collect(),
		rules: vec![RenameRule::Sequence {
			pattern: "{date:%Y-%m-%d}_{n:04}".into(),
			start: 1,
			step: 1,
		}],
	};
	let ctx = fixture.preview();
	let started = Instant::now();
	let validation = FileRenameBatchAction::validate(&input, &ctx)
		.await
		.expect("validated");
	let plan = FileRenameBatchAction::preview(input, &ctx)
		.await
		.expect("planned");
	let elapsed = started.elapsed();
	assert!(!validation.refuses(), "{:?}", validation.findings);
	assert_eq!(plan.summary.moves.files, 1000);
	println!("batch rename validate and preview over 1000 files: {elapsed:?}");
	assert!(
		elapsed < Duration::from_secs(60),
		"validate and preview took {elapsed:?} for 1000 files"
	);
}

/// File operations acceptance "undo the mirror": the extras the mirror
/// trashed come back from their recorded locations, the replaced file gets
/// its previous bytes, and the file the mirror created is removed.
///
/// Rests on trash restore, so it is ignored where
/// `a_trashed_file_is_listed_with_its_location_and_comes_back` is.
#[cfg_attr(
	not(target_os = "macos"),
	ignore = "FDA: undo of a mirror on Linux and Windows: restoring the trashed extras renames their .trashinfo files over the originals"
)]
#[tokio::test]
async fn undoing_a_mirror_restores_the_extras_and_the_replaced_bytes() {
	let fixture = populated().await;
	let before = tree(&fixture.destination);
	let mut mirror = merge_input(&fixture, MergeConflictPolicy::Overwrite);
	mirror.remove_extras = true;
	let (job, _) = run(&fixture, FolderMergeJob::new(mirror)).await;
	assert!(!fixture.destination.join("extra.txt").exists());

	let output = undo(&fixture, job).await;
	assert_eq!(output["left"].as_array().map(Vec::len), Some(0), "{output}");
	assert_eq!(tree(&fixture.destination), before);
}
