//! Media metadata extraction utilities

use crate::infra::db::entities::{audio_media_data, image_media_data, video_media_data};
use chrono::{DateTime, Utc};
use sea_orm::ActiveValue::Set;
use std::path::Path;
use uuid::Uuid;

/// Extract image metadata from EXIF
pub async fn extract_image_metadata(
	path: &Path,
	uuid: Uuid,
) -> Result<image_media_data::ActiveModel, Box<dyn std::error::Error + Send + Sync>> {
	// Extract EXIF metadata
	let exif = sd_media_metadata::exif::ExifMetadata::from_path(path)
		.await?
		.ok_or("No EXIF data found")?;

	// Convert MediaDate to DateTime<Utc>
	let date_taken = exif.date_taken.map(|d| match d {
		sd_media_metadata::exif::MediaDate::Utc(dt) => dt.with_timezone(&chrono::Utc),
		sd_media_metadata::exif::MediaDate::Naive(dt) => dt.and_utc(),
	});

	Ok(image_media_data::ActiveModel {
		id: sea_orm::ActiveValue::NotSet,
		uuid: Set(uuid),
		width: Set(exif.resolution.width as i32),
		height: Set(exif.resolution.height as i32),
		date_taken: Set(date_taken.map(Into::into)),
		latitude: Set(None),  // TODO: Extract from EXIF location
		longitude: Set(None), // TODO: Extract from EXIF location
		camera_make: Set(exif.camera_data.device_make),
		camera_model: Set(exif.camera_data.device_model),
		lens_model: Set(exif.camera_data.lens_model),
		focal_length: Set(exif.camera_data.focal_length.map(|f| f.to_string())),
		aperture: Set(None), // TODO: Extract from EXIF
		shutter_speed: Set(exif.camera_data.shutter_speed.map(|s| s.to_string())),
		iso: Set(None), // TODO: Extract from EXIF
		orientation: Set(Some(exif.camera_data.orientation as i16)),
		color_space: Set(exif.camera_data.color_space),
		color_profile: Set(exif.camera_data.color_profile.map(|p| format!("{:?}", p))),
		bit_depth: Set(exif.camera_data.bit_depth.map(|b| b.to_string())),
		artist: Set(exif.artist),
		copyright: Set(exif.copyright),
		description: Set(exif.description),
		created_at: Set(chrono::Utc::now().into()),
		updated_at: Set(chrono::Utc::now().into()),
	})
}

/// Extract video metadata from FFmpeg
#[cfg(feature = "ffmpeg")]
pub async fn extract_video_metadata(
	path: &Path,
	uuid: Uuid,
) -> Result<video_media_data::ActiveModel, Box<dyn std::error::Error + Send + Sync>> {
	// Probe with FFmpeg
	let metadata = sd_ffmpeg::probe(path).await?;

	// Get first video stream for dimensions
	let video_stream = metadata
		.programs
		.iter()
		.flat_map(|p| &p.streams)
		.find(|s| s.codec.as_ref().map(|c| c.kind.as_deref()) == Some(Some("video")));

	use sd_ffmpeg::model::FFmpegProps;

	let (width, height) = video_stream
		.and_then(|s| s.codec.as_ref())
		.and_then(|c| match &c.props {
			Some(FFmpegProps::Video(v)) => Some((v.width, v.height)),
			_ => None,
		})
		.unwrap_or((0, 0));

	// Get first audio stream
	let audio_stream = metadata
		.programs
		.iter()
		.flat_map(|p| &p.streams)
		.find(|s| s.codec.as_ref().map(|c| c.kind.as_deref()) == Some(Some("audio")));

	let (audio_codec, audio_channels, audio_sample_rate, audio_bit_rate) = audio_stream
		.and_then(|s| s.codec.as_ref())
		.and_then(|c| match &c.props {
			Some(FFmpegProps::Audio(a)) => Some((
				c.name.clone(),
				a.channel_layout.clone(),
				a.sample_rate,
				Some(c.bit_rate),
			)),
			_ => None,
		})
		.unwrap_or((None, None, None, None));

	// Extract Apple QuickTime capture date or fall back to standard creation time
	tracing::debug!(
		"Available custom metadata keys: {:?}",
		metadata.metadata.custom.keys().collect::<Vec<_>>()
	);

	let date_captured = metadata
		.metadata
		.custom
		.get("com.apple.quicktime.creationdate")
		.and_then(|date_str| {
			tracing::debug!("Found Apple QuickTime creation date: {}", date_str);
			// Try parsing as RFC3339
			if let Ok(dt) = DateTime::parse_from_rfc3339(date_str) {
				tracing::debug!("Successfully parsed Apple QuickTime date via RFC3339");
				Some(dt.with_timezone(&Utc))
			} else {
				tracing::debug!("RFC3339 parsing failed, trying custom format");
				// Try parsing with custom format for Apple's variant
				// Format: "2025-10-18T09:21:58-0700"
				if let Ok(dt) = DateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S%z") {
					tracing::debug!("Successfully parsed Apple QuickTime date via custom format");
					Some(dt.with_timezone(&Utc))
				} else {
					tracing::warn!("Failed to parse Apple QuickTime date: {}", date_str);
					None
				}
			}
		})
		.or_else(|| {
			tracing::debug!(
				"No Apple QuickTime date found, using creation_time: {:?}",
				metadata.metadata.creation_time
			);
			metadata.metadata.creation_time
		});

	// Get video codec info
	let (
		codec,
		pixel_format,
		color_space,
		color_range,
		color_primaries,
		color_transfer,
		fps_num,
		fps_den,
	) = video_stream
		.and_then(|s| s.codec.as_ref())
		.map(|c| {
			let (
				pixel_format,
				color_space,
				color_range,
				color_primaries,
				color_transfer,
				fps_num,
				fps_den,
			) = match &c.props {
				Some(FFmpegProps::Video(v)) => (
					v.pixel_format.clone(),
					v.color_space.clone(),
					v.color_range.clone(),
					v.color_primaries.clone(),
					v.color_transfer.clone(),
					Some(0), // TODO: Extract from stream
					Some(1),
				),
				_ => (None, None, None, None, None, None, None),
			};
			(
				c.name.clone(),
				pixel_format,
				color_space,
				color_range,
				color_primaries,
				color_transfer,
				fps_num,
				fps_den,
			)
		})
		.unwrap_or((None, None, None, None, None, None, None, None));

	Ok(video_media_data::ActiveModel {
		id: sea_orm::ActiveValue::NotSet,
		uuid: Set(uuid),
		width: Set(width),
		height: Set(height),
		duration_seconds: Set(metadata.duration.map(|d| d as f64 / 1_000_000.0)),
		bit_rate: Set(Some(metadata.bit_rate)),
		codec: Set(codec),
		pixel_format: Set(pixel_format),
		color_space: Set(color_space),
		color_range: Set(color_range),
		color_primaries: Set(color_primaries),
		color_transfer: Set(color_transfer),
		fps_num: Set(fps_num),
		fps_den: Set(fps_den),
		audio_codec: Set(audio_codec),
		audio_channels: Set(audio_channels),
		audio_sample_rate: Set(audio_sample_rate),
		audio_bit_rate: Set(audio_bit_rate),
		title: Set(metadata.metadata.title),
		artist: Set(metadata.metadata.artist),
		album: Set(metadata.metadata.album),
		creation_time: Set(metadata.metadata.creation_time),
		date_captured: Set(date_captured),
		created_at: Set(chrono::Utc::now().into()),
		updated_at: Set(chrono::Utc::now().into()),
	})
}

/// Extract audio metadata from FFmpeg
#[cfg(feature = "ffmpeg")]
pub async fn extract_audio_metadata(
	path: &Path,
	uuid: Uuid,
) -> Result<audio_media_data::ActiveModel, Box<dyn std::error::Error + Send + Sync>> {
	use sd_ffmpeg::model::FFmpegProps;

	// Probe with FFmpeg
	let metadata = sd_ffmpeg::probe(path).await?;

	// Get first audio stream
	let audio_stream = metadata
		.programs
		.iter()
		.flat_map(|p| &p.streams)
		.find(|s| s.codec.as_ref().map(|c| c.kind.as_deref()) == Some(Some("audio")));

	let (codec, sample_rate, channels) = audio_stream
		.and_then(|s| s.codec.as_ref())
		.map(|c| {
			let (sample_rate, channels) = match &c.props {
				Some(FFmpegProps::Audio(a)) => (a.sample_rate, a.channel_layout.clone()),
				_ => (None, None),
			};
			(c.name.clone(), sample_rate, channels)
		})
		.unwrap_or((None, None, None));

	Ok(audio_media_data::ActiveModel {
		id: sea_orm::ActiveValue::NotSet,
		uuid: Set(uuid),
		duration_seconds: Set(metadata.duration.map(|d| d as f64 / 1_000_000.0)),
		bit_rate: Set(Some(metadata.bit_rate)),
		sample_rate: Set(sample_rate),
		channels: Set(channels),
		codec: Set(codec),
		title: Set(metadata.metadata.title),
		artist: Set(metadata.metadata.artist),
		album: Set(metadata.metadata.album),
		album_artist: Set(metadata.metadata.album_artist),
		genre: Set(metadata.metadata.genre),
		year: Set(None), // TODO: Parse from date field
		track_number: Set(metadata.metadata.track.map(|t| t as i32)),
		disc_number: Set(metadata.metadata.disc.map(|d| d as i32)),
		composer: Set(metadata.metadata.composer),
		publisher: Set(metadata.metadata.publisher),
		copyright: Set(metadata.metadata.copyright),
		created_at: Set(chrono::Utc::now().into()),
		updated_at: Set(chrono::Utc::now().into()),
	})
}
