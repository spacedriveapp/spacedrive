use std::{
	fs::File,
	io::{BufReader, Cursor},
	path::Path,
	str::FromStr,
};

use sd_prisma::prisma::media_data;

use exif::{Exif, In, Tag};

use crate::{
	orientation::Orientation, ColorProfile, Dimensions, Error, Flash, MediaLocation, MediaTime,
	Result,
};

#[derive(Default, Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct MediaDataImage {
	pub dimensions: Dimensions,
	pub date_taken: MediaTime,
	pub location: Option<MediaLocation>,
	pub camera_data: CameraData,
	pub artist: Option<String>,
	pub copyright: Option<String>,
	pub exif_version: Option<String>,
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
	pub red_eye: Option<bool>,
	pub zoom: Option<f64>,
	pub iso: Option<i32>,
	pub software: Option<String>,
}

impl MediaDataImage {
	pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
		Ok(Self::from_reader(&ExifReader::from_path(path)?)?)
	}

	pub fn from_slice(slice: &[u8]) -> Result<Self> {
		Self::from_reader(&ExifReader::from_slice(slice)?)
	}

	#[allow(clippy::field_reassign_with_default)]
	pub fn from_reader(reader: &ExifReader) -> Result<Self> {
		let mut data = Self::default();

		data.date_taken = MediaTime::from_reader(reader);
		data.dimensions = Dimensions::from_reader(reader);

		data.camera_data.device_make = reader.get_tag(Tag::Make);
		data.camera_data.device_model = reader.get_tag(Tag::Model);
		data.camera_data.focal_length = reader.get_tag(Tag::FocalLength);
		data.camera_data.shutter_speed = reader.get_tag(Tag::ShutterSpeedValue);
		data.camera_data.color_space = reader.get_tag(Tag::ColorSpace);
		data.camera_data.color_profile = ColorProfile::from_reader(reader);

		data.camera_data.lens_make = reader.get_tag(Tag::LensMake);
		data.camera_data.lens_model = reader.get_tag(Tag::LensModel);
		data.camera_data.iso = reader.get_tag(Tag::PhotographicSensitivity);
		data.camera_data.zoom = reader
			.get_tag(Tag::DigitalZoomRatio)
			.map(|x: String| x.replace("unused", "1").parse().ok())
			.unwrap_or_default();

		data.camera_data.bit_depth = reader.get_tag::<String>(Tag::BitsPerSample).map_or_else(
			|| {
				reader
					.get_tag::<String>(Tag::CompressedBitsPerPixel)
					.unwrap_or_default()
					.parse()
					.ok()
			},
			|x| x.parse::<i32>().ok(),
		);

		data.camera_data.orientation = Orientation::from_reader(reader).unwrap_or_default();
		data.camera_data.flash = Flash::from_reader(reader);
		data.camera_data.software = reader.get_tag(Tag::Software);
		data.artist = reader.get_tag(Tag::Artist);
		data.copyright = reader.get_tag(Tag::Copyright);
		data.exif_version = reader.get_tag(Tag::ExifVersion);
		data.location = MediaLocation::from_exif_reader(reader).ok();

		Ok(data)
	}

	/// This is only here as there's no easy impl from this foreign type to prisma's `CreateUnchecked`
	pub fn to_query(self) -> Result<sd_prisma::prisma::media_data::CreateUnchecked> {
		let kc = media_data::CreateUnchecked {
			dimensions: serde_json::to_vec(&self.dimensions)?,
			media_date: serde_json::to_vec(&self.date_taken)?,
			camera_data: serde_json::to_vec(&self.camera_data)?,
			_params: vec![
				media_data::media_location::set(serde_json::to_vec(&self.location).ok()),
				media_data::artist::set(serde_json::to_vec(&self.artist).ok()),
				media_data::copyright::set(serde_json::to_vec(&self.copyright).ok()),
				media_data::exif_version::set(serde_json::to_vec(&self.exif_version).ok()),
			],
		};

		Ok(kc)
	}

	pub fn from_prisma_data(data: sd_prisma::prisma::media_data::Data) -> Result<Self> {
		Ok(Self {
			dimensions: serde_json::from_slice(&data.dimensions)?,
			camera_data: serde_json::from_slice(&data.camera_data)?,
			date_taken: serde_json::from_slice(&data.media_date)?,
			copyright: from_slice_option_to_option(data.copyright),
			artist: from_slice_option_to_option(data.artist),
			location: from_slice_option_to_option(data.media_location),
			exif_version: from_slice_option_to_option(data.exif_version),
		})
	}
}

// pub struct MediaDataVideo;

/// An [`ExifReader`]. This can get exif tags from images (either files or slices).
pub struct ExifReader(Exif);

impl ExifReader {
	pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
		let file = File::open(path)?;
		let mut reader = BufReader::new(file);
		Ok(Self(
			exif::Reader::new()
				.read_from_container(&mut reader)
				.map_err(|_| Error::Init)?,
		))
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

	pub(crate) fn get_orientation_int(&self) -> Option<u32> {
		self.0
			.get_field(Tag::Orientation, In::PRIMARY)
			.map(|x| x.value.get_uint(0))
			.unwrap_or_default()
	}

	pub(crate) fn get_flash_int(&self) -> Option<u32> {
		self.0
			.get_field(Tag::Flash, In::PRIMARY)
			.map(|x| x.value.get_uint(0))
			.unwrap_or_default()
	}

	pub(crate) fn get_color_profile_int(&self) -> Option<u32> {
		self.0
			.get_field(Tag::CustomRendered, In::PRIMARY)
			.map(|x| x.value.get_uint(0))
			.unwrap_or_default()
	}
}

pub fn from_slice_option_to_option<T: serde::Serialize + serde::de::DeserializeOwned>(
	value: Option<Vec<u8>>,
) -> Option<T> {
	value
		.map(|x| serde_json::from_slice(&x).ok())
		.unwrap_or_default()
}
