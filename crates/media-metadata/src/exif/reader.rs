use crate::Result;

use std::{
	fs::File,
	io::{BufReader, Cursor},
	path::Path,
	str::FromStr,
};

use exif::{Exif, In, Tag};
use sd_utils::error::FileIOError;

/// An [`ExifReader`]. This can get exif tags from images (either files or slices).
pub struct ExifReader(Exif);

impl ExifReader {
	pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
		exif::Reader::new()
			.read_from_container(&mut BufReader::new(
				File::open(&path).map_err(|e| FileIOError::from((path, e)))?,
			))
			.map(Self)
			.map_err(Into::into)
	}

	pub fn from_slice(slice: &[u8]) -> Result<Self> {
		exif::Reader::new()
			.read_from_container(&mut Cursor::new(slice))
			.map(Self)
			.map_err(Into::into)
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
