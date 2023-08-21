use std::{
	fs::File,
	io::{BufReader, Cursor},
	path::Path,
	str::FromStr,
};

use exif::{Exif, In, Tag};

use crate::{Error, Result};

/// An [`ExifReader`]. This can get exif tags from images (either files or slices).
pub struct ExifReader(Exif);

impl ExifReader {
	pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
		let file = File::open(&path)?;
		let mut reader = BufReader::new(file);
		exif::Reader::new()
			.read_from_container(&mut reader)
			.map_or_else(
				|_| Err(Error::NoExifData(path.as_ref().to_path_buf())),
				|reader| Ok(Self(reader)),
			)
	}

	pub fn from_slice(slice: &[u8]) -> Result<Self> {
		Ok(Self(
			exif::Reader::new().read_from_container(&mut Cursor::new(slice))?,
		))
	}

	/// A helper function which gets the target `Tag` as `T`, provided `T` impls `FromStr`.
	///
	/// This function strips any erroneous newlines
	#[must_use]
	pub fn get_tag<T>(&self, tag: Tag) -> Option<T>
	where
		T: FromStr,
	{
		self.0.get_field(tag, In::PRIMARY).map(|x| {
			x.display_value()
				.to_string()
				.replace(['\\', '\"'], "")
				.parse::<T>()
				.ok()
		})?
	}

	pub(crate) fn get_tag_int(&self, tag: Tag) -> Option<u32> {
		self.0
			.get_field(tag, In::PRIMARY)
			.map(|x| x.value.get_uint(0))
			.unwrap_or_default()
	}
}
