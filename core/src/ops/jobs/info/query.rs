use super::output::JobInfoOutput;
use crate::{context::CoreContext, infra::query::LibraryQuery};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct JobInfoQueryInput {
	pub job_id: uuid::Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct JobInfoQuery {
	pub input: JobInfoQueryInput,
}

impl LibraryQuery for JobInfoQuery {
	type Input = JobInfoQueryInput;
	type Output = Option<JobInfoOutput>;

	fn from_input(input: Self::Input) -> Result<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> Result<Self::Output> {
		let library_id = session
			.current_library_id
			.ok_or_else(|| anyhow::anyhow!("No library selected"))?;
		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| anyhow::anyhow!("Library not found"))?;
		let info = library.jobs().get_job_info(self.input.job_id).await?;
		Ok(info.map(|j| JobInfoOutput {
			id: j.id,
			name: j.name,
			status: j.status,
			progress: j.progress,
			started_at: j.started_at,
			completed_at: j.completed_at,
			error_message: j.error_message,
		}))
	}
}

crate::register_library_query!(JobInfoQuery, "jobs.info");
