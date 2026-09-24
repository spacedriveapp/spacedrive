//! Whether and how a copy or move would run, and what would exist after.
//!
//! Each source is written at its own name in the destination, or at the
//! destination itself when there is one source and the destination is a new
//! name. A folder source plans like a merge into that place: the same engine
//! sorts every path beneath it, so a copy onto an existing folder shows the
//! files it would replace instead of silently merging. A move plans the same
//! way with the source consumed, since that is the end state whether
//! the volume renames in place or the job copies and deletes; validation
//! says which, as on one volume the records keep their identity.

use std::path::{Path, PathBuf};

use super::{
	action::{FileConflictResolution, FileCopyAction},
	database::CopyDatabaseQuery,
	input::FileCopyInput,
	routing::CopyStrategyRouter,
};
use crate::{
	domain::SdPath,
	infra::action::{
		error::ActionError,
		preflight::{
			ExecutionFacts, Finding, PreviewContext, PreviewableAction, ValidatedAction, Validation,
		},
	},
	ops::files::{
		merge::MergeConflictPolicy,
		plan::{ChangeKind, FsPlan, ReplaceReason, SkipReason},
		planner::Planner,
	},
};

pub const NO_SOURCES: &str = "copy.no_sources";
pub const CROSS_DEVICE: &str = "copy.cross_device";
pub const SOURCE_MISSING: &str = "copy.source_missing";
pub const DESTINATION_MISSING: &str = "copy.destination_missing";
pub const CYCLE: &str = "copy.cycle";
pub const FOLDER_COLLISION: &str = "copy.folder_collision";
pub const SPACE: &str = "copy.space";
pub const ATOMIC: &str = "move.atomic";
pub const IDENTITY_LOSS: &str = "move.identity_loss";

impl ValidatedAction for FileCopyAction {
	async fn validate(
		input: &FileCopyInput,
		ctx: &PreviewContext,
	) -> Result<Validation, ActionError> {
		validate(input, ctx).await
	}
}

impl PreviewableAction for FileCopyAction {
	type Plan = FsPlan;

	async fn preview(input: FileCopyInput, ctx: &PreviewContext) -> Result<FsPlan, ActionError> {
		preview(&input, ctx).await
	}
}

crate::register_validate!(FileCopyAction, "files.copy");
crate::register_preview!(FileCopyAction, "files.copy");

/// Where each source is written: at its own name in the destination, or at the
/// destination itself for one source given a new name.
async fn targets(input: &FileCopyInput, destination: &Path) -> Vec<(SdPath, PathBuf)> {
	let destination_is_dir = tokio::fs::metadata(destination)
		.await
		.is_ok_and(|meta| meta.is_dir());
	let as_new_name = input.sources.paths.len() == 1 && !destination_is_dir;
	input
		.sources
		.paths
		.iter()
		.filter_map(|source| {
			let path = source.as_local_path()?;
			let target = if as_new_name {
				destination.to_path_buf()
			} else {
				destination.join(path.file_name()?)
			};
			Some((source.clone(), target))
		})
		.collect()
}

async fn validate(input: &FileCopyInput, ctx: &PreviewContext) -> Result<Validation, ActionError> {
	let mut findings = Vec::new();
	let mut facts = ExecutionFacts {
		executes_on: ctx.executes_on(),
		..Default::default()
	};
	if input.sources.paths.is_empty() {
		findings.push(Finding::error(NO_SOURCES, "nothing to copy"));
		return Ok(Validation { findings, facts });
	}

	if let Some(source) = input.sources.paths.first() {
		let (_, metadata) = CopyStrategyRouter::select_strategy_with_metadata(
			source,
			&input.destination,
			input.move_files,
			&input.copy_method,
			Some(ctx.volumes()),
		)
		.await;
		facts.strategy = Some(metadata.strategy_name);
		if metadata.is_cross_device {
			findings.push(Finding::info(
				CROSS_DEVICE,
				"runs across devices as a transfer; the plan cannot be read from this index",
			));
		}
	}

	let Some(destination) = input.destination.as_local_path() else {
		return Ok(Validation { findings, facts });
	};
	let destination_exists = tokio::fs::metadata(destination).await.is_ok();
	if !destination_exists {
		let parent_exists = destination.parent().is_some_and(|parent| parent.exists());
		if input.sources.paths.len() > 1 || !parent_exists {
			findings.push(
				Finding::error(
					DESTINATION_MISSING,
					"the destination does not exist; several sources need an existing folder",
				)
				.at(input.destination.clone()),
			);
		}
	}

	let mut local = Vec::new();
	for source in &input.sources.paths {
		let Some(path) = source.as_local_path() else {
			continue;
		};
		match tokio::fs::symlink_metadata(path).await {
			Ok(meta) => {
				if meta.is_dir() && destination.starts_with(path) {
					findings.push(
						Finding::error(CYCLE, "a folder cannot be copied into itself")
							.at(source.clone()),
					);
				}
				local.push((source.clone(), path.to_path_buf(), meta));
			}
			Err(_) => findings.push(
				Finding::error(SOURCE_MISSING, "a source is not on this device right now")
					.at(source.clone()),
			),
		}
	}

	let keeps_both = input.on_conflict == Some(FileConflictResolution::AutoModifyName);
	for (source, target) in targets(input, destination).await {
		let is_dir = local
			.iter()
			.any(|(known, _, meta)| *known == source && meta.is_dir());
		if is_dir
			&& tokio::fs::metadata(&target)
				.await
				.is_ok_and(|meta| meta.is_dir())
		{
			findings.push(
				Finding::info(
					FOLDER_COLLISION,
					if keeps_both {
						"a folder of that name is already there; this one is written beside it under a numbered name"
					} else {
						"a folder of that name is already there; the plan merges this one into it"
					},
				)
				.at(SdPath::local(target)),
			);
		}
	}

	// The arena keys its rollups by the volume's spelling of a path.
	let mut spelled = Vec::with_capacity(local.len());
	for (source, path, _) in &local {
		spelled.push(match ctx.volumes().locate_path(path).await {
			Some((_, path)) => SdPath::local(path),
			None => source.clone(),
		});
	}
	let estimates = CopyDatabaseQuery::new(ctx.index())
		.get_estimates_for_paths(&spelled)
		.await
		.ok();
	facts.estimated_files = estimates.as_ref().map(|estimate| estimate.file_count);
	facts.estimated_bytes = estimates.as_ref().map(|estimate| estimate.total_size);

	let destination_volume = ctx.volumes().volume_for_path(destination).await;
	if let (Some(volume), Some(estimate)) = (&destination_volume, &estimates) {
		facts.free_space_after = Some(volume.available_space as i64 - estimate.total_size as i64);
		if estimate.total_size > volume.available_space {
			findings.push(Finding::warning(
				SPACE,
				format!(
					"{} bytes to copy against {} free on the destination volume",
					estimate.total_size, volume.available_space
				),
			));
		}
	}

	if input.move_files && !local.is_empty() {
		let mut stranded = 0;
		let mut same = true;
		let mut known = true;
		for (source, path, _) in &local {
			match same_filesystem(path, destination).await {
				Some(true) => {}
				Some(false) => {
					same = false;
					stranded += assertions_beneath(ctx, source).await;
				}
				None => known = false,
			}
		}
		if known && same {
			findings.push(Finding::info(
				ATOMIC,
				"renamed in place on one volume; records keep their identity",
			));
		} else if stranded > 0 {
			findings.push(Finding::warning(
				IDENTITY_LOSS,
				format!(
					"{stranded} tag assertions on these records stay behind: across volumes a move writes new records at the destination"
				),
			));
		}
	}

	Ok(Validation { findings, facts })
}

async fn preview(input: &FileCopyInput, ctx: &PreviewContext) -> Result<FsPlan, ActionError> {
	let destination = input
		.destination
		.as_local_path()
		.ok_or_else(|| ActionError::InvalidInput("the destination is not on this device".into()))?
		.to_path_buf();
	let policy = match input.on_conflict {
		Some(FileConflictResolution::Overwrite) => MergeConflictPolicy::Overwrite,
		Some(FileConflictResolution::AutoModifyName) => MergeConflictPolicy::KeepBoth,
		_ if input.overwrite => MergeConflictPolicy::Overwrite,
		_ => MergeConflictPolicy::Skip,
	};
	let mut planner = Planner::new(ctx, policy, false);

	for (source, target) in targets(input, &destination).await {
		let Some(path) = source.as_local_path() else {
			return Err(ActionError::InvalidInput(format!(
				"{source} is not on this device; its plan cannot be read from this index"
			)));
		};
		let meta = tokio::fs::symlink_metadata(path).await.map_err(|_| {
			ActionError::InvalidInput(format!("{source} is not on this device right now"))
		})?;
		let existing = tokio::fs::symlink_metadata(&target).await.ok();
		// Keeping both writes the source beside whatever holds its name, so
		// the plan is a create at the numbered name, as the job writes it.
		let (target, existing) = if existing.is_some() && policy == MergeConflictPolicy::KeepBoth {
			(planner.unique_name(&target).await, None)
		} else {
			(target, existing)
		};

		if meta.is_dir() {
			match existing {
				Some(there) if there.is_dir() => {
					planner
						.pair(&source, &SdPath::local(&target), input.move_files)
						.await?;
				}
				Some(_) => planner.note(
					target,
					ChangeKind::Conflict {
						kind: crate::ops::files::plan::ConflictKind::FileVsDirectory,
					},
					0,
				),
				None => {
					planner.note(target.clone(), ChangeKind::CreateDirectory, 0);
					planner
						.pair(&source, &SdPath::local(&target), input.move_files)
						.await?;
				}
			}
			continue;
		}

		let size = meta.len();
		let change = match existing {
			None if input.move_files => ChangeKind::Move { from: source },
			None => ChangeKind::Create { size },
			Some(there) if there.is_dir() => ChangeKind::Conflict {
				kind: crate::ops::files::plan::ConflictKind::FileVsDirectory,
			},
			Some(there) => match policy {
				MergeConflictPolicy::Overwrite => ChangeKind::Replace {
					existing_size: there.len(),
					incoming_size: size,
					reason: ReplaceReason::Overwrite,
				},
				_ => ChangeKind::Skip {
					reason: SkipReason::Policy,
				},
			},
		};
		planner.note(target, change, size);
	}

	Ok(ctx.plans().retain(planner.finish()))
}

/// Whether two paths sit on one filesystem as the OS sees it, which is what
/// decides whether a move is a rename. A destination that does not exist
/// yet answers for the nearest parent that does.
async fn same_filesystem(a: &Path, b: &Path) -> Option<bool> {
	Some(filesystem_of(a).await? == filesystem_of(b).await?)
}

async fn filesystem_of(path: &Path) -> Option<u64> {
	let mut path = path.to_path_buf();
	loop {
		match tokio::fs::metadata(&path).await {
			Ok(meta) => {
				#[cfg(unix)]
				{
					use std::os::unix::fs::MetadataExt;
					return Some(meta.dev());
				}
				#[cfg(not(unix))]
				{
					use std::hash::{Hash, Hasher};
					let _ = meta;
					let mut hasher = std::collections::hash_map::DefaultHasher::new();
					path.components().next()?.hash(&mut hasher);
					return Some(hasher.finish());
				}
			}
			Err(_) => path = path.parent()?.to_path_buf(),
		}
	}
}

/// How many tag assertions stand on records under a source root.
async fn assertions_beneath(ctx: &PreviewContext, source: &SdPath) -> i64 {
	let mut count = 0;
	for reach in ctx.reach(source).await {
		let Some(db) = ctx.index().read_store(reach.source.id).await else {
			continue;
		};
		count += sd_store::read::assertions_beneath(db.pool(), &reach.scope)
			.await
			.unwrap_or(0);
	}
	count
}

#[cfg(test)]
mod tests {
	use sd_store::file::FileKind;

	use super::*;
	use crate::{
		domain::SdPathBatch,
		infra::action::preflight::Severity,
		ops::files::{
			copy::input::CopyMethod,
			fixture::{Fixture, T},
			plan::{ChangeKind, SkipReason},
		},
	};

	fn input(sources: &[&std::path::Path], destination: &std::path::Path) -> FileCopyInput {
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

	fn change_at<'a>(plan: &'a FsPlan, path: &std::path::Path) -> Option<&'a ChangeKind> {
		plan.changes
			.iter()
			.find(|change| change.path.path().map(PathBuf::as_path) == Some(path))
			.map(|change| &change.change)
	}

	/// A folder copied beside a folder of the same name plans a merge into
	/// it, file by file, and is refused when copied into itself.
	#[tokio::test]
	async fn a_folder_copy_plans_each_file_and_refuses_a_cycle() {
		let fixture = Fixture::new().await;
		let file = FileKind::File;
		let source = [
			("a.txt", file, 4, T, Some("a"), None, None),
			("same.txt", file, 2, T, Some("s"), None, None),
			("sub/b.txt", file, 3, T, Some("b"), None, None),
		];
		fixture.materialize(&fixture.source, &source);
		fixture.index(&fixture.source, &source).await;
		// The destination already holds a folder of the source's name with
		// one file in common.
		let name = fixture
			.source
			.file_name()
			.expect("name")
			.to_string_lossy()
			.into_owned();
		let target_dir = fixture.destination.join(&name);
		fixture.materialize(
			&target_dir,
			&[("same.txt", file, 2, T, Some("s"), None, None)],
		);
		let present = format!("{name}/same.txt");
		fixture
			.index(
				&fixture.destination,
				&[(present.as_str(), file, 2, T, Some("s"), None, None)],
			)
			.await;

		let copy = input(&[&fixture.source], &fixture.destination);
		let validation = validate(&copy, &fixture.preview())
			.await
			.expect("validated");
		assert!(!validation.refuses(), "{:?}", validation.findings);
		assert!(
			validation
				.findings
				.iter()
				.any(|finding| finding.code == FOLDER_COLLISION),
			"the collision is named"
		);
		assert_eq!(
			validation.facts.executes_on,
			crate::device::get_current_device_slug()
		);

		let plan = preview(&copy, &fixture.preview()).await.expect("planned");
		assert!(plan.handle.is_some(), "the plan is retained for browsing");
		assert_eq!(
			change_at(&plan, &target_dir.join("a.txt")),
			Some(&ChangeKind::Create { size: 4 })
		);
		assert_eq!(
			change_at(&plan, &target_dir.join("same.txt")),
			Some(&ChangeKind::Skip {
				reason: SkipReason::DuplicateCandidate
			})
		);
		assert_eq!(
			change_at(&plan, &target_dir.join("sub")),
			Some(&ChangeKind::CreateDirectory)
		);
		assert_eq!(
			change_at(&plan, &target_dir.join("sub/b.txt")),
			Some(&ChangeKind::Create { size: 3 })
		);
		assert_eq!(plan.summary.creates.files, 2);
		assert_eq!(plan.summary.creates.bytes, 7);
		assert_eq!(plan.roots.len(), 1);
		assert!(!plan.roots[0].consumes);

		let cycle = input(&[&fixture.source], &fixture.source.join("sub"));
		let validation = validate(&cycle, &fixture.preview())
			.await
			.expect("validated");
		assert!(validation.refuses());
		assert!(validation
			.errors()
			.any(|finding| finding.code == CYCLE
				&& finding.path == Some(SdPath::local(&fixture.source))));
	}

	/// A move on one volume is a rename that keeps identity, which the
	/// validation says; the plan consumes the source so both the source and
	/// the destination after the move show.
	#[tokio::test]
	async fn a_move_on_one_volume_is_atomic_and_consumes_its_source() {
		let fixture = Fixture::new().await;
		let file = FileKind::File;
		let source = [("a.txt", file, 4, T, Some("a"), None, None)];
		fixture.materialize(&fixture.source, &source);
		fixture.index(&fixture.source, &source).await;
		fixture.index(&fixture.destination, &[]).await;

		let mut moving = input(&[&fixture.source], &fixture.destination);
		moving.move_files = true;
		let validation = validate(&moving, &fixture.preview())
			.await
			.expect("validated");
		assert!(!validation.refuses(), "{:?}", validation.findings);
		let atomic = validation
			.findings
			.iter()
			.find(|finding| finding.code == ATOMIC)
			.expect("the rename is named");
		assert_eq!(atomic.severity, Severity::Info);
		assert!(!validation
			.findings
			.iter()
			.any(|finding| finding.code == IDENTITY_LOSS));

		let plan = preview(&moving, &fixture.preview()).await.expect("planned");
		assert!(plan.roots.iter().all(|root| root.consumes));
		assert_eq!(plan.summary.creates.files, 1);
	}

	/// A file or folder copied into its own directory with keep both is
	/// written beside the original under a numbered name, which is what a
	/// duplicate is.
	#[tokio::test]
	async fn a_duplicate_is_a_numbered_create_beside_the_original() {
		let fixture = Fixture::new().await;
		let file = FileKind::File;
		let tree = [
			("a.txt", file, 4, T, Some("a"), None, None),
			("sub/b.txt", file, 3, T, Some("b"), None, None),
		];
		fixture.materialize(&fixture.source, &tree);
		fixture.index(&fixture.source, &tree).await;

		let mut duplicate = input(&[&fixture.source.join("a.txt")], &fixture.source);
		duplicate.on_conflict = Some(FileConflictResolution::AutoModifyName);
		let plan = preview(&duplicate, &fixture.preview())
			.await
			.expect("planned");
		assert_eq!(
			change_at(&plan, &fixture.source.join("a (1).txt")),
			Some(&ChangeKind::Create { size: 4 })
		);
		assert_eq!(plan.summary.creates.files, 1);
		assert_eq!(plan.summary.skips.policy.files, 0);

		let mut folder = input(&[&fixture.source.join("sub")], &fixture.source);
		folder.on_conflict = Some(FileConflictResolution::AutoModifyName);
		let validation = validate(&folder, &fixture.preview())
			.await
			.expect("validated");
		let collision = validation
			.findings
			.iter()
			.find(|finding| finding.code == FOLDER_COLLISION)
			.expect("the collision is named");
		assert!(
			collision.message.contains("numbered name"),
			"{}",
			collision.message
		);
		let plan = preview(&folder, &fixture.preview()).await.expect("planned");
		let beside = fixture.source.join("sub (1)");
		assert_eq!(
			change_at(&plan, &beside),
			Some(&ChangeKind::CreateDirectory)
		);
		assert_eq!(
			change_at(&plan, &beside.join("b.txt")),
			Some(&ChangeKind::Create { size: 3 })
		);
		assert_eq!(plan.summary.creates.files, 1);
		assert_eq!(plan.summary.directories_created, 1);
	}

	/// A single file moved to a new name is one move in the plan, and a
	/// source that is not there is an error at its path.
	#[tokio::test]
	async fn a_file_move_is_one_move_and_a_missing_source_refuses() {
		let fixture = Fixture::new().await;
		let file = FileKind::File;
		let source = [("a.txt", file, 4, T, Some("a"), None, None)];
		fixture.materialize(&fixture.source, &source);
		fixture.index(&fixture.source, &source).await;

		let from = fixture.source.join("a.txt");
		let to = fixture.destination.join("renamed.txt");
		let mut moving = input(&[&from], &to);
		moving.move_files = true;
		let plan = preview(&moving, &fixture.preview()).await.expect("planned");
		assert_eq!(
			change_at(&plan, &to),
			Some(&ChangeKind::Move {
				from: SdPath::local(&from)
			})
		);
		assert_eq!(plan.summary.moves.files, 1);

		let missing = input(&[&fixture.source.join("gone.txt")], &fixture.destination);
		let validation = validate(&missing, &fixture.preview())
			.await
			.expect("validated");
		assert!(validation
			.errors()
			.any(|finding| finding.code == SOURCE_MISSING));
	}
}
