use exif::{Exif, In, Tag};
use sd_prisma::prisma::media_data;
use std::{fs::File, io::BufReader, path::Path, str::FromStr};

use crate::{
	orientation::Orientation,
	utils::{from_slice_option_to_option, from_slice_option_to_res, to_slice_option},
	Dimensions, MediaLocation, MediaTime, Result,
};

#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct MediaDataImage {
	pub date_taken: MediaTime,
	pub dimensions: Dimensions,
	pub location: Option<MediaLocation>,
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
	pub flash: bool,
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
			.get_tag::<String>(Tag::Flash)
			.map(|x: String| x.to_lowercase().contains("fired") || x.to_lowercase().contains("on"))
			.unwrap_or_default();

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

		data.location = MediaLocation::from_exif_reader(&reader).ok();

		Ok(data)
	}

	/// This is only here as there's no easy impl from this foreign type to prisma's `CreateUnchecked`
	pub fn to_query(self) -> Result<sd_prisma::prisma::media_data::CreateUnchecked> {
		let kc = media_data::CreateUnchecked {
			_params: vec![
				media_data::dimensions::set(to_slice_option(&self.dimensions)),
				media_data::media_date::set(to_slice_option(&self.date_taken)),
				media_data::camera_data::set(to_slice_option(&self.camera_data)),
				media_data::location::set(to_slice_option(&self.location)),
				media_data::copyright::set(to_slice_option(&self.copyright)),
				media_data::artist::set(to_slice_option(&self.artist)),
				media_data::exif_version::set(to_slice_option(&self.exif_version)),
			],
		};

		Ok(kc)
	}

	pub fn from_prisma_data(data: sd_prisma::prisma::media_data::Data) -> Result<Self> {
		Ok(Self {
			dimensions: from_slice_option_to_res(data.dimensions)?,
			camera_data: from_slice_option_to_res(data.camera_data)?,
			date_taken: from_slice_option_to_res(data.media_date)?,
			copyright: from_slice_option_to_option(data.copyright),
			artist: from_slice_option_to_option(data.artist),
			location: from_slice_option_to_option(data.location),
			exif_version: from_slice_option_to_option(data.exif_version),
		})
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
