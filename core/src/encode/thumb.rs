use crate::{
	api::{locations::LocationExplorerArgs, CoreEvent, LibraryArgs},
	invalidate_query,
	job::{JobError, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext},
	library::LibraryContext,
	prisma::{file_path, location},
};

use image::{self, imageops, DynamicImage, GenericImageView};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::{
	error::Error,
	ops::Deref,
	path::{Path, PathBuf},
};
use tokio::{fs, task::block_in_place};
use tracing::{error, info, trace, warn};
use webp::Encoder;

static THUMBNAIL_SIZE_FACTOR: f32 = 0.2;
static THUMBNAIL_QUALITY: f32 = 30.0;
pub static THUMBNAIL_CACHE_DIR_NAME: &str = "thumbnails";
pub const THUMBNAIL_JOB_NAME: &str = "thumbnailer";

pub struct ThumbnailJob {}

#[derive(Serialize, Deserialize, Clone)]
pub struct ThumbnailJobInit {
	pub location_id: i32,
	pub path: PathBuf,
	pub background: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ThumbnailJobState {
	thumbnail_dir: PathBuf,
	root_path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
enum ThumbnailJobStepKind {
	Image,
	Video,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ThumbnailJobStep {
	file: file_path::Data,
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

	async fn init(
		&self,
		ctx: WorkerContext,
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> JobResult {
		let library_ctx = ctx.library_ctx();
		let thumbnail_dir = library_ctx
			.config()
			.data_directory()
			.join(THUMBNAIL_CACHE_DIR_NAME);
		// .join(state.init.location_id.to_string());

		let location = library_ctx
			.db
			.location()
			.find_unique(location::id::equals(state.init.location_id))
			.exec()
			.await?
			.unwrap();

		info!(
			"Searching for images in location {} at path {}",
			location.id,
			state.init.path.display()
		);

		// create all necessary directories if they don't exist
		fs::create_dir_all(&thumbnail_dir).await?;
		let root_path = location.local_path.map(PathBuf::from).unwrap();

		// query database for all image files in this location that need thumbnails
		let image_files = get_files_by_extension(
			&library_ctx,
			state.init.location_id,
			&state.init.path,
			vec![
				"png".to_string(),
				"jpeg".to_string(),
				"jpg".to_string(),
				"gif".to_string(),
				"webp".to_string(),
			],
			ThumbnailJobStepKind::Image,
		)
		.await?;
		info!("Found {:?} image files", image_files.len());

		// query database for all video files in this location that need thumbnails
		let video_files = get_files_by_extension(
			&library_ctx,
			state.init.location_id,
			&state.init.path,
			// Some formats extracted from https://ffmpeg.org/ffmpeg-formats.html
			vec![
				"mp4".to_string(),
				"mkv".to_string(),
				"avi".to_string(),
				"mov".to_string(),
				"ogg".to_string(),
				"webm".to_string(),
				"flv".to_string(),
			],
			ThumbnailJobStepKind::Video,
		)
		.await?;
		info!("Found {:?} video files", video_files.len());

		let all_files = image_files
			.into_iter()
			.chain(video_files.into_iter())
			.collect::<VecDeque<_>>();

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
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> JobResult {
		let step = &state.steps[0];
		ctx.progress(vec![JobReportUpdate::Message(format!(
			"Processing {}",
			step.file.materialized_path
		))]);

		let data = state
			.data
			.as_ref()
			.expect("critical error: missing data on job state");

		// assemble the file path
		let path = data.root_path.join(&step.file.materialized_path);
		trace!("image_file {:?}", step);

		// get cas_id, if none found skip
		let cas_id = match step.file.file() {
			Ok(file) => {
				if let Some(f) = file {
					f.cas_id.clone()
				} else {
					warn!(
						"skipping thumbnail generation for {}",
						step.file.materialized_path
					);
					return Ok(());
				}
			}
			Err(_) => {
				error!("Error getting cas_id {:?}", step.file.materialized_path);
				return Ok(());
			}
		};

		// Define and write the WebP-encoded file to a given path
		let output_path = data.thumbnail_dir.join(&cas_id).with_extension("webp");

		// check if file exists at output path
		if !output_path.exists() {
			info!("Writing {:?} to {:?}", path, output_path);

			match step.kind {
				ThumbnailJobStepKind::Image => {
					if let Err(e) = generate_image_thumbnail(&path, &output_path).await {
						error!("Error generating thumb for image {:?}", e);
					}
				}
				ThumbnailJobStepKind::Video => {
					if let Err(e) = generate_video_thumbnail(&path, &output_path).await {
						error!("Error generating thumb for image {:?}", e);
					}
				}
			}

			if !state.init.background {
				ctx.library_ctx().emit(CoreEvent::NewThumbnail { cas_id });
			};
		} else {
			info!("Thumb exists, skipping... {}", output_path.display());
		}

		// With this invalidate query, we update the user interface to show each new thumbnail
		let library_ctx = ctx.library_ctx();
		invalidate_query!(
			library_ctx,
			"locations.getExplorerData": LibraryArgs<LocationExplorerArgs>,
			LibraryArgs::new(
				library_ctx.id,
				LocationExplorerArgs {
					location_id: state.init.location_id,
					path: "".to_string(),
					limit: 100,
					cursor: None,
				}
			)
		);

		ctx.progress(vec![JobReportUpdate::CompletedTaskCount(
			state.step_number + 1,
		)]);

		Ok(())
	}

	async fn finalize(
		&self,
		_ctx: WorkerContext,
		state: &mut JobState<Self::Init, Self::Data, Self::Step>,
	) -> Result<(), JobError> {
		let data = state
			.data
			.as_ref()
			.expect("critical error: missing data on job state");
		info!(
			"Finished thumbnail generation for location {} at {}",
			state.init.location_id,
			data.root_path.display()
		);
		Ok(())
	}
}

async fn generate_image_thumbnail<P: AsRef<Path>>(
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

	fs::write(output_path, &webp).await?;

	Ok(())
}

async fn generate_video_thumbnail<P: AsRef<Path>>(
	_file_path: P,
	_output_path: P,
) -> Result<(), Box<dyn Error>> {
	todo!()
}

async fn get_files_by_extension(
	ctx: &LibraryContext,
	location_id: i32,
	path: impl AsRef<Path>,
	extensions: Vec<String>,
	kind: ThumbnailJobStepKind,
) -> Result<Vec<ThumbnailJobStep>, JobError> {
	let mut params = vec![
		file_path::location_id::equals(location_id),
		file_path::extension::in_vec(extensions),
	];

	let path_str = path.as_ref().to_string_lossy().to_string();

	if !path_str.is_empty() {
		params.push(file_path::materialized_path::starts_with(path_str));
	}

	Ok(ctx
		.db
		.file_path()
		.find_many(params)
		.with(file_path::file::fetch())
		.exec()
		.await?
		.into_iter()
		.map(|file| ThumbnailJobStep { file, kind })
		.collect())
}
