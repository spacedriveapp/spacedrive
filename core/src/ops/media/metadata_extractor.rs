//! Media metadata extraction utilities

use crate::infra::db::entities::{audio_media_data, image_media_data, video_media_data};
use chrono::{DateTime, Utc};
use sea_orm::ActiveValue::Set;
use std::path::Path;
use uuid::Uuid;

/// Content kind ids as stored on `content_identity` rows.
pub const KIND_IMAGE: i32 = 1;
pub const KIND_VIDEO: i32 = 2;
pub const KIND_AUDIO: i32 = 3;

/// Whether the content's media-data row for this kind still needs work:
/// the row is absent, or (for image/video) present without a blurhash. Lets
/// callers skip the extraction and blurhash cost on already-complete files.
pub async fn media_metadata_missing(
	db: &sea_orm::DatabaseConnection,
	content_uuid: &Uuid,
	content_kind_id: i32,
) -> bool {
	use crate::infra::db::entities::content_identity;
	use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

	let Ok(Some(content)) = content_identity::Entity::find()
		.filter(content_identity::Column::Uuid.eq(*content_uuid))
		.one(db)
		.await
	else {
		return false;
	};

	match content_kind_id {
		KIND_IMAGE => match content.image_media_data_id {
			None => true,
			Some(id) => matches!(
				image_media_data::Entity::find_by_id(id).one(db).await,
				Ok(Some(row)) if row.blurhash.is_none()
			),
		},
		KIND_VIDEO => match content.video_media_data_id {
			None => true,
			Some(id) => matches!(
				video_media_data::Entity::find_by_id(id).one(db).await,
				Ok(Some(row)) if row.blurhash.is_none()
			),
		},
		KIND_AUDIO => content.audio_media_data_id.is_none(),
		_ => false,
	}
}

/// Extract and store the media-data row for a content identity, linking it
/// via the kind's FK. A row that already exists only receives the blurhash
/// when it lacks one; nothing is re-extracted.
pub async fn backfill_media_metadata(
	db: &sea_orm::DatabaseConnection,
	content_uuid: &Uuid,
	source_path: &Path,
	content_kind_id: i32,
	blurhash: Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
	use crate::infra::db::entities::content_identity;
	use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter};

	let Some(content) = content_identity::Entity::find()
		.filter(content_identity::Column::Uuid.eq(*content_uuid))
		.one(db)
		.await?
	else {
		return Ok(());
	};

	match content_kind_id {
		KIND_IMAGE => {
			if let Some(id) = content.image_media_data_id {
				if let (Some(hash), Ok(Some(row))) = (
					blurhash,
					image_media_data::Entity::find_by_id(id).one(db).await,
				) {
					if row.blurhash.is_none() {
						let mut active: image_media_data::ActiveModel = row.into();
						active.blurhash = Set(Some(hash));
						active.updated_at = Set(Utc::now().into());
						active.update(db).await?;
					}
				}
				return Ok(());
			}

			let uuid = Uuid::new_v5(content_uuid, b"image");
			let model = extract_image_metadata_with_blurhash(source_path, uuid, blurhash).await?;
			let inserted = model.insert(db).await?;
			let mut active: content_identity::ActiveModel = content.into();
			active.image_media_data_id = Set(Some(inserted.id));
			active.update(db).await?;
		}
		KIND_VIDEO => {
			if let Some(id) = content.video_media_data_id {
				if let (Some(hash), Ok(Some(row))) = (
					blurhash,
					video_media_data::Entity::find_by_id(id).one(db).await,
				) {
					if row.blurhash.is_none() {
						let mut active: video_media_data::ActiveModel = row.into();
						active.blurhash = Set(Some(hash));
						active.updated_at = Set(Utc::now().into());
						active.update(db).await?;
					}
				}
				return Ok(());
			}

			// Probing a fresh video row needs FFmpeg; without it the row waits
			// for a build that has the feature.
			#[cfg(feature = "ffmpeg")]
			{
				let uuid = Uuid::new_v5(content_uuid, b"video");
				let model =
					extract_video_metadata_with_blurhash(source_path, uuid, blurhash).await?;
				let inserted = model.insert(db).await?;
				let mut active: content_identity::ActiveModel = content.into();
				active.video_media_data_id = Set(Some(inserted.id));
				active.update(db).await?;
			}
		}
		KIND_AUDIO => {
			if content.audio_media_data_id.is_some() {
				return Ok(());
			}

			#[cfg(feature = "ffmpeg")]
			{
				let uuid = Uuid::new_v5(content_uuid, b"audio");
				let model = extract_audio_metadata(source_path, uuid).await?;
				let inserted = model.insert(db).await?;
				let mut active: content_identity::ActiveModel = content.into();
				active.audio_media_data_id = Set(Some(inserted.id));
				active.update(db).await?;
			}
		}
		_ => {}
	}

	Ok(())
}

/// Extract image metadata from EXIF
///
/// Optionally pass a blurhash if it has already been generated elsewhere
/// (e.g., during thumbnail generation).
pub async fn extract_image_metadata(
	path: &Path,
	uuid: Uuid,
) -> Result<image_media_data::ActiveModel, Box<dyn std::error::Error + Send + Sync>> {
	extract_image_metadata_with_blurhash(path, uuid, None).await
}

/// Extract image metadata from EXIF with optional blurhash
pub async fn extract_image_metadata_with_blurhash(
	path: &Path,
	uuid: Uuid,
	blurhash: Option<String>,
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

	// Extract GPS coordinates from location data
	let (latitude, longitude) = exif
		.location
		.as_ref()
		.map(|loc| {
			let (lat, lon) = loc.coordinates();
			(Some(lat), Some(lon))
		})
		.unwrap_or((None, None));

	Ok(image_media_data::ActiveModel {
		id: sea_orm::ActiveValue::NotSet,
		uuid: Set(uuid),
		width: Set(exif.resolution.width as i32),
		height: Set(exif.resolution.height as i32),
		blurhash: Set(blurhash),
		date_taken: Set(date_taken.map(Into::into)),
		latitude: Set(latitude),
		longitude: Set(longitude),
		camera_make: Set(exif.camera_data.device_make),
		camera_model: Set(exif.camera_data.device_model),
		lens_model: Set(exif.camera_data.lens_model),
		focal_length: Set(exif.camera_data.focal_length.map(|f| f.to_string())),
		aperture: Set(None), // TODO: Extract from EXIF
		shutter_speed: Set(exif.camera_data.shutter_speed.map(|s| s.to_string())),
		iso: Set(exif.camera_data.iso),
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
///
/// Optionally pass a blurhash if it has already been generated elsewhere
/// (e.g., during thumbnail generation).
#[cfg(feature = "ffmpeg")]
pub async fn extract_video_metadata(
	path: &Path,
	uuid: Uuid,
) -> Result<video_media_data::ActiveModel, Box<dyn std::error::Error + Send + Sync>> {
	extract_video_metadata_with_blurhash(path, uuid, None).await
}

/// Extract video metadata from FFmpeg with optional blurhash
#[cfg(feature = "ffmpeg")]
pub async fn extract_video_metadata_with_blurhash(
	path: &Path,
	uuid: Uuid,
	blurhash: Option<String>,
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
		blurhash: Set(blurhash),
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
