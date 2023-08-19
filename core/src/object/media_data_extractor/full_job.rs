use crate::{
	invalidate_query,
	job::{
		CurrentStep, JobError, JobInitOutput, JobResult, JobStepOutput, StatefulJob, WorkerContext,
	},
	library::Library,
	location::file_path_helper::{
		ensure_file_path_exists, ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
		file_path_for_media_data, IsolatedFilePathData,
	},
	prisma::{file_path, location, PrismaClient},
	util::db::maybe_missing,
};

use sd_file_ext::extensions::Extension;

use std::{
	hash::Hash,
	path::{Path, PathBuf},
};

use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, info};

use super::{
	inner_process_step, MediaDataError, MediaDataJobRunMetadata, MediaDataJobStep,
	FILTERED_IMAGE_EXTENSIONS,
};

const BATCH_SIZE: usize = 100;

#[derive(Serialize, Deserialize, Debug)]
pub struct MediaDataJobInit {
	pub location: location::Data,
	pub sub_path: Option<PathBuf>,
}

impl Hash for MediaDataJobInit {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.location.id.hash(state);
		if let Some(ref sub_path) = self.sub_path {
			sub_path.hash(state);
		}
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MediaDataJobData {
	location_path: PathBuf,
	path: PathBuf,
}

#[async_trait::async_trait]
impl StatefulJob for MediaDataJobInit {
	type Data = MediaDataJobData;
	type Step = MediaDataJobStep;
	type RunMetadata = MediaDataJobRunMetadata;

	const NAME: &'static str = "media_data_extractor";

	async fn init(
		&self,
		ctx: &WorkerContext,
		data: &mut Option<Self::Data>,
	) -> Result<JobInitOutput<Self::RunMetadata, Self::Step>, JobError> {
		let init = self;
		let Library { db, .. } = ctx.library.as_ref();

		let location_id = init.location.id;
		let location_path =
			maybe_missing(&init.location.path, "location.path").map(PathBuf::from)?;

		let (path, iso_file_path) = match &init.sub_path {
			Some(sub_path) if sub_path != Path::new("") => {
				let full_path = ensure_sub_path_is_in_location(&location_path, sub_path)
					.await
					.map_err(MediaDataError::from)?;
				ensure_sub_path_is_directory(&location_path, sub_path)
					.await
					.map_err(MediaDataError::from)?;

				let sub_iso_file_path =
					IsolatedFilePathData::new(location_id, &location_path, &full_path, true)
						.map_err(MediaDataError::from)?;

				ensure_file_path_exists(
					sub_path,
					&sub_iso_file_path,
					db,
					MediaDataError::SubPathNotFound,
				)
				.await?;

				(full_path, sub_iso_file_path)
			}
			_ => (
				location_path.to_path_buf(),
				IsolatedFilePathData::new(location_id, &location_path, &location_path, true)
					.map_err(MediaDataError::from)?,
			),
		};

		debug!("Searching for images in location {location_id} at directory \"{iso_file_path}\"");

		let image_files =
			get_files_by_extensions(db, &iso_file_path, &FILTERED_IMAGE_EXTENSIONS).await?;

		debug!("Found {:?} image files", image_files.len());

		ctx.progress_msg(format!("Preparing to process {} files", image_files.len()));

		*data = Some(MediaDataJobData {
			location_path,
			path,
		});

		Ok(image_files
			.into_iter()
			.chunks(BATCH_SIZE)
			.into_iter()
			.map(|chunk| chunk.collect::<Vec<_>>())
			.collect::<Vec<_>>()
			.into())
	}

	async fn execute_step(
		&self,
		ctx: &WorkerContext,
		CurrentStep { step, .. }: CurrentStep<'_, Self::Step>,
		data: &Self::Data,
		_: &Self::RunMetadata,
	) -> Result<JobStepOutput<Self::Step, Self::RunMetadata>, JobError> {
		inner_process_step(step, &data.location_path, &self.location, &ctx.library)
			.await
			.map(Into::into)
			.map_err(Into::into)
	}

	async fn finalize(
		&self,
		ctx: &WorkerContext,
		data: &Option<Self::Data>,
		run_metadata: &Self::RunMetadata,
	) -> JobResult {
		info!(
			"Finished media data extraction for location {} at {}",
			self.location.id,
			data.as_ref()
				.expect("critical error: missing data on job state")
				.path
				.display()
		);

		if run_metadata.media_data_extracted > 0 {
			invalidate_query!(ctx.library, "search.paths");
		}

		Ok(Some(json!({"init: ": self, "run_metadata": run_metadata})))
	}
}

async fn get_files_by_extensions(
	db: &PrismaClient,
	iso_file_path: &IsolatedFilePathData<'_>,
	extensions: &[Extension],
) -> Result<MediaDataJobStep, MediaDataError> {
	db.file_path()
		.find_many(vec![
			file_path::location_id::equals(Some(iso_file_path.location_id())),
			file_path::extension::in_vec(extensions.iter().map(ToString::to_string).collect()),
			file_path::materialized_path::starts_with(
				iso_file_path
					.materialized_path_for_children()
					.expect("sub path iso_file_path must be a directory"),
			),
		])
		.select(file_path_for_media_data::select())
		.exec()
		.await
		.map_err(Into::into)
}
