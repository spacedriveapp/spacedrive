//! Query for retrieving copy job metadata.

use super::output::CopyMetadataOutput;
use crate::infra::job::traits::Job;
use crate::ops::files::copy::FileCopyJob;
use crate::{
	context::CoreContext,
	infra::{
		job::database,
		query::{LibraryQuery, QueryError, QueryResult},
	},
};
use sea_orm::EntityTrait;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

/// Input for the copy metadata query.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CopyMetadataQueryInput {
	/// The job ID to query metadata for
	pub job_id: Uuid,
}

/// Query for retrieving copy job metadata.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CopyMetadataQuery {
	pub input: CopyMetadataQueryInput,
}

impl LibraryQuery for CopyMetadataQuery {
	type Input = CopyMetadataQueryInput;
	type Output = CopyMetadataOutput;

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

		// Load job from database using the job manager's database
		let job_db = library.jobs().database();
		let job = database::jobs::Entity::find_by_id(self.input.job_id.to_string())
			.one(job_db.conn())
			.await
			.map_err(|e| QueryError::Internal(format!("Database error: {}", e)))?;

		let Some(job_record) = job else {
			return Ok(CopyMetadataOutput::with_error(format!(
				"Job {} not found",
				self.input.job_id
			)));
		};

		// Check if this is a file_copy job
		if job_record.name != FileCopyJob::NAME {
			return Ok(CopyMetadataOutput::with_error(format!(
				"Job {} is not a copy job (type: {})",
				self.input.job_id, job_record.name
			)));
		}

		// Deserialize the job state
		let copy_job: FileCopyJob = rmp_serde::from_slice(&job_record.state)
			.map_err(|e| QueryError::Internal(format!("Failed to deserialize job state: {}", e)))?;

		// Build File domain objects from entry UUIDs
		let mut metadata = copy_job.job_metadata;

		// Collect entry UUIDs that are available
		let entry_uuids: Vec<uuid::Uuid> = metadata
			.files
			.iter()
			.filter_map(|entry| entry.entry_id)
			.collect();

		// Batch load File objects
		if !entry_uuids.is_empty() {
			match crate::domain::file::File::from_entry_uuids(library.db().conn(), &entry_uuids)
				.await
			{
				Ok(files) => {
					metadata.file_objects = files;
				}
				Err(e) => {
					// Log error but don't fail the query
					tracing::warn!("Failed to load File objects: {}", e);
				}
			}
		}

		// Return the metadata
		Ok(CopyMetadataOutput::with_metadata(metadata))
	}
}

crate::register_library_query!(CopyMetadataQuery, "jobs.get_copy_metadata");
