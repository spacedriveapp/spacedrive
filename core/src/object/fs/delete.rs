use crate::{
	invalidate_query,
	job::{
		CurrentStep, JobError, JobInitOutput, JobResult, JobStepOutput, StatefulJob, WorkerContext,
	},
	library::LoadedLibrary,
	prisma::{file_path, location},
	util::{db::maybe_missing, error::FileIOError},
};

use std::hash::Hash;

use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;
use tokio::{fs, io};
use tracing::warn;

use super::{get_location_path_from_location_id, get_many_files_datas, FileData};

#[derive(Serialize, Deserialize, Hash, Type, Debug)]
pub struct FileDeleterJobInit {
	pub location_id: location::id::Type,
	pub file_path_ids: Vec<file_path::id::Type>,
}

#[async_trait::async_trait]
impl StatefulJob for FileDeleterJobInit {
	type Data = ();
	type Step = FileData;
	type RunMetadata = ();

	const NAME: &'static str = "file_deleter";

	async fn init(
		&self,
		ctx: &WorkerContext,
		data: &mut Option<Self::Data>,
	) -> Result<JobInitOutput<Self::RunMetadata, Self::Step>, JobError> {
		let init = self;
		let LoadedLibrary { db, .. } = &*ctx.library;

		let steps = get_many_files_datas(
			db,
			get_location_path_from_location_id(db, init.location_id).await?,
			&init.file_path_ids,
		)
		.await?;

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
		// maybe a files.countOccurances/and or files.getPath(location_id, path_id) to show how many of these files would be deleted (and where?)

		match if maybe_missing(step.file_path.is_dir, "file_path.is_dir")? {
			fs::remove_dir_all(&step.full_path).await
		} else {
			fs::remove_file(&step.full_path).await
		} {
			Ok(()) => { /*	Everything is awesome! */ }
			Err(e) if e.kind() == io::ErrorKind::NotFound => {
				warn!(
					"File not found in the file system, will remove from database: {}",
					step.full_path.display()
				);
				ctx.library
					.db
					.file_path()
					.delete(file_path::id::equals(step.file_path.id))
					.exec()
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

		ctx.library.orphan_remover.invoke().await;

		Ok(Some(json!({ "init": init })))
	}
}
