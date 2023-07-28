use exif::{Exif, In, Tag};
use sd_prisma::prisma::media_data;
use std::{fs::File, io::BufReader, path::Path, str::FromStr};

use crate::{orientation::Orientation, Dimensions, Location, MediaTime, Result};

#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct MediaDataImage {
	pub date_taken: MediaTime,
	pub dimensions: Dimensions,
	pub location: Option<Location>,
	pub camera_data: CameraData,
	pub artist: Option<String>,
	pub copyright: Option<String>,
	pub exif_version: Option<String>,
}

#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct CameraData {
	pub device_make: Option<String>,
	pub device_model: Option<String>,
	pub focal_length: Option<f64>,
	pub shutter_speed: Option<f64>,
	pub flash: Option<bool>,
	pub orientation: Orientation,
	pub lens_make: Option<String>,
	pub lens_model: Option<String>,
	pub zoom: Option<f64>,
	pub iso: Option<i32>,
	pub software: Option<String>,
}

impl MediaDataImage {
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
		let reader = ExifReader::new(path)?;
		let mut data = Self::new();

		data.date_taken = MediaTime::from_reader(&reader);
		data.dimensions = Dimensions::from_reader(&reader);

		data.camera_data.device_make = reader.get_tag(Tag::LensModel);
		data.camera_data.device_model = reader.get_tag(Tag::LensModel);
		data.camera_data.focal_length = reader.get_tag(Tag::FocalLength);
		data.camera_data.shutter_speed = reader.get_tag(Tag::ShutterSpeedValue);

		data.camera_data.flash = reader
			.get_tag(Tag::Flash)
			.map(|x: String| x.contains("fired") || x.contains("on"));

		data.camera_data.lens_make = reader.get_tag(Tag::LensMake);
		data.camera_data.lens_model = reader.get_tag(Tag::LensModel);
		data.camera_data.iso = reader.get_tag(Tag::PhotographicSensitivity);
		data.camera_data.zoom = reader
			.get_tag(Tag::DigitalZoomRatio)
			.map(|x: String| x.replace("unused", "1").parse().ok())
			.unwrap_or_default();

		data.camera_data.orientation =
			Orientation::int_to_orientation(reader.get_orientation_ints().unwrap_or_default());

		data.camera_data.software = reader.get_tag(Tag::Software);

		data.artist = reader.get_tag(Tag::Artist);
		data.copyright = reader.get_tag(Tag::Copyright);
		data.exif_version = reader.get_tag(Tag::ExifVersion);

		data.location = Location::from_exif_reader(&reader).ok();

		Ok(data)
	}

	/// This is only here as there's no easy impl from this foreign type to prisma's `CreateUnchecked`
	pub fn to_query(self) -> Result<sd_prisma::prisma::media_data::CreateUnchecked> {
		let kc = media_data::CreateUnchecked {
			dimensions: serde_json::to_vec(&self.dimensions)?,
			media_date: serde_json::to_vec(&self.date_taken)?,
			camera_data: serde_json::to_vec(&self.camera_data)?,
			_params: vec![
				media_data::location::set(serde_json::to_vec(&self.location).ok()),
				media_data::copyright::set(self.copyright),
				media_data::artist::set(self.artist),
			],
		};

		Ok(kc)
	}
}

// pub struct MediaDataVideo;

pub struct ExifReader(Exif);

impl ExifReader {
	pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
		let file = File::open(path)?;
		let mut reader = BufReader::new(file);
		Ok(Self(exif::Reader::new().read_from_container(&mut reader)?))
	}

	/// A helper function which gets the target `Tag` as `T`, provided `T` impls `FromStr`.
	///
	/// This function strips any erroneous newlines
	#[must_use]
	pub fn get_tag<T>(&self, tag: Tag) -> Option<T>
	where
		T: FromStr,
	{
		self.0
			.get_field(tag, In::PRIMARY)
			.map(|x| x.display_value().to_string().replace(['\\', '\"'], ""))
			.map(|x| x.parse::<T>().ok())?
	}

	pub(crate) fn get_orientation_ints(&self) -> Option<u32> {
		self.0
			.get_field(Tag::Orientation, In::PRIMARY)
			.map(|x| x.value.get_uint(0))
			.unwrap_or_default()
	}
}
