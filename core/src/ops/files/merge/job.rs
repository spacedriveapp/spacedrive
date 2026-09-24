//! The merge itself, leaf by leaf.
//!
//! The job walks each source on disk in one deterministic order, sorted
//! names with a directory's children before its next sibling, and settles
//! every leaf against what the destination holds right then: copy what is
//! missing, merge into a directory both have, skip a file whose bytes are
//! proven identical, resolve a collision as the policy says, and leave a
//! conflict alone. Identity is settled by reading: equal sizes, then equal
//! integrity hashes computed here, since the plan could only call a
//! duplicate a candidate.
//!
//! The plan is read again from the index when the job starts, so each leaf's
//! outcome records whether it matched what the plan said; the filesystem is
//! live and a divergence is information, not a failure. The cursor is the
//! last leaf settled, so a paused or interrupted job resumes past it rather
//! than repeating a half-merged tree. With `consume_sources` a settled leaf
//! is removed from its source and emptied directories are pruned from the
//! bottom up, so the source ends holding exactly what the merge left
//! unsettled; on one volume a consumed leaf is renamed into place rather
//! than copied and removed. A replaced file's previous bytes go to the
//! trash, and every effect is journaled. The job writes no records; the
//! watcher observes the results.

use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	time::Instant,
};

use serde::{Deserialize, Serialize};
use specta::Type;

use super::input::{FileMergeInput, MergeConflictPolicy};
use crate::{
	domain::{content_identity::ContentHashGenerator, SdPath},
	infra::{
		action::preflight::PreviewContext,
		api::SessionContext,
		job::{generic_progress::GenericProgress, journal::Effect, prelude::*},
	},
	ops::{
		files::{
			copy::{input::CopyMethod, routing::CopyStrategyRouter},
			plan::{ChangeKind, ConflictKind, SkipReason},
			planner::{is_junk, Planner},
			trash,
		},
		paths::reach::stores_beneath,
	},
};

/// Leaves settled between checkpoints.
const CHECKPOINT_EVERY: usize = 64;

#[derive(Debug, Serialize, Deserialize, Job)]
pub struct FolderMergeJob {
	pub input: FileMergeInput,
	/// The last leaf settled, which a resumed job walks up to and past.
	pub cursor: Option<Cursor>,
	pub outcomes: Vec<MergeOutcome>,

	#[serde(skip, default = "Instant::now")]
	started_at: Instant,
}

/// One leaf in walk order: which source, and its path relative to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor {
	pub source: usize,
	pub path: String,
}

/// What became of one leaf.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MergeOutcome {
	/// Which source the leaf came from, by position in the input.
	pub source: usize,
	/// Where it was written or would have been, relative to the destination.
	pub path: String,
	pub result: MergeResult,
	/// What the plan said, when it knew the leaf.
	pub planned: Option<ChangeKind>,
	/// Whether the outcome differs from the plan, or the plan did not know
	/// the leaf at all.
	pub diverged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MergeResult {
	Copied { bytes: u64 },
	CreatedDirectory,
	MergedInto,
	Replaced { bytes: u64 },
	Skipped { reason: SkipReason },
	Conflict { kind: ConflictKind },
	Failed { error: String },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct MergeOutput {
	pub copied: u64,
	pub created_directories: u64,
	pub merged_into: u64,
	pub replaced: u64,
	pub skipped_duplicates: u64,
	pub skipped_policy: u64,
	pub junk: u64,
	pub conflicts: u64,
	pub failed: u64,
	/// Leaves removed from their source.
	pub consumed: u64,
	pub pruned_directories: u64,
	/// Files the destination held that no source did, trashed by a mirror.
	pub removed_extras: u64,
	/// Tag assertions still standing on records under the sources, which a
	/// consuming merge across volumes leaves behind.
	pub assertions_left: i64,
	pub bytes: u64,
	pub diverged: u64,
	pub outcomes: Vec<MergeOutcome>,
}

impl From<MergeOutput> for JobOutput {
	fn from(output: MergeOutput) -> Self {
		JobOutput::custom(output)
	}
}

impl Job for FolderMergeJob {
	const NAME: &'static str = "folder_merge";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> = Some("Merge folders");
}

impl crate::infra::job::traits::DynJob for FolderMergeJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}

	/// One merge per source and destination pair at a time; an impatient
	/// second dispatch gets the live job.
	fn dedup_key(&self) -> Option<String> {
		let sources: Vec<String> = self
			.input
			.sources
			.paths
			.iter()
			.map(ToString::to_string)
			.collect();
		Some(format!(
			"{} -> {}",
			sources.join(","),
			self.input.destination
		))
	}
}

/// What a checkpoint carries, so an interrupted job picks up where it was.
#[derive(Debug, Serialize, Deserialize)]
struct Resume {
	cursor: Option<Cursor>,
	outcomes: Vec<MergeOutcome>,
}

impl FolderMergeJob {
	pub fn new(input: FileMergeInput) -> Self {
		Self {
			input,
			cursor: None,
			outcomes: Vec::new(),
			started_at: Instant::now(),
		}
	}
}

#[async_trait::async_trait]
impl JobHandler for FolderMergeJob {
	type Output = MergeOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		if let Some(resume) = ctx.load_state::<Resume>().await? {
			self.cursor = resume.cursor;
			self.outcomes = resume.outcomes;
		}
		if let Some(cursor) = &self.cursor {
			ctx.log(format!("Resuming past {}", cursor.path));
		}

		let destination = self
			.input
			.destination
			.as_local_path()
			.ok_or_else(|| JobError::execution("the destination is not on this device"))?
			.to_path_buf();

		ctx.progress(Progress::Indeterminate("Planning".to_string()));
		let plan = self.plan(&ctx).await;
		let total = plan.len() as u64;
		ctx.log(format!("{total} planned changes from the index"));

		let sources: Vec<PathBuf> = self
			.input
			.sources
			.paths
			.iter()
			.filter_map(|source| source.as_local_path().map(Path::to_path_buf))
			.collect();
		// A consuming merge on one volume renames each leaf into place.
		let moving = self.input.consume_sources
			&& match ctx.volume_manager() {
				Some(volumes) => {
					let mut same = true;
					for source in &sources {
						same &= volumes.same_volume(source, &destination).await;
					}
					same
				}
				None => false,
			};

		let mut merge = Merge::new(
			&ctx,
			self.input.on_conflict,
			self.input.consume_sources,
			moving,
			plan,
		);
		merge.resuming = self.cursor.clone();
		merge.cursor = self.cursor.take();
		merge.outcomes = std::mem::take(&mut self.outcomes);
		merge.total = total;

		for (index, source) in sources.iter().enumerate() {
			merge
				.directory(index, source, source, &destination, "")
				.await?;
		}
		let run = merge.finish().await?;

		let removed_extras = if self.input.remove_extras {
			ctx.progress(Progress::Indeterminate(
				"Removing what no source holds".to_string(),
			));
			remove_extras(&ctx, &sources, &destination).await?
		} else {
			0
		};

		let assertions_left = if self.input.consume_sources {
			assertions_left(&ctx, &self.input.sources.paths).await
		} else {
			0
		};

		let mut output = MergeOutput {
			pruned_directories: run.pruned,
			removed_extras,
			assertions_left,
			bytes: run.bytes,
			..Default::default()
		};
		for outcome in &run.outcomes {
			match &outcome.result {
				MergeResult::Copied { .. } => output.copied += 1,
				MergeResult::CreatedDirectory => output.created_directories += 1,
				MergeResult::MergedInto => output.merged_into += 1,
				MergeResult::Replaced { .. } => output.replaced += 1,
				MergeResult::Skipped {
					reason: SkipReason::DuplicateCandidate | SkipReason::DuplicateConfirmed,
				} => output.skipped_duplicates += 1,
				MergeResult::Skipped {
					reason: SkipReason::Policy,
				} => output.skipped_policy += 1,
				MergeResult::Skipped {
					reason: SkipReason::Junk,
				} => output.junk += 1,
				MergeResult::Conflict { .. } => output.conflicts += 1,
				MergeResult::Failed { .. } => output.failed += 1,
			}
			if outcome.diverged {
				output.diverged += 1;
			}
		}
		output.consumed = run.consumed;
		self.cursor = run.cursor;
		self.outcomes = run.outcomes;
		output.outcomes = self.outcomes.clone();

		ctx.progress(Progress::generic(
			GenericProgress::new(
				1.0,
				"Complete",
				format!(
					"{} copied, {} skipped, {} conflicts",
					output.copied,
					output.skipped_duplicates + output.skipped_policy + output.junk,
					output.conflicts
				),
			)
			.with_completion(total, total)
			.with_bytes(output.bytes, output.bytes)
			.with_performance(0.0, None, Some(self.started_at.elapsed()))
			.with_errors(output.failed, output.conflicts + output.diverged),
		));
		ctx.log(format!(
			"Merge completed: {} copied, {} replaced, {} duplicates skipped, {} conflicts, {} diverged from the plan",
			output.copied, output.replaced, output.skipped_duplicates, output.conflicts, output.diverged
		));
		Ok(output)
	}
}

impl FolderMergeJob {
	/// The plan's decisions, read from the index as the job starts, so each
	/// outcome can say whether it matched. Empty when a root is untracked,
	/// in which case nothing diverges.
	async fn plan(&self, ctx: &JobContext<'_>) -> HashMap<String, ChangeKind> {
		let context = ctx.library().core_context();
		let Ok(device_id) = context.device_manager.device_id() else {
			return HashMap::new();
		};
		let session = SessionContext::device_session(device_id, "Core Device".to_string())
			.with_library(ctx.library().id());
		let preview = PreviewContext::new(context, ctx.library_arc(), session);
		let mut planner = Planner::new(&preview, self.input.on_conflict, true);
		for source in &self.input.sources.paths {
			if let Err(error) = planner
				.pair(source, &self.input.destination, self.input.consume_sources)
				.await
			{
				ctx.log(format!("No plan to check against: {error}"));
				return HashMap::new();
			}
		}
		planner.into_decisions()
	}
}

/// What a run of the merge amounts to.
pub(crate) struct MergeRun {
	pub(crate) outcomes: Vec<MergeOutcome>,
	pub(crate) cursor: Option<Cursor>,
	pub(crate) pruned: u64,
	pub(crate) bytes: u64,
	/// Leaves removed from their source: everything settled while
	/// consuming.
	pub(crate) consumed: u64,
}

/// One run of the merge over the live filesystem. The copy job runs one too,
/// for a folder copied onto a folder that is already there.
pub(crate) struct Merge<'a, 'c> {
	ctx: &'a JobContext<'c>,
	policy: MergeConflictPolicy,
	consume: bool,
	/// Rename each leaf into place rather than copy and remove it; only on
	/// one volume.
	moving: bool,
	plan: HashMap<String, ChangeKind>,
	total: u64,
	/// The cursor a resumed job walks up to; leaves before it are done.
	resuming: Option<Cursor>,
	cursor: Option<Cursor>,
	outcomes: Vec<MergeOutcome>,
	since_checkpoint: usize,
	pruned: u64,
	bytes: u64,
	consumed: u64,
	/// Effects not yet written to the journal.
	effects: Vec<Effect>,
}

/// What settling one leaf came to.
struct Settled {
	result: MergeResult,
	/// Where a leaf kept beside the existing file was written.
	written: Option<PathBuf>,
	/// Where a replaced file's previous bytes went.
	previous: Option<PathBuf>,
	/// Whether the leaf was renamed into place, so nothing is left to
	/// remove from the source.
	moved: bool,
}

impl Settled {
	fn of(result: MergeResult) -> Self {
		Self {
			result,
			written: None,
			previous: None,
			moved: false,
		}
	}
}

impl<'a, 'c> Merge<'a, 'c> {
	pub(crate) fn new(
		ctx: &'a JobContext<'c>,
		policy: MergeConflictPolicy,
		consume: bool,
		moving: bool,
		plan: HashMap<String, ChangeKind>,
	) -> Self {
		Self {
			ctx,
			policy,
			consume,
			moving,
			total: plan.len() as u64,
			plan,
			resuming: None,
			cursor: None,
			outcomes: Vec::new(),
			since_checkpoint: 0,
			pruned: 0,
			bytes: 0,
			consumed: 0,
			effects: Vec::new(),
		}
	}

	/// Write what is pending to the journal and checkpoint, then hand back
	/// the run.
	pub(crate) async fn finish(mut self) -> JobResult<MergeRun> {
		self.checkpoint().await?;
		Ok(MergeRun {
			outcomes: self.outcomes,
			cursor: self.cursor,
			pruned: self.pruned,
			bytes: self.bytes,
			consumed: self.consumed,
		})
	}

	/// Settle every entry of one source directory, children of a directory
	/// before its next sibling, and prune the directory afterward when the
	/// merge consumes.
	pub(crate) fn directory<'s>(
		&'s mut self,
		source: usize,
		root: &'s Path,
		directory: &'s Path,
		destination: &'s Path,
		relative: &'s str,
	) -> std::pin::Pin<Box<dyn std::future::Future<Output = JobResult<()>> + Send + 's>> {
		Box::pin(async move {
			let mut names = Vec::new();
			let mut entries = tokio::fs::read_dir(directory)
				.await
				.map_err(|e| JobError::execution(format!("read {}: {e}", directory.display())))?;
			while let Some(entry) = entries
				.next_entry()
				.await
				.map_err(|e| JobError::execution(format!("read {}: {e}", directory.display())))?
			{
				names.push(entry.file_name());
			}
			names.sort();

			for name in names {
				self.ctx.check_interrupt().await?;
				let name = name.to_string_lossy().into_owned();
				let relative = if relative.is_empty() {
					name.clone()
				} else {
					format!("{relative}/{name}")
				};
				let from = directory.join(&name);
				let to = destination.join(&relative);
				let Ok(meta) = tokio::fs::symlink_metadata(&from).await else {
					continue;
				};

				if meta.is_dir() {
					let result = if self.done(source, &relative) {
						None
					} else {
						Some(self.settle_directory(&to).await)
					};
					let descend = !matches!(result, Some(MergeResult::Conflict { .. }));
					if let Some(result) = result {
						if result == MergeResult::CreatedDirectory {
							let subject = tokio::fs::symlink_metadata(&to).await.ok();
							self.effects
								.push(Effect::created(to.clone(), subject.as_ref()));
						}
						self.record(source, relative.clone(), result).await?;
					}
					if descend {
						self.directory(source, root, &from, destination, &relative)
							.await?;
						if self.consume && tokio::fs::remove_dir(&from).await.is_ok() {
							self.pruned += 1;
							self.effects.push(Effect::removed(from.clone()));
						}
					}
					continue;
				}

				if self.done(source, &relative) {
					continue;
				}
				let settled = if meta.file_type().is_symlink() {
					self.settle_link(&from, &to).await
				} else {
					self.settle_file(&from, &to, meta.len()).await
				};
				let landed = settled.written.clone().unwrap_or_else(|| to.clone());
				let consumed = self.consume
					&& matches!(
						settled.result,
						MergeResult::Copied { .. }
							| MergeResult::Replaced { .. }
							| MergeResult::Skipped {
								reason: SkipReason::DuplicateConfirmed | SkipReason::Junk
							}
					);
				let mut removed = settled.moved;
				if consumed && !settled.moved {
					match tokio::fs::remove_file(&from).await {
						Ok(()) => removed = true,
						Err(error) => self
							.ctx
							.log(format!("Could not remove {}: {error}", from.display())),
					}
				}
				if consumed && removed {
					self.consumed += 1;
				}
				self.journal_leaf(&from, &landed, &settled, removed).await;

				// A leaf kept beside the existing file is recorded where it
				// was written, which is where the plan put it too.
				let written = settled
					.written
					.as_deref()
					.and_then(|path| path.strip_prefix(destination).ok())
					.map(|path| {
						path.to_string_lossy()
							.replace(std::path::MAIN_SEPARATOR, "/")
					})
					.unwrap_or(relative);
				self.record(source, written, settled.result).await?;
			}
			Ok(())
		})
	}

	/// The effects one settled leaf had, for the journal.
	async fn journal_leaf(&mut self, from: &Path, landed: &Path, settled: &Settled, removed: bool) {
		let subject = tokio::fs::symlink_metadata(landed).await.ok();
		match &settled.result {
			MergeResult::Copied { .. } => {
				if removed {
					self.effects.push(Effect::moved(
						from.to_path_buf(),
						landed.to_path_buf(),
						subject.as_ref(),
					));
				} else {
					self.effects
						.push(Effect::created(landed.to_path_buf(), subject.as_ref()));
				}
			}
			MergeResult::Replaced { .. } => {
				self.effects.push(Effect::replaced(
					landed.to_path_buf(),
					settled.previous.clone(),
					subject.as_ref(),
				));
				if removed {
					self.effects.push(Effect::moved(
						from.to_path_buf(),
						landed.to_path_buf(),
						subject.as_ref(),
					));
				}
			}
			MergeResult::Skipped { .. } if removed => {
				self.effects.push(Effect::removed(from.to_path_buf()));
			}
			_ => {}
		}
	}

	/// Whether a resumed job has already settled this leaf. The cursor is
	/// the last leaf done; reaching it ends the skipping.
	fn done(&mut self, source: usize, relative: &str) -> bool {
		let Some(cursor) = &self.resuming else {
			return false;
		};
		if cursor.source == source && cursor.path == relative {
			self.resuming = None;
		}
		true
	}

	async fn settle_directory(&self, to: &Path) -> MergeResult {
		match tokio::fs::metadata(to).await {
			Ok(meta) if meta.is_dir() => MergeResult::MergedInto,
			Ok(_) => MergeResult::Conflict {
				kind: ConflictKind::FileVsDirectory,
			},
			Err(_) => match tokio::fs::create_dir(to).await {
				Ok(()) => MergeResult::CreatedDirectory,
				Err(error) => MergeResult::Failed {
					error: error.to_string(),
				},
			},
		}
	}

	/// Settle a symlink.
	async fn settle_link(&mut self, from: &Path, to: &Path) -> Settled {
		let target = match tokio::fs::read_link(from).await {
			Ok(target) => target,
			Err(error) => {
				return Settled::of(MergeResult::Failed {
					error: error.to_string(),
				})
			}
		};
		match tokio::fs::symlink_metadata(to).await {
			Err(_) => self.place_link(from, &target, to).await,
			Ok(meta) if meta.file_type().is_symlink() => {
				if tokio::fs::read_link(to).await.ok().as_deref() == Some(target.as_path()) {
					return Settled::of(MergeResult::Skipped {
						reason: SkipReason::DuplicateConfirmed,
					});
				}
				match self.resolve(from, to, None).await {
					Resolution::Replace => {
						let previous = match self.stash(to).await {
							Ok(previous) => previous,
							Err(error) => {
								return Settled::of(MergeResult::Failed {
									error: error.to_string(),
								})
							}
						};
						let mut settled = self.place_link(from, &target, to).await;
						if let MergeResult::Copied { bytes } = settled.result {
							settled.result = MergeResult::Replaced { bytes };
						}
						settled.previous = previous;
						settled
					}
					Resolution::KeepBoth(renamed) => {
						let mut settled = self.place_link(from, &target, &renamed).await;
						settled.written = Some(renamed);
						settled
					}
					Resolution::Skip => Settled::of(MergeResult::Skipped {
						reason: SkipReason::Policy,
					}),
				}
			}
			Ok(meta) if meta.is_dir() => Settled::of(MergeResult::Conflict {
				kind: ConflictKind::FileVsDirectory,
			}),
			Ok(_) => Settled::of(MergeResult::Conflict {
				kind: ConflictKind::LinkVsFile,
			}),
		}
	}

	/// A link at `at`: renamed from the source when moving, else made anew.
	async fn place_link(&self, from: &Path, target: &Path, at: &Path) -> Settled {
		if self.moving {
			return match tokio::fs::rename(from, at).await {
				Ok(()) => Settled {
					result: MergeResult::Copied { bytes: 0 },
					written: None,
					previous: None,
					moved: true,
				},
				Err(error) => Settled::of(MergeResult::Failed {
					error: error.to_string(),
				}),
			};
		}
		Settled::of(link(target, at).await)
	}

	/// Settle a file.
	async fn settle_file(&mut self, from: &Path, to: &Path, size: u64) -> Settled {
		let name = from
			.file_name()
			.map(|name| name.to_string_lossy().into_owned())
			.unwrap_or_default();
		if is_junk(&name) {
			return Settled::of(MergeResult::Skipped {
				reason: SkipReason::Junk,
			});
		}
		match tokio::fs::symlink_metadata(to).await {
			Err(_) => self.place(from, to).await,
			Ok(meta) if meta.is_dir() => Settled::of(MergeResult::Conflict {
				kind: ConflictKind::FileVsDirectory,
			}),
			Ok(meta) if meta.file_type().is_symlink() => Settled::of(MergeResult::Conflict {
				kind: ConflictKind::LinkVsFile,
			}),
			Ok(meta) => {
				if meta.len() == size {
					match identical(from, to).await {
						Ok(true) => {
							return Settled::of(MergeResult::Skipped {
								reason: SkipReason::DuplicateConfirmed,
							})
						}
						Ok(false) => {}
						Err(error) => return Settled::of(MergeResult::Failed { error }),
					}
				}
				match self.resolve(from, to, Some(&meta)).await {
					Resolution::Replace => {
						let previous = match self.stash(to).await {
							Ok(previous) => previous,
							Err(error) => {
								return Settled::of(MergeResult::Failed {
									error: error.to_string(),
								})
							}
						};
						let mut settled = self.place(from, to).await;
						if let MergeResult::Copied { bytes } = settled.result {
							settled.result = MergeResult::Replaced { bytes };
						}
						settled.previous = previous;
						settled
					}
					Resolution::KeepBoth(renamed) => {
						let mut settled = self.place(from, &renamed).await;
						settled.written = Some(renamed);
						settled
					}
					Resolution::Skip => Settled::of(MergeResult::Skipped {
						reason: SkipReason::Policy,
					}),
				}
			}
		}
	}

	/// The previous bytes at `to`, moved to the trash with their location
	/// recorded, so the replacement can be undone.
	async fn stash(&self, to: &Path) -> std::io::Result<Option<PathBuf>> {
		trash::trash(to, self.ctx.volume_manager().as_deref(), self.ctx.id()).await
	}

	/// A file at `to`: renamed from the source when moving, else copied
	/// with the strategy the router picks.
	async fn place(&self, from: &Path, to: &Path) -> Settled {
		if self.moving {
			let size = tokio::fs::symlink_metadata(from)
				.await
				.map(|meta| meta.len())
				.unwrap_or(0);
			return match tokio::fs::rename(from, to).await {
				Ok(()) => Settled {
					result: MergeResult::Copied { bytes: size },
					written: None,
					previous: None,
					moved: true,
				},
				Err(error) => Settled::of(MergeResult::Failed {
					error: error.to_string(),
				}),
			};
		}
		Settled::of(self.copy(from, to).await)
	}

	/// What the policy makes of a collision at execution.
	async fn resolve(
		&self,
		from: &Path,
		to: &Path,
		existing: Option<&std::fs::Metadata>,
	) -> Resolution {
		match self.policy {
			MergeConflictPolicy::Skip => Resolution::Skip,
			MergeConflictPolicy::Overwrite => Resolution::Replace,
			MergeConflictPolicy::KeepBoth => Resolution::KeepBoth(unique_name(to).await),
			MergeConflictPolicy::KeepNewer => {
				let incoming = tokio::fs::symlink_metadata(from)
					.await
					.ok()
					.and_then(|meta| meta.modified().ok());
				let existing = existing.and_then(|meta| meta.modified().ok());
				match (incoming, existing) {
					(Some(incoming), Some(existing)) if incoming > existing => Resolution::Replace,
					_ => Resolution::Skip,
				}
			}
		}
	}

	/// Copy one file with the strategy the router picks for it.
	async fn copy(&self, from: &Path, to: &Path) -> MergeResult {
		let (source, destination) = (SdPath::local(from), SdPath::local(to));
		let (strategy, _) = CopyStrategyRouter::select_strategy_with_metadata(
			&source,
			&destination,
			false,
			&CopyMethod::Auto,
			self.ctx.volume_manager().as_deref(),
		)
		.await;
		match strategy
			.execute(self.ctx, &source, &destination, false, None)
			.await
		{
			Ok(bytes) => MergeResult::Copied { bytes },
			Err(error) => MergeResult::Failed {
				error: error.to_string(),
			},
		}
	}

	/// Keep the outcome, move the cursor past the leaf, and checkpoint every
	/// so often.
	async fn record(&mut self, source: usize, path: String, result: MergeResult) -> JobResult<()> {
		if let MergeResult::Copied { bytes } | MergeResult::Replaced { bytes } = &result {
			self.bytes += *bytes;
		}
		let planned = self.plan.get(&path).cloned();
		// Junk the index never held is not a surprise; anything else the
		// plan did not know is.
		let diverged = match &planned {
			Some(planned) => !agrees(planned, &result),
			None => {
				!self.plan.is_empty()
					&& !matches!(
						result,
						MergeResult::Skipped {
							reason: SkipReason::Junk
						}
					)
			}
		};
		if let MergeResult::Failed { error } = &result {
			self.ctx.log(format!("{path}: {error}"));
		}
		self.outcomes.push(MergeOutcome {
			source,
			path: path.clone(),
			result,
			planned,
			diverged,
		});
		self.cursor = Some(Cursor { source, path });

		let done = self.outcomes.len() as u64;
		self.ctx.progress(Progress::generic(
			GenericProgress::new(
				(done as f32 / self.total.max(done).max(1) as f32).min(1.0),
				"Merging",
				format!("{done} of {} settled", self.total.max(done)),
			)
			.with_completion(done, self.total.max(done))
			.with_bytes(self.bytes, self.bytes),
		));

		self.since_checkpoint += 1;
		if self.since_checkpoint >= CHECKPOINT_EVERY {
			self.checkpoint().await?;
		}
		Ok(())
	}

	/// The journal first, so a checkpoint never claims a leaf whose effects
	/// are not written.
	async fn checkpoint(&mut self) -> JobResult<()> {
		self.since_checkpoint = 0;
		self.ctx.record(std::mem::take(&mut self.effects)).await;
		self.ctx
			.checkpoint_with_state(&Resume {
				cursor: self.cursor.clone(),
				outcomes: self.outcomes.clone(),
			})
			.await
	}
}

enum Resolution {
	Replace,
	KeepBoth(PathBuf),
	Skip,
}

/// Whether two files hold the same bytes, by integrity hash computed now.
async fn identical(a: &Path, b: &Path) -> Result<bool, String> {
	let ours = ContentHashGenerator::generate_integrity_hash(a)
		.await
		.map_err(|e| e.to_string())?;
	let theirs = ContentHashGenerator::generate_integrity_hash(b)
		.await
		.map_err(|e| e.to_string())?;
	Ok(ours == theirs)
}

/// A numbered name beside `path` that nothing holds, in the convention copy
/// uses.
pub(crate) async fn unique_name(path: &Path) -> PathBuf {
	let parent = path.parent().unwrap_or(Path::new(""));
	let name = path
		.file_name()
		.map(|name| name.to_string_lossy().into_owned())
		.unwrap_or_default();
	let (stem, extension) = match name.rsplit_once('.') {
		Some((stem, extension)) => (stem.to_string(), format!(".{extension}")),
		None => (name.clone(), String::new()),
	};
	for counter in 1.. {
		let candidate = parent.join(format!("{stem} ({counter}){extension}"));
		if tokio::fs::symlink_metadata(&candidate).await.is_err() {
			return candidate;
		}
	}
	unreachable!("the counter never runs out")
}

async fn link(target: &Path, at: &Path) -> MergeResult {
	#[cfg(unix)]
	let made = tokio::fs::symlink(target, at).await;
	#[cfg(windows)]
	let made = tokio::fs::symlink_file(target, at).await;
	match made {
		Ok(()) => MergeResult::Copied { bytes: 0 },
		Err(error) => MergeResult::Failed {
			error: error.to_string(),
		},
	}
}

/// Whether an outcome is what the plan said. A candidate duplicate the job
/// confirmed is agreement: the plan said the same thing with less proof.
fn agrees(planned: &ChangeKind, result: &MergeResult) -> bool {
	match (planned, result) {
		(ChangeKind::Create { .. }, MergeResult::Copied { .. }) => true,
		(ChangeKind::CreateDirectory, MergeResult::CreatedDirectory) => true,
		(ChangeKind::Replace { .. }, MergeResult::Replaced { .. }) => true,
		(ChangeKind::MergeInto, MergeResult::MergedInto) => true,
		(ChangeKind::Skip { reason: planned }, MergeResult::Skipped { reason }) => {
			planned == reason
				|| (*planned == SkipReason::DuplicateCandidate
					&& *reason == SkipReason::DuplicateConfirmed)
		}
		(ChangeKind::Conflict { kind: planned }, MergeResult::Conflict { kind }) => planned == kind,
		_ => false,
	}
}

/// Trash every file beneath the destination that no source holds at the
/// same relative path, then prune the directories no source has, bottom
/// up. Junk is left alone, as it is never copied. Answers with how many
/// files went.
async fn remove_extras(
	ctx: &JobContext<'_>,
	sources: &[PathBuf],
	destination: &Path,
) -> JobResult<u64> {
	let mut removed = 0;
	let mut directories: Vec<PathBuf> = Vec::new();
	let mut stack = vec![destination.to_path_buf()];
	while let Some(directory) = stack.pop() {
		let mut entries = match tokio::fs::read_dir(&directory).await {
			Ok(entries) => entries,
			Err(error) => {
				ctx.log(format!("Could not read {}: {error}", directory.display()));
				continue;
			}
		};
		while let Ok(Some(entry)) = entries.next_entry().await {
			ctx.check_interrupt().await?;
			let path = entry.path();
			let Ok(relative) = path.strip_prefix(destination) else {
				continue;
			};
			let name = entry.file_name().to_string_lossy().into_owned();
			if relative.components().count() == 1 && name == ".spacedrive" {
				continue;
			}
			let Ok(meta) = tokio::fs::symlink_metadata(&path).await else {
				continue;
			};
			let mut held = false;
			for source in sources {
				if tokio::fs::symlink_metadata(source.join(relative))
					.await
					.is_ok()
				{
					held = true;
					break;
				}
			}
			if meta.is_dir() {
				stack.push(path.clone());
				if !held {
					directories.push(path);
				}
				continue;
			}
			if held || is_junk(&name) {
				continue;
			}
			match trash::trash(&path, ctx.volume_manager().as_deref(), ctx.id()).await {
				Ok(location) => {
					let subject = match &location {
						Some(location) => tokio::fs::symlink_metadata(location).await.ok(),
						None => None,
					};
					ctx.record(vec![Effect::trashed(path, location, subject.as_ref())])
						.await;
					removed += 1;
				}
				Err(error) => ctx.log(format!("Could not remove {}: {error}", path.display())),
			}
		}
	}
	// Deepest first, so a directory is empty by the time its turn comes.
	directories.sort_by(|a, b| b.components().count().cmp(&a.components().count()));
	for directory in directories {
		if tokio::fs::remove_dir(&directory).await.is_ok() {
			ctx.record(vec![Effect::removed(directory)]).await;
		}
	}
	Ok(removed)
}

/// Tag assertions still standing on records under the sources.
async fn assertions_left(ctx: &JobContext<'_>, sources: &[SdPath]) -> i64 {
	let context = ctx.library().core_context();
	let mut count = 0;
	for source in sources {
		for reach in stores_beneath(context, source).await {
			let Some(db) = context.volume_index().read_store(reach.source.id).await else {
				continue;
			};
			count += sd_store::read::assertions_beneath(db.pool(), &reach.scope)
				.await
				.unwrap_or(0);
		}
	}
	count
}

#[cfg(test)]
mod tests {
	use super::*;

	/// The job agrees with a plan that said the same with less proof, and
	/// disagrees where the filesystem changed.
	#[test]
	fn an_outcome_agrees_with_the_plan_it_confirms() {
		let candidate = ChangeKind::Skip {
			reason: SkipReason::DuplicateCandidate,
		};
		assert!(agrees(
			&candidate,
			&MergeResult::Skipped {
				reason: SkipReason::DuplicateConfirmed
			}
		));
		assert!(agrees(
			&ChangeKind::Create { size: 1 },
			&MergeResult::Copied { bytes: 1 }
		));
		assert!(!agrees(&candidate, &MergeResult::Copied { bytes: 1 }));
		assert!(!agrees(
			&ChangeKind::Create { size: 1 },
			&MergeResult::Failed {
				error: "gone".into()
			}
		));
	}
}
