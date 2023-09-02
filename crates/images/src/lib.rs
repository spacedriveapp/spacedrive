mod consts;
mod error;
mod formatter;
mod generic;
#[cfg(not(target_os = "linux"))]
mod heif;
mod svg;

pub use error::{Error, Result};
pub use formatter::format_image;
pub use image::DynamicImage;
use std::{fs, io::Read, path::Path};

pub trait ConvertImage {
	fn maximum_size(&self) -> u64
	where
		Self: Sized; // thanks vtables

	fn get_data(&self, path: &Path) -> Result<Vec<u8>>
	where
		Self: Sized,
	{
		let mut file = fs::File::open(path)?;
		if file.metadata()?.len() > self.maximum_size() {
			Err(Error::TooLarge)
		} else {
			let mut data = vec![];
			file.read_to_end(&mut data)?;
			Ok(data)
		}
	}

	fn validate_image(&self, bits_per_pixel: u8, length: usize) -> Result<()>
	where
		Self: Sized;

	fn handle_image(&self, path: &Path) -> Result<DynamicImage>;
}
