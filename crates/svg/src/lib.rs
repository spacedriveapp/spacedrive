use image::DynamicImage;
use resvg::{tiny_skia, usvg};
use std::path::Path;
use thiserror::Error;
use tokio::fs;
use tracing::error;
use usvg::{fontdb, TreeParsing, TreeTextToPath};

type SvgResult<T> = Result<T, SvgError>;

const THUMB_SIZE: u32 = 512;

/// The maximum file size that an image can be in order to have a thumbnail generated.
///
/// This value is in MiB.
const SVG_MAXIMUM_FILE_SIZE: u64 = 1048576 * 20;

#[derive(Error, Debug)]
pub enum SvgError {
	#[error("error with usvg: {0}")]
	USvg(#[from] resvg::usvg::Error),
	#[error("error while loading the image (via the `image` crate): {0}")]
	Image(#[from] image::ImageError),
	#[error("io error: {0}")]
	Io(#[from] std::io::Error),
	#[error("failed to allocate `Pixbuf`")]
	Pixbuf,
	#[error("there was an error while converting the image to an `RgbImage`")]
	RgbImageConversion,
	#[error("failed to calculate thumbnail size")]
	InvalidSize,
	#[error("the image provided is too large (over 20MiB)")]
	TooLarge,
}

pub async fn svg_to_dynamic_image(path: &Path) -> SvgResult<DynamicImage> {
	if fs::metadata(path).await?.len() > SVG_MAXIMUM_FILE_SIZE {
		return Err(SvgError::TooLarge);
	}

	let data = fs::read(path).await?;

	let opt = usvg::Options::default();

	let mut fontdb = fontdb::Database::new();
	fontdb.load_system_fonts();

	let mut tree = usvg::Tree::from_data(&data, &opt)?;
	tree.convert_text(&fontdb);

	let rtree = resvg::Tree::from_usvg(&tree);

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
		return Err(SvgError::Pixbuf)
	};
	rtree.render(transform, &mut pixmap.as_mut());

	let Some(rgb_img) = image::RgbaImage::from_raw(
		pixmap.width(), pixmap.height(), pixmap.data_mut().into()
	) else {
		return Err(SvgError::RgbImageConversion)
	};

	Ok(DynamicImage::ImageRgba8(rgb_img))
}
