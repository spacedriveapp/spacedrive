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
	object::media::media_data_extractor,
	object::media::thumbnail::{self, init_thumbnail_dir},
	prisma::{location, PrismaClient},
	util::db::maybe_missing,
};

use std::{
	collections::HashMap,
	hash::Hash,
	path::{Path, PathBuf},
};

use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, info};

use super::{
	get_all_children_files_by_extensions, process, MediaProcessorEntry, MediaProcessorEntryKind,
	MediaProcessorError, MediaProcessorMetadata, ThumbnailerEntryKind,
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
	thumbnails_base_dir: PathBuf,
	location_path: PathBuf,
	to_process_path: PathBuf,
}

type MediaProcessorJobStep = Vec<MediaProcessorEntry>;

#[async_trait::async_trait]
impl StatefulJob for MediaProcessorJobInit {
	type Data = MediaProcessorJobData;
	type Step = MediaProcessorJobStep;
	type RunMetadata = MediaProcessorMetadata;

	const NAME: &'static str = "media_processor";
	const IS_BATCHED: bool = true;

	async fn init(
		&self,
		ctx: &WorkerContext,
		data: &mut Option<Self::Data>,
	) -> Result<JobInitOutput<Self::RunMetadata, Self::Step>, JobError> {
		let Library { db, .. } = ctx.library.as_ref();

		let thumbnails_base_dir = init_thumbnail_dir(ctx.node.config.data_directory())
			.await
			.map_err(MediaProcessorError::from)?;

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

		let thumbnailer_files = get_files_for_thumbnailer(db, &iso_file_path).await?;

		let mut media_data_files_map = get_files_for_media_data_extraction(db, &iso_file_path)
			.await?
			.map(|file_path| (file_path.id, file_path))
			.collect::<HashMap<_, _>>();

		let mut total_files_for_thumbnailer = 0;

		let chunked_files = thumbnailer_files
			.into_iter()
			.map(|(file_path, thumb_kind)| {
				total_files_for_thumbnailer += 1;
				MediaProcessorEntry {
					operation_kind: if media_data_files_map.remove(&file_path.id).is_some() {
						MediaProcessorEntryKind::MediaDataAndThumbnailer(thumb_kind)
					} else {
						MediaProcessorEntryKind::Thumbnailer(thumb_kind)
					},
					file_path,
				}
			})
			.collect::<Vec<_>>()
			.into_iter()
			.chain(
				media_data_files_map
					.into_values()
					.map(|file_path| MediaProcessorEntry {
						operation_kind: MediaProcessorEntryKind::MediaData,
						file_path,
					}),
			)
			.chunks(BATCH_SIZE)
			.into_iter()
			.map(|chunk| chunk.collect::<Vec<_>>())
			.collect::<Vec<_>>();

		ctx.progress(vec![
			JobReportUpdate::TaskCount(total_files_for_thumbnailer),
			JobReportUpdate::Message(format!(
				"Preparing to process {total_files_for_thumbnailer} files in {} chunks",
				chunked_files.len()
			)),
		]);

		*data = Some(MediaProcessorJobData {
			thumbnails_base_dir,
			location_path,
			to_process_path,
		});

		Ok(chunked_files.into())
	}

	async fn execute_step(
		&self,
		ctx: &WorkerContext,
		CurrentStep { step, step_number }: CurrentStep<'_, Self::Step>,
		data: &Self::Data,
		_: &Self::RunMetadata,
	) -> Result<JobStepOutput<Self::Step, Self::RunMetadata>, JobError> {
		process(
			step,
			self.location.id,
			&data.location_path,
			&data.thumbnails_base_dir,
			self.regenerate_thumbnails,
			&ctx.library,
			|completed_count| {
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

		if run_metadata.thumbnailer.created > 0 || run_metadata.media_data.extracted > 0 {
			invalidate_query!(ctx.library, "search.paths");
		}

		Ok(Some(json!({"init: ": self, "run_metadata": run_metadata})))
	}
}

async fn get_files_for_thumbnailer(
	db: &PrismaClient,
	parent_iso_file_path: &IsolatedFilePathData<'_>,
) -> Result<
	impl Iterator<Item = (file_path_for_media_processor::Data, ThumbnailerEntryKind)>,
	MediaProcessorError,
> {
	// query database for all image files in this location that need thumbnails
	let image_thumb_files = get_all_children_files_by_extensions(
		db,
		parent_iso_file_path,
		&thumbnail::FILTERED_IMAGE_EXTENSIONS,
	)
	.await?
	.into_iter()
	.map(|file_path| (file_path, ThumbnailerEntryKind::Image));

	#[cfg(feature = "ffmpeg")]
	let all_files = {
		// query database for all video files in this location that need thumbnails
		let video_files = get_all_children_files_by_extensions(
			db,
			parent_iso_file_path,
			&thumbnail::FILTERED_VIDEO_EXTENSIONS,
		)
		.await?;

		image_thumb_files.chain(
			video_files
				.into_iter()
				.map(|file_path| (file_path, ThumbnailerEntryKind::Video)),
		)
	};
	#[cfg(not(feature = "ffmpeg"))]
	let all_files = { image_thumb_files };

	Ok(all_files)
}

async fn get_files_for_media_data_extraction(
	db: &PrismaClient,
	parent_iso_file_path: &IsolatedFilePathData<'_>,
) -> Result<impl Iterator<Item = file_path_for_media_processor::Data>, MediaProcessorError> {
	get_all_children_files_by_extensions(
		db,
		parent_iso_file_path,
		&media_data_extractor::FILTERED_IMAGE_EXTENSIONS,
	)
	.await
	.map(|file_paths| file_paths.into_iter())
	.map_err(Into::into)
}
