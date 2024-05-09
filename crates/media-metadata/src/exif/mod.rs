use std::path::Path;

use exif::Tag;
use sd_utils::error::FileIOError;
use tokio::task::spawn_blocking;

mod composite;
mod consts;
mod datetime;
mod flash;
mod geographic;
mod orientation;
mod profile;
mod reader;
mod resolution;

pub use composite::Composite;
pub use consts::DMS_DIVISION;
pub use datetime::MediaDate;
pub use flash::{Flash, FlashMode, FlashValue};
pub use geographic::{MediaLocation, PlusCode};
pub use orientation::Orientation;
pub use profile::ColorProfile;
pub use reader::ExifReader;
pub use resolution::Resolution;

use crate::{Error, Result};

#[derive(Default, Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct ExifMetadata {
	pub resolution: Resolution,
	pub date_taken: Option<MediaDate>,
	pub location: Option<MediaLocation>,
	pub camera_data: CameraData,
	pub artist: Option<String>,
	pub description: Option<String>,
	pub copyright: Option<String>,
	pub exif_version: Option<String>,
}

impl ExifMetadata {
	pub async fn from_path(path: impl AsRef<Path> + Send) -> Result<Option<Self>> {
		match spawn_blocking({
			let path = path.as_ref().to_owned();
			move || ExifReader::from_path(path).map(|reader| Self::from_reader(&reader))
		})
		.await?
		{
			Ok(data) => Ok(Some(data)),
			Err(Error::Exif(
				exif::Error::NotFound(_)
				| exif::Error::NotSupported(_)
				| exif::Error::BlankValue(_),
			)) => Ok(None),
			Err(Error::Exif(exif::Error::Io(e))) => Err(FileIOError::from((path, e)).into()),
			Err(e) => Err(e),
		}
	}

	pub fn from_slice(bytes: &[u8]) -> Result<Option<Self>> {
		let res = ExifReader::from_slice(bytes).map(|reader| Self::from_reader(&reader));

		if matches!(
			res,
			Err(Error::Exif(
				exif::Error::NotFound(_)
					| exif::Error::NotSupported(_)
					| exif::Error::BlankValue(_)
			))
		) {
			return Ok(None);
		}

		res.map(Some)
	}

	#[allow(clippy::field_reassign_with_default)]
	fn from_reader(reader: &ExifReader) -> Self {
		Self {
			resolution: Resolution::from_reader(reader),
			date_taken: MediaDate::from_reader(reader),
			location: MediaLocation::from_exif_reader(reader).ok(),
			camera_data: CameraData {
				device_make: reader.get_tag(Tag::Make),
				device_model: reader.get_tag(Tag::Model),
				color_space: reader.get_tag(Tag::ColorSpace),
				color_profile: ColorProfile::from_reader(reader),
				focal_length: reader.get_tag(Tag::FocalLength),
				shutter_speed: reader.get_tag(Tag::ShutterSpeedValue),
				flash: Flash::from_reader(reader),
				orientation: Orientation::from_reader(reader).unwrap_or_default(),
				lens_make: reader.get_tag(Tag::LensMake),
				lens_model: reader.get_tag(Tag::LensModel),
				bit_depth: reader.get_tag::<String>(Tag::BitsPerSample).map_or_else(
					|| {
						reader
							.get_tag::<String>(Tag::CompressedBitsPerPixel)
							.unwrap_or_default()
							.parse()
							.ok()
					},
					|x| x.parse::<i32>().ok(),
				),
				zoom: reader
					.get_tag(Tag::DigitalZoomRatio)
					.map(|x: String| x.replace("unused", "1").parse().ok())
					.unwrap_or_default(),
				iso: reader.get_tag(Tag::PhotographicSensitivity),
				software: reader.get_tag(Tag::Software),
				serial_number: reader.get_tag(Tag::BodySerialNumber),
				lens_serial_number: reader.get_tag(Tag::LensSerialNumber),
				contrast: reader.get_tag(Tag::Contrast),
				saturation: reader.get_tag(Tag::Saturation),
				sharpness: reader.get_tag(Tag::Sharpness),
				composite: Composite::from_reader(reader),
			},
			artist: reader.get_tag(Tag::Artist),
			description: reader.get_tag(Tag::ImageDescription),
			copyright: reader.get_tag(Tag::Copyright),
			exif_version: reader.get_tag(Tag::ExifVersion),
		}
	}
}

#[derive(Default, Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct CameraData {
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

// TODO(brxken128): more exif spec reading so we can source color spaces correctly too
// pub enum ImageColorSpace {
// 	Rgb,
// 	RgbP,
// 	SRgb,
// 	Cmyk,
// 	DciP3,
// 	Wiz,
// 	Biz,
// }
