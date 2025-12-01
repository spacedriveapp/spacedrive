use super::output::{ActiveJobItem, ActiveJobsOutput};
use crate::infra::job::types::JobStatus;
use crate::{
	context::CoreContext,
	infra::query::{LibraryQuery, QueryError, QueryResult},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ActiveJobsInput {}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ActiveJobsQuery {
	pub input: ActiveJobsInput,
}

impl LibraryQuery for ActiveJobsQuery {
	type Input = ActiveJobsInput;
	type Output = ActiveJobsOutput;

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

		// Use list_running_jobs which only returns in-memory active jobs
		let jobs = library.jobs().list_running_jobs().await;

		let mut running_count = 0u32;
		let mut paused_count = 0u32;

		let items: Vec<ActiveJobItem> = jobs
			.into_iter()
			.map(|j| {
				match j.status {
					JobStatus::Running => running_count += 1,
					JobStatus::Paused => paused_count += 1,
					_ => {}
				}

				ActiveJobItem {
					id: j.id,
					name: j.name,
					status: j.status,
					progress: j.progress,
					action_type: j.action_type,
					action_context: j.action_context,
				}
			})
			.collect();

		Ok(ActiveJobsOutput {
			jobs: items,
			running_count,
			paused_count,
		})
	}
}

crate::register_library_query!(ActiveJobsQuery, "jobs.active");
