use exif::Tag;

use super::ExifReader;

#[derive(
	Default, Clone, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize, specta::Type,
)]
pub struct Resolution {
	pub width: i32,
	pub height: i32,
}

impl Resolution {
	#[must_use]
	/// Creates a new width and height container
	///
	/// # Examples
	///
	/// ```
	/// use sd_media_metadata::exif::Resolution;
	///
	/// Resolution::new(1920, 1080);
	/// ```
	pub const fn new(width: i32, height: i32) -> Self {
		Self { width, height }
	}

	/// Reads the pixel dimensions, or zero when the file does not record them.
	///
	/// `XResolution` and `YResolution` are dots per inch, usually 72, so
	/// they are never a stand-in for the pixel size.
	#[must_use]
	pub fn from_reader(reader: &ExifReader) -> Self {
		Self {
			width: reader.get_tag(Tag::PixelXDimension).unwrap_or_default(),
			height: reader.get_tag(Tag::PixelYDimension).unwrap_or_default(),
		}
	}
}

#[cfg(test)]
mod tests {
	use exif::{experimental::Writer, Field, In, Tag, Value};

	use super::{ExifReader, Resolution};

	fn tiff(fields: &[Field]) -> Vec<u8> {
		let mut writer = Writer::new();
		for field in fields {
			writer.push_field(field);
		}
		let mut buffer = std::io::Cursor::new(Vec::new());
		writer.write(&mut buffer, false).expect("a TIFF");
		buffer.into_inner()
	}

	fn field(tag: Tag, value: Value) -> Field {
		Field {
			tag,
			ifd_num: In::PRIMARY,
			value,
		}
	}

	#[test]
	fn dots_per_inch_are_not_pixels() {
		let bytes = tiff(&[
			field(Tag::XResolution, Value::Rational(vec![(72, 1).into()])),
			field(Tag::YResolution, Value::Rational(vec![(72, 1).into()])),
		]);
		let reader = ExifReader::from_slice(&bytes).expect("readable");
		assert_eq!(Resolution::from_reader(&reader), Resolution::new(0, 0));
	}

	#[test]
	fn pixel_dimensions_are_read() {
		let bytes = tiff(&[
			field(Tag::XResolution, Value::Rational(vec![(72, 1).into()])),
			field(Tag::PixelXDimension, Value::Long(vec![800])),
			field(Tag::PixelYDimension, Value::Long(vec![533])),
		]);
		let reader = ExifReader::from_slice(&bytes).expect("readable");
		assert_eq!(Resolution::from_reader(&reader), Resolution::new(800, 533));
	}
}
