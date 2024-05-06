use once_cell::sync::Lazy;
use sd_file_ext::extensions::{
	DocumentExtension, Extension, ImageExtension, ALL_DOCUMENT_EXTENSIONS, ALL_IMAGE_EXTENSIONS,
};

#[cfg(feature = "ffmpeg")]
use sd_file_ext::extensions::{VideoExtension, ALL_VIDEO_EXTENSIONS};

use std::time::Duration;

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

// Files names constants
pub const THUMBNAIL_CACHE_DIR_NAME: &str = "thumbnails";
pub const WEBP_EXTENSION: &str = "webp";
pub const EPHEMERAL_DIR: &str = "ephemeral";

/// This is the target pixel count for all thumbnails to be resized to, and it is eventually downscaled
/// to [`TARGET_QUALITY`].
pub const TARGET_PX: f32 = 262_144.0;

/// This is the target quality that we render thumbnails at, it is a float between 0-100
/// and is treated as a percentage (so 30% in this case, or it's the same as multiplying by `0.3`).
pub const TARGET_QUALITY: f32 = 30.0;

/// How much time we allow for the thumbnail generation process to complete before we give up.
pub const THUMBNAIL_GENERATION_TIMEOUT: Duration = Duration::from_secs(60);

#[cfg(feature = "ffmpeg")]
pub static THUMBNAILABLE_VIDEO_EXTENSIONS: Lazy<Vec<Extension>> = Lazy::new(|| {
	ALL_VIDEO_EXTENSIONS
		.iter()
		.copied()
		.filter(|&ext| can_generate_thumbnail_for_video(ext))
		.map(Extension::Video)
		.collect()
});

pub static THUMBNAILABLE_EXTENSIONS: Lazy<Vec<Extension>> = Lazy::new(|| {
	ALL_IMAGE_EXTENSIONS
		.iter()
		.copied()
		.filter(|&ext| can_generate_thumbnail_for_image(ext))
		.map(Extension::Image)
		.chain(
			ALL_DOCUMENT_EXTENSIONS
				.iter()
				.copied()
				.filter(|&ext| can_generate_thumbnail_for_document(ext))
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

/// This type is used to pass the relevant data to the frontend so it can request the thumbnail.
/// Tt supports extending the shard hex to support deeper directory structures in the future
#[derive(Debug, Serialize, Deserialize, Type)]
pub struct ThumbKey {
	pub shard_hex: String,
	pub cas_id: String,
	pub base_directory_str: String,
}

impl ThumbKey {
	#[must_use]
	pub fn new(cas_id: &str, kind: &ThumbnailKind) -> Self {
		Self {
			shard_hex: get_shard_hex(cas_id).to_string(),
			cas_id: cas_id.to_string(),
			base_directory_str: match kind {
				ThumbnailKind::Ephemeral => String::from(EPHEMERAL_DIR),
				ThumbnailKind::Indexed(library_id) => library_id.to_string(),
			},
		}
	}
}

#[derive(Debug, Serialize, Deserialize, Type, Clone, Copy)]
pub enum ThumbnailKind {
	Ephemeral,
	Indexed(Uuid),
}

/// The practice of dividing files into hex coded folders, often called "sharding,"
/// is mainly used to optimize file system performance. File systems can start to slow down
/// as the number of files in a directory increases. Thus, it's often beneficial to split
/// files into multiple directories to avoid this performance degradation.
///
/// `get_shard_hex` takes a `cas_id` (a hexadecimal hash) as input and returns the first
/// three characters of the hash as the directory name. Because we're using these first
/// three characters of a the hash, this will give us 4096 (16^3) possible directories,
/// named 000 to fff.
#[inline]
pub fn get_shard_hex(cas_id: &str) -> &str {
	// Use the first three characters of the hash as the directory name
	&cas_id[0..3]
}

#[cfg(feature = "ffmpeg")]
pub const fn can_generate_thumbnail_for_video(video_extension: VideoExtension) -> bool {
	use VideoExtension::{Hevc, M2ts, M2v, Mpg, Mts, Swf, Ts};
	// File extensions that are specifically not supported by the thumbnailer
	!matches!(video_extension, Mpg | Swf | M2v | Hevc | M2ts | Mts | Ts)
}

pub const fn can_generate_thumbnail_for_image(image_extension: ImageExtension) -> bool {
	use ImageExtension::{
		Avif, Bmp, Gif, Heic, Heics, Heif, Heifs, Ico, Jpeg, Jpg, Png, Svg, Webp,
	};

	matches!(
		image_extension,
		Jpg | Jpeg | Png | Webp | Gif | Svg | Heic | Heics | Heif | Heifs | Avif | Bmp | Ico
	)
}

pub const fn can_generate_thumbnail_for_document(document_extension: DocumentExtension) -> bool {
	use DocumentExtension::Pdf;

	matches!(document_extension, Pdf)
}
