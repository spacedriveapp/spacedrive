use crate::{
	invalidate_query,
	library::Library,
	location::get_location_path_from_location_id,
	old_job::{
		CurrentStep, JobError, JobInitOutput, JobResult, JobStepOutput, StatefulJob, WorkerContext,
	},
};

use sd_prisma::{
	prisma::{file_path, location},
	prisma_sync,
};
use sd_sync::OperationFactory;
use sd_utils::{db::maybe_missing, error::FileIOError};

use std::hash::Hash;

use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;
use tokio::{fs, io};
use tracing::warn;

use super::{error::FileSystemJobsError, get_many_files_datas, FileData};

#[derive(Serialize, Deserialize, Hash, Type, Debug)]
pub struct OldFileDeleterJobInit {
	pub location_id: location::id::Type,
	pub file_path_ids: Vec<file_path::id::Type>,
}

#[async_trait::async_trait]
impl StatefulJob for OldFileDeleterJobInit {
	type Data = ();
	type Step = FileData;
	type RunMetadata = ();

	const NAME: &'static str = "file_deleter";

	fn target_location(&self) -> location::id::Type {
		self.location_id
	}

	async fn init(
		&self,
		ctx: &WorkerContext,
		data: &mut Option<Self::Data>,
	) -> Result<JobInitOutput<Self::RunMetadata, Self::Step>, JobError> {
		let init = self;
		let Library { db, .. } = &*ctx.library;

		let steps = get_many_files_datas(
			db,
			get_location_path_from_location_id(db, init.location_id).await?,
			&init.file_path_ids,
		)
		.await
		.map_err(FileSystemJobsError::from)?;

		// Must fill in the data, otherwise the job will not run
		*data = Some(());

		Ok(steps.into())
	}

	async fn execute_step(
		&self,
		ctx: &WorkerContext,
		CurrentStep { step, .. }: CurrentStep<'_, Self::Step>,
		_: &Self::Data,
		_: &Self::RunMetadata,
	) -> Result<JobStepOutput<Self::Step, Self::RunMetadata>, JobError> {
		// need to handle stuff such as querying prisma for all paths of a file, and deleting all of those if requested (with a checkbox in the ui)
		// maybe a files.countOccurrences/and or files.getPath(location_id, path_id) to show how many of these files would be deleted (and where?)

		let Library { db, sync, .. } = ctx.library.as_ref();

		match if maybe_missing(step.file_path.is_dir, "file_path.is_dir")? {
			fs::remove_dir_all(&step.full_path).await
		} else {
			fs::remove_file(&step.full_path).await
		} {
			Ok(()) => { /*	Everything is awesome! */ }
			Err(e) if e.kind() == io::ErrorKind::NotFound => {
				warn!(
					path = %step.full_path.display(),
					"File not found in the file system, will remove from database;",
				);

				sync.write_op(
					db,
					sync.shared_delete(prisma_sync::file_path::SyncId {
						pub_id: step.file_path.pub_id.clone(),
					}),
					db.file_path()
						.delete(file_path::id::equals(step.file_path.id)),
				)
				.await?;
			}
			Err(e) => {
				return Err(JobError::from(FileIOError::from((&step.full_path, e))));
			}
		}

		Ok(().into())
	}

	async fn finalize(
		&self,
		ctx: &WorkerContext,
		_data: &Option<Self::Data>,
		_run_metadata: &Self::RunMetadata,
	) -> JobResult {
		let init = self;
		invalidate_query!(ctx.library, "search.paths");

		// ctx.library.orphan_remover.invoke().await;

		Ok(Some(json!({ "init": init })))
	}
}
