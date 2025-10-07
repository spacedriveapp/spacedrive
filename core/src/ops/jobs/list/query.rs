use super::output::{JobListItem, JobListOutput};
use crate::infra::job::types::JobStatus;
use crate::{context::CoreContext, infra::query::LibraryQuery};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct JobListInput {
	pub status: Option<JobStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct JobListQuery {
	pub input: JobListInput,
}

impl LibraryQuery for JobListQuery {
	type Input = JobListInput;
	type Output = JobListOutput;

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

		let jobs = library.jobs().list_jobs(self.input.status).await?;
		let items = jobs
			.into_iter()
			.map(|j| JobListItem {
				id: j.id,
				name: j.name,
				status: j.status,
				progress: j.progress,
				action_type: j.action_type,
				action_context: j.action_context,
			})
			.collect();
		Ok(JobListOutput { jobs: items })
	}
}

crate::register_library_query!(JobListQuery, "jobs.list");
