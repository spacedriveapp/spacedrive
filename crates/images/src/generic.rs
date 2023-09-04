use crate::consts::GENERIC_MAXIMUM_FILE_SIZE;
pub use crate::error::{Error, Result};
use crate::ToImage;
use image::DynamicImage;
use std::path::Path;

pub struct GenericHandler {}

impl ToImage for GenericHandler {
	fn maximum_size(&self) -> u64 {
		GENERIC_MAXIMUM_FILE_SIZE
	}

	fn validate_image(&self, _bits_per_pixel: u8, _length: usize) -> Result<()> {
		Ok(())
	}

	fn handle_image(&self, path: &Path) -> Result<DynamicImage> {
		let data = self.get_data(path)?; // this also makes sure the file isn't above the maximum size
		Ok(image::load_from_memory(&data)?)
	}
}
