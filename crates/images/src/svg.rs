use std::path::Path;

use crate::{consts::SVG_TARGET_PX, scale_dimensions, Error, ImageHandler, Result};
use image::DynamicImage;
use resvg::{
	tiny_skia::{self},
	usvg,
};
use usvg::{fontdb, TreeParsing, TreeTextToPath};

#[derive(PartialEq, Eq)]
pub struct SvgHandler {}

impl ImageHandler for SvgHandler {
	#[allow(
		clippy::cast_possible_truncation,
		clippy::cast_sign_loss,
		clippy::as_conversions,
		clippy::cast_precision_loss
	)]
	fn handle_image(&self, path: &Path) -> Result<DynamicImage> {
		let data = self.get_data(path)?;
		let rtree = usvg::Tree::from_data(&data, &usvg::Options::default()).map(|mut tree| {
			let mut fontdb = fontdb::Database::new();
			fontdb.load_system_fonts();
			tree.convert_text(&fontdb);
			resvg::Tree::from_usvg(&tree)
		})?;

		let (scaled_w, scaled_h) =
			scale_dimensions(rtree.size.width(), rtree.size.height(), SVG_TARGET_PX);

		let size = if rtree.size.width() > rtree.size.height() {
			rtree.size.to_int_size().scale_to_width(scaled_w as u32)
		} else {
			rtree.size.to_int_size().scale_to_height(scaled_h as u32)
		}
		.ok_or(Error::InvalidLength)?;

		let transform = tiny_skia::Transform::from_scale(
			size.width() as f32 / rtree.size.width(),
			size.height() as f32 / rtree.size.height(),
		);

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
