use std::{
	fs::File,
	io::{BufReader, Cursor},
	path::{Path, PathBuf},
	str::FromStr,
};

use sd_prisma::prisma::media_data;

use exif::{Exif, In, Tag};
use tokio::task::spawn_blocking;

use crate::{
	orientation::Orientation,
	utils::{from_slice_option_to_option, from_slice_option_to_res, to_slice_option},
	ColorProfile, Dimensions, Flash, MediaLocation, MediaTime, Result,
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
	pub async fn from_path<P: AsRef<Path> + Send>(path: P) -> Result<Self> {
		Self::from_reader(&ExifReader::from_path(path).await?)
	}

	pub fn from_slice(slice: &[u8]) -> Result<Self> {
		Self::from_reader(&ExifReader::from_slice(slice)?)
	}

	#[allow(clippy::field_reassign_with_default)]
	pub fn from_reader(reader: &ExifReader) -> Result<Self> {
		let mut data = Self::default();

		data.date_taken = MediaTime::from_reader(reader);
		data.dimensions = Dimensions::from_reader(reader);

		data.camera_data.device_make = reader.get_tag(Tag::LensModel);
		data.camera_data.device_model = reader.get_tag(Tag::LensModel);
		data.camera_data.focal_length = reader.get_tag(Tag::FocalLength);
		data.camera_data.shutter_speed = reader.get_tag(Tag::ShutterSpeedValue);
		data.camera_data.color_space = reader.get_tag(Tag::ColorSpace);

		// data.camera_data.flash = reader
		// 	.get_tag::<String>(Tag::Flash)
		// 	.map(|x: String| x.to_lowercase().contains("fired") || x.to_lowercase().contains("on"))
		// 	.unwrap_or_default();

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

		data.camera_data.orientation =
			Orientation::int_to_orientation(reader.get_orientation_int().unwrap_or_default());

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

/// An [`ExifReader`]. This can get exif tags from images via files or slices.
///
/// If it is constructed from a slice, a temporary file will be created in your system's temp dir ([`std::env::temp_dir()`]).
/// This will be removed once the [`ExifReader`] has been dropped.
pub struct ExifReader(Exif);

impl ExifReader {
	// https://github.com/rust-lang/rust-clippy/issues/11087
	#[allow(clippy::future_not_send)]
	pub async fn from_path(path: impl AsRef<Path>) -> Result<Self> {
		fn inner(path: PathBuf) -> Result<ExifReader> {
			let file = File::open(&path).map_err(|e| (e, path.clone().into_boxed_path()))?;
			let mut reader = BufReader::new(file);
			Ok(Self(
				exif::Reader::new()
					.read_from_container(&mut reader)
					.map_err(|e| (e, path.into_boxed_path()))?,
			))
		}
		let p = path.as_ref().to_owned();
		spawn_blocking(move || inner(p)).await?
	}

	pub fn from_slice(slice: &[u8]) -> Result<Self> {
		// This one can be sync as we already have the data in memory and no I/O is performed
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
		self.0
			.get_field(tag, In::PRIMARY)
			.map(|x| x.display_value().to_string().replace(['\\', '\"'], ""))
			.map(|x| x.parse::<T>().ok())?
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

	pub(crate) fn get_color_profile_ints(&self) -> Option<u32> {
		self.0
			.get_field(Tag::CustomRendered, In::PRIMARY)
			.map(|x| x.value.get_uint(0))
			.unwrap_or_default()
	}
}

#[cfg(test)]

mod tests {
	use crate::MediaDataImage;

	const FILE_SLICE: &[u8] = include_bytes!("../test-assets/test.heif");

	#[test]
	#[should_panic]
	fn test() {
		let media_data_image = MediaDataImage::from_slice(FILE_SLICE).unwrap();
		panic!("{media_data_image:?}");
	}
}
