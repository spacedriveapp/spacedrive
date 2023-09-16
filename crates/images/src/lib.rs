#![warn(
	clippy::all,
	clippy::pedantic,
	clippy::correctness,
	clippy::perf,
	clippy::style,
	clippy::suspicious,
	clippy::complexity,
	clippy::nursery,
	clippy::unwrap_used,
	unused_qualifications,
	rust_2018_idioms,
	clippy::expect_used,
	trivial_casts,
	trivial_numeric_casts,
	unused_allocation,
	clippy::as_conversions,
	clippy::dbg_macro
)]
#![forbid(unsafe_code)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

mod consts;
mod error;
mod formatter;
mod generic;
#[cfg(all(feature = "heif", any(not(target_os = "linux"), linux_heif)))]
mod heif;
mod svg;

pub use error::{Error, Result};
pub use formatter::format_image;
pub use image::DynamicImage;
use std::{fs, io::Read, path::Path};

pub trait ImageHandler {
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
