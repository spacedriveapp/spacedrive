use super::output::JobInfoOutput;
use crate::{context::CoreContext, cqrs::Query};
use anyhow::Result;
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JobInfoQuery {
	pub library_id: uuid::Uuid,
	pub job_id: uuid::Uuid,
}

impl Query for JobInfoQuery {
	type Output = Option<JobInfoOutput>;

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output> {
		let library = context
			.library_manager
			.get_library(self.library_id)
			.await
			.ok_or_else(|| anyhow::anyhow!("Library not found"))?;
		let info = library.jobs().get_job_info(self.job_id).await?;
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

crate::register_query!(JobInfoQuery, "jobs.info");

