use std::{fmt::Display, fs::File, io::BufReader, path::Path, str::FromStr};

use chrono::{DateTime, FixedOffset};
use exif::{Exif, In, Tag};
use sd_prisma::prisma::media_data;

use crate::{
	consts::{DECIMAL_SF, DMS_DIVISION, OFFSET_TAGS, TIME_TAGS},
	Error, Result,
};

#[derive(Default, Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MediaDataImage {
	pub date_taken: ImageTime,
	pub dimensions: Dimensions,
	pub location: Option<Location>, // this is the formatted lat/long string to be used by the frontend
	pub camera_data: CameraData,
	pub artist: Option<String>,
	pub copyright: Option<String>,
	pub exif_version: Option<String>,
}

#[derive(Default, Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CameraData {
	pub device_make: Option<String>,
	pub device_model: Option<String>,
	pub focal_length: Option<f64>,
	pub shutter_speed: Option<f64>,
	pub flash: Option<bool>,
	pub orientation: Option<String>,
	pub lens_make: Option<String>,
	pub lens_model: Option<String>,
	pub zoom: Option<f64>,
	pub iso: Option<i32>,
	pub software: Option<String>,
}

#[derive(Default, Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Dimensions {
	width: i32,
	height: i32,
}

impl Dimensions {
	#[must_use]
	pub const fn new(width: i32, height: i32) -> Self {
		Self { width, height }
	}

	#[must_use]
	pub fn from_reader(reader: &ExifReader) -> Self {
		Self {
			width: reader.get_tag(Tag::PixelXDimension).unwrap_or_default(),
			height: reader.get_tag(Tag::PixelYDimension).unwrap_or_default(),
		}
	}

	#[must_use]
	pub const fn get_width(&self) -> i32 {
		self.width
	}

	#[must_use]
	pub const fn get_height(&self) -> i32 {
		self.height
	}
}

impl Display for Dimensions {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_fmt(format_args!("{}x{}", self.width, self.height))
	}
}

#[derive(Default, Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// This can be either local with no TZ (e.g. `YYYY-MM-DD HH-MM-SS`) or UTC with a fixed offset.
///
/// This may also be `undefined`.
pub enum ImageTime {
	Local(String),
	Utc(DateTime<FixedOffset>),
	#[default]
	Undefined,
}

impl ImageTime {
	pub fn from_reader(reader: &ExifReader) -> Self {
		let z = TIME_TAGS
			.into_iter()
			.zip(OFFSET_TAGS.into_iter())
			.filter_map(|(time_tag, offset_tag)| {
				let time = reader.get_tag::<String>(time_tag);
				let offset = reader.get_tag::<String>(offset_tag);

				if let (Some(t), Some(o)) = (time.clone(), offset) {
					DateTime::parse_and_remainder(&format!("{t} {o}"), "%F %X %#z")
						.ok()
						.map(|x| Self::Utc(x.0))
				} else {
					time.map(Self::Local)
				}
			})
			.collect::<Vec<_>>();

		z.iter()
			.find(|x| match x {
				Self::Utc(_) | Self::Local(_) => true,
				Self::Undefined => false,
			})
			.map_or(Self::Undefined, Clone::clone)
	}
}

impl ToString for ImageTime {
	fn to_string(&self) -> String {
		match self {
			Self::Local(t) => t.clone(),
			Self::Utc(t) => t.to_rfc3339(),
			Self::Undefined => String::from("Undefined"),
		}
	}
}

impl MediaDataImage {
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
		let reader = ExifReader::new(path)?;
		let mut data = Self::new();

		data.date_taken = ImageTime::from_reader(&reader);
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

		// TODO(brxken128): maybe use ints and enums to handle this, could be messy
		data.camera_data.orientation = reader.get_tag(Tag::Orientation);
		data.camera_data.software = reader.get_tag(Tag::Software);

		data.artist = reader.get_tag(Tag::Artist);
		data.copyright = reader.get_tag(Tag::Copyright);
		data.exif_version = reader.get_tag(Tag::ExifVersion);

		data.location = Location::from_exif_reader(&reader).ok();

		Ok(data)
	}

	pub fn to_query(self) -> Result<sd_prisma::prisma::media_data::CreateUnchecked> {
		let kc = media_data::CreateUnchecked {
			_params: vec![
				media_data::dimensions::set(Some(serde_json::to_vec(&self.dimensions)?)),
				media_data::camera_data::set(Some(serde_json::to_vec(&self.camera_data)?)),
				media_data::location::set(Some(serde_json::to_vec(&self.location)?)),
				media_data::image_date::set(Some(serde_json::to_vec(&self.date_taken)?)),
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
	pub fn get_tag<T>(&self, tag: Tag) -> Option<T>
	where
		T: FromStr,
	{
		self.0
			.get_field(tag, In::PRIMARY)
			.map(|x| x.display_value().to_string().replace(['\\', '\"'], ""))
			.map(|x| x.parse::<T>().ok())?
	}
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
	use std::{ffi::OsStr, time::Instant};

	use walkdir::{DirEntry, WalkDir};

	use crate::MediaDataImage;

	const EXTENSIONS: [&str; 4] = ["jpg", "jpeg", "heif", "tiff"];

	#[test]
	fn main_test() {
		let start = Instant::now();

		let all_paths = WalkDir::new("/Users/broken/exif")
			.into_iter()
			.collect::<Vec<_>>()
			.into_iter()
			.flatten();

		let filtered_image_files = all_paths
			.into_iter()
			.filter(|file| {
				EXTENSIONS
					.into_iter()
					.any(|x| file.path().extension().unwrap_or_default() == OsStr::new(x))
			})
			.map(DirEntry::into_path);

		let exif_data = filtered_image_files
			.into_iter()
			.filter_map(|x| MediaDataImage::from_path(x).ok())
			.collect::<Vec<_>>();

		println!("{}", serde_json::to_string_pretty(&exif_data).unwrap());

		println!(
			"{} files in {:.4}s",
			exif_data.len(),
			start.elapsed().as_secs_f32()
		);
	}
}

#[derive(Default, Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Location {
	latitude: f64,
	longitude: f64,
	altitude: Option<i32>,
	direction: Option<i32>, // the direction that the image was taken in, as a bearing (should always be <= 0 && <= 360)
}

impl Location {
	/// Create a new [`Location`] from a latitude and longitude pair.
	///
	/// Both of the provided values will be rounded to 8 digits after the decimal point ([`DECIMAL_SF`]),
	///
	/// # Examples
	///
	/// ```
	/// use sd_media_data::Location;
	///
	/// let home = Location::new(38.89767633, -77.03656035, Some(32), Some(20));
	/// ```
	#[must_use]
	pub fn new(lat: f64, long: f64, altitude: Option<i32>, direction: Option<i32>) -> Self {
		let latitude = (lat.clamp(-90.0, 90.0) * DECIMAL_SF).round() / DECIMAL_SF;
		let longitude = (long.clamp(-180.0, 180.0) * DECIMAL_SF).round() / DECIMAL_SF;

		Self {
			latitude,
			longitude,
			altitude,
			direction,
		}
	}

	/// Create a new [`Location`] from a latitude and longitude pair of EXIF-style strings.
	///
	/// Both of the provided values will be rounded to 8 digits after the decimal point ([`DECIMAL_SF`]),
	///
	/// # Examples
	///
	/// ```
	/// use sd_media_data::Location;
	///
	/// let home = Location::from_exif_strings("1 deg 5 min 10.34 sec", "23 deg 39 min 14.97").unwrap();
	/// ```
	pub fn from_exif_strings(lat: &str, long: &str) -> Result<Self> {
		let res = [lat, long]
			.into_iter()
			.map(ToString::to_string)
			.filter_map(|mut item| {
				item.retain(|x| x.is_numeric() || x.is_whitespace() || x == '.' || x == '/');
				let i = item
					.split_whitespace()
					.filter_map(|x| x.parse::<f64>().ok());
				(i.clone().count() == 3)
					.then(|| i.zip(DMS_DIVISION.iter()).map(|(x, y)| x / y).sum::<f64>())
			})
			.collect::<Vec<_>>();

		(!res.is_empty() && res.len() == 2)
			.then(|| Self::new(res[0], res[1], None, None))
			.ok_or(Error::LocationParse)
	}

	/// Create a new [`Location`] from a latitude and longitude pair of EXIF-style strings.
	///
	/// Both of the provided values will be rounded to 8 digits after the decimal point ([`DECIMAL_SF`]),
	///
	/// # Examples
	///
	/// ```ignore
	/// use sd_media_data::Location;
	/// let reader = ExifReader::new(path)?;
	/// let home = Location::from_exif_reader(&mut reader);
	/// ```
	pub fn from_exif_reader(reader: &ExifReader) -> Result<Self> {
		let res = [
			reader.get_tag(Tag::GPSLatitude),
			reader.get_tag(Tag::GPSLongitude),
		]
		.into_iter()
		.filter_map(|item: Option<String>| {
			let mut item = item.unwrap_or_default();
			item.retain(|x| x.is_numeric() || x.is_whitespace() || x == '.' || x == '/');
			let i = item
				.split_whitespace()
				.filter_map(|x| x.parse::<f64>().ok());
			(i.clone().count() == 3)
				.then(|| i.zip(DMS_DIVISION.iter()).map(|(x, y)| x / y).sum::<f64>())
		})
		.collect::<Vec<_>>();

		(!res.is_empty() && res.len() == 2)
			.then(|| {
				Self::new(
					res[0],
					res[1],
					reader.get_tag(Tag::GPSAltitude),
					reader
						.get_tag(Tag::GPSImgDirection)
						.map(|x: i32| x.clamp(0, 360)),
				)
			})
			.ok_or(Error::LocationParse)
	}

	/// # Examples
	///
	/// ```
	/// use sd_media_data::Location;
	///
	/// let mut home = Location::from_exif_strings("1 deg 5 min 10.34 sec", "23 deg 39 min 14.97").unwrap();
	/// home.update_latitude(-60.0);
	/// ```
	pub fn update_latitude(&mut self, lat: f64) {
		self.latitude = (lat.clamp(-90.0, 90.0) * DECIMAL_SF).round() / DECIMAL_SF;
	}

	/// # Examples
	///
	/// ```
	/// use sd_media_data::Location;
	///
	/// let mut home = Location::from_exif_strings("1 deg 5 min 10.34 sec", "23 deg 39 min 14.97").unwrap();
	/// home.update_longitude(20.0);
	/// ```
	pub fn update_longitude(&mut self, long: f64) {
		self.longitude = (long.clamp(-180.0, 180.0) * DECIMAL_SF).round() / DECIMAL_SF;
	}

	/// # Examples
	///
	/// ```
	/// use sd_media_data::Location;
	///
	/// let mut home = Location::from_exif_strings("1 deg 5 min 10.34 sec", "23 deg 39 min 14.97").unwrap();
	/// home.update_altitude(20);
	/// ```
	pub fn update_altitude(&mut self, altitude: i32) {
		self.altitude = Some(altitude);
	}

	/// # Examples
	///
	/// ```
	/// use sd_media_data::Location;
	///
	/// let mut home = Location::from_exif_strings("1 deg 5 min 10.34 sec", "23 deg 39 min 14.97").unwrap();
	/// home.update_direction(233);
	/// ```
	pub fn update_direction(&mut self, bearing: i32) {
		self.direction = Some(bearing.clamp(0, 360));
	}
}

impl TryFrom<String> for Location {
	type Error = Error;

	fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
		let iter = value
			.split_terminator(", ")
			.filter_map(|x| x.parse::<f64>().ok());
		if iter.clone().count() == 2 {
			let items = iter.collect::<Vec<_>>();
			Ok(Self::new(items[0], items[1], None, None))
		} else {
			Err(Error::Conversion)
		}
	}
}

impl Display for Location {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_fmt(format_args!("{}, {}", self.latitude, self.longitude))
	}
}
