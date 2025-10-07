use super::output::JobInfoOutput;
use crate::{context::CoreContext, infra::query::{LibraryQuery, QueryError, QueryResult}};
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
		let info = library.jobs().get_job_info(self.input.job_id).await
			.map_err(|e| QueryError::Internal(e.to_string()))?;
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
