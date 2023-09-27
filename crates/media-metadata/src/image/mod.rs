use exif::Tag;
use std::path::Path;

mod composite;
mod consts;
mod dimensions;
mod flash;
mod geographic;
mod orientation;
mod profile;
mod reader;
mod time;

pub use composite::Composite;
pub use consts::DMS_DIVISION;
pub use dimensions::Dimensions;
pub use flash::{Flash, FlashMode, FlashValue};
pub use geographic::{MediaLocation, PlusCode};
pub use orientation::Orientation;
pub use profile::ColorProfile;
pub use reader::ExifReader;
pub use time::MediaTime;

use crate::Result;

#[derive(Default, Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct ImageMetadata {
	pub dimensions: Dimensions,
	pub date_taken: MediaTime,
	pub location: Option<MediaLocation>,
	pub camera_data: ImageData,
	pub artist: Option<String>,
	pub description: Option<String>,
	pub copyright: Option<String>,
	pub exif_version: Option<String>,
}

#[derive(Default, Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct ImageData {
	pub device_make: Option<String>,
	pub device_model: Option<String>,
	pub color_space: Option<String>,
	pub color_profile: Option<ColorProfile>,
	pub focal_length: Option<f64>,
	pub shutter_speed: Option<f64>,
	pub flash: Option<Flash>,
	pub orientation: Orientation,
	pub lens_make: Option<String>,
	pub lens_model: Option<String>,
	pub bit_depth: Option<i32>,
	pub red_eye: Option<bool>,
	pub zoom: Option<f64>,
	pub iso: Option<i32>,
	pub software: Option<String>,
	pub serial_number: Option<String>,
	pub lens_serial_number: Option<String>,
	pub contrast: Option<i32>,
	pub saturation: Option<i32>,
	pub sharpness: Option<i32>,
	pub composite: Option<Composite>,
}

impl ImageMetadata {
	pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
		Self::from_reader(&ExifReader::from_path(path)?)
	}

	pub fn from_slice(bytes: &[u8]) -> Result<Self> {
		Self::from_reader(&ExifReader::from_slice(bytes)?)
	}

	#[allow(clippy::field_reassign_with_default)]
	pub fn from_reader(reader: &ExifReader) -> Result<Self> {
		let mut data = Self::default();
		let camera_data = &mut data.camera_data;

		data.date_taken = MediaTime::from_reader(reader);
		data.dimensions = Dimensions::from_reader(reader);
		data.artist = reader.get_tag(Tag::Artist);
		data.description = reader.get_tag(Tag::ImageDescription);
		data.copyright = reader.get_tag(Tag::Copyright);
		data.exif_version = reader.get_tag(Tag::ExifVersion);
		data.location = MediaLocation::from_exif_reader(reader).ok();

		camera_data.device_make = reader.get_tag(Tag::Make);
		camera_data.device_model = reader.get_tag(Tag::Model);
		camera_data.focal_length = reader.get_tag(Tag::FocalLength);
		camera_data.shutter_speed = reader.get_tag(Tag::ShutterSpeedValue);
		camera_data.color_space = reader.get_tag(Tag::ColorSpace);
		camera_data.color_profile = ColorProfile::from_reader(reader);

		camera_data.lens_make = reader.get_tag(Tag::LensMake);
		camera_data.lens_model = reader.get_tag(Tag::LensModel);
		camera_data.iso = reader.get_tag(Tag::PhotographicSensitivity);
		camera_data.zoom = reader
			.get_tag(Tag::DigitalZoomRatio)
			.map(|x: String| x.replace("unused", "1").parse().ok())
			.unwrap_or_default();

		camera_data.bit_depth = reader.get_tag::<String>(Tag::BitsPerSample).map_or_else(
			|| {
				reader
					.get_tag::<String>(Tag::CompressedBitsPerPixel)
					.unwrap_or_default()
					.parse()
					.ok()
			},
			|x| x.parse::<i32>().ok(),
		);

		camera_data.orientation = Orientation::from_reader(reader).unwrap_or_default();
		camera_data.flash = Flash::from_reader(reader);
		camera_data.software = reader.get_tag(Tag::Software);
		camera_data.serial_number = reader.get_tag(Tag::BodySerialNumber);
		camera_data.lens_serial_number = reader.get_tag(Tag::LensSerialNumber);
		camera_data.software = reader.get_tag(Tag::Software);
		camera_data.contrast = reader.get_tag(Tag::Contrast);
		camera_data.saturation = reader.get_tag(Tag::Saturation);
		camera_data.sharpness = reader.get_tag(Tag::Sharpness);
		camera_data.composite = Composite::from_reader(reader);

		Ok(data)
	}
}
