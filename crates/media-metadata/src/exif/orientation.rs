use super::ExifReader;
use exif::Tag;
use image::DynamicImage;
use std::path::Path;

#[derive(
	Default, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type,
)]
pub enum Orientation {
	#[default]
	Normal,
	CW90,
	CW180,
	CW270,
	MirroredVertical,
	MirroredHorizontal,
	MirroredHorizontalAnd90CW,
	MirroredHorizontalAnd270CW,
}

impl Orientation {
	/// This is used for quickly sourcing [`Orientation`] data from a path, to be later used by one of the modification functions.
	#[allow(clippy::future_not_send)]
	pub fn from_path(path: impl AsRef<Path>) -> Option<Self> {
		let reader = ExifReader::from_path(path).ok()?;
		reader.get_tag_int(Tag::Orientation).map(Into::into)
	}

	/// This is used for quickly sourcing an [`Orientation`] data from an [`ExifReader`]
	pub fn from_reader(reader: &ExifReader) -> Option<Self> {
		reader.get_tag_int(Tag::Orientation).map(Into::into)
	}

	/// This is used to correct thumbnails in the thumbnailer, if we are able to source orientation data for the file at hand.
	#[must_use]
	pub fn correct_thumbnail(&self, img: DynamicImage) -> DynamicImage {
		match self {
			Self::Normal => img,
			Self::CW180 => img.rotate180(),
			Self::CW270 => img.rotate270(),
			Self::CW90 => img.rotate90(),
			Self::MirroredHorizontal => img.fliph(),
			Self::MirroredVertical => img.flipv(),
			Self::MirroredHorizontalAnd90CW => img.fliph().rotate90(),
			Self::MirroredHorizontalAnd270CW => img.fliph().rotate270(),
		}
	}
}

impl From<u32> for Orientation {
	fn from(value: u32) -> Self {
		match value {
			2 => Self::MirroredHorizontal,
			3 => Self::CW180,
			4 => Self::MirroredVertical,
			5 => Self::MirroredHorizontalAnd270CW,
			6 => Self::CW90,
			7 => Self::MirroredHorizontalAnd90CW,
			8 => Self::CW270,
			_ => Self::Normal,
		}
	}
}
