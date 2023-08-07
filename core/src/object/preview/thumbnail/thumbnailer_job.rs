use crate::{
	invalidate_query,
	job::{
		CurrentStep, JobError, JobInitOutput, JobResult, JobRunMetadata, JobStepOutput,
		StatefulJob, WorkerContext,
	},
	library::Library,
	location::file_path_helper::{
		ensure_file_path_exists, ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
		file_path_for_thumbnailer, IsolatedFilePathData,
	},
	object::preview::thumbnail::directory::init_thumbnail_dir,
	prisma::{file_path, location, PrismaClient},
	util::db::maybe_missing,
};

use std::{
	hash::Hash,
	path::{Path, PathBuf},
};

use sd_file_ext::extensions::Extension;

use serde::{Deserialize, Serialize};

use serde_json::json;
use tracing::{debug, info, trace};

use super::{
	inner_process_step, ThumbnailerError, ThumbnailerJobStep, ThumbnailerJobStepKind,
	FILTERED_IMAGE_EXTENSIONS,
};

#[cfg(feature = "ffmpeg")]
use super::FILTERED_VIDEO_EXTENSIONS;

#[derive(Serialize, Deserialize, Debug)]
pub struct ThumbnailerJobInit {
	pub location: location::Data,
	pub sub_path: Option<PathBuf>,
}

impl Hash for ThumbnailerJobInit {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.location.id.hash(state);
		if let Some(ref sub_path) = self.sub_path {
			sub_path.hash(state);
		}
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ThumbnailerJobData {
	thumbnail_dir: PathBuf,
	location_path: PathBuf,
	path: PathBuf,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct ThumbnailerJobRunMetadata {
	thumbnails_created: u32,
	thumbnails_skipped: u32,
}

impl JobRunMetadata for ThumbnailerJobRunMetadata {
	fn update(&mut self, new_data: Self) {
		self.thumbnails_created += new_data.thumbnails_created;
		self.thumbnails_skipped += new_data.thumbnails_skipped;
	}
}

#[async_trait::async_trait]
impl StatefulJob for ThumbnailerJobInit {
	type Data = ThumbnailerJobData;
	type Step = ThumbnailerJobStep;
	type RunMetadata = ThumbnailerJobRunMetadata;

	const NAME: &'static str = "thumbnailer";

	async fn init(
		&self,
		ctx: &WorkerContext,
		data: &mut Option<Self::Data>,
	) -> Result<JobInitOutput<Self::RunMetadata, Self::Step>, JobError> {
		let init = self;
		let Library { db, .. } = &*ctx.library;

		let thumbnail_dir = init_thumbnail_dir(ctx.node.config.data_directory()).await?;

		let location_id = init.location.id;
		let location_path =
			maybe_missing(&init.location.path, "location.path").map(PathBuf::from)?;

		let (path, iso_file_path) = match &init.sub_path {
			Some(sub_path) if sub_path != Path::new("") => {
				let full_path = ensure_sub_path_is_in_location(&location_path, sub_path)
					.await
					.map_err(ThumbnailerError::from)?;
				ensure_sub_path_is_directory(&location_path, sub_path)
					.await
					.map_err(ThumbnailerError::from)?;

				let sub_iso_file_path =
					IsolatedFilePathData::new(location_id, &location_path, &full_path, true)
						.map_err(ThumbnailerError::from)?;

				ensure_file_path_exists(
					sub_path,
					&sub_iso_file_path,
					db,
					ThumbnailerError::SubPathNotFound,
				)
				.await?;

				(full_path, sub_iso_file_path)
			}
			_ => (
				location_path.to_path_buf(),
				IsolatedFilePathData::new(location_id, &location_path, &location_path, true)
					.map_err(ThumbnailerError::from)?,
			),
		};

		debug!("Searching for images in location {location_id} at directory {iso_file_path}");

		// query database for all image files in this location that need thumbnails
		let image_files = get_files_by_extensions(
			db,
			&iso_file_path,
			&FILTERED_IMAGE_EXTENSIONS,
			ThumbnailerJobStepKind::Image,
		)
		.await?;
		trace!("Found {:?} image files", image_files.len());

		#[cfg(feature = "ffmpeg")]
		let all_files = {
			// query database for all video files in this location that need thumbnails
			let video_files = get_files_by_extensions(
				db,
				&iso_file_path,
				&FILTERED_VIDEO_EXTENSIONS,
				ThumbnailerJobStepKind::Video,
			)
			.await?;
			trace!("Found {:?} video files", video_files.len());

			image_files
				.into_iter()
				.chain(video_files.into_iter())
				.collect::<Vec<_>>()
		};
		#[cfg(not(feature = "ffmpeg"))]
		let all_files = { image_files.into_iter().collect::<Vec<_>>() };

		ctx.progress_msg(format!("Preparing to process {} files", all_files.len()));

		*data = Some(ThumbnailerJobData {
			thumbnail_dir,
			location_path,
			path,
		});

		Ok((
			ThumbnailerJobRunMetadata {
				thumbnails_created: 0,
				thumbnails_skipped: 0,
			},
			all_files,
		)
			.into())
	}

	async fn execute_step(
		&self,
		ctx: &WorkerContext,
		CurrentStep { step, .. }: CurrentStep<'_, Self::Step>,
		data: &Self::Data,
		_: &Self::RunMetadata,
	) -> Result<JobStepOutput<Self::Step, Self::RunMetadata>, JobError> {
		let init = self;
		ctx.progress_msg(format!(
			"Processing {}",
			maybe_missing(
				&step.file_path.materialized_path,
				"file_path.materialized_path"
			)?
		));

		let mut new_metadata = Self::RunMetadata::default();

		let step_result = inner_process_step(
			step,
			&data.location_path,
			&data.thumbnail_dir,
			&init.location,
			&ctx.library,
		)
		.await;

		step_result.map(|thumbnail_was_created| {
			if thumbnail_was_created {
				new_metadata.thumbnails_created += 1;
			} else {
				new_metadata.thumbnails_skipped += 1;
			}
		})?;

		Ok(new_metadata.into())
	}

	async fn finalize(
		&self,
		ctx: &WorkerContext,
		data: &Option<Self::Data>,
		run_metadata: &Self::RunMetadata,
	) -> JobResult {
		let init = self;
		info!(
			"Finished thumbnail generation for location {} at {}",
			init.location.id,
			data.as_ref()
				.expect("critical error: missing data on job state")
				.path
				.display()
		);

		if run_metadata.thumbnails_created > 0 {
			invalidate_query!(ctx.library, "search.paths");
		}

		Ok(Some(json!({"init: ": init, "run_metadata": run_metadata})))
	}
}

async fn get_files_by_extensions(
	db: &PrismaClient,
	iso_file_path: &IsolatedFilePathData<'_>,
	extensions: &[Extension],
	kind: ThumbnailerJobStepKind,
) -> Result<Vec<ThumbnailerJobStep>, JobError> {
	Ok(db
		.file_path()
		.find_many(vec![
			file_path::location_id::equals(Some(iso_file_path.location_id())),
			file_path::extension::in_vec(extensions.iter().map(ToString::to_string).collect()),
			file_path::materialized_path::starts_with(
				iso_file_path
					.materialized_path_for_children()
					.expect("sub path iso_file_path must be a directory"),
			),
		])
		.select(file_path_for_thumbnailer::select())
		.exec()
		.await?
		.into_iter()
		.map(|file_path| ThumbnailerJobStep { file_path, kind })
		.collect())
}
