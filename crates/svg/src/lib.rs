use std::sync::Arc;

use image::DynamicImage;
use resvg::{
	tiny_skia::{self, Pixmap},
	usvg,
};
use thiserror::Error;
use tokio::task::{spawn_blocking, JoinError};
use tracing::error;
use usvg::{fontdb, TreeParsing, TreeTextToPath};

type SvgResult<T> = Result<T, SvgError>;

const THUMB_SIZE: u32 = 512;

// The maximum file size that an image can be in order to have a thumbnail generated.
pub const MAXIMUM_FILE_SIZE: u64 = 20 * 1024 * 1024; // 20MB

#[derive(Error, Debug)]
pub enum SvgError {
	#[error("error with usvg: {0}")]
	USvg(#[from] resvg::usvg::Error),
	#[error("error while loading the image (via the `image` crate): {0}")]
	Image(#[from] image::ImageError),
	#[error("Blocking task failed to execute to completion")]
	Join(#[from] JoinError),
	#[error("failed to allocate `Pixbuf`")]
	Pixbuf,
	#[error("there was an error while converting the image to an `RgbImage`")]
	RgbImageConversion,
	#[error("failed to calculate thumbnail size")]
	InvalidSize,
}

pub async fn svg_to_dynamic_image(data: Arc<Vec<u8>>) -> SvgResult<DynamicImage> {
	let mut pixmap = spawn_blocking(move || -> Result<Pixmap, SvgError> {
		let rtree = usvg::Tree::from_data(&data, &usvg::Options::default()).map(|mut tree| {
			let mut fontdb = fontdb::Database::new();
			fontdb.load_system_fonts();

			tree.convert_text(&fontdb);

			resvg::Tree::from_usvg(&tree)
		})?;

		let size = if rtree.size.width() > rtree.size.height() {
			rtree.size.to_int_size().scale_to_width(THUMB_SIZE)
		} else {
			rtree.size.to_int_size().scale_to_height(THUMB_SIZE)
		}
		.ok_or(SvgError::InvalidSize)?;

		let transform = tiny_skia::Transform::from_scale(
			size.width() as f32 / rtree.size.width(),
			size.height() as f32 / rtree.size.height(),
		);

		let Some(mut pixmap) = tiny_skia::Pixmap::new(size.width(), size.height()) else {
			return Err(SvgError::Pixbuf);
		};

		rtree.render(transform, &mut pixmap.as_mut());

		Ok(pixmap)
	})
	.await??;

	let Some(rgb_img) =
		image::RgbaImage::from_raw(pixmap.width(), pixmap.height(), pixmap.data_mut().into())
	else {
		return Err(SvgError::RgbImageConversion);
	};

	Ok(DynamicImage::ImageRgba8(rgb_img))
}
