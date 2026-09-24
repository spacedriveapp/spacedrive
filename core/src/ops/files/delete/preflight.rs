//! Whether a deletion may run, and what would be gone.
//!
//! The warning only an indexed filesystem can give is the point of this
//! check: which of the files being deleted are the last copy of their bytes
//! anywhere in the library. A file's content is looked up by its sampled
//! hash across every store, and one holder in all of them means nothing else
//! has those bytes. For named files that count is cheap enough for
//! validation; for a comparison it is the preview's, which streams the set
//! and puts the flag on each row.

use std::collections::{HashMap, HashSet};

use super::{
	action::FileDeleteAction,
	input::{DeleteTargets, FileDeleteInput},
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
			ChangeKind, FsPlan, FsPlanSummary, PlanBasis, PlanChanges, PlannedChange, StoreRevision,
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
	}

	Ok(Validation { findings, facts })
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

#[cfg(test)]
mod tests {
	use sd_store::file::FileKind;

	use super::*;
	use crate::ops::{
		files::fixture::{Fixture, T},
		paths::compare::{CompareSet, Comparison},
	};

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
