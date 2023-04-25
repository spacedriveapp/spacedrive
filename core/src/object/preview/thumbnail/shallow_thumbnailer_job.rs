use crate::{
	job::{
		JobError, JobInitData, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext,
	},
	library::Library,
	location::{
		file_path_helper::{
			ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
			file_path_just_materialized_path_cas_id, get_existing_file_path_id, MaterializedPath,
		},
		LocationId,
	},
	prisma::{file_path, location, PrismaClient},
	util::db::uuid_to_bytes,
};

use std::{
	collections::VecDeque,
	hash::Hash,
	path::{Path, PathBuf, MAIN_SEPARATOR_STR},
};

use sd_file_ext::extensions::Extension;

use serde::{Deserialize, Serialize};
use tokio::fs;
use tracing::info;
use uuid::Uuid;

use super::{
	finalize_thumbnailer, process_step, ThumbnailerError, ThumbnailerJobReport,
	ThumbnailerJobState, ThumbnailerJobStep, ThumbnailerJobStepKind, FILTERED_IMAGE_EXTENSIONS,
	THUMBNAIL_CACHE_DIR_NAME,
};

#[cfg(feature = "ffmpeg")]
use super::FILTERED_VIDEO_EXTENSIONS;

pub struct ShallowThumbnailerJob {}

#[derive(Serialize, Deserialize, Clone)]
pub struct ShallowThumbnailerJobInit {
	pub location: location::Data,
	pub sub_path: PathBuf,
}

impl Hash for ShallowThumbnailerJobInit {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.location.id.hash(state);
		self.sub_path.hash(state);
	}
}

impl JobInitData for ShallowThumbnailerJobInit {
	type Job = ShallowThumbnailerJob;
}

#[async_trait::async_trait]
impl StatefulJob for ShallowThumbnailerJob {
	type Init = ShallowThumbnailerJobInit;
	type Data = ThumbnailerJobState;
	type Step = ThumbnailerJobStep;

	const NAME: &'static str = "shallow_thumbnailer";
	const IS_BACKGROUND: bool = true;

	fn new() -> Self {
		Self {}
	}

	async fn init(&self, ctx: WorkerContext, state: &mut JobState<Self>) -> Result<(), JobError> {
		let Library { db, .. } = &ctx.library;

		let thumbnail_dir = ctx
			.library
			.config()
			.data_directory()
			.join(THUMBNAIL_CACHE_DIR_NAME);

		let location_id = state.init.location.id;
		let location_path = PathBuf::from(&state.init.location.path);

		let sub_path_id = if state.init.sub_path != Path::new("") {
			let full_path = ensure_sub_path_is_in_location(&location_path, &state.init.sub_path)
				.await
				.map_err(ThumbnailerError::from)?;
			ensure_sub_path_is_directory(&location_path, &state.init.sub_path)
				.await
				.map_err(ThumbnailerError::from)?;

			get_existing_file_path_id(
				&MaterializedPath::new(location_id, &location_path, &full_path, true)
					.map_err(ThumbnailerError::from)?,
				db,
			)
			.await
			.map_err(ThumbnailerError::from)?
			.expect("Sub path should already exist in the database")
		} else {
			get_existing_file_path_id(
				&MaterializedPath::new(location_id, &location_path, &location_path, true)
					.map_err(ThumbnailerError::from)?,
				db,
			)
			.await
			.map_err(ThumbnailerError::from)?
			.expect("Location root path should already exist in the database")
		};

		info!("Searching for images in location {location_id} at parent directory with id {sub_path_id}");

		// create all necessary directories if they don't exist
		fs::create_dir_all(&thumbnail_dir).await?;

		// query database for all image files in this location that need thumbnails
		let image_files = get_files_by_extensions(
			db,
			location_id,
			sub_path_id,
			&FILTERED_IMAGE_EXTENSIONS,
			ThumbnailerJobStepKind::Image,
		)
		.await?;
		info!("Found {:?} image files", image_files.len());

		#[cfg(feature = "ffmpeg")]
		let all_files = {
			// query database for all video files in this location that need thumbnails
			let video_files = get_files_by_extensions(
				db,
				location_id,
				sub_path_id,
				&FILTERED_VIDEO_EXTENSIONS,
				ThumbnailerJobStepKind::Video,
			)
			.await?;
			info!("Found {:?} video files", video_files.len());

			image_files
				.into_iter()
				.chain(video_files.into_iter())
				.collect::<VecDeque<_>>()
		};
		#[cfg(not(feature = "ffmpeg"))]
		let all_files = { image_files.into_iter().collect::<VecDeque<_>>() };

		ctx.progress(vec![
			JobReportUpdate::TaskCount(all_files.len()),
			JobReportUpdate::Message(format!("Preparing to process {} files", all_files.len())),
		]);

		state.data = Some(ThumbnailerJobState {
			thumbnail_dir,
			location_path,
			report: ThumbnailerJobReport {
				location_id,
				materialized_path: if state.init.sub_path != Path::new("") {
					// SAFETY: We know that the sub_path is a valid UTF-8 string because we validated it before
					state.init.sub_path.to_str().unwrap().to_string()
				} else {
					MAIN_SEPARATOR_STR.to_string()
				},
				thumbnails_created: 0,
			},
		});
		state.steps = all_files;

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		process_step(
			false, // On shallow thumbnailer, we want to show thumbnails ASAP
			state.step_number,
			&state.steps[0],
			state
				.data
				.as_mut()
				.expect("critical error: missing data on job state"),
			ctx,
		)
		.await
	}

	async fn finalize(&mut self, ctx: WorkerContext, state: &mut JobState<Self>) -> JobResult {
		finalize_thumbnailer(
			state
				.data
				.as_ref()
				.expect("critical error: missing data on job state"),
			ctx,
		)
	}
}

async fn get_files_by_extensions(
	db: &PrismaClient,
	location_id: LocationId,
	parent_id: Uuid,
	extensions: &[Extension],
	kind: ThumbnailerJobStepKind,
) -> Result<Vec<ThumbnailerJobStep>, JobError> {
	Ok(db
		.file_path()
		.find_many(vec![
			file_path::location_id::equals(location_id),
			file_path::extension::in_vec(extensions.iter().map(ToString::to_string).collect()),
			file_path::parent_id::equals(Some(uuid_to_bytes(parent_id))),
		])
		.select(file_path_just_materialized_path_cas_id::select())
		.exec()
		.await?
		.into_iter()
		.map(|file_path| ThumbnailerJobStep { file_path, kind })
		.collect())
}
