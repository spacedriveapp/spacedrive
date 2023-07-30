use crate::ExifReader;
use image_rs::DynamicImage;
use std::path::Path;

#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum Orientation {
	#[default]
	Normal,
	MirroredHorizontal,
	CW90,
	MirroredVertical,
	MirroredHorizontalAnd270CW,
	MirroredHorizontalAnd90CW,
	CW180,
	CW270,
}

impl Orientation {
	/// This is used for quickly sourcing [`Orientation`] data from a path, to be later used by one of the modification functions.
	// https://github.com/rust-lang/rust-clippy/issues/11087
	#[allow(clippy::future_not_send)]
	pub async fn source_orientation<P: AsRef<Path>>(path: P) -> Option<Self> {
		// TODO: We should have some error logging here
		let reader = ExifReader::from_path(path).await.ok()?;
		reader.get_orientation_ints().map(Self::int_to_orientation)
	}

	/// This follows the EXIF specification as to how images are supposed to be rotated/flipped/etc depending on their associated value
	pub(crate) const fn int_to_orientation(i: u32) -> Self {
		match i {
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
