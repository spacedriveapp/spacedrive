//! A job's journal: what it did to the filesystem, in order.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
	context::CoreContext,
	infra::{
		job::{journal::Recorded, types::JobId},
		query::{LibraryQuery, QueryError, QueryResult},
	},
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct JobJournalInput {
	pub job_id: uuid::Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct JobJournalOutput {
	pub effects: Vec<Recorded>,
}

pub struct JobJournalQuery {
	input: JobJournalInput,
}

impl LibraryQuery for JobJournalQuery {
	type Input = JobJournalInput;
	type Output = JobJournalOutput;

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
		let effects = library
			.jobs()
			.database()
			.journal(JobId(self.input.job_id))
			.await
			.map_err(|e| QueryError::Internal(e.to_string()))?;
		Ok(JobJournalOutput { effects })
	}
}

crate::register_library_query!(JobJournalQuery, "jobs.journal");
