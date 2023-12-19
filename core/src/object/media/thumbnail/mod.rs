use crate::{library::LibraryId, util::version_manager::VersionManagerError, Node};

use sd_file_ext::extensions::{
	DocumentExtension, Extension, ImageExtension, ALL_DOCUMENT_EXTENSIONS, ALL_IMAGE_EXTENSIONS,
};
use sd_utils::error::FileIOError;

#[cfg(feature = "ffmpeg")]
use sd_file_ext::extensions::{VideoExtension, ALL_VIDEO_EXTENSIONS};

use std::{
	path::{Path, PathBuf},
	time::Duration,
};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::task;
use tracing::error;

pub mod actor;
mod clean_up;
mod directory;
pub mod preferences;
mod process;
mod shard;
mod state;
mod worker;

pub use process::{BatchToProcess, GenerateThumbnailArgs};
pub use shard::get_shard_hex;

use directory::ThumbnailVersion;

// Files names constants
const THUMBNAIL_CACHE_DIR_NAME: &str = "thumbnails";
const SAVE_STATE_FILE: &str = "thumbs_to_process.bin";
const VERSION_FILE: &str = "version.txt";
pub const WEBP_EXTENSION: &str = "webp";
const EPHEMERAL_DIR: &str = "ephemeral";

/// This is the target pixel count for all thumbnails to be resized to, and it is eventually downscaled
/// to [`TARGET_QUALITY`].
const TARGET_PX: f32 = 262144_f32;

/// This is the target quality that we render thumbnails at, it is a float between 0-100
/// and is treated as a percentage (so 30% in this case, or it's the same as multiplying by `0.3`).
const TARGET_QUALITY: f32 = 30_f32;

// Some time constants
const ONE_SEC: Duration = Duration::from_secs(1);
const THIRTY_SECS: Duration = Duration::from_secs(30);
const HALF_HOUR: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ThumbnailKind {
	Ephemeral,
	Indexed(LibraryId),
}

pub fn get_indexed_thumbnail_path(node: &Node, cas_id: &str, library_id: LibraryId) -> PathBuf {
	get_thumbnail_path(node, cas_id, ThumbnailKind::Indexed(library_id))
}

/// This does not check if a thumbnail exists, it just returns the path that it would exist at
fn get_thumbnail_path(node: &Node, cas_id: &str, kind: ThumbnailKind) -> PathBuf {
	let mut thumb_path = node.config.data_directory();

	thumb_path.push(THUMBNAIL_CACHE_DIR_NAME);
	match kind {
		ThumbnailKind::Ephemeral => thumb_path.push(EPHEMERAL_DIR),
		ThumbnailKind::Indexed(library_id) => {
			thumb_path.push(library_id.to_string());
		}
	}
	thumb_path.push(get_shard_hex(cas_id));
	thumb_path.push(cas_id);
	thumb_path.set_extension(WEBP_EXTENSION);

	thumb_path
}

pub fn get_indexed_thumb_key(cas_id: &str, library_id: LibraryId) -> Vec<String> {
	get_thumb_key(cas_id, ThumbnailKind::Indexed(library_id))
}

pub fn get_ephemeral_thumb_key(cas_id: &str) -> Vec<String> {
	get_thumb_key(cas_id, ThumbnailKind::Ephemeral)
}

// this is used to pass the relevant data to the frontend so it can request the thumbnail
// it supports extending the shard hex to support deeper directory structures in the future
fn get_thumb_key(cas_id: &str, kind: ThumbnailKind) -> Vec<String> {
	vec![
		match kind {
			ThumbnailKind::Ephemeral => String::from(EPHEMERAL_DIR),
			ThumbnailKind::Indexed(library_id) => library_id.to_string(),
		},
		get_shard_hex(cas_id).to_string(),
		cas_id.to_string(),
	]
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

pub(super) static ALL_THUMBNAILABLE_EXTENSIONS: Lazy<Vec<Extension>> = Lazy::new(|| {
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
	VersionManager(#[from] VersionManagerError<ThumbnailVersion>),
	#[error("failed to encode webp")]
	WebPEncoding { path: Box<Path>, reason: String },
	#[error("error while converting the image")]
	SdImages {
		path: Box<Path>,
		error: sd_images::Error,
	},
	#[error("failed to execute converting task: {0}")]
	Task(#[from] task::JoinError),
	#[cfg(feature = "ffmpeg")]
	#[error(transparent)]
	FFmpeg(#[from] sd_ffmpeg::Error),
	#[error("thumbnail generation timed out for {}", .0.display())]
	TimedOut(Box<Path>),
}

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
