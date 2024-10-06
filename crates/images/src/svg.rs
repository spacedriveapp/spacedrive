use std::{path::Path, sync::Arc};

use crate::{consts::SVG_TARGET_PX, scale_dimensions, Error, ImageHandler, Result};
use image::DynamicImage;
use resvg::{tiny_skia, usvg};

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

		let mut fontdb = usvg::fontdb::Database::new();
		fontdb.load_system_fonts();

		let options = usvg::Options {
			resources_dir: None,
			dpi: 96.0,
			// Default font is user-agent dependent so we can use whichever we like.
			font_family: "Times New Roman".to_owned(),
			font_size: 12.0,
			languages: vec!["en".to_string()],
			shape_rendering: usvg::ShapeRendering::default(),
			text_rendering: usvg::TextRendering::default(),
			image_rendering: usvg::ImageRendering::default(),
			#[allow(clippy::expect_used)]
			default_size: usvg::Size::from_wh(100.0, 100.0).expect("Must be a valid size"),
			image_href_resolver: usvg::ImageHrefResolver::default(),
			font_resolver: usvg::FontResolver::default(),
			fontdb: Arc::new(fontdb),
			style_sheet: None,
		};

		let rtree = usvg::Tree::from_data(&data, &options)?;

		let (scaled_w, scaled_h) =
			scale_dimensions(rtree.size().width(), rtree.size().height(), SVG_TARGET_PX);

		let size = if rtree.size().width() > rtree.size().height() {
			rtree.size().to_int_size().scale_to_width(scaled_w)
		} else {
			rtree.size().to_int_size().scale_to_height(scaled_h)
		}
		.ok_or(Error::InvalidLength)?;

		let transform = tiny_skia::Transform::from_scale(
			size.width() as f32 / rtree.size().width(),
			size.height() as f32 / rtree.size().height(),
		);

		let Some(mut pixmap) = tiny_skia::Pixmap::new(size.width(), size.height()) else {
			return Err(Error::Pixbuf);
		};

		resvg::render(&rtree, transform, &mut pixmap.as_mut());

		image::RgbaImage::from_raw(pixmap.width(), pixmap.height(), pixmap.data().into())
			.map_or_else(
				|| Err(Error::RgbImageConversion),
				|x| Ok(DynamicImage::ImageRgba8(x)),
			)
	}
}
