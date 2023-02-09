use crate::{
	api::CoreEvent,
	invalidate_query,
	job::{JobError, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext},
	library::LibraryContext,
	prisma::{file_path, location},
};

use std::{
	collections::VecDeque,
	error::Error,
	ops::Deref,
	path::{Path, PathBuf},
};

use image::{self, imageops, DynamicImage, GenericImageView};
use sd_file_ext::extensions::{Extension, ImageExtension, VideoExtension};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{fs, task::block_in_place};
use tracing::{error, info, trace, warn};
use webp::Encoder;

static THUMBNAIL_SIZE_FACTOR: f32 = 0.2;
static THUMBNAIL_QUALITY: f32 = 30.0;
pub static THUMBNAIL_CACHE_DIR_NAME: &str = "thumbnails";
pub const THUMBNAIL_JOB_NAME: &str = "thumbnailer";

pub struct ThumbnailJob {}

#[derive(Serialize, Deserialize, Clone, Hash)]
pub struct ThumbnailJobInit {
	pub location_id: i32,
	pub root_path: PathBuf,
	pub background: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ThumbnailJobState {
	thumbnail_dir: PathBuf,
	root_path: PathBuf,
}

#[derive(Error, Debug)]
pub enum ThumbnailError {
	#[error("Location not found: <id = '{0}'>")]
	MissingLocation(i32),
	#[error("Root file path not found: <path = '{0}'>")]
	MissingRootFilePath(PathBuf),
	#[error("Location without local path: <id = '{0}'>")]
	LocationLocalPath(i32),
}

file_path::include!(file_path_with_object { object });
file_path::select!(file_path_id_only { id });

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
enum ThumbnailJobStepKind {
	Image,
	#[cfg(feature = "ffmpeg")]
	Video,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ThumbnailJobStep {
	file_path: file_path_with_object::Data,
	object_id: i32,
	kind: ThumbnailJobStepKind,
}

#[async_trait::async_trait]
impl StatefulJob for ThumbnailJob {
	type Init = ThumbnailJobInit;
	type Data = ThumbnailJobState;
	type Step = ThumbnailJobStep;

	fn name(&self) -> &'static str {
		THUMBNAIL_JOB_NAME
	}

	async fn init(&self, ctx: WorkerContext, state: &mut JobState<Self>) -> Result<(), JobError> {
		let thumbnail_dir = ctx
			.library_ctx
			.config()
			.data_directory()
			.join(THUMBNAIL_CACHE_DIR_NAME);

		let location = ctx
			.library_ctx
			.db
			.location()
			.find_unique(location::id::equals(state.init.location_id))
			.exec()
			.await?
			.ok_or(ThumbnailError::MissingLocation(state.init.location_id))?;

		let root_path_str = state
			.init
			.root_path
			.to_str()
			.expect("Found non-UTF-8 path")
			.to_string();

		let parent_directory_id = ctx
			.library_ctx
			.db
			.file_path()
			.find_first(vec![
				file_path::location_id::equals(state.init.location_id),
				file_path::materialized_path::equals(if !root_path_str.is_empty() {
					root_path_str
				} else {
					"/".to_string()
				}),
				file_path::is_dir::equals(true),
			])
			.select(file_path_id_only::select())
			.exec()
			.await?
			.ok_or_else(|| ThumbnailError::MissingRootFilePath(state.init.root_path.clone()))?
			.id;

		info!(
			"Searching for images in location {} at directory {}",
			location.id, parent_directory_id
		);

		// create all necessary directories if they don't exist
		fs::create_dir_all(&thumbnail_dir).await?;
		let root_path = location
			.local_path
			.map(PathBuf::from)
			.ok_or(ThumbnailError::LocationLocalPath(location.id))?;

		// query database for all image files in this location that need thumbnails
		let image_files = get_files_by_extensions(
			&ctx.library_ctx,
			state.init.location_id,
			parent_directory_id,
			&sd_file_ext::extensions::ALL_IMAGE_EXTENSIONS
				.iter()
				.map(Clone::clone)
				.filter(can_generate_thumbnail_for_image)
				.map(Extension::Image)
				.collect::<Vec<_>>(),
			ThumbnailJobStepKind::Image,
		)
		.await?;
		info!("Found {:?} image files", image_files.len());

		#[cfg(feature = "ffmpeg")]
		let all_files = {
			// query database for all video files in this location that need thumbnails
			let video_files = get_files_by_extensions(
				&ctx.library_ctx,
				state.init.location_id,
				parent_directory_id,
				&sd_file_ext::extensions::ALL_VIDEO_EXTENSIONS
					.iter()
					.map(Clone::clone)
					.filter(can_generate_thumbnail_for_video)
					.map(Extension::Video)
					.collect::<Vec<_>>(),
				ThumbnailJobStepKind::Video,
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

		state.data = Some(ThumbnailJobState {
			thumbnail_dir,
			root_path,
		});
		state.steps = all_files;

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let step = &state.steps[0];
		ctx.progress(vec![JobReportUpdate::Message(format!(
			"Processing {}",
			step.file_path.materialized_path
		))]);

		let data = state
			.data
			.as_ref()
			.expect("critical error: missing data on job state");

		// assemble the file path
		let path = data.root_path.join(&step.file_path.materialized_path);
		trace!("image_file {:?}", step);

		// get cas_id, if none found skip
		let Some(cas_id) = &step.file_path.cas_id else {
			warn!(
				"skipping thumbnail generation for {}",
				step.file_path.materialized_path
			);

			return Ok(());
		};

		// Define and write the WebP-encoded file to a given path
		let output_path = data.thumbnail_dir.join(cas_id).with_extension("webp");

		// check if file exists at output path
		if !output_path.try_exists().unwrap() {
			info!("Writing {:?} to {:?}", path, output_path);

			match step.kind {
				ThumbnailJobStepKind::Image => {
					if let Err(e) = generate_image_thumbnail(&path, &output_path).await {
						error!("Error generating thumb for image {:#?}", e);
					}
				}
				#[cfg(feature = "ffmpeg")]
				ThumbnailJobStepKind::Video => {
					// use crate::{
					// 	object::preview::{extract_media_data, StreamKind},
					// 	prisma::media_data,
					// };

					// use
					if let Err(e) = generate_video_thumbnail(&path, &output_path).await {
						error!("Error generating thumb for video: {:?} {:#?}", &path, e);
					}
					// extract MediaData from video and put in the database
					// TODO: this is bad here, maybe give it its own job?
					// if let Ok(media_data) = extract_media_data(&path) {
					// 	info!(
					// 		"Extracted media data for object {}: {:?}",
					// 		step.object_id, media_data
					// 	);

					// 	// let primary_video_stream = media_data
					// 	// 	.steams
					// 	// 	.iter()
					// 	// 	.find(|s| s.kind == Some(StreamKind::Video(_)));

					// 	let params = vec![
					// 		media_data::duration_seconds::set(Some(media_data.duration_seconds)),
					// 		// media_data::pixel_width::set(Some(media_data.width)),
					// 		// media_data::pixel_height::set(Some(media_data.height)),
					// 	];
					// 	let _ = ctx
					// 		.library_ctx()
					// 		.db
					// 		.media_data()
					// 		.upsert(
					// 			media_data::id::equals(step.object_id),
					// 			params.clone(),
					// 			params,
					// 		)
					// 		.exec()
					// 		.await?;
					// }
				}
			}

			if !state.init.background {
				ctx.library_ctx.emit(CoreEvent::NewThumbnail {
					cas_id: cas_id.clone(),
				});
			};

			// With this invalidate query, we update the user interface to show each new thumbnail
			invalidate_query!(ctx.library_ctx, "locations.getExplorerData");
		} else {
			info!("Thumb exists, skipping... {}", output_path.display());
		}

		ctx.progress(vec![JobReportUpdate::CompletedTaskCount(
			state.step_number + 1,
		)]);

		Ok(())
	}

	async fn finalize(&mut self, _ctx: WorkerContext, state: &mut JobState<Self>) -> JobResult {
		let data = state
			.data
			.as_ref()
			.expect("critical error: missing data on job state");
		info!(
			"Finished thumbnail generation for location {} at {}",
			state.init.location_id,
			data.root_path.display()
		);

		// TODO: Serialize and return metadata here
		Ok(None)
	}
}

pub async fn generate_image_thumbnail<P: AsRef<Path>>(
	file_path: P,
	output_path: P,
) -> Result<(), Box<dyn Error>> {
	// Webp creation has blocking code
	let webp = block_in_place(|| -> Result<Vec<u8>, Box<dyn Error>> {
		// Using `image` crate, open the included .jpg file
		let img = image::open(file_path)?;
		let (w, h) = img.dimensions();
		// Optionally, resize the existing photo and convert back into DynamicImage
		let img = DynamicImage::ImageRgba8(imageops::resize(
			&img,
			// FIXME : Think of a better heuristic to get the thumbnail size
			(w as f32 * THUMBNAIL_SIZE_FACTOR) as u32,
			(h as f32 * THUMBNAIL_SIZE_FACTOR) as u32,
			imageops::FilterType::Triangle,
		));
		// Create the WebP encoder for the above image
		let encoder = Encoder::from_image(&img)?;

		// Encode the image at a specified quality 0-100

		// Type WebPMemory is !Send, which makes the Future in this function !Send,
		// this make us `deref` to have a `&[u8]` and then `to_owned` to make a Vec<u8>
		// which implies on a unwanted clone...
		Ok(encoder.encode(THUMBNAIL_QUALITY).deref().to_owned())
	})?;

	fs::write(output_path, &webp).await.map_err(Into::into)
}

#[cfg(feature = "ffmpeg")]
pub async fn generate_video_thumbnail<P: AsRef<Path>>(
	file_path: P,
	output_path: P,
) -> Result<(), Box<dyn Error>> {
	use sd_ffmpeg::to_thumbnail;

	to_thumbnail(file_path, output_path, 256, THUMBNAIL_QUALITY).await?;

	Ok(())
}

async fn get_files_by_extensions(
	ctx: &LibraryContext,
	location_id: i32,
	_parent_file_path_id: i32,
	extensions: &[Extension],
	kind: ThumbnailJobStepKind,
) -> Result<Vec<ThumbnailJobStep>, JobError> {
	Ok(ctx
		.db
		.file_path()
		.find_many(vec![
			file_path::location_id::equals(location_id),
			file_path::extension::in_vec(extensions.iter().map(ToString::to_string).collect()),
			// file_path::parent_id::equals(Some(parent_file_path_id)),
		])
		.include(file_path_with_object::include())
		.exec()
		.await?
		.into_iter()
		.map(|file_path| ThumbnailJobStep {
			object_id: file_path.object.as_ref().unwrap().id,
			file_path,
			kind,
		})
		.collect())
}

#[allow(unused)]
pub fn can_generate_thumbnail_for_video(video_extension: &VideoExtension) -> bool {
	use VideoExtension::*;
	// File extensions that are specifically not supported by the thumbnailer
	!matches!(video_extension, Mpg | Swf | M2v | Hevc)
}
#[allow(unused)]
pub fn can_generate_thumbnail_for_image(image_extension: &ImageExtension) -> bool {
	use ImageExtension::*;
	matches!(image_extension, Jpg | Jpeg | Png | Webp | Gif)
}
