//! The rename itself.
//!
//! The set is resolved again against the live filesystem when the job
//! starts, so a name that would overwrite a file is refused here as it was
//! in validation. The renames then run in an order that never overwrites:
//! a rename whose new name another file of the set still holds waits for
//! that file to move, and a cycle, `a` to `b` while `b` goes to `a`, is
//! broken by parking one file under a temporary name in its directory.
//! Each rename is one `rename` call, so records keep their identity.

use std::{
	path::{Path, PathBuf},
	time::Instant,
};

use serde::{Deserialize, Serialize};
use specta::Type;

use super::resolve::{resolve, Rename, RenameSet};
use crate::{
	domain::SdPath,
	infra::job::{generic_progress::GenericProgress, journal::Effect, prelude::*},
};

/// Renames between checkpoints.
const CHECKPOINT_EVERY: usize = 50;

#[derive(Debug, Serialize, Deserialize, Job)]
pub struct RenameJob {
	pub set: RenameSet,
	/// The original paths already renamed, for a resumed job.
	pub done: Vec<PathBuf>,
	#[serde(skip, default = "Instant::now")]
	started_at: Instant,
}

impl RenameJob {
	pub fn named(target: SdPath, new_name: String) -> Self {
		Self::named_many(vec![(target, new_name)])
	}

	pub fn named_many(pairs: Vec<(SdPath, String)>) -> Self {
		Self::new(RenameSet::Named(pairs))
	}

	pub fn ruled(targets: Vec<SdPath>, rules: Vec<super::rules::RenameRule>) -> Self {
		Self::new(RenameSet::Ruled { targets, rules })
	}

	fn new(set: RenameSet) -> Self {
		Self {
			set,
			done: Vec::new(),
			started_at: Instant::now(),
		}
	}
}

impl Job for RenameJob {
	const NAME: &'static str = "rename_files";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> = Some("Rename files");
}

impl crate::infra::job::traits::DynJob for RenameJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct RenameOutput {
	pub renamed: u64,
	pub unchanged: u64,
	/// Targets a finding refused, with the finding's message.
	pub refused: Vec<RenameProblem>,
	pub failed: Vec<RenameProblem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RenameProblem {
	pub path: PathBuf,
	pub reason: String,
}

impl From<RenameOutput> for JobOutput {
	fn from(output: RenameOutput) -> Self {
		JobOutput::custom(output)
	}
}

#[derive(Debug, Serialize, Deserialize)]
struct Resume {
	done: Vec<PathBuf>,
}

#[async_trait::async_trait]
impl JobHandler for RenameJob {
	type Output = RenameOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		if let Some(resume) = ctx.load_state::<Resume>().await? {
			self.done = resume.done;
			ctx.log(format!("Resuming past {} renames", self.done.len()));
		}
		let context = ctx.library().core_context();
		ctx.progress(Progress::Indeterminate("Resolving names".to_string()));
		let resolved = resolve(&context.volume_manager, context.volume_index(), &self.set).await;

		let mut output = RenameOutput {
			unchanged: resolved.unchanged,
			..Default::default()
		};
		for finding in resolved
			.findings
			.iter()
			.filter(|finding| finding.severity == crate::infra::action::preflight::Severity::Error)
		{
			output.refused.push(RenameProblem {
				path: finding
					.path
					.as_ref()
					.and_then(|path| path.as_local_path())
					.map(Path::to_path_buf)
					.unwrap_or_default(),
				reason: finding.message.clone(),
			});
			ctx.log(format!("Refused: {}", finding.message));
		}

		// Each rename with where its file is right now, which differs from
		// its original path once the file is parked mid-cycle.
		let mut pending: Vec<(Rename, PathBuf)> = resolved
			.renames
			.into_iter()
			.filter(|rename| !rename.refused)
			.filter(|rename| !self.done.contains(&rename.from))
			.map(|rename| {
				let at = rename.from.clone();
				(rename, at)
			})
			.collect();
		let total = pending.len() as u64 + self.done.len() as u64;
		output.renamed = self.done.len() as u64;
		ctx.log(format!("{} names to change", pending.len()));

		let mut since_checkpoint = 0;
		while !pending.is_empty() {
			ctx.check_interrupt().await?;
			let mut progressed = false;
			let mut remaining = Vec::with_capacity(pending.len());
			for (rename, at) in pending {
				let free =
					rename.case_only || tokio::fs::symlink_metadata(&rename.to).await.is_err();
				if !free {
					remaining.push((rename, at));
					continue;
				}
				progressed = true;
				match tokio::fs::rename(&at, &rename.to).await {
					Ok(()) => {
						let subject = tokio::fs::symlink_metadata(&rename.to).await.ok();
						ctx.record(vec![Effect::moved(
							rename.from.clone(),
							rename.to.clone(),
							subject.as_ref(),
						)])
						.await;
						output.renamed += 1;
						self.done.push(rename.from.clone());
						since_checkpoint += 1;
					}
					Err(error) => {
						ctx.log(format!("{}: {error}", rename.from.display()));
						output.failed.push(RenameProblem {
							path: rename.from.clone(),
							reason: error.to_string(),
						});
					}
				}
				ctx.progress(Progress::generic(
					GenericProgress::new(
						(output.renamed as f32 / total.max(1) as f32).min(1.0),
						"Renaming",
						format!("{} of {total}", output.renamed),
					)
					.with_completion(output.renamed, total)
					.with_errors(output.failed.len() as u64, output.refused.len() as u64),
				));
				if since_checkpoint >= CHECKPOINT_EVERY {
					since_checkpoint = 0;
					ctx.checkpoint_with_state(&Resume {
						done: self.done.clone(),
					})
					.await?;
				}
			}
			pending = remaining;
			if !progressed && !pending.is_empty() {
				// Every remaining name is held by another file of the set:
				// a cycle. Park the first file under a temporary name so
				// its own name frees up, and let it wait for its turn. The
				// journal sees the whole move, from the original name to
				// the final one, once it lands.
				let (first, at) = &mut pending[0];
				let parked = temporary_name(&first.from);
				match tokio::fs::rename(&*at, &parked).await {
					Ok(()) => {
						ctx.log(format!(
							"Parked {} as {} to break a cycle",
							at.display(),
							parked.display()
						));
						*at = parked;
					}
					Err(error) => {
						let (rename, _) = pending.remove(0);
						output.failed.push(RenameProblem {
							path: rename.from,
							reason: error.to_string(),
						});
					}
				}
			}
		}

		ctx.progress(Progress::generic(
			GenericProgress::new(
				1.0,
				"Complete",
				format!(
					"{} renamed, {} refused",
					output.renamed,
					output.refused.len()
				),
			)
			.with_completion(total, total)
			.with_performance(0.0, None, Some(self.started_at.elapsed()))
			.with_errors(output.failed.len() as u64, output.refused.len() as u64),
		));
		ctx.log(format!(
			"Rename completed: {} renamed, {} unchanged, {} refused, {} failed",
			output.renamed,
			output.unchanged,
			output.refused.len(),
			output.failed.len()
		));
		Ok(output)
	}
}

/// A name beside `path` that nothing holds, for a file parked mid-cycle.
fn temporary_name(path: &Path) -> PathBuf {
	let name = path
		.file_name()
		.map(|name| name.to_string_lossy().into_owned())
		.unwrap_or_default();
	let short = uuid::Uuid::new_v4().simple().to_string();
	path.with_file_name(format!(".{name}.renaming-{}", &short[..8]))
}
