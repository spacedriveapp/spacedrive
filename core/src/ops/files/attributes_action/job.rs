use std::{path::PathBuf, time::Instant};

use serde::{Deserialize, Serialize};
use specta::Type;

use super::{action::FileSetAttributesInput, fs};
use crate::infra::job::{generic_progress::GenericProgress, journal::Effect, prelude::*};

#[derive(Debug, Serialize, Deserialize, Job)]
pub struct AttributesJob {
	pub input: FileSetAttributesInput,
	#[serde(skip, default = "Instant::now")]
	started_at: Instant,
}

impl AttributesJob {
	pub fn new(input: FileSetAttributesInput) -> Self {
		Self {
			input,
			started_at: Instant::now(),
		}
	}
}

impl Job for AttributesJob {
	const NAME: &'static str = "set_attributes";
	const RESUMABLE: bool = false;
	const DESCRIPTION: Option<&'static str> = Some("Set file attributes");
}

impl crate::infra::job::traits::DynJob for AttributesJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct AttributesOutput {
	pub changed: u64,
	pub unchanged: u64,
	pub failed: Vec<AttributesProblem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AttributesProblem {
	pub path: PathBuf,
	pub reason: String,
}

impl From<AttributesOutput> for JobOutput {
	fn from(output: AttributesOutput) -> Self {
		JobOutput::custom(output)
	}
}

#[async_trait::async_trait]
impl JobHandler for AttributesJob {
	type Output = AttributesOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		let mut output = AttributesOutput::default();
		let total = self.input.paths.len() as u64;
		for (done, path) in self.input.paths.iter().enumerate() {
			ctx.check_interrupt().await?;
			let Some(local) = path.as_local_path() else {
				continue;
			};
			let before = match fs::read(local).await {
				Ok(before) => before,
				Err(error) => {
					output.failed.push(AttributesProblem {
						path: local.to_path_buf(),
						reason: error.to_string(),
					});
					continue;
				}
			};
			if !fs::differs(&before, &self.input.attributes) {
				output.unchanged += 1;
				continue;
			}
			let to = fs::changing(&before, &self.input.attributes);
			match fs::apply(local, &to).await {
				Ok(()) => {
					let from = fs::changing(&to, &before);
					ctx.record(vec![Effect::Attributes {
						path: local.to_path_buf(),
						from,
						to,
					}])
					.await;
					output.changed += 1;
				}
				Err(error) => {
					ctx.log(format!("{}: {error}", local.display()));
					output.failed.push(AttributesProblem {
						path: local.to_path_buf(),
						reason: error.to_string(),
					});
				}
			}
			ctx.progress(Progress::generic(
				GenericProgress::new(
					((done + 1) as f32 / total.max(1) as f32).min(1.0),
					"Setting attributes",
					format!("{} of {total}", done + 1),
				)
				.with_completion(done as u64 + 1, total)
				.with_errors(output.failed.len() as u64, 0),
			));
		}
		ctx.progress(Progress::generic(
			GenericProgress::new(1.0, "Complete", format!("{} changed", output.changed))
				.with_completion(total, total)
				.with_performance(0.0, None, Some(self.started_at.elapsed()))
				.with_errors(output.failed.len() as u64, 0),
		));
		Ok(output)
	}
}
