//! Media data domain types for image, video, and audio metadata

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::infra::db::entities::*;

/// Image metadata extracted from EXIF
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ImageMediaData {
	pub uuid: Uuid,
	pub width: Option<u32>,
	pub height: Option<u32>,
	pub blurhash: Option<String>,
	pub date_taken: Option<DateTime<Utc>>,
	pub latitude: Option<f64>,
	pub longitude: Option<f64>,
	pub camera_make: Option<String>,
	pub camera_model: Option<String>,
	pub lens_model: Option<String>,
	pub focal_length: Option<String>,
	pub aperture: Option<String>,
	pub shutter_speed: Option<String>,
	pub iso: Option<u32>,
	pub orientation: Option<u8>,
	pub color_space: Option<String>,
	pub color_profile: Option<String>,
	pub bit_depth: Option<String>,
	pub artist: Option<String>,
	pub copyright: Option<String>,
	pub description: Option<String>,
}

impl ImageMediaData {
	/// What a record's image facet says, under the record's uuid. `None`
	/// for the empty row the EXIF pass writes when the bytes carried no
	/// EXIF, so a file without metadata shows none rather than a blank card.
	pub fn from_facet(record: Uuid, facet: &sd_store::ImageFacet) -> Option<Self> {
		if *facet == sd_store::ImageFacet::default() {
			return None;
		}
		let as_u32 = |value: Option<i64>| value.and_then(|v| u32::try_from(v).ok());
		Some(Self {
			uuid: record,
			width: as_u32(facet.width),
			height: as_u32(facet.height),
			blurhash: None,
			date_taken: facet
				.date_taken
				.as_deref()
				.and_then(|date| DateTime::parse_from_rfc3339(date).ok())
				.map(|date| date.with_timezone(&Utc)),
			latitude: facet.latitude,
			longitude: facet.longitude,
			camera_make: facet.camera_make.clone(),
			camera_model: facet.camera_model.clone(),
			lens_model: facet.lens_model.clone(),
			focal_length: facet.focal_length.clone(),
			aperture: facet.aperture.clone(),
			shutter_speed: facet.shutter_speed.clone(),
			iso: as_u32(facet.iso),
			orientation: facet.orientation.and_then(|o| u8::try_from(o).ok()),
			color_space: facet.color_space.clone(),
			color_profile: facet.color_profile.clone(),
			bit_depth: facet.bit_depth.clone(),
			artist: facet.artist.clone(),
			copyright: facet.copyright.clone(),
			description: facet.description.clone(),
		})
	}
}

/// Video metadata extracted from FFmpeg
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VideoMediaData {
	pub uuid: Uuid,
	pub width: u32,
	pub height: u32,
	pub blurhash: Option<String>,
	pub duration_seconds: Option<f64>,
	pub bit_rate: Option<i64>,
	pub codec: Option<String>,
	pub pixel_format: Option<String>,
	pub color_space: Option<String>,
	pub color_range: Option<String>,
	pub color_primaries: Option<String>,
	pub color_transfer: Option<String>,
	pub fps_num: Option<i32>,
	pub fps_den: Option<i32>,
	pub audio_codec: Option<String>,
	pub audio_channels: Option<String>,
	pub audio_sample_rate: Option<i32>,
	pub audio_bit_rate: Option<i32>,
	pub title: Option<String>,
	pub artist: Option<String>,
	pub album: Option<String>,
	pub creation_time: Option<DateTime<Utc>>,
	pub date_captured: Option<DateTime<Utc>>,
}

/// Audio metadata extracted from FFmpeg
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AudioMediaData {
	pub uuid: Uuid,
	pub duration_seconds: Option<f64>,
	pub bit_rate: Option<i64>,
	pub sample_rate: Option<i32>,
	pub channels: Option<String>,
	pub codec: Option<String>,
	pub title: Option<String>,
	pub artist: Option<String>,
	pub album: Option<String>,
	pub album_artist: Option<String>,
	pub genre: Option<String>,
	pub year: Option<u32>,
	pub track_number: Option<u32>,
	pub disc_number: Option<u32>,
	pub composer: Option<String>,
	pub publisher: Option<String>,
	pub copyright: Option<String>,
}

// Convert from database entity to domain model

impl VideoMediaData {
	/// Calculate framerate as float from numerator/denominator
	pub fn framerate(&self) -> Option<f32> {
		match (self.fps_num, self.fps_den) {
			(Some(num), Some(den)) if den != 0 => Some(num as f32 / den as f32),
			_ => None,
		}
	}
}
