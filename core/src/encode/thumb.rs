use crate::{
	job::{Job, JobReportUpdate, JobResult, WorkerContext},
	library::LibraryContext,
	prisma::file_path,
	sys,
};
use image::{self, imageops, DynamicImage, GenericImageView};
use std::error::Error;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, error, info};
use webp::Encoder;

#[derive(Debug, Clone)]
pub struct ThumbnailJob {
	pub location_id: i32,
	pub path: PathBuf,
	pub background: bool,
}

static THUMBNAIL_SIZE_FACTOR: f32 = 0.2;
static THUMBNAIL_QUALITY: f32 = 30.0;
pub static THUMBNAIL_CACHE_DIR_NAME: &str = "thumbnails";

#[async_trait::async_trait]
impl Job for ThumbnailJob {
	fn name(&self) -> &'static str {
		"thumbnailer"
	}
	async fn run(&self, ctx: WorkerContext) -> JobResult {
		let library_ctx = ctx.library_ctx();
		let thumbnail_dir = library_ctx
			.config()
			.data_directory()
			.join(THUMBNAIL_CACHE_DIR_NAME)
			.join(self.location_id.to_string());

		let location = sys::get_location(&library_ctx, self.location_id).await?;

		info!(
			"Searching for images in location {} at path {:#?}",
			location.id, self.path
		);

		// create all necessary directories if they don't exist
		fs::create_dir_all(&thumbnail_dir).await?;
		let root_path = location.path.unwrap();

		// query database for all files in this location that need thumbnails
		let image_files = get_images(&library_ctx, self.location_id, &self.path).await?;
		info!("Found {:?} files", image_files.len());

		ctx.progress(vec![
			JobReportUpdate::TaskCount(image_files.len()),
			JobReportUpdate::Message(format!("Preparing to process {} files", image_files.len())),
		]);

		for (i, image_file) in image_files.iter().enumerate() {
			ctx.progress(vec![JobReportUpdate::Message(format!(
				"Processing {}",
				image_file.materialized_path
			))]);

			// assemble the file path
			let path = Path::new(&root_path).join(&image_file.materialized_path);
			debug!("image_file {:?}", image_file);

			// get cas_id, if none found skip
			let cas_id = match image_file.file() {
				Ok(file) => {
					if let Some(f) = file {
						f.cas_id.clone()
					} else {
						info!(
							"skipping thumbnail generation for {}",
							image_file.materialized_path
						);
						continue;
					}
				}
				Err(_) => {
					error!("Error getting cas_id {:?}", image_file.materialized_path);
					continue;
				}
			};

			// Define and write the WebP-encoded file to a given path
			let output_path = thumbnail_dir.join(&cas_id).with_extension("webp");

			// check if file exists at output path
			if !output_path.exists() {
				info!("Writing {:?} to {:?}", path, output_path);
				tokio::spawn(async move {
					if let Err(e) = generate_thumbnail(&path, &output_path).await {
						error!("Error generating thumb {:?}", e);
					}
				});

				ctx.progress(vec![JobReportUpdate::CompletedTaskCount(i + 1)]);

				if !self.background {
					// ctx.library_ctx()
					// 	.emit(CoreEvent::NewThumbnail { cas_id })
					// 	.await;
				};
			} else {
				info!("Thumb exists, skipping... {}", output_path.display());
			}
		}

		Ok(())
	}
}

pub async fn generate_thumbnail<P: AsRef<Path>>(
	file_path: P,
	output_path: P,
) -> Result<(), Box<dyn Error>> {
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
	let webp = encoder.encode(THUMBNAIL_QUALITY).deref().to_owned();
	fs::write(output_path, &webp).await?;

	Ok(())
}

pub async fn get_images(
	ctx: &LibraryContext,
	location_id: i32,
	path: impl AsRef<Path>,
) -> Result<Vec<file_path::Data>, std::io::Error> {
	let mut params = vec![
		file_path::location_id::equals(Some(location_id)),
		file_path::extension::in_vec(vec![
			"png".to_string(),
			"jpeg".to_string(),
			"jpg".to_string(),
			"gif".to_string(),
			"webp".to_string(),
		]),
	];

	let path_str = path.as_ref().to_string_lossy().to_string();

	if !path_str.is_empty() {
		params.push(file_path::materialized_path::starts_with(path_str))
	}

	let image_files = ctx
		.db
		.file_path()
		.find_many(params)
		.with(file_path::file::fetch())
		.exec()
		.await
		.unwrap();

	Ok(image_files)
}
