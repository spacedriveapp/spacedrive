pub use crate::error::Result;
use crate::ImageHandler;
use image::DynamicImage;
use std::path::Path;

pub struct GenericHandler {}

impl ImageHandler for GenericHandler {
	fn handle_image(&self, path: &Path) -> Result<DynamicImage> {
		let data = self.get_data(path)?; // this also makes sure the file isn't above the maximum size
		Ok(image::load_from_memory(&data)?)
	}
}
