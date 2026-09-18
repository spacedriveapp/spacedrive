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
	pub width: u32,
	pub height: u32,
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
