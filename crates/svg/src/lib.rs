use image::DynamicImage;
use resvg::{tiny_skia, usvg};
use std::path::Path;
use thiserror::Error;
use tokio::fs;
use tracing::error;
use usvg::{fontdb, TreeParsing, TreeTextToPath};

type SvgResult<T> = Result<T, SvgError>;

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
	#[error("the image provided is unsupported")]
	Unsupported,
	#[error("the image provided is too large (over 20MiB)")]
	TooLarge,
	#[error("the provided bit depth is invalid")]
	InvalidBitDepth,
	#[error("invalid path provided (non UTF-8)")]
	InvalidPath,
}

pub async fn svg_to_dynamic_image(path: &Path) -> SvgResult<DynamicImage> {
	if fs::metadata(path).await?.len() > SVG_MAXIMUM_FILE_SIZE {
		return Err(SvgError::TooLarge);
	}

	let opt = usvg::Options::default();

	let mut fontdb = fontdb::Database::new();
	fontdb.load_system_fonts();

	let data = fs::read(path).await?;

	let mut tree = usvg::Tree::from_data(&data, &opt)?;
	tree.convert_text(&fontdb);

	let rtree = resvg::Tree::from_usvg(&tree);

	let Some(mut pixmap) = tiny_skia::Pixmap::new(rtree.size.width() as u32, rtree.size.height() as u32) else {
		return Err(SvgError::Pixbuf)
	};

	rtree.render(tiny_skia::Transform::default(), &mut pixmap.as_mut());

	let Some(rgb_img) = image::RgbImage::from_raw(pixmap.width(), pixmap.height(), pixmap.data_mut().into()) else {
		return Err(SvgError::RgbImageConversion)
	};

	Ok(DynamicImage::ImageRgb8(rgb_img))
}
