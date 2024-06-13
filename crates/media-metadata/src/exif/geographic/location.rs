use crate::{
	exif::{
		consts::{
			ALT_MAX_HEIGHT, ALT_MIN_HEIGHT, DECIMAL_SF, DIRECTION_MAX, DMS_DIVISION, LAT_MAX_POS,
			LONG_MAX_POS,
		},
		ExifReader, PlusCode,
	},
	Error, Result,
};
use exif::Tag;
use std::ops::Neg;

#[derive(Default, Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct MediaLocation {
	latitude: f64,
	longitude: f64,
	pluscode: PlusCode,
	altitude: Option<i32>,
	direction: Option<i32>, // the direction that the image was taken in, as a bearing (should always be <= 0 && <= 360)
}

impl MediaLocation {
	/// Create a new [`MediaLocation`] from a latitude and longitude pair.
	///
	/// Both of the provided values will be rounded to 8 digits after the decimal point ([`DECIMAL_SF`]),
	///
	/// # Examples
	///
	/// ```
	/// use sd_media_metadata::image::MediaLocation;
	///
	/// let x = MediaLocation::new(38.89767633, -7.36560353, Some(32), Some(20));
	/// ```
	#[must_use]
	pub fn new(lat: f64, long: f64, altitude: Option<i32>, direction: Option<i32>) -> Self {
		let latitude = Self::format_coordinate(lat, LAT_MAX_POS);
		let longitude = Self::format_coordinate(long, LONG_MAX_POS);
		let altitude = altitude.map(Self::format_altitude);
		let direction = direction.map(Self::format_direction);
		let pluscode = PlusCode::new(latitude, longitude);

		Self {
			latitude,
			longitude,
			pluscode,
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
	/// use sd_media_metadata::image::{ExifReader, Location};
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
			item.retain(|x| x.is_numeric() || x.is_whitespace() || x == '.');
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
					reader.get_tag(Tag::GPSAltitude).map(Self::format_altitude),
					reader
						.get_tag(Tag::GPSImgDirection)
						.map(Self::format_direction),
				)
			})
			.ok_or(Error::MediaLocationParse)
	}

	/// This returns the contained coordinates as `(latitude, longitude)`
	///
	/// # Examples
	///
	/// ```
	/// use sd_media_metadata::image::MediaLocation;
	///
	/// let mut home = MediaLocation::new(38.89767633, -7.36560353, Some(32), Some(20));
	/// assert_eq!(home.coordinates(), (38.89767633, -7.36560353));
	/// ```
	#[inline]
	#[must_use]
	pub const fn coordinates(&self) -> (f64, f64) {
		(self.latitude, self.longitude)
	}

	/// This returns the contained Plus Code/Open Location Code
	///
	/// # Examples
	///
	/// ```
	/// use sd_media_metadata::image::MediaLocation;
	///
	/// let mut home = MediaLocation::new(38.89767633, -7.36560353, Some(32), Some(20));
	/// assert_eq!(home.pluscode().to_string(), "894HFGG5+82".to_string());
	/// ```
	#[inline]
	#[must_use]
	pub fn pluscode(&self) -> PlusCode {
		self.pluscode.clone()
	}

	/// This also re-generates the Plus Code for your coordinates
	///
	/// # Examples
	///
	/// ```
	/// use sd_media_metadata::image::MediaLocation;
	///
	/// let mut home = MediaLocation::new(38.89767633, -7.36560353, Some(32), Some(20));
	/// home.update_latitude(60_f64);
	/// ```
	#[inline]
	pub fn update_latitude(&mut self, lat: f64) {
		self.latitude = Self::format_coordinate(lat, LAT_MAX_POS);
		self.pluscode = PlusCode::new(self.latitude, self.longitude);
	}

	/// This also re-generates the Plus Code for your coordinates
	///
	/// # Examples
	///
	/// ```
	/// use sd_media_metadata::image::MediaLocation;
	///
	/// let mut home = MediaLocation::new(38.89767633, -7.36560353, Some(32), Some(20));
	/// home.update_longitude(20_f64);
	/// ```
	#[inline]
	pub fn update_longitude(&mut self, long: f64) {
		self.longitude = Self::format_coordinate(long, LONG_MAX_POS);
		self.pluscode = PlusCode::new(self.latitude, self.longitude);
	}

	/// # Examples
	///
	/// ```
	/// use sd_media_metadata::image::MediaLocation;
	///
	/// let mut home = MediaLocation::new(38.89767633, -7.36560353, Some(32), Some(20));
	/// home.update_altitude(20);
	/// ```
	#[inline]
	pub fn update_altitude(&mut self, altitude: i32) {
		self.altitude = Some(Self::format_altitude(altitude));
	}

	/// # Examples
	///
	/// ```
	/// use sd_media_metadata::image::MediaLocation;
	///
	/// let mut home = MediaLocation::new(38.89767633, -7.36560353, Some(32), Some(20));
	/// home.update_direction(233);
	/// ```
	#[inline]
	pub fn update_direction(&mut self, direction: i32) {
		self.direction = Some(Self::format_direction(direction));
	}

	/// This is used to clamp and format coordinates. They are rounded to 8 significant figures after the decimal point.
	///
	/// `max` must positive, and it should be the maximum distance allowed (e.g. 180 degrees)
	#[inline]
	#[must_use]
	fn format_coordinate(v: f64, max: f64) -> f64 {
		(v.clamp(max.neg(), max) * DECIMAL_SF).round() / DECIMAL_SF
	}

	/// This is used to clamp altitudes to appropriate values.
	#[inline]
	#[must_use]
	fn format_altitude(v: i32) -> i32 {
		v.clamp(ALT_MIN_HEIGHT, ALT_MAX_HEIGHT)
	}

	/// This is used to ensure an image direction/bearing is a valid bearing (anywhere from 0-360 degrees).
	#[inline]
	#[must_use]
	fn format_direction(v: i32) -> i32 {
		v.clamp(0, DIRECTION_MAX)
	}
}

impl TryFrom<String> for MediaLocation {
	type Error = Error;

	/// This tries to parse a standard "34.2493458, -23.4923843" string to a [`MediaLocation`]
	///
	/// # Examples:
	///
	/// ```
	/// use sd_media_metadata::image::MediaLocation;
	///
	/// let s = String::from("32.47583923, -28.49238495");
	/// let location = MediaLocation::try_from(s).unwrap();
	/// assert_eq!(location.to_string(), "32.47583923, -28.49238495".to_string());
	/// ```
	fn try_from(mut value: String) -> std::result::Result<Self, Self::Error> {
		value.retain(|c| !c.is_whitespace() || c.is_numeric() || c == '-' || c == '.');
		let iter = value.split(',').filter_map(|x| x.parse::<f64>().ok());
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
