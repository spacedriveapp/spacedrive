//! Whether a deletion may run, and what would be gone.
//!
//! The warning only an indexed filesystem can give is the point of this
//! check: which of the files being deleted are the last copy of their bytes
//! anywhere in the library. A file's content is looked up by its sampled
//! hash across every store, and one holder in all of them means nothing else
//! has those bytes. For named files that count is cheap enough for
//! validation; for a comparison it is the preview's, which streams the set
//! and puts the flag on each row. A set of duplicates previews as the copies
//! that go beside the copy of each content that stays.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use super::{
	action::FileDeleteAction,
	duplicates::{duplicated_in, keeper_at, other_copies, size_of, surplus},
	input::{DeleteTargets, Duplicates, FileDeleteInput, Keep},
};
use crate::{
	domain::SdPath,
	infra::{
		action::{
			error::ActionError,
			preflight::{
				ExecutionFacts, Finding, PreviewContext, PreviewableAction, ValidatedAction,
				Validation,
			},
		},
		query::QueryError,
	},
	ops::{
		files::plan::{
			ChangeKind, FsPlan, FsPlanSummary, PlanBasis, PlanChanges, PlannedChange, SkipReason,
			StoreRevision,
		},
		paths::{
			compare::{CompareBy, Folder, Keyed, Matcher, Side},
			reach::{every_store_in, Reach},
		},
	},
};

pub const REMOTE_ROOT: &str = "delete.remote_root";
pub const MISSING: &str = "delete.missing";
pub const TRASH_UNSUPPORTED: &str = "delete.trash_unsupported";
pub const UNTRACKED: &str = "delete.untracked";
pub const LAST_COPY: &str = "delete.last_copy";
pub const NO_SCOPE: &str = "delete.no_scope";
pub const UNHASHED: &str = "delete.unhashed";
pub const UNVERIFIED: &str = "delete.unverified";

/// How many files a comparison preview records per last-copy lookup.
const BATCH: usize = 1000;

impl ValidatedAction for FileDeleteAction {
	async fn validate(
		input: &FileDeleteInput,
		ctx: &PreviewContext,
	) -> Result<Validation, ActionError> {
		validate(input, ctx).await
	}
}

impl PreviewableAction for FileDeleteAction {
	type Plan = FsPlan;

	async fn preview(input: FileDeleteInput, ctx: &PreviewContext) -> Result<FsPlan, ActionError> {
		preview(&input, ctx).await
	}
}

crate::register_validate!(FileDeleteAction, "files.delete");
crate::register_preview!(FileDeleteAction, "files.delete");

async fn validate(
	input: &FileDeleteInput,
	ctx: &PreviewContext,
) -> Result<Validation, ActionError> {
	let mut findings = Vec::new();
	let mut facts = ExecutionFacts {
		executes_on: ctx.executes_on(),
		strategy: Some(
			if input.permanent {
				"permanent"
			} else {
				"trash"
			}
			.to_string(),
		),
		..Default::default()
	};

	match &input.targets {
		DeleteTargets::Paths { paths } => {
			let mut named = Vec::new();
			for path in paths {
				if path.is_cloud() {
					if !input.permanent {
						findings.push(
							Finding::error(
								TRASH_UNSUPPORTED,
								"a cloud file has no trash; delete it permanently",
							)
							.at(path.clone()),
						);
					}
					continue;
				}
				let Some(local) = path.as_local_path() else {
					findings.push(
						Finding::error(
							REMOTE_ROOT,
							"a file is on another device; delete it there with --device",
						)
						.at(path.clone()),
					);
					continue;
				};
				if tokio::fs::symlink_metadata(local).await.is_err() {
					findings.push(Finding::error(MISSING, "a file is not there").at(path.clone()));
					continue;
				}
				named.push(path.clone());
			}
			let files = indexed(ctx, &named).await;
			facts.estimated_files = Some(named.len() as u64);
			facts.estimated_bytes = Some(files.iter().map(|file| file.size).sum());
			let last = last_copies(
				ctx,
				files
					.iter()
					.filter_map(|file| file.sampled.clone())
					.collect(),
			)
			.await;
			let stranded = files
				.iter()
				.filter(|file| {
					file.sampled
						.as_ref()
						.is_some_and(|hash| last.contains(hash))
				})
				.count();
			if stranded > 0 {
				findings.push(Finding::warning(
					LAST_COPY,
					format!(
						"{stranded} of these files are the last copy of their content anywhere in your library"
					),
				));
			}
		}
		DeleteTargets::Comparison { comparison } => {
			for folder in [&comparison.a, &comparison.b] {
				if folder.as_local_path().is_none() {
					findings.push(
						Finding::error(
							REMOTE_ROOT,
							"a folder is on another device; delete there with --device",
						)
						.at(folder.clone()),
					);
				} else if ctx.reach(folder).await.is_empty() {
					findings.push(
						Finding::error(
							UNTRACKED,
							"a folder is outside every tracked source; the comparison needs its index",
						)
						.at(folder.clone()),
					);
				}
			}
		}
		DeleteTargets::Duplicates { duplicates } => {
			validate_duplicates(duplicates, ctx, &mut findings, &mut facts).await?;
		}
	}

	Ok(Validation { findings, facts })
}

/// A scope that is tracked, chosen copies that are there and hashed, and
/// for chosen copies the count of what would go and how many pairs rest
/// on a sampled hash alone.
async fn validate_duplicates(
	duplicates: &Duplicates,
	ctx: &PreviewContext,
	findings: &mut Vec<Finding>,
	facts: &mut ExecutionFacts,
) -> Result<(), ActionError> {
	if let Some(scope) = &duplicates.scope {
		if scope.as_local_path().is_none() {
			findings.push(
				Finding::error(
					REMOTE_ROOT,
					"the folder is on another device; delete there with --device",
				)
				.at(scope.clone()),
			);
		} else if ctx.reach(scope).await.is_empty() {
			findings.push(
				Finding::error(
					UNTRACKED,
					"the folder is outside every tracked source; finding copies needs its index",
				)
				.at(scope.clone()),
			);
		}
	}
	match &duplicates.keep {
		Keep::First => {
			if duplicates.scope.is_none() {
				findings.push(Finding::error(
					NO_SCOPE,
					"keeping the first copy needs a folder to look in",
				));
			}
		}
		Keep::These { paths } => {
			let mut kept = Vec::new();
			for path in paths {
				let Some(local) = path.as_local_path() else {
					findings.push(
						Finding::error(
							REMOTE_ROOT,
							"a file to keep is on another device; delete there with --device",
						)
						.at(path.clone()),
					);
					continue;
				};
				if tokio::fs::symlink_metadata(local).await.is_err() {
					findings.push(
						Finding::error(MISSING, "a file to keep is not there").at(path.clone()),
					);
					continue;
				}
				match keeper_at(ctx.volumes(), ctx.index(), path).await {
					Some(file) if file.entry.sampled_hash.is_some() => kept.push(file),
					_ => findings.push(
						Finding::error(
							UNHASHED,
							"a file to keep has not been hashed yet, so its copies cannot be found",
						)
						.at(path.clone()),
					),
				}
			}
			let reaches = match &duplicates.scope {
				Some(scope) => ctx.reach(scope).await,
				None => every_store_in(ctx.index()),
			};
			let hashes: Vec<String> = kept
				.iter()
				.filter_map(|file| file.entry.sampled_hash.clone())
				.collect();
			let kept_paths: HashSet<PathBuf> = kept.iter().map(|file| file.path.clone()).collect();
			let copies = other_copies(
				ctx.index(),
				&reaches,
				&hashes,
				&kept_paths,
				duplicates.min_size.unwrap_or(0),
			)
			.await
			.map_err(read_failed)?;
			facts.estimated_files = Some(copies.len() as u64);
			facts.estimated_bytes = Some(copies.iter().map(|copy| size_of(&copy.entry)).sum());
			let unverified = copies
				.iter()
				.chain(kept.iter())
				.filter(|file| file.entry.integrity_hash.is_none())
				.count();
			if unverified > 0 {
				findings.push(Finding::info(
					UNVERIFIED,
					format!(
						"{unverified} of these copies match on a sampled hash alone; the job reads each in full before removing it"
					),
				));
			}
		}
	}
	Ok(())
}

async fn preview(input: &FileDeleteInput, ctx: &PreviewContext) -> Result<FsPlan, ActionError> {
	let device = crate::device::get_current_device_slug();
	let mut summary = FsPlanSummary::default();
	let mut changes = PlanChanges::default();
	let mut revisions: Vec<StoreRevision> = Vec::new();

	match &input.targets {
		DeleteTargets::Paths { paths } => {
			let files = indexed(ctx, paths).await;
			let last = last_copies(
				ctx,
				files
					.iter()
					.filter_map(|file| file.sampled.clone())
					.collect(),
			)
			.await;
			for file in files {
				let change = ChangeKind::Delete {
					last_copy: file
						.sampled
						.as_ref()
						.is_some_and(|hash| last.contains(hash)),
				};
				summary.count(&change, file.size);
				changes.push(PlannedChange {
					path: file.path,
					change,
				});
			}
		}
		DeleteTargets::Comparison { comparison } => {
			let (mut a, mut b) = (
				folder(ctx, &comparison.a, comparison.include_hidden).await?,
				folder(ctx, &comparison.b, comparison.include_hidden).await?,
			);
			for (source, revision) in a
				.revisions()
				.await
				.map_err(read_failed)?
				.into_iter()
				.chain(b.revisions().await.map_err(read_failed)?)
			{
				if !revisions.iter().any(|known| known.source == source) {
					revisions.push(StoreRevision { source, revision });
				}
			}
			let mut matcher = match comparison.by {
				CompareBy::Path => Matcher::by_path(&mut a, &mut b),
				CompareBy::Content => Matcher::by_content(&mut a, &b, Side::A),
			};
			// The last-copy lookup is one query per batch across every
			// store, so the stream is recorded in batches rather than per file.
			let mut batch: Vec<Keyed> = Vec::new();
			loop {
				let next = matcher.next().await.map_err(read_failed)?;
				let drained = next.is_none();
				if let Some(file) = next
					.filter(|sorted| sorted.set == comparison.show)
					.and_then(|sorted| sorted.a)
				{
					batch.push(file);
				}
				if batch.len() >= BATCH || (drained && !batch.is_empty()) {
					record_batch(
						ctx,
						&device,
						std::mem::take(&mut batch),
						&mut summary,
						&mut changes,
					)
					.await;
				}
				if drained {
					break;
				}
			}
		}
		DeleteTargets::Duplicates { duplicates } => {
			let reaches = match &duplicates.scope {
				Some(scope) => ctx.reach(scope).await,
				None => every_store_in(ctx.index()),
			};
			if reaches.is_empty() {
				return Err(ActionError::InvalidInput(
					"the folder is not in a tracked source".to_string(),
				));
			}
			for reach in &reaches {
				let Some(db) = ctx.index().read_store(reach.source.id).await else {
					continue;
				};
				let revision = db.revision().await.map_err(store_failed)?;
				if !revisions
					.iter()
					.any(|known| known.source == reach.source.id)
				{
					revisions.push(StoreRevision {
						source: reach.source.id,
						revision: revision.value,
					});
				}
			}
			let min_size = duplicates.min_size.unwrap_or(0);
			let mut note = |file: Keyed, keeper: bool| {
				let size = size_of(&file.entry);
				let change = if keeper {
					ChangeKind::Skip {
						reason: SkipReason::Policy,
					}
				} else {
					ChangeKind::Delete { last_copy: false }
				};
				summary.count(&change, size);
				changes.push(PlannedChange {
					path: SdPath::Physical {
						device_slug: device.clone(),
						path: file.path,
					},
					change,
				});
			};
			match &duplicates.keep {
				Keep::First => {
					let duplicated = duplicated_in(ctx.index(), &reaches, min_size)
						.await
						.map_err(read_failed)?;
					for (file, keeper) in surplus(ctx.index(), &reaches, &duplicated)
						.await
						.map_err(read_failed)?
					{
						note(file, keeper);
					}
				}
				Keep::These { paths } => {
					let mut kept = Vec::new();
					for path in paths {
						if let Some(file) = keeper_at(ctx.volumes(), ctx.index(), path).await {
							if file.entry.sampled_hash.is_some() {
								kept.push(file);
							}
						}
					}
					let hashes: Vec<String> = kept
						.iter()
						.filter_map(|file| file.entry.sampled_hash.clone())
						.collect();
					let kept_paths: HashSet<PathBuf> =
						kept.iter().map(|file| file.path.clone()).collect();
					let copies =
						other_copies(ctx.index(), &reaches, &hashes, &kept_paths, min_size)
							.await
							.map_err(read_failed)?;
					for file in kept {
						note(file, true);
					}
					for file in copies {
						note(file, false);
					}
				}
			}
		}
	}

	let (changes, truncated) = changes.finish();
	Ok(ctx.plans().retain(FsPlan {
		handle: None,
		basis: PlanBasis::Index { revisions },
		roots: Vec::new(),
		summary,
		changes,
		truncated,
	}))
}

/// A named file as its store knows it.
struct Indexed {
	path: SdPath,
	size: u64,
	sampled: Option<String>,
}

/// What the index knows of each named local file: its size and the sampled
/// hash its content is keyed by. A file the index does not hold is listed
/// with what the filesystem says of it and no content to look up.
async fn indexed(ctx: &PreviewContext, paths: &[SdPath]) -> Vec<Indexed> {
	let mut files = Vec::with_capacity(paths.len());
	for path in paths {
		let Some(local) = path.as_local_path() else {
			continue;
		};
		let mut found = None;
		for reach in ctx.reach(path).await {
			if !reach.prefix.is_empty() {
				continue;
			}
			let Some(db) = ctx.index().read_store(reach.source.id).await else {
				continue;
			};
			if let Ok(Some(entry)) = sd_store::read::entry_by_path(db.pool(), &reach.scope).await {
				found = Some(Indexed {
					path: path.clone(),
					size: entry.size.unwrap_or(0).max(0) as u64,
					sampled: entry.sampled_hash,
				});
				break;
			}
		}
		files.push(
			found.unwrap_or(Indexed {
				path: path.clone(),
				size: tokio::fs::symlink_metadata(local)
					.await
					.map(|meta| meta.len())
					.unwrap_or(0),
				sampled: None,
			}),
		);
	}
	files
}

/// Which of `hashes` one record in the whole library holds: the contents a
/// deletion would remove the last copy of.
async fn last_copies(ctx: &PreviewContext, hashes: Vec<String>) -> HashSet<String> {
	if hashes.is_empty() {
		return HashSet::new();
	}
	let mut holders: HashMap<String, i64> = HashMap::new();
	for reach in every_store_in(ctx.index()) {
		let Some(db) = ctx.index().read_store(reach.source.id).await else {
			continue;
		};
		if let Ok(counts) = sd_store::read::content_holders(db.pool(), &hashes).await {
			for (hash, count) in counts {
				*holders.entry(hash).or_default() += count;
			}
		}
	}
	hashes
		.into_iter()
		.filter(|hash| holders.get(hash).copied().unwrap_or(0) <= 1)
		.collect()
}

/// One side of the comparison, open to stream.
async fn folder(
	ctx: &PreviewContext,
	path: &SdPath,
	include_hidden: bool,
) -> Result<Folder, ActionError> {
	let reached: Vec<Reach> = ctx.reach(path).await;
	if reached.is_empty() {
		return Err(ActionError::InvalidInput(format!(
			"{path} is not in a tracked source"
		)));
	}
	Folder::open(ctx.index(), reached, include_hidden, None)
		.await
		.map_err(read_failed)
}

/// Note a batch of files the comparison sorted into the deleted set, each
/// flagged when it is the last copy of its content.
async fn record_batch(
	ctx: &PreviewContext,
	device: &str,
	batch: Vec<Keyed>,
	summary: &mut FsPlanSummary,
	changes: &mut PlanChanges,
) {
	let hashes: Vec<String> = batch
		.iter()
		.filter_map(|file| file.entry.sampled_hash.clone())
		.collect();
	let last = last_copies(ctx, hashes).await;
	for file in batch {
		let size = file.entry.size.unwrap_or(0).max(0) as u64;
		let change = ChangeKind::Delete {
			last_copy: file
				.entry
				.sampled_hash
				.as_ref()
				.is_some_and(|hash| last.contains(hash)),
		};
		summary.count(&change, size);
		changes.push(PlannedChange {
			path: SdPath::Physical {
				device_slug: device.to_string(),
				path: file.path,
			},
			change,
		});
	}
}

fn read_failed(error: QueryError) -> ActionError {
	ActionError::Internal(error.to_string())
}

fn store_failed(error: sd_store::Error) -> ActionError {
	ActionError::Internal(error.to_string())
}

#[cfg(test)]
mod tests {
	use std::time::Duration;

	use sd_store::file::FileKind;

	use super::*;
	use crate::{
		infra::job::output::JobOutput,
		ops::{
			files::{
				delete::job::{DeleteJob, DeleteMode},
				fixture::{Entry, Fixture, T},
			},
			paths::compare::{CompareSet, Comparison},
		},
	};

	/// Four files of three contents: `x.jpg` twice, `y.jpg` twice under one
	/// folder, and one file nothing else holds.
	fn duplicated_tree() -> [Entry<'static>; 5] {
		let file = FileKind::File;
		[
			("a/x.jpg", file, 4, T, Some("h1"), None, None),
			("b/x.jpg", file, 4, T, Some("h1"), None, None),
			("c/y.jpg", file, 3, T, Some("h2"), None, None),
			("c/z.jpg", file, 3, T, Some("h2"), None, None),
			("d/only.jpg", file, 5, T, Some("u"), None, None),
		]
	}

	fn change_at(plan: &FsPlan, path: &std::path::Path) -> Option<ChangeKind> {
		plan.changes
			.iter()
			.find(|change| change.path.path().map(PathBuf::as_path) == Some(path))
			.map(|change| change.change.clone())
	}

	async fn run(fixture: &Fixture, targets: DeleteTargets) -> usize {
		let handle = fixture
			.library
			.jobs()
			.dispatch(DeleteJob::new(targets, DeleteMode::Permanent))
			.await
			.expect("dispatched");
		let output = tokio::time::timeout(Duration::from_secs(30), handle.wait())
			.await
			.expect("in time")
			.expect("completed");
		match output {
			JobOutput::FileDelete { deleted_count, .. } => deleted_count,
			other => panic!("not a delete output: {other:?}"),
		}
	}

	/// Of each duplicated content beneath the folder the first copy in walk
	/// order stays, previewed as kept, and the job removes the rest after
	/// reading them.
	#[tokio::test]
	async fn the_first_copy_stays_and_the_others_go() {
		let fixture = Fixture::new().await;
		let tree = duplicated_tree();
		fixture.materialize(&fixture.source, &tree);
		fixture.index(&fixture.source, &tree).await;

		let targets = DeleteTargets::Duplicates {
			duplicates: Duplicates {
				scope: Some(SdPath::local(&fixture.source)),
				keep: Keep::First,
				min_size: None,
			},
		};
		let input = delete(targets.clone());
		let validation = validate(&input, &fixture.preview())
			.await
			.expect("validated");
		assert!(!validation.refuses(), "{:?}", validation.findings);

		let plan = preview(&input, &fixture.preview()).await.expect("planned");
		let kept = ChangeKind::Skip {
			reason: SkipReason::Policy,
		};
		let gone = ChangeKind::Delete { last_copy: false };
		assert_eq!(
			change_at(&plan, &fixture.source.join("a/x.jpg")),
			Some(kept.clone())
		);
		assert_eq!(
			change_at(&plan, &fixture.source.join("b/x.jpg")),
			Some(gone.clone())
		);
		assert_eq!(
			change_at(&plan, &fixture.source.join("c/y.jpg")),
			Some(kept)
		);
		assert_eq!(
			change_at(&plan, &fixture.source.join("c/z.jpg")),
			Some(gone)
		);
		assert_eq!(change_at(&plan, &fixture.source.join("d/only.jpg")), None);
		assert_eq!(plan.summary.deletes.files, 2);
		assert_eq!(plan.summary.deletes.bytes, 7);
		assert_eq!(plan.summary.skips.policy.files, 2);

		assert_eq!(run(&fixture, targets).await, 2);
		for (path, present) in [
			("a/x.jpg", true),
			("b/x.jpg", false),
			("c/y.jpg", true),
			("c/z.jpg", false),
			("d/only.jpg", true),
		] {
			assert_eq!(fixture.source.join(path).exists(), present, "{path}");
		}
	}

	/// Chosen copies stay wherever they are, every other copy of their
	/// content goes, and a copy that is not hashed cannot be chosen.
	#[tokio::test]
	async fn chosen_copies_stay() {
		let fixture = Fixture::new().await;
		let tree = duplicated_tree();
		fixture.materialize(&fixture.source, &tree);
		fixture.index(&fixture.source, &tree).await;

		let keep = SdPath::local(fixture.source.join("b/x.jpg"));
		let targets = DeleteTargets::Duplicates {
			duplicates: Duplicates {
				scope: None,
				keep: Keep::These {
					paths: vec![keep.clone()],
				},
				min_size: None,
			},
		};
		let input = delete(targets.clone());
		let validation = validate(&input, &fixture.preview())
			.await
			.expect("validated");
		assert!(!validation.refuses(), "{:?}", validation.findings);
		assert_eq!(validation.facts.estimated_files, Some(1));
		assert_eq!(validation.facts.estimated_bytes, Some(4));
		assert!(validation
			.findings
			.iter()
			.any(|finding| finding.code == UNVERIFIED));

		let plan = preview(&input, &fixture.preview()).await.expect("planned");
		assert_eq!(
			change_at(&plan, &fixture.source.join("b/x.jpg")),
			Some(ChangeKind::Skip {
				reason: SkipReason::Policy
			})
		);
		assert_eq!(
			change_at(&plan, &fixture.source.join("a/x.jpg")),
			Some(ChangeKind::Delete { last_copy: false })
		);
		assert_eq!(plan.changes.len(), 2);

		assert_eq!(run(&fixture, targets).await, 1);
		assert!(!fixture.source.join("a/x.jpg").exists());
		assert!(fixture.source.join("b/x.jpg").exists());
		assert!(
			fixture.source.join("c/z.jpg").exists(),
			"untouched content stays"
		);

		std::fs::write(fixture.source.join("d/fresh.jpg"), b"fresh").expect("file");
		let unhashed = delete(DeleteTargets::Duplicates {
			duplicates: Duplicates {
				scope: None,
				keep: Keep::These {
					paths: vec![SdPath::local(fixture.source.join("d/fresh.jpg"))],
				},
				min_size: None,
			},
		});
		let validation = validate(&unhashed, &fixture.preview())
			.await
			.expect("validated");
		assert!(validation.errors().any(|finding| finding.code == UNHASHED));
	}

	fn delete(targets: DeleteTargets) -> FileDeleteInput {
		FileDeleteInput {
			targets,
			permanent: false,
			recursive: true,
		}
	}

	/// The last copy of a content anywhere in the library is counted in the
	/// warning and flagged on its row; a content held elsewhere is not.
	#[tokio::test]
	async fn the_last_copy_is_warned_of_and_flagged() {
		let fixture = Fixture::new().await;
		let file = FileKind::File;
		let source = [
			("unique.txt", file, 6, T, Some("u"), None, None),
			("same.txt", file, 4, T, Some("s"), None, None),
		];
		let other = [("same.txt", file, 4, T, Some("s"), None, None)];
		fixture.materialize(&fixture.source, &source);
		fixture.index(&fixture.source, &source).await;
		fixture.materialize(&fixture.destination, &other);
		fixture.index(&fixture.destination, &other).await;

		let unique = SdPath::local(fixture.source.join("unique.txt"));
		let same = SdPath::local(fixture.source.join("same.txt"));
		let input = delete(DeleteTargets::Paths {
			paths: vec![unique.clone(), same.clone()],
		});
		let validation = validate(&input, &fixture.preview())
			.await
			.expect("validated");
		assert!(!validation.refuses(), "{:?}", validation.findings);
		let warning = validation
			.findings
			.iter()
			.find(|finding| finding.code == LAST_COPY)
			.expect("the last copy is warned of");
		assert!(
			warning.message.starts_with("1 of these files"),
			"{}",
			warning.message
		);
		assert_eq!(validation.facts.estimated_files, Some(2));
		assert_eq!(validation.facts.estimated_bytes, Some(10));

		let plan = preview(&input, &fixture.preview()).await.expect("planned");
		let flag = |path: &SdPath| {
			plan.changes
				.iter()
				.find(|change| &change.path == path)
				.map(|change| change.change.clone())
		};
		assert_eq!(flag(&unique), Some(ChangeKind::Delete { last_copy: true }));
		assert_eq!(flag(&same), Some(ChangeKind::Delete { last_copy: false }));
		assert_eq!(plan.summary.deletes.files, 2);
		assert_eq!(plan.summary.deletes.bytes, 10);

		let gone = delete(DeleteTargets::Paths {
			paths: vec![SdPath::local(fixture.source.join("gone.txt"))],
		});
		let validation = validate(&gone, &fixture.preview())
			.await
			.expect("validated");
		assert!(validation.errors().any(|finding| finding.code == MISSING));
	}

	/// A comparison target previews exactly the set it names, from the
	/// index, with each row's flag.
	#[tokio::test]
	async fn a_comparison_previews_the_set_it_names() {
		let fixture = Fixture::new().await;
		let file = FileKind::File;
		let source = [
			("unique.txt", file, 6, T, Some("u"), None, None),
			("same.txt", file, 4, T, Some("s"), None, None),
		];
		let other = [("same.txt", file, 4, T, Some("s"), None, None)];
		fixture.index(&fixture.source, &source).await;
		fixture.index(&fixture.destination, &other).await;

		let input = delete(DeleteTargets::Comparison {
			comparison: Comparison {
				a: SdPath::local(&fixture.source),
				b: SdPath::local(&fixture.destination),
				by: CompareBy::Path,
				show: CompareSet::Both,
				include_hidden: false,
			},
		});
		let validation = validate(&input, &fixture.preview())
			.await
			.expect("validated");
		assert!(!validation.refuses(), "{:?}", validation.findings);

		let plan = preview(&input, &fixture.preview()).await.expect("planned");
		assert_eq!(plan.changes.len(), 1);
		assert_eq!(
			plan.changes[0].path,
			SdPath::local(fixture.source.join("same.txt"))
		);
		assert_eq!(
			plan.changes[0].change,
			ChangeKind::Delete { last_copy: false }
		);
		let PlanBasis::Index { revisions } = &plan.basis;
		assert_eq!(revisions.len(), 2, "both stores are named");

		let untracked = delete(DeleteTargets::Comparison {
			comparison: Comparison {
				a: SdPath::local(&fixture.source),
				b: SdPath::local(std::env::temp_dir().join("nowhere-tracked")),
				by: CompareBy::Path,
				show: CompareSet::Both,
				include_hidden: false,
			},
		});
		let validation = validate(&untracked, &fixture.preview())
			.await
			.expect("validated");
		assert!(validation.errors().any(|finding| finding.code == UNTRACKED));
	}
}
