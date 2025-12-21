use super::output::{JobListItem, JobListOutput};
use crate::infra::job::types::JobStatus;
use crate::{
	context::CoreContext,
	infra::query::{LibraryQuery, QueryError, QueryResult},
};
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

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library selected".to_string()))?;
		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| QueryError::LibraryNotFound(library_id))?;

		let jobs = library
			.jobs()
			.list_jobs(self.input.status)
			.await
			.map_err(|e| QueryError::Internal(e.to_string()))?;
		let items = jobs
			.into_iter()
			.map(|j| JobListItem {
				id: j.id,
				name: j.name,
				device_id: j.device_id,
				status: j.status,
				progress: j.progress,
				action_type: j.action_type,
				action_context: j.action_context,
				created_at: j.created_at,
				started_at: j.started_at,
				completed_at: j.completed_at,
			})
			.collect();
		Ok(JobListOutput { jobs: items })
	}
}

crate::register_library_query!(JobListQuery, "jobs.list");
