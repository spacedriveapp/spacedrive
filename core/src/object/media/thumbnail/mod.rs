use crate::{
	util::{error::FileIOError, version_manager::VersionManagerError},
	Node,
};

use sd_file_ext::extensions::{
	DocumentExtension, Extension, ImageExtension, ALL_DOCUMENT_EXTENSIONS, ALL_IMAGE_EXTENSIONS,
};
use sd_images::{format_image, scale_dimensions};
use sd_media_metadata::image::Orientation;

#[cfg(feature = "ffmpeg")]
use sd_file_ext::extensions::{VideoExtension, ALL_VIDEO_EXTENSIONS};

use std::{
	ops::Deref,
	path::{Path, PathBuf},
};

use image::{self, imageops, DynamicImage, GenericImageView};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{fs, task};
use tracing::error;
use webp::Encoder;

pub mod actor;
mod directory;
mod shard;

pub use directory::init_thumbnail_dir;
pub use shard::get_shard_hex;

pub const THUMBNAIL_CACHE_DIR_NAME: &str = "thumbnails";

/// This does not check if a thumbnail exists, it just returns the path that it would exist at
pub fn get_thumbnail_path(node: &Node, cas_id: &str) -> PathBuf {
	let mut thumb_path = node.config.data_directory();

	thumb_path.push(THUMBNAIL_CACHE_DIR_NAME);
	thumb_path.push(get_shard_hex(cas_id));
	thumb_path.push(cas_id);
	thumb_path.set_extension("webp");

	thumb_path
}

// this is used to pass the relevant data to the frontend so it can request the thumbnail
// it supports extending the shard hex to support deeper directory structures in the future
pub fn get_thumb_key(cas_id: &str) -> Vec<String> {
	vec![get_shard_hex(cas_id), cas_id.to_string()]
}

#[cfg(feature = "ffmpeg")]
pub(super) static THUMBNAILABLE_VIDEO_EXTENSIONS: Lazy<Vec<Extension>> = Lazy::new(|| {
	ALL_VIDEO_EXTENSIONS
		.iter()
		.cloned()
		.filter(can_generate_thumbnail_for_video)
		.map(Extension::Video)
		.collect()
});

pub(super) static THUMBNAILABLE_EXTENSIONS: Lazy<Vec<Extension>> = Lazy::new(|| {
	ALL_IMAGE_EXTENSIONS
		.iter()
		.cloned()
		.filter(can_generate_thumbnail_for_image)
		.map(Extension::Image)
		.chain(
			ALL_DOCUMENT_EXTENSIONS
				.iter()
				.cloned()
				.filter(can_generate_thumbnail_for_document)
				.map(Extension::Document),
		)
		.collect()
});

pub static ALL_THUMBNAILABLE_EXTENSIONS: Lazy<Vec<Extension>> = Lazy::new(|| {
	#[cfg(feature = "ffmpeg")]
	return THUMBNAILABLE_EXTENSIONS
		.iter()
		.cloned()
		.chain(THUMBNAILABLE_VIDEO_EXTENSIONS.iter().cloned())
		.collect();

	#[cfg(not(feature = "ffmpeg"))]
	THUMBNAILABLE_EXTENSIONS.clone()
});

#[derive(Error, Debug)]
pub enum ThumbnailerError {
	// Internal errors
	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error(transparent)]
	VersionManager(#[from] VersionManagerError),
	#[error("failed to encode webp")]
	Encoding,
	#[error("error while converting the image: {0}")]
	SdImages(#[from] sd_images::Error),
	#[error("failed to execute converting task: {0}")]
	Task(#[from] task::JoinError),
	#[cfg(feature = "ffmpeg")]
	#[error(transparent)]
	FFmpeg(#[from] sd_ffmpeg::ThumbnailerError),
	#[error("thumbnail generation timed out for {}", .0.display())]
	TimedOut(Box<Path>),
}

/// This is the target pixel count for all thumbnails to be resized to, and it is eventually downscaled
/// to [`TARGET_QUALITY`].
const TARGET_PX: f32 = 262144_f32;

/// This is the target quality that we render thumbnails at, it is a float between 0-100
/// and is treated as a percentage (so 30% in this case, or it's the same as multiplying by `0.3`).
const TARGET_QUALITY: f32 = 30_f32;

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum ThumbnailerEntryKind {
	Image,
	#[cfg(feature = "ffmpeg")]
	Video,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct ThumbnailerMetadata {
	pub created: u32,
	pub skipped: u32,
}

pub async fn generate_image_thumbnail(
	file_path: impl AsRef<Path>,
	output_path: impl AsRef<Path>,
) -> Result<(), ThumbnailerError> {
	let file_path = file_path.as_ref().to_path_buf();

	let webp = task::spawn_blocking(move || -> Result<_, ThumbnailerError> {
		let img = format_image(&file_path).map_err(|_| ThumbnailerError::Encoding)?;

		let (w, h) = img.dimensions();
		let (w_scaled, h_scaled) = scale_dimensions(w as f32, h as f32, TARGET_PX);

		// Optionally, resize the existing photo and convert back into DynamicImage
		let mut img = DynamicImage::ImageRgba8(imageops::resize(
			&img,
			w_scaled as u32,
			h_scaled as u32,
			imageops::FilterType::Triangle,
		));

		// this corrects the rotation/flip of the image based on the *available* exif data
		// not all images have exif data, so we don't error
		if let Some(orientation) = Orientation::from_path(file_path) {
			img = orientation.correct_thumbnail(img);
		}

		// Create the WebP encoder for the above image
		let Ok(encoder) = Encoder::from_image(&img) else {
			return Err(ThumbnailerError::Encoding);
		};

		// Type WebPMemory is !Send, which makes the Future in this function !Send,
		// this make us `deref` to have a `&[u8]` and then `to_owned` to make a Vec<u8>
		// which implies on a unwanted clone...
		Ok(encoder.encode(TARGET_QUALITY).deref().to_owned())
	})
	.await??;

	let output_path = output_path.as_ref();

	if let Some(shard_dir) = output_path.parent() {
		fs::create_dir_all(shard_dir)
			.await
			.map_err(|e| FileIOError::from((shard_dir, e)))?;
	} else {
		return Err(ThumbnailerError::Encoding);
	}

	fs::write(output_path, &webp)
		.await
		.map_err(|e| FileIOError::from((output_path, e)))
		.map_err(Into::into)
}

#[cfg(feature = "ffmpeg")]
pub async fn generate_video_thumbnail(
	file_path: impl AsRef<Path>,
	output_path: impl AsRef<Path>,
) -> Result<(), ThumbnailerError> {
	use sd_ffmpeg::to_thumbnail;

	to_thumbnail(file_path, output_path, 256, TARGET_QUALITY)
		.await
		.map_err(Into::into)
}

#[cfg(feature = "ffmpeg")]
pub const fn can_generate_thumbnail_for_video(video_extension: &VideoExtension) -> bool {
	use VideoExtension::*;
	// File extensions that are specifically not supported by the thumbnailer
	!matches!(video_extension, Mpg | Swf | M2v | Hevc | M2ts | Mts | Ts)
}

pub const fn can_generate_thumbnail_for_image(image_extension: &ImageExtension) -> bool {
	use ImageExtension::*;

	matches!(
		image_extension,
		Jpg | Jpeg | Png | Webp | Gif | Svg | Heic | Heics | Heif | Heifs | Avif | Bmp | Ico
	)
}

pub const fn can_generate_thumbnail_for_document(document_extension: &DocumentExtension) -> bool {
	use DocumentExtension::*;

	matches!(document_extension, Pdf)
}
