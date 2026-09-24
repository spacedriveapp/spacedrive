//! Undoing a job, effect by effect, newest first.
//!
//! The journal is reversed as it stands when the undo runs: each effect's
//! subject is checked again, so a file that changed between the preview and
//! the run is left alone and reported. The undo writes its own journal, so
//! undoing an undo is the same action over it.

use std::{path::PathBuf, time::Instant};

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use super::reverse::{reversals, Left, Step};
use crate::infra::job::{
	generic_progress::GenericProgress,
	journal::{Attributes, Effect},
	prelude::*,
	types::JobId,
};
use crate::ops::files::trash;

#[derive(Debug, Serialize, Deserialize, Job)]
pub struct UndoJob {
	pub job: Uuid,
	pub effects: Option<Vec<i64>>,
	/// Sequences already reversed, for a resumed job.
	pub done: Vec<i64>,
	#[serde(skip, default = "Instant::now")]
	started_at: Instant,
}

impl UndoJob {
	pub fn new(job: Uuid, effects: Option<Vec<i64>>) -> Self {
		Self {
			job,
			effects,
			done: Vec::new(),
			started_at: Instant::now(),
		}
	}
}

impl Job for UndoJob {
	const NAME: &'static str = "undo";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> = Some("Undo what a job did");
}

impl crate::infra::job::traits::DynJob for UndoJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}

	fn dedup_key(&self) -> Option<String> {
		Some(self.job.to_string())
	}
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct UndoOutput {
	pub reversed: u64,
	pub left: Vec<UndoLeft>,
	pub failed: Vec<UndoLeft>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UndoLeft {
	pub sequence: i64,
	pub reason: String,
}

impl From<UndoOutput> for JobOutput {
	fn from(output: UndoOutput) -> Self {
		JobOutput::custom(output)
	}
}

#[derive(Debug, Serialize, Deserialize)]
struct Resume {
	done: Vec<i64>,
}

#[async_trait::async_trait]
impl JobHandler for UndoJob {
	type Output = UndoOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		if let Some(resume) = ctx.load_state::<Resume>().await? {
			self.done = resume.done;
		}
		let journal = ctx
			.library()
			.jobs()
			.database()
			.journal(JobId(self.job))
			.await?;
		ctx.log(format!(
			"Undoing job {}: {} effects recorded",
			self.job,
			journal.len()
		));
		let steps = reversals(&journal, self.effects.as_deref()).await;
		let total = steps.len() as u64;
		let mut output = UndoOutput::default();
		let mut handled = 0u64;

		for reversal in steps {
			ctx.check_interrupt().await?;
			handled += 1;
			if self.done.contains(&reversal.sequence) {
				output.reversed += 1;
				continue;
			}
			let step = match reversal.outcome {
				Ok(step) => step,
				Err(left) => {
					let reason = match left {
						Left::Irreversible => "cannot be reversed",
						Left::Changed => "changed since the job ran",
						Left::Occupied => "something else is where it would go back to",
					};
					output.left.push(UndoLeft {
						sequence: reversal.sequence,
						reason: reason.to_string(),
					});
					continue;
				}
			};
			match apply(&ctx, step).await {
				Ok(effects) => {
					ctx.record(effects).await;
					output.reversed += 1;
					self.done.push(reversal.sequence);
					ctx.checkpoint_with_state(&Resume {
						done: self.done.clone(),
					})
					.await?;
				}
				Err(error) => {
					ctx.log(format!("Effect {}: {error}", reversal.sequence));
					output.failed.push(UndoLeft {
						sequence: reversal.sequence,
						reason: error.to_string(),
					});
				}
			}
			ctx.progress(Progress::generic(
				GenericProgress::new(
					(handled as f32 / total.max(1) as f32).min(1.0),
					"Undoing",
					format!("{handled} of {total} effects"),
				)
				.with_completion(handled, total)
				.with_errors(output.failed.len() as u64, output.left.len() as u64),
			));
		}

		ctx.progress(Progress::generic(
			GenericProgress::new(
				1.0,
				"Complete",
				format!("{} reversed, {} left", output.reversed, output.left.len()),
			)
			.with_completion(total, total)
			.with_performance(0.0, None, Some(self.started_at.elapsed()))
			.with_errors(output.failed.len() as u64, output.left.len() as u64),
		));
		ctx.log(format!(
			"Undo completed: {} reversed, {} left, {} failed",
			output.reversed,
			output.left.len(),
			output.failed.len()
		));
		Ok(output)
	}
}

/// Apply one step, answering with what it did for this job's journal.
async fn apply(ctx: &JobContext<'_>, step: Step) -> std::io::Result<Vec<Effect>> {
	match step {
		Step::Trash { path, .. } => {
			let location = trash::trash(&path, ctx.volume_manager().as_deref(), ctx.id()).await?;
			let subject = match &location {
				Some(location) => tokio::fs::symlink_metadata(location).await.ok(),
				None => None,
			};
			Ok(vec![Effect::trashed(path, location, subject.as_ref())])
		}
		Step::MoveBack { from, to, .. } => {
			if let Some(parent) = to.parent() {
				tokio::fs::create_dir_all(parent).await?;
			}
			move_back(&from, &to).await?;
			let subject = tokio::fs::symlink_metadata(&to).await.ok();
			Ok(vec![Effect::moved(from, to, subject.as_ref())])
		}
		Step::Restore { path, previous, .. } => {
			let location = trash::trash(&path, ctx.volume_manager().as_deref(), ctx.id()).await?;
			let stashed = match &location {
				Some(location) => tokio::fs::symlink_metadata(location).await.ok(),
				None => None,
			};
			move_back(&previous, &path).await?;
			let subject = tokio::fs::symlink_metadata(&path).await.ok();
			Ok(vec![
				Effect::trashed(path.clone(), location, stashed.as_ref()),
				Effect::moved(previous, path, subject.as_ref()),
			])
		}
		Step::Attributes { path, to } => {
			let from = crate::ops::files::attributes_action::fs::read(&path).await?;
			crate::ops::files::attributes_action::fs::apply(&path, &to).await?;
			Ok(vec![Effect::Attributes {
				path,
				from,
				to: Attributes {
					mode: to.mode,
					modified_ms: to.modified_ms,
					hidden: to.hidden,
				},
			}])
		}
	}
}

/// A rename where both sit on one volume, and the trash's restore where
/// the item is in the platform's trash.
async fn move_back(from: &PathBuf, to: &PathBuf) -> std::io::Result<()> {
	match tokio::fs::rename(from, to).await {
		Ok(()) => Ok(()),
		Err(_) => trash::restore(from, to).await,
	}
}
