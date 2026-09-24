use std::{path::PathBuf, time::Instant};

use serde::{Deserialize, Serialize};
use specta::Type;

use super::action::{FileLinkInput, LinkKind};
use crate::infra::job::{generic_progress::GenericProgress, journal::Effect, prelude::*};

#[derive(Debug, Serialize, Deserialize, Job)]
pub struct LinkJob {
	pub input: FileLinkInput,
	#[serde(skip, default = "Instant::now")]
	started_at: Instant,
}

impl LinkJob {
	pub fn new(input: FileLinkInput) -> Self {
		Self {
			input,
			started_at: Instant::now(),
		}
	}
}

impl Job for LinkJob {
	const NAME: &'static str = "link";
	const RESUMABLE: bool = false;
	const DESCRIPTION: Option<&'static str> = Some("Make a link");
}

impl crate::infra::job::traits::DynJob for LinkJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LinkOutput {
	pub at: PathBuf,
	pub kind: LinkKind,
}

impl From<LinkOutput> for JobOutput {
	fn from(output: LinkOutput) -> Self {
		JobOutput::custom(output)
	}
}

#[async_trait::async_trait]
impl JobHandler for LinkJob {
	type Output = LinkOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		let (Some(at), Some(target)) = (
			self.input.at.as_local_path().map(PathBuf::from),
			self.input.target.as_local_path().map(PathBuf::from),
		) else {
			return Err(JobError::execution(
				"the link and its target must be on this device",
			));
		};
		if tokio::fs::symlink_metadata(&at).await.is_ok() {
			return Err(JobError::execution(format!(
				"{} is already there",
				at.display()
			)));
		}
		let made = match self.input.kind {
			LinkKind::Symlink => symlink(&target, &at).await,
			LinkKind::Hardlink => tokio::fs::hard_link(&target, &at).await,
		};
		made.map_err(|error| JobError::execution(format!("could not make the link: {error}")))?;
		let subject = tokio::fs::symlink_metadata(&at).await.ok();
		ctx.record(vec![Effect::created(at.clone(), subject.as_ref())])
			.await;
		ctx.progress(Progress::generic(
			GenericProgress::new(1.0, "Complete", "linked")
				.with_completion(1, 1)
				.with_performance(0.0, None, Some(self.started_at.elapsed())),
		));
		Ok(LinkOutput {
			at,
			kind: self.input.kind,
		})
	}
}

async fn symlink(target: &std::path::Path, at: &std::path::Path) -> std::io::Result<()> {
	#[cfg(unix)]
	{
		tokio::fs::symlink(target, at).await
	}
	#[cfg(windows)]
	{
		if tokio::fs::metadata(target)
			.await
			.is_ok_and(|meta| meta.is_dir())
		{
			tokio::fs::symlink_dir(target, at).await
		} else {
			tokio::fs::symlink_file(target, at).await
		}
	}
}
