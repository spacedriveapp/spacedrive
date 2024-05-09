use std::path::Path;

use exif::Tag;

use super::ExifReader;

#[derive(
	Default, Clone, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize, specta::Type,
)]
pub enum Composite {
	/// The data is present, but we're unable to determine what they mean
	#[default]
	Unknown,
	/// Not a composite image
	False,
	/// A general composite image
	General,
	/// The composite image was captured while shooting
	Live,
}

impl Composite {
	/// This is used for quickly sourcing [`Composite`] data from a path
	#[allow(clippy::future_not_send)]
	pub fn source_composite(path: impl AsRef<Path>) -> Option<Self> {
		let reader = ExifReader::from_path(path).ok()?;
		reader.get_tag_int(Tag::CompositeImage).map(Into::into)
	}

	/// This is used for quickly sourcing a [`Composite`] type from an [`ExifReader`]
	pub fn from_reader(reader: &ExifReader) -> Option<Self> {
		reader.get_tag_int(Tag::CompositeImage).map(Into::into)
	}
}

impl From<u32> for Composite {
	fn from(value: u32) -> Self {
		match value {
			1 => Self::False,
			2 => Self::General,
			3 => Self::Live,
			_ => Self::Unknown,
		}
	}
}
