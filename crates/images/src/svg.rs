use std::path::Path;

use crate::{consts::SVG_MAXIMUM_FILE_SIZE, ConvertImage, Error, Result};
use image::DynamicImage;
use resvg::{
	tiny_skia::{self},
	usvg,
};
use usvg::{fontdb, TreeParsing, TreeTextToPath};

pub struct SvgHandler {}

impl ConvertImage for SvgHandler {
	fn maximum_size(&self) -> u64 {
		SVG_MAXIMUM_FILE_SIZE
	}

	fn validate_image(&self, bits_per_pixel: u8, length: usize) -> Result<()> {
		if bits_per_pixel != 8 {
			return Err(Error::InvalidBitDepth);
		} else if length % 3 != 0 {
			return Err(Error::InvalidLength);
		}

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

		let transform = tiny_skia::Transform::from_scale(rtree.size.width(), rtree.size.height());

		let Some(mut pixmap) =
			tiny_skia::Pixmap::new(rtree.size.width() as u32, rtree.size.height() as u32)
		else {
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
