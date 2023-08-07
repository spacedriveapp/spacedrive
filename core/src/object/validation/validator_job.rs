use crate::{
	job::{
		CurrentStep, JobError, JobInitOutput, JobResult, JobStepOutput, StatefulJob, WorkerContext,
	},
	library::LoadedLibrary,
	location::file_path_helper::{
		ensure_file_path_exists, ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
		file_path_for_object_validator, IsolatedFilePathData,
	},
	prisma::{file_path, location},
	util::{db::maybe_missing, error::FileIOError},
};

use std::{
	hash::{Hash, Hasher},
	path::{Path, PathBuf},
};

use sd_prisma::prisma_sync;
use sd_sync::OperationFactory;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::info;

use super::{hash::file_checksum, ValidatorError};

#[derive(Serialize, Deserialize, Debug)]
pub struct ObjectValidatorJobData {
	pub location_path: PathBuf,
	pub task_count: usize,
}

// The validator can
#[derive(Serialize, Deserialize, Debug)]
pub struct ObjectValidatorJobInit {
	pub location: location::Data,
	pub sub_path: Option<PathBuf>,
}

impl Hash for ObjectValidatorJobInit {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.location.id.hash(state);
		if let Some(ref sub_path) = self.sub_path {
			sub_path.hash(state);
		}
	}
}

// The Validator is able to:
// - generate a full byte checksum for Objects in a Location
// - generate checksums for all Objects missing without one
// - compare two objects and return true if they are the same
#[async_trait::async_trait]
impl StatefulJob for ObjectValidatorJobInit {
	type Data = ObjectValidatorJobData;
	type Step = file_path_for_object_validator::Data;
	type RunMetadata = ();

	const NAME: &'static str = "object_validator";

	async fn init(
		&self,
		ctx: &WorkerContext,
		data: &mut Option<Self::Data>,
	) -> Result<JobInitOutput<Self::RunMetadata, Self::Step>, JobError> {
		let init = self;
		let LoadedLibrary { db, .. } = &*ctx.library;

		let location_id = init.location.id;

		let location_path =
			maybe_missing(&init.location.path, "location.path").map(PathBuf::from)?;

		let maybe_sub_iso_file_path = match &init.sub_path {
			Some(sub_path) if sub_path != Path::new("") => {
				let full_path = ensure_sub_path_is_in_location(&location_path, sub_path)
					.await
					.map_err(ValidatorError::from)?;
				ensure_sub_path_is_directory(&location_path, sub_path)
					.await
					.map_err(ValidatorError::from)?;

				let sub_iso_file_path =
					IsolatedFilePathData::new(location_id, &location_path, &full_path, true)
						.map_err(ValidatorError::from)?;

				ensure_file_path_exists(
					sub_path,
					&sub_iso_file_path,
					db,
					ValidatorError::SubPathNotFound,
				)
				.await?;

				Some(sub_iso_file_path)
			}
			_ => None,
		};

		let steps = db
			.file_path()
			.find_many(sd_utils::chain_optional_iter(
				[
					file_path::location_id::equals(Some(init.location.id)),
					file_path::is_dir::equals(Some(false)),
					file_path::integrity_checksum::equals(None),
				],
				[maybe_sub_iso_file_path.and_then(|iso_sub_path| {
					iso_sub_path
						.materialized_path_for_children()
						.map(file_path::materialized_path::starts_with)
				})],
			))
			.select(file_path_for_object_validator::select())
			.exec()
			.await?;

		*data = Some(ObjectValidatorJobData {
			location_path,
			task_count: steps.len(),
		});

		Ok(steps.into())
	}

	async fn execute_step(
		&self,
		ctx: &WorkerContext,
		CurrentStep {
			step: file_path, ..
		}: CurrentStep<'_, Self::Step>,
		data: &Self::Data,
		_: &Self::RunMetadata,
	) -> Result<JobStepOutput<Self::Step, Self::RunMetadata>, JobError> {
		let init = self;
		let LoadedLibrary { db, sync, .. } = &*ctx.library;

		// this is to skip files that already have checksums
		// i'm unsure what the desired behaviour is in this case
		// we can also compare old and new checksums here
		// This if is just to make sure, we already queried objects where integrity_checksum is null
		if file_path.integrity_checksum.is_none() {
			let full_path = data.location_path.join(IsolatedFilePathData::try_from((
				init.location.id,
				file_path,
			))?);
			let checksum = file_checksum(&full_path)
				.await
				.map_err(|e| ValidatorError::FileIO(FileIOError::from((full_path, e))))?;

			sync.write_op(
				db,
				sync.shared_update(
					prisma_sync::file_path::SyncId {
						pub_id: file_path.pub_id.clone(),
					},
					file_path::integrity_checksum::NAME,
					json!(&checksum),
				),
				db.file_path().update(
					file_path::pub_id::equals(file_path.pub_id.clone()),
					vec![file_path::integrity_checksum::set(Some(checksum))],
				),
			)
			.await?;
		}

		Ok(().into())
	}

	async fn finalize(
		&self,
		_: &WorkerContext,
		data: &Option<Self::Data>,
		_run_metadata: &Self::RunMetadata,
	) -> JobResult {
		let init = self;
		let data = data
			.as_ref()
			.expect("critical error: missing data on job state");

		info!(
			"finalizing validator job at {}{}: {} tasks",
			data.location_path.display(),
			init.sub_path
				.as_ref()
				.map(|p| format!("{}", p.display()))
				.unwrap_or_default(),
			data.task_count
		);

		Ok(Some(json!({ "init": init })))
	}
}
