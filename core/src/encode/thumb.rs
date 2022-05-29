use crate::job::jobs::JobReportUpdate;
use crate::node::state;
use crate::{
	job::{jobs::Job, worker::WorkerContext},
	prisma::file_path,
	CoreContext,
};
use crate::{sys, CoreEvent};
use anyhow::Result;
use futures::executor::block_on;
use image::*;
use std::fs;
use std::path::{Path, PathBuf};
use webp::*;

#[derive(Debug, Clone)]
pub struct ThumbnailJob {
	pub location_id: i32,
	pub path: String,
	pub background: bool,
}

static THUMBNAIL_SIZE_FACTOR: f32 = 0.2;
static THUMBNAIL_QUALITY: f32 = 30.0;
pub static THUMBNAIL_CACHE_DIR_NAME: &str = "thumbnails";

#[async_trait::async_trait]
impl Job for ThumbnailJob {
	fn name(&self) -> &'static str {
		"file_identifier"
	}
	async fn run(&self, ctx: WorkerContext) -> Result<()> {
		let config = state::get();
		let core_ctx = ctx.core_ctx.clone();

		let location = sys::locations::get_location(&core_ctx, self.location_id).await?;

		fs::create_dir_all(
			Path::new(&config.data_path)
				.join(THUMBNAIL_CACHE_DIR_NAME)
				.join(format!("{}", self.location_id)),
		)?;

		let root_path = location.path.unwrap();

		let image_files = get_images(&core_ctx, self.location_id, &self.path).await?;

		println!("Found {:?} files", image_files.len());

		let is_background = self.background.clone();

		tokio::task::spawn_blocking(move || {
			ctx.progress(vec![
				JobReportUpdate::TaskCount(image_files.len()),
				JobReportUpdate::Message(format!(
					"Preparing to process {} files",
					image_files.len()
				)),
			]);

			for (i, image_file) in image_files.iter().enumerate() {
				ctx.progress(vec![JobReportUpdate::Message(format!(
					"Processing {}",
					image_file.materialized_path.clone()
				))]);
				let path = format!("{}{}", root_path, image_file.materialized_path);
				println!("image_file {:?}", image_file);

				let cas_id = match image_file.file() {
					Ok(i) => i.unwrap().cas_id.clone(),
					Err(_) => {
						println!("Error getting cas_id {:?}", image_file.materialized_path);
						continue;
					}
				};

				// Define and write the WebP-encoded file to a given path
				let output_path = Path::new(&config.data_path)
					.join(THUMBNAIL_CACHE_DIR_NAME)
					.join(format!("{}", location.id))
					.join(&cas_id)
					.with_extension("webp");

				// check if file exists at output path
				if !output_path.exists() {
					println!("writing {:?} to {}", output_path, path);
					generate_thumbnail(&path, &output_path)
						.map_err(|e| {
							println!("error generating thumb {:?}", e);
						})
						.unwrap_or(());

					ctx.progress(vec![JobReportUpdate::CompletedTaskCount(i + 1)]);

					if !is_background {
						block_on(ctx.core_ctx.emit(CoreEvent::NewThumbnail { cas_id }));
					};
				} else {
					println!("Thumb exists, skipping... {}", output_path.display());
				}
			}
		})
		.await?;

		Ok(())
	}
}

pub fn generate_thumbnail(file_path: &str, output_path: &PathBuf) -> Result<()> {
	// Using `image` crate, open the included .jpg file
	let img = image::open(file_path)?;
	let (w, h) = img.dimensions();
	// Optionally, resize the existing photo and convert back into DynamicImage
	let img: DynamicImage = image::DynamicImage::ImageRgba8(imageops::resize(
		&img,
		(w as f32 * THUMBNAIL_SIZE_FACTOR) as u32,
		(h as f32 * THUMBNAIL_SIZE_FACTOR) as u32,
		imageops::FilterType::Triangle,
	));
	// Create the WebP encoder for the above image
	let encoder: Encoder = Encoder::from_image(&img).map_err(|_| anyhow::anyhow!("jeff"))?;

	// Encode the image at a specified quality 0-100
	let webp: WebPMemory = encoder.encode(THUMBNAIL_QUALITY);

	println!("Writing to {}", output_path.display());

	std::fs::write(&output_path, &*webp)?;

	Ok(())
}

pub async fn get_images(
	ctx: &CoreContext,
	location_id: i32,
	path: &str,
) -> Result<Vec<file_path::Data>> {
	let mut params = vec![
		file_path::location_id::equals(location_id),
		file_path::extension::in_vec(vec![
			"png".to_string(),
			"jpeg".to_string(),
			"jpg".to_string(),
			"gif".to_string(),
			"webp".to_string(),
		]),
	];

	if !path.is_empty() {
		params.push(file_path::materialized_path::starts_with(path.to_string()))
	}

	let image_files = ctx
		.database
		.file_path()
		.find_many(params)
		.with(file_path::file::fetch())
		.exec()
		.await?;

	Ok(image_files)
}
