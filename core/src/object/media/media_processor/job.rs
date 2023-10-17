use crate::{
	invalidate_query,
	job::{
		CurrentStep, JobError, JobInitOutput, JobReportUpdate, JobResult, JobStepOutput,
		StatefulJob, WorkerContext,
	},
	library::Library,
	location::file_path_helper::{
		ensure_file_path_exists, ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
		file_path_for_media_processor, IsolatedFilePathData,
	},
	prisma::{location, PrismaClient},
	util::db::maybe_missing,
};

use std::{
	future::Future,
	hash::Hash,
	path::{Path, PathBuf},
};

use itertools::Itertools;
use prisma_client_rust::{raw, PrismaValue};
use sd_file_ext::extensions::Extension;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, info};

use super::{
	dispatch_thumbnails_for_processing, media_data_extractor, process, MediaProcessorError,
	MediaProcessorMetadata,
};

const BATCH_SIZE: usize = 10;

#[derive(Serialize, Deserialize, Debug)]
pub struct MediaProcessorJobInit {
	pub location: location::Data,
	pub sub_path: Option<PathBuf>,
	pub regenerate_thumbnails: bool,
}

impl Hash for MediaProcessorJobInit {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.location.id.hash(state);
		if let Some(ref sub_path) = self.sub_path {
			sub_path.hash(state);
		}
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MediaProcessorJobData {
	location_path: PathBuf,
	to_process_path: PathBuf,
}

#[async_trait::async_trait]
impl StatefulJob for MediaProcessorJobInit {
	type Data = MediaProcessorJobData;
	type Step = Vec<file_path_for_media_processor::Data>;
	type RunMetadata = MediaProcessorMetadata;

	const NAME: &'static str = "media_processor";
	const IS_BATCHED: bool = true;

	fn target_location(&self) -> location::id::Type {
		self.location.id
	}

	async fn init(
		&self,
		ctx: &WorkerContext,
		data: &mut Option<Self::Data>,
	) -> Result<JobInitOutput<Self::RunMetadata, Self::Step>, JobError> {
		let Library { db, .. } = ctx.library.as_ref();

		let location_id = self.location.id;
		let location_path =
			maybe_missing(&self.location.path, "location.path").map(PathBuf::from)?;

		let (to_process_path, iso_file_path) = match &self.sub_path {
			Some(sub_path) if sub_path != Path::new("") => {
				let full_path = ensure_sub_path_is_in_location(&location_path, sub_path)
					.await
					.map_err(MediaProcessorError::from)?;
				ensure_sub_path_is_directory(&location_path, sub_path)
					.await
					.map_err(MediaProcessorError::from)?;

				let sub_iso_file_path =
					IsolatedFilePathData::new(location_id, &location_path, &full_path, true)
						.map_err(MediaProcessorError::from)?;

				ensure_file_path_exists(
					sub_path,
					&sub_iso_file_path,
					db,
					MediaProcessorError::SubPathNotFound,
				)
				.await?;

				(full_path, sub_iso_file_path)
			}
			_ => (
				location_path.to_path_buf(),
				IsolatedFilePathData::new(location_id, &location_path, &location_path, true)
					.map_err(MediaProcessorError::from)?,
			),
		};

		debug!(
			"Searching for media files in location {location_id} at directory \"{iso_file_path}\""
		);

		dispatch_thumbnails_for_processing(
			location_id,
			&location_path,
			&iso_file_path,
			&ctx.library,
			&ctx.node,
			false,
			get_all_children_files_by_extensions,
		)
		.await?;

		let file_paths = get_files_for_media_data_extraction(db, &iso_file_path).await?;

		let total_files = file_paths.len();

		let chunked_files = file_paths
			.into_iter()
			.chunks(BATCH_SIZE)
			.into_iter()
			.map(|chunk| chunk.collect::<Vec<_>>())
			.collect::<Vec<_>>();

		ctx.progress(vec![
			JobReportUpdate::TaskCount(total_files),
			JobReportUpdate::Message(format!(
				"Preparing to process {total_files} files in {} chunks",
				chunked_files.len()
			)),
		]);

		*data = Some(MediaProcessorJobData {
			location_path,
			to_process_path,
		});

		Ok(chunked_files.into())
	}

	async fn execute_step(
		&self,
		ctx: &WorkerContext,
		CurrentStep {
			step: file_paths,
			step_number,
		}: CurrentStep<'_, Self::Step>,
		data: &Self::Data,
		_: &Self::RunMetadata,
	) -> Result<JobStepOutput<Self::Step, Self::RunMetadata>, JobError> {
		process(
			file_paths,
			self.location.id,
			&data.location_path,
			&ctx.library.db,
			&|completed_count| {
				ctx.progress(vec![JobReportUpdate::CompletedTaskCount(
					step_number * BATCH_SIZE + completed_count,
				)]);
			},
		)
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
			"Finished media processing for location {} at {}",
			self.location.id,
			data.as_ref()
				.expect("critical error: missing data on job state")
				.to_process_path
				.display()
		);

		if run_metadata.media_data.extracted > 0 {
			invalidate_query!(ctx.library, "search.paths");
		}

		Ok(Some(json!({"init: ": self, "run_metadata": run_metadata})))
	}
}

async fn get_files_for_media_data_extraction(
	db: &PrismaClient,
	parent_iso_file_path: &IsolatedFilePathData<'_>,
) -> Result<Vec<file_path_for_media_processor::Data>, MediaProcessorError> {
	get_all_children_files_by_extensions(
		db,
		parent_iso_file_path,
		&media_data_extractor::FILTERED_IMAGE_EXTENSIONS,
	)
	.await
	.map_err(Into::into)
}

fn get_all_children_files_by_extensions<'d, 'p, 'e, 'ret>(
	db: &'d PrismaClient,
	parent_iso_file_path: &'p IsolatedFilePathData<'_>,
	extensions: &'e [Extension],
) -> impl Future<Output = Result<Vec<file_path_for_media_processor::Data>, MediaProcessorError>> + 'ret
where
	'd: 'ret,
	'p: 'ret,
	'e: 'ret,
{
	async move {
		// FIXME: Had to use format! macro because PCR doesn't support IN with Vec for SQLite
		// We have no data coming from the user, so this is sql injection safe
		db._query_raw(raw!(
			&format!(
				"SELECT id, materialized_path, is_dir, name, extension, cas_id, object_id
			FROM file_path
			WHERE
				location_id={{}}
				AND cas_id IS NOT NULL
				AND LOWER(extension) IN ({})
				AND materialized_path LIKE {{}}
			ORDER BY materialized_path ASC",
				// Orderind by materialized_path so we can prioritize processing the first files
				// in the above part of the directories tree
				extensions
					.iter()
					.map(|ext| format!("LOWER('{ext}')"))
					.collect::<Vec<_>>()
					.join(",")
			),
			PrismaValue::Int(parent_iso_file_path.location_id() as i64),
			PrismaValue::String(format!(
				"{}%",
				parent_iso_file_path
					.materialized_path_for_children()
					.expect("sub path iso_file_path must be a directory")
			))
		))
		.exec()
		.await
		.map_err(Into::into)
	}
}
