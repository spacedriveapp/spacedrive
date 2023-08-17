use super::{
	consts::{DECIMAL_SF, DMS_DIVISION},
	ExifReader,
};
use crate::{Error, Result};
use exif::Tag;
use std::{fmt::Display, ops::Neg};

#[derive(Default, Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct MediaLocation {
	latitude: f64,
	longitude: f64,
	altitude: Option<i32>,
	direction: Option<i32>, // the direction that the image was taken in, as a bearing (should always be <= 0 && <= 360)
}

const LAT_MAX_POS: f64 = 90_f64;
const LONG_MAX_POS: f64 = 180_f64;

impl MediaLocation {
	/// This is used to clamp and format coordinates. They are rounded to 8 significant figures after the decimal point.
	///
	/// `max` must be a positive `f64`, and it should be the maximum distance allowed (e.g. 90 or 180 degrees)
	#[must_use]
	fn format_coordinate(v: f64, max: f64) -> f64 {
		(v.clamp(max.neg(), max) * DECIMAL_SF).round() / DECIMAL_SF
	}

	/// Create a new [`MediaLocation`] from a latitude and longitude pair.
	///
	/// Both of the provided values will be rounded to 8 digits after the decimal point ([`DECIMAL_SF`]),
	///
	/// # Examples
	///
	/// ```
	/// use sd_media_data::MediaLocation;
	///
	/// let x = MediaLocation::new(38.89767633, -7.36560353, Some(32), Some(20));
	/// ```
	#[must_use]
	pub fn new(lat: f64, long: f64, altitude: Option<i32>, direction: Option<i32>) -> Self {
		let latitude = Self::format_coordinate(lat, LAT_MAX_POS);
		let longitude = Self::format_coordinate(long, LONG_MAX_POS);

		Self {
			latitude,
			longitude,
			altitude,
			direction,
		}
	}

	/// Create a new [`MediaLocation`] from an [`ExifReader`] instance.
	///
	/// Both of the provided values will be rounded to 8 digits after the decimal point ([`DECIMAL_SF`]),
	///
	/// This does not take into account the poles, e.g. N/E/S/W, but should still produce valid results (Untested!)
	///
	/// # Examples
	///
	/// ```ignore
	/// use sd_media_data::{MediaLocation, ExifReader};
	///
	/// let mut reader = ExifReader::from_path("path").unwrap();
	/// MediaLocation::from_exif_reader(&mut reader).unwrap();
	/// ```
	pub fn from_exif_reader(reader: &ExifReader) -> Result<Self> {
		let res = [
			(
				reader.get_tag(Tag::GPSLatitude),
				reader.get_tag(Tag::GPSLatitudeRef),
			),
			(
				reader.get_tag(Tag::GPSLongitude),
				reader.get_tag(Tag::GPSLongitudeRef),
			),
		]
		.into_iter()
		.filter_map(|(item, reference)| {
			let mut item: String = item.unwrap_or_default();
			let reference: String = reference.unwrap_or_default();
			item.retain(|x| {
				x.is_numeric() || x.is_whitespace() || x == '.' || x == '/' || x == '-'
			});
			let i = item
				.split_whitespace()
				.filter_map(|x| x.parse::<f64>().ok());
			(i.clone().count() == 3)
				.then(|| i.zip(DMS_DIVISION.iter()).map(|(x, y)| x / y).sum::<f64>())
				.map(|mut x| {
					(reference == "W" || reference == "S" || reference == "3" || reference == "1")
						.then(|| x = x.neg());
					x
				})
		})
		.collect::<Vec<_>>();

		(!res.is_empty() && res.len() == 2)
			.then(|| {
				Self::new(
					Self::format_coordinate(res[0], LAT_MAX_POS),
					Self::format_coordinate(res[1], LONG_MAX_POS),
					reader.get_tag(Tag::GPSAltitude),
					reader
						.get_tag(Tag::GPSImgDirection)
						.map(|x: i32| x.clamp(0, 360)),
				)
			})
			.ok_or(Error::MediaLocationParse)
	}

	/// # Examples
	///
	/// ```
	/// use sd_media_data::MediaLocation;
	///
	/// let mut home = MediaLocation::new(38.89767633, -7.36560353, Some(32), Some(20));
	/// home.update_latitude(60_f64);
	/// ```
	pub fn update_latitude(&mut self, lat: f64) {
		self.latitude = Self::format_coordinate(lat, LAT_MAX_POS);
	}

	/// # Examples
	///
	/// ```
	/// use sd_media_data::MediaLocation;
	///
	/// let mut home = MediaLocation::new(38.89767633, -7.36560353, Some(32), Some(20));
	/// home.update_longitude(20_f64);
	/// ```
	pub fn update_longitude(&mut self, long: f64) {
		self.longitude = Self::format_coordinate(long, LONG_MAX_POS);
	}

	/// # Examples
	///
	/// ```
	/// use sd_media_data::MediaLocation;
	///
	/// let mut home = MediaLocation::new(38.89767633, -7.36560353, Some(32), Some(20));
	/// home.update_altitude(20);
	/// ```
	pub fn update_altitude(&mut self, altitude: i32) {
		self.altitude = Some(altitude);
	}

	/// # Examples
	///
	/// ```
	/// use sd_media_data::MediaLocation;
	///
	/// let mut home = MediaLocation::new(38.89767633, -7.36560353, Some(32), Some(20));
	/// home.update_direction(233);
	/// ```
	pub fn update_direction(&mut self, bearing: i32) {
		self.direction = Some(bearing.clamp(0, 360));
	}
}

impl TryFrom<String> for MediaLocation {
	type Error = Error;

	/// This tries to parse a standard "34.2493458, -23.4923843" string to a [`MediaLocation`]
	///
	/// # Examples:
	///
	/// ```
	/// use sd_media_data::MediaLocation;
	///
	/// let s = String::from("32.47583923, -28.49238495");
	/// MediaLocation::try_from(s).unwrap();
	///
	/// ```
	fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
		let iter = value
			.split_terminator(", ")
			.filter_map(|x| x.parse::<f64>().ok());
		if iter.clone().count() == 2 {
			let items = iter.collect::<Vec<_>>();
			Ok(Self::new(
				Self::format_coordinate(items[0], LAT_MAX_POS),
				Self::format_coordinate(items[1], LONG_MAX_POS),
				None,
				None,
			))
		} else {
			Err(Error::Conversion)
		}
	}
}

impl Display for MediaLocation {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_fmt(format_args!("{}, {}", self.latitude, self.longitude))
	}
}
