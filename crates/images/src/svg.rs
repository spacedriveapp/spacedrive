use std::path::Path;

use crate::{
	consts::{SVG_MAXIMUM_FILE_SIZE, SVG_RENDER_SIZE},
	Error, ImageHandler, Result,
};
use image::DynamicImage;
use resvg::{
	tiny_skia::{self},
	usvg,
};
use usvg::{fontdb, TreeParsing, TreeTextToPath};

pub struct SvgHandler {}

impl ImageHandler for SvgHandler {
	fn maximum_size(&self) -> u64 {
		SVG_MAXIMUM_FILE_SIZE
	}

	fn validate_image(&self, _bits_per_pixel: u8, _length: usize) -> Result<()> {
		Ok(())
	}

	fn handle_image(&self, path: &Path) -> Result<DynamicImage> {
		let data = self.get_data(path)?;
		let rtree = usvg::Tree::from_data(&data, &usvg::Options::default()).map(|mut tree| {
			let mut fontdb = fontdb::Database::new();
			fontdb.load_system_fonts();
			tree.convert_text(&fontdb);
			resvg::Tree::from_usvg(&tree)
		})?;

		let size = if rtree.size.width() > rtree.size.height() {
			rtree.size.to_int_size().scale_to_width(SVG_RENDER_SIZE) // make this a const
		} else {
			rtree.size.to_int_size().scale_to_height(SVG_RENDER_SIZE)
		}
		.ok_or(Error::InvalidLength)?;

		#[allow(clippy::cast_precision_loss)]
		#[allow(clippy::as_conversions)]
		let transform = tiny_skia::Transform::from_scale(
			size.width() as f32 / rtree.size.width(),
			size.height() as f32 / rtree.size.height(),
		);

		#[allow(clippy::cast_possible_truncation)]
		#[allow(clippy::cast_sign_loss)]
		#[allow(clippy::as_conversions)]
		let Some(mut pixmap) = tiny_skia::Pixmap::new(size.width(), size.height()) else {
			return Err(Error::Pixbuf);
		};

		rtree.render(transform, &mut pixmap.as_mut());

		image::RgbaImage::from_raw(pixmap.width(), pixmap.height(), pixmap.data().into())
			.map_or_else(
				|| Err(Error::RgbImageConversion),
				|x| Ok(DynamicImage::ImageRgba8(x)),
			)
	}
}
