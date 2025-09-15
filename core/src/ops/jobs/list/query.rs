use super::output::{JobListItem, JobListOutput};
use crate::{context::CoreContext, cqrs::Query};
use anyhow::Result;
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JobListQuery {
	pub status: Option<crate::infra::job::types::JobStatus>,
}

impl Query for JobListQuery {
	type Output = JobListOutput;

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output> {
		// Get current library ID from session
		let session_state = context.session.get().await;
		let library_id = session_state
			.current_library_id
			.ok_or_else(|| anyhow::anyhow!("No active library selected"))?;

		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| anyhow::anyhow!("Library not found"))?;
		let jobs = library.jobs().list_jobs(self.status).await?;
		let items = jobs
			.into_iter()
			.map(|j| JobListItem {
				id: j.id,
				name: j.name,
				status: j.status,
				progress: j.progress,
			})
			.collect();
		Ok(JobListOutput { jobs: items })
	}
}

crate::register_query!(JobListQuery, "jobs.list");
