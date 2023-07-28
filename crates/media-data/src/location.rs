use std::fmt::Display;

use exif::Tag;

use crate::{
	consts::{DECIMAL_SF, DMS_DIVISION},
	Error, ExifReader, Result,
};

#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
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
	/// Location::new(38.89767633, -77.03656035, Some(32), Some(20));
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
	/// Location::from_exif_strings("1 deg 5 min 10.34 sec", "23 deg 39 min 14.97").unwrap();
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
			.then(|| {
				Self::new(
					res[0].clamp(-90.0, 90.0),
					res[1].clamp(-180.0, 180.0),
					None,
					None,
				)
			})
			.ok_or(Error::LocationParse)
	}

	/// Create a new [`Location`] from an [`ExifReader`] instance.
	///
	/// Both of the provided values will be rounded to 8 digits after the decimal point ([`DECIMAL_SF`]),
	///
	/// This does not take into account the poles, e.g. N/E/S/W, but should still produce valid results (Untested!)
	///
	/// # Examples
	///
	/// ```ignore
	/// use sd_media_data::{Location, ExifReader};
	/// let mut reader = ExifReader::new("path").unwrap();
	/// Location::from_exif_reader(&mut reader).unwrap();
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

	/// This tries to parse a standard "34.2493458, -23.4923843" string to a [`Location`]
	///
	/// # Examples:
	///
	/// ```
	/// use sd_media_data::Location;
	/// let s = String::from("32.47583923, -28.49238495");
	/// Location::try_from(s).unwrap();
	///
	/// ```
	fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
		let iter = value
			.split_terminator(", ")
			.filter_map(|x| x.parse::<f64>().ok());
		if iter.clone().count() == 2 {
			let items = iter.collect::<Vec<_>>();
			Ok(Self::new(
				items[0].clamp(-90.0, 90.0),
				items[1].clamp(-180.0, 180.0),
				None,
				None,
			))
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
