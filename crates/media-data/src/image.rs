use std::{fs::File, io::BufReader, path::Path};

use chrono::{DateTime, FixedOffset};
use exif::{In, Tag};
use thiserror::Error;

#[derive(Default, Debug)]
pub struct MediaDataImage {
	pub timestamp: Option<DateTime<FixedOffset>>,
	pub width: Option<i32>,
	pub height: Option<i32>,
	pub color_space: Option<String>, // enum this probsç
	// pub compression: Option<String>, // enum this probsç
	// lat: f64, // custom parsing
	// long: f64. // meed custom parsing
	// altitude: f64, // custom parsing
	pub device_make: Option<String>,
	pub device_model: Option<String>,
	pub focal_length: Option<f64>,
	pub shutter_speed: Option<f64>,
	pub flash: Option<bool>,
	pub orientation: Option<String>, // custom parsing but we should grab this anyway
	pub copyright: Option<String>,
	pub artist: Option<String>,
	// pub software: Option<String>, // commenting as this varies and is very iffy
}

// TODO(brxken128): will probably be modifieid a bit
// pub struct MediaDataVideo {
// 	timestamp: DateTime<FixedOffset>,
// 	width: i32,
// 	height: i32,
// 	lat: f64, // not sure if 32 or 64
// 	long: f64,
// 	altitude: f64,
// 	fps: i32,
// 	device_make: String,
// 	device_model: String,
// 	device_software: String,
// 	duration: i32,
// 	video_codec: String, // enum thse
// 	audio_codec: String, // enum these
// 	stream_count: i32,   // we'll need to ues the ffmpeg crate for this one
// }

#[derive(Debug, Error)]
pub enum MediaDataError {
	#[error("there was an i/o error: {0}")]
	Io(#[from] std::io::Error),
	#[error("error from the exif crate: {0}")]
	Exif(#[from] exif::Error),
	#[error("there was an error while parsing the time")]
	TimeParse,
	#[error("there was an error while parsing time with chrono: {0}")]
	Chrono(#[from] chrono::ParseError),
	#[error("there was an error while converting between types")]
	Conversion,
}

const HAS_FLASH: &str = "fired, no return light detection function, forced";
// i don't think we need this at all. the `.map` will catch auto flash being enabled/disabled,
// and [`HAS_FLASH`] does the job
// const HAS_FLASH_DISABLED: &str = "not fired, no return light detection function, suppressed";

type Result<T> = std::result::Result<T, MediaDataError>;

pub fn get_data_for_image<P: AsRef<Path>>(path: P) -> Result<MediaDataImage> {
	let file = File::open(path)?;
	let mut reader = BufReader::new(file);
	let exif_data = exif::Reader::new().read_from_container(&mut reader)?;

	let mut data = MediaDataImage::default();

	let local_time = exif_data
		.get_field(Tag::DateTimeOriginal, In::PRIMARY)
		.map(|x| x.display_value().to_string())
		.ok_or(MediaDataError::Conversion);

	let offset = exif_data
		.get_field(Tag::OffsetTimeOriginal, In::PRIMARY)
		.map(|x| x.display_value().to_string().as_str()[1..7].to_string())
		.ok_or(MediaDataError::Conversion);

	if let Ok(local) = local_time {
		if let Ok(offset) = offset {
			data.timestamp = Some(DateTime::parse_from_str(
				&format!("{} {}", local, offset),
				"%Y-%m-%d %H:%M:%S %z",
			)?)
		}
	};

	data.width = exif_data
		.get_field(Tag::PixelXDimension, In::PRIMARY)
		.map(|x| {
			x.value
				.display_as(Tag::PixelXDimension)
				.to_string()
				.parse::<i32>()
				.ok()
		})
		.unwrap_or_default();

	data.height = exif_data
		.get_field(Tag::PixelYDimension, In::PRIMARY)
		.map(|x| {
			x.value
				.display_as(Tag::PixelYDimension)
				.to_string()
				.parse::<i32>()
				.ok()
		})
		.unwrap_or_default();

	data.color_space = exif_data
		.get_field(Tag::ColorSpace, In::PRIMARY)
		.map(|x| x.value.display_as(Tag::ColorSpace).to_string());

	// data.compression = exif_data
	// 	.get_field(Tag::Compression, In::PRIMARY)
	// 	.map(|x| x.value.display_as(Tag::Compression).to_string());

	data.device_make = exif_data.get_field(Tag::Make, In::PRIMARY).map(|x| {
		x.value
			.display_as(Tag::Make)
			.to_string()
			.replace(['\\', '\"'], "")
	});

	data.device_model = exif_data.get_field(Tag::Model, In::PRIMARY).map(|x| {
		x.value
			.display_as(Tag::Model)
			.to_string()
			.replace(['\\', '\"'], "")
	});

	data.focal_length = exif_data
		.get_field(Tag::FocalLength, In::PRIMARY)
		.map(|x| {
			x.value
				.display_as(Tag::FocalLength)
				.to_string()
				.parse::<f64>()
				.ok()
		})
		.unwrap_or_default();

	data.shutter_speed = exif_data
		.get_field(Tag::ShutterSpeedValue, In::PRIMARY)
		.map(|x| {
			x.value
				.display_as(Tag::ShutterSpeedValue)
				.to_string()
				.parse::<f64>()
				.ok()
		})
		.unwrap_or_default();

	data.flash = exif_data.get_field(Tag::Flash, In::PRIMARY).map(|x| {
		x.value
			.display_as(Tag::Flash)
			.to_string()
			.contains(HAS_FLASH)
	});

	data.orientation = exif_data
		.get_field(Tag::Orientation, In::PRIMARY)
		.map(|x| x.value.display_as(Tag::Orientation).to_string());

	data.copyright = exif_data
		.get_field(Tag::Copyright, In::PRIMARY)
		.map(|x| x.value.display_as(Tag::Copyright).to_string());

	data.artist = exif_data
		.get_field(Tag::Artist, In::PRIMARY)
		.map(|x| x.value.display_as(Tag::Artist).to_string());

	// temporarily disabled until i can get more test data
	// data.software = exif_data
	// 	.get_field(Tag::Software, In::PRIMARY)
	// 	.map(|x| x.value.display_as(Tag::Software).to_string());

	Ok(data)
}
