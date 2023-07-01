use std::{fs::File, io::BufReader, path::Path};

use chrono::{DateTime, FixedOffset};
use exif::{In, Tag};
use thiserror::Error;

#[derive(Default, Debug)]
pub struct MediaDataImage {
	pub timestamp: Option<DateTime<FixedOffset>>,
	pub width: Option<u32>,
	pub height: Option<u32>,
	pub color_space: Option<String>, // enum this probs
	// lat: f32, // not sure if 32 or 64
	// long: f32,
	// altitude: f32,
	pub device_make: Option<String>,
	pub device_model: Option<String>,
	pub focal_length: Option<f32>,
	pub shutter_speed: Option<f32>,
	pub flash: Option<bool>,
	pub copyright: Option<String>,
}

// pub struct MediaDataVideo {
// 	timestamp: DateTime<FixedOffset>,
// 	width: u32,
// 	height: u32,
// 	lat: f32, // not sure if 32 or 64
// 	long: f32,
// 	altitude: f32,
// 	fps: u32,
// 	device_make: String,
// 	device_model: String,
// 	device_software: String,
// 	duration: u32,
// 	video_codec: String, // enum thse
// 	audio_codec: String, // enum these
// 	stream_count: u32,   // we'll need to ues the ffmpeg crate for this one
// }

#[derive(Debug, Error)]
pub enum MediaDataError {
	#[error("there was an i/o error: {0}")]
	Io(#[from] std::io::Error),
	#[error("error from the exif crate: {0}")]
	Exif(#[from] exif::Error),
	#[error("a primary field doesn't exist")]
	NonExistant,
	#[error("there was an error while parsing the time")]
	TimeParse,
	#[error("there was an error while parsing time with chrono: {0}")]
	Chrono(#[from] chrono::ParseError),
	#[error("there was an error while converting between types")]
	Conversion, // #[error("tried to get a value that was out of bounds")]
	            // OutOfBoundsInt,
}

fn has_flash(s: &'static str) -> Result<bool> {
	match s {
		"fired, no return light detection function, forced" => Ok(true),
		"not fired, no return light detection function, suppressed" => Ok(false),
		_ => Err(MediaDataError::NonExistant),
	}
}

// const TAGS: [Tag; 10] = [
// 	Tag::PixelXDimension,
// 	Tag::PixelYDimension,
// 	Tag::Make,
// 	Tag::Model,
// 	Tag::DateTimeOriginal,
// 	Tag::OffsetTimeOriginal,
// 	Tag::ExposureTime,
// 	// Tag::ISOSpeed,
// 	// Tag::ApertureValue,
// 	// Tag::FlashEnergy,
// 	Tag::FocalLength,
// 	// Tag::FNumber,
// 	Tag::ColorSpace,
// 	Tag::ShutterSpeedValue,
// 	// Tag::WhiteBalance,
// 	// Tag::GPSLatitude, // Tag::GPS,
// ];

type Result<T> = std::result::Result<T, MediaDataError>;

pub fn get_data_for_image<P: AsRef<Path>>(path: P) -> Result<MediaDataImage> {
	let file = File::open(path).unwrap();
	let mut reader = BufReader::new(file);
	let exif_data = exif::Reader::new().read_from_container(&mut reader)?;

	let mut data = MediaDataImage::default();

	// dbg!(
	// 	"software: {}",
	// 	exif_data
	// 		.get_field(Tag::Flash, In::PRIMARY)
	// 		.unwrap()
	// 		.display_value()
	// 		.to_string(),
	// );

	let local_time = exif_data
		.get_field(Tag::DateTimeOriginal, In::PRIMARY)
		.ok_or(MediaDataError::NonExistant)?
		.display_value()
		.to_string();

	let offset = exif_data
		.get_field(Tag::OffsetTimeOriginal, In::PRIMARY)
		.ok_or(MediaDataError::NonExistant)?
		.display_value()
		.to_string()
		.as_str()[1..7]
		.to_string();

	data.timestamp = Some(DateTime::parse_from_str(
		&format!("{} {}", local_time, offset),
		"%Y-%m-%d %H:%M:%S %z",
	)?);

	data.width = Some(
		exif_data
			.get_field(Tag::PixelXDimension, In::PRIMARY)
			.ok_or(MediaDataError::NonExistant)?
			.value
			.display_as(Tag::PixelXDimension)
			.to_string()
			.parse::<u32>()
			.map_err(|_| MediaDataError::Conversion)?,
	);

	data.height = Some(
		exif_data
			.get_field(Tag::PixelYDimension, In::PRIMARY)
			.ok_or(MediaDataError::NonExistant)?
			.value
			.display_as(Tag::PixelYDimension)
			.to_string()
			.parse::<u32>()
			.map_err(|_| MediaDataError::Conversion)?,
	);

	data.color_space = Some(
		exif_data
			.get_field(Tag::ColorSpace, In::PRIMARY)
			.ok_or(MediaDataError::NonExistant)?
			.value
			.display_as(Tag::ColorSpace)
			.to_string(),
	);

	data.device_make = Some(
		exif_data
			.get_field(Tag::Make, In::PRIMARY)
			.ok_or(MediaDataError::NonExistant)?
			.value
			.display_as(Tag::Make)
			.to_string()
			.trim_matches('\\')
			.trim_matches('\"')
			.to_string(),
	);

	data.device_model = Some(
		exif_data
			.get_field(Tag::Model, In::PRIMARY)
			.ok_or(MediaDataError::NonExistant)?
			.value
			.display_as(Tag::Model)
			.to_string()
			.trim_matches('\\')
			.trim_matches('\"')
			.to_string(),
	);

	data.focal_length = exif_data
		.get_field(Tag::FocalLength, In::PRIMARY)
		.ok_or(MediaDataError::NonExistant)?
		.value
		.display_as(Tag::FocalLength)
		.to_string()
		.parse()
		.ok();

	data.shutter_speed = exif_data
		.get_field(Tag::ShutterSpeedValue, In::PRIMARY)
		.ok_or(MediaDataError::NonExistant)?
		.value
		.display_as(Tag::ShutterSpeedValue)
		.to_string()
		.parse()
		.ok();

	Ok(data)
}

#[cfg(test)]
mod tests {
	use super::get_data_for_image;

	#[test]
	fn t() {
		dbg!(get_data_for_image("./SA704136.JPG")).unwrap();
		// dbg!(get_data_for_image("./img.jpg")).unwrap();
		// dbg!(get_data_for_image("./img3.jpg")).unwrap();
	}
}
