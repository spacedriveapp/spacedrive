//! Organize and flatten: the moves, from the index, applied as renames.
//!
//! The places are read again from the index when the job starts, so the job
//! moves what the plan showed and nothing the folder gained since goes
//! anywhere unplanned. Every move is one `rename` inside the folder, so
//! records keep their identity; a place taken by the time its turn comes is
//! left with why. Flatten prunes the directories it emptied afterward,
//! deepest first. Every move, folder created and folder pruned is journaled.

use std::{path::PathBuf, time::Instant};

use serde::{Deserialize, Serialize};
use specta::Type;

use super::{
	input::{FileFlattenInput, FileOrganizeInput},
	plan::{directories_beneath, flatten, organize, PlanError, Rearrangement},
};
use crate::infra::job::{generic_progress::GenericProgress, journal::Effect, prelude::*};

const CHECKPOINT_EVERY: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Rearrange {
	Organize(FileOrganizeInput),
	Flatten(FileFlattenInput),
}

#[derive(Debug, Serialize, Deserialize, Job)]
pub struct RearrangeJob {
	pub what: Rearrange,
	/// Origins already moved, for a resumed job.
	pub done: Vec<PathBuf>,
	#[serde(skip, default = "Instant::now")]
	started_at: Instant,
}

impl RearrangeJob {
	pub fn new(what: Rearrange) -> Self {
		Self {
			what,
			done: Vec::new(),
			started_at: Instant::now(),
		}
	}
}

impl Job for RearrangeJob {
	const NAME: &'static str = "rearrange_files";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> = Some("Organize or flatten a folder");
}

impl crate::infra::job::traits::DynJob for RearrangeJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}

	fn dedup_key(&self) -> Option<String> {
		Some(match &self.what {
			Rearrange::Organize(input) => format!("organize {}", input.scope),
			Rearrange::Flatten(input) => format!("flatten {}", input.scope),
		})
	}
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct RearrangeOutput {
	pub moved: u64,
	pub directories_created: u64,
	pub pruned_directories: u64,
	/// Files left where they were, with why.
	pub left: Vec<RearrangeProblem>,
	pub failed: Vec<RearrangeProblem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RearrangeProblem {
	pub path: PathBuf,
	pub reason: String,
}

impl From<RearrangeOutput> for JobOutput {
	fn from(output: RearrangeOutput) -> Self {
		JobOutput::custom(output)
	}
}

#[derive(Debug, Serialize, Deserialize)]
struct Resume {
	done: Vec<PathBuf>,
}

#[async_trait::async_trait]
impl JobHandler for RearrangeJob {
	type Output = RearrangeOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		if let Some(resume) = ctx.load_state::<Resume>().await? {
			self.done = resume.done;
			ctx.log(format!("Resuming past {} moves", self.done.len()));
		}
		let context = ctx.library().core_context();
		ctx.progress(Progress::Indeterminate("Planning".to_string()));
		let (rearrangement, scope): (Rearrangement, PathBuf) = match &self.what {
			Rearrange::Organize(input) => (
				organize(&context.volume_manager, context.volume_index(), input)
					.await
					.map_err(failed)?,
				input
					.scope
					.as_local_path()
					.map(PathBuf::from)
					.unwrap_or_default(),
			),
			Rearrange::Flatten(input) => (
				flatten(&context.volume_manager, context.volume_index(), input)
					.await
					.map_err(failed)?,
				input
					.scope
					.as_local_path()
					.map(PathBuf::from)
					.unwrap_or_default(),
			),
		};
		let mut output = RearrangeOutput::default();
		for (from, wanted, _) in &rearrangement.left {
			output.left.push(RearrangeProblem {
				path: from.clone(),
				reason: format!("{} is taken", wanted.display()),
			});
		}

		for directory in &rearrangement.directories {
			match tokio::fs::create_dir_all(directory).await {
				Ok(()) => {
					let subject = tokio::fs::symlink_metadata(directory).await.ok();
					ctx.record(vec![Effect::created(directory.clone(), subject.as_ref())])
						.await;
					output.directories_created += 1;
				}
				Err(error) => {
					return Err(JobError::execution(format!(
						"could not create {}: {error}",
						directory.display()
					)))
				}
			}
		}

		let total = rearrangement.moves.len() as u64;
		ctx.log(format!("{total} files to move"));
		let mut since_checkpoint = 0;
		for planned in rearrangement.moves {
			ctx.check_interrupt().await?;
			if self.done.contains(&planned.from) {
				output.moved += 1;
				continue;
			}
			if tokio::fs::symlink_metadata(&planned.to).await.is_ok() {
				output.left.push(RearrangeProblem {
					path: planned.from,
					reason: format!("{} is taken", planned.to.display()),
				});
				continue;
			}
			match tokio::fs::rename(&planned.from, &planned.to).await {
				Ok(()) => {
					let subject = tokio::fs::symlink_metadata(&planned.to).await.ok();
					ctx.record(vec![Effect::moved(
						planned.from.clone(),
						planned.to.clone(),
						subject.as_ref(),
					)])
					.await;
					output.moved += 1;
					self.done.push(planned.from);
					since_checkpoint += 1;
					if since_checkpoint >= CHECKPOINT_EVERY {
						since_checkpoint = 0;
						ctx.checkpoint_with_state(&Resume {
							done: self.done.clone(),
						})
						.await?;
					}
				}
				Err(error) => {
					ctx.log(format!("{}: {error}", planned.from.display()));
					output.failed.push(RearrangeProblem {
						path: planned.from,
						reason: error.to_string(),
					});
				}
			}
			ctx.progress(Progress::generic(
				GenericProgress::new(
					(output.moved as f32 / total.max(1) as f32).min(1.0),
					"Moving",
					format!("{} of {total}", output.moved),
				)
				.with_completion(output.moved, total)
				.with_errors(output.failed.len() as u64, output.left.len() as u64),
			));
		}

		if matches!(self.what, Rearrange::Flatten(_)) {
			for directory in directories_beneath(&scope).await {
				if tokio::fs::remove_dir(&directory).await.is_ok() {
					ctx.record(vec![Effect::removed(directory)]).await;
					output.pruned_directories += 1;
				}
			}
		}

		ctx.progress(Progress::generic(
			GenericProgress::new(
				1.0,
				"Complete",
				format!("{} moved, {} left", output.moved, output.left.len()),
			)
			.with_completion(total, total)
			.with_performance(0.0, None, Some(self.started_at.elapsed()))
			.with_errors(output.failed.len() as u64, output.left.len() as u64),
		));
		ctx.log(format!(
			"Rearrange completed: {} moved, {} folders created, {} pruned, {} left, {} failed",
			output.moved,
			output.directories_created,
			output.pruned_directories,
			output.left.len(),
			output.failed.len()
		));
		Ok(output)
	}
}

fn failed(error: PlanError) -> JobError {
	JobError::execution(error.to_string())
}
