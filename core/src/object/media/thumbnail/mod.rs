use crate::{
	api::CoreEvent,
	job::JobRunErrors,
	library::Library,
	location::file_path_helper::{file_path_for_media_processor, IsolatedFilePathData},
	prisma::location,
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
	collections::HashMap,
	ops::Deref,
	path::{Path, PathBuf},
	str::FromStr,
	sync::Arc,
};

use futures::future::{join_all, try_join_all};
use image::{self, imageops, DynamicImage, GenericImageView};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{fs, io, task};
use tracing::{error, trace, warn};
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

pub(super) async fn process(
	entries: impl IntoIterator<Item = (&file_path_for_media_processor::Data, ThumbnailerEntryKind)>,
	location_id: location::id::Type,
	location_path: impl AsRef<Path>,
	thumbnails_base_dir: impl AsRef<Path>,
	regenerate: bool,
	library: &Library,
	ctx_update_fn: impl Fn(usize),
) -> Result<(ThumbnailerMetadata, JobRunErrors), ThumbnailerError> {
	let mut run_metadata = ThumbnailerMetadata::default();

	let location_path = location_path.as_ref();
	let thumbnails_base_dir = thumbnails_base_dir.as_ref();
	let mut errors = vec![];

	let mut to_create_dirs = HashMap::new();

	struct WorkTable<'a> {
		kind: ThumbnailerEntryKind,
		input_path: PathBuf,
		cas_id: &'a str,
		output_path: PathBuf,
		metadata_res: io::Result<()>,
	}

	let entries = entries
		.into_iter()
		.filter_map(|(file_path, kind)| {
			IsolatedFilePathData::try_from((location_id, file_path))
				.map(|iso_file_path| (file_path, kind, location_path.join(iso_file_path)))
				.map_err(|e| {
					errors.push(format!(
						"Failed to build path for file with id {}: {e}",
						file_path.id
					))
				})
				.ok()
		})
		.filter_map(|(file_path, kind, path)| {
			if let Some(cas_id) = &file_path.cas_id {
				Some((kind, path, cas_id))
			} else {
				warn!(
					"Skipping thumbnail generation for {} due to missing cas_id",
					path.display()
				);
				run_metadata.skipped += 1;
				None
			}
		})
		.map(|(kind, input_path, cas_id)| {
			let thumbnails_shard_dir = thumbnails_base_dir.join(get_shard_hex(cas_id));
			let output_path = thumbnails_shard_dir.join(format!("{cas_id}.webp"));

			// Putting all sharding directories in a map to avoid trying to create repeteaded ones
			to_create_dirs
				.entry(thumbnails_shard_dir.clone())
				.or_insert_with(|| async move {
					fs::create_dir_all(&thumbnails_shard_dir)
						.await
						.map_err(|e| FileIOError::from((thumbnails_shard_dir, e)))
				});

			async move {
				WorkTable {
					kind,
					input_path,
					cas_id,
					// Discarding the ok part as we don't actually care about metadata here, maybe avoiding extra space
					metadata_res: fs::metadata(&output_path).await.map(|_| ()),
					output_path,
				}
			}
		})
		.collect::<Vec<_>>();
	if entries.is_empty() {
		return Ok((run_metadata, errors.into()));
	}

	// Resolving these futures first, as we want to fail early if we can't create the directories
	try_join_all(to_create_dirs.into_values()).await?;

	// Running thumbs generation sequentially to don't overload the system, if we're wasting too much time on I/O we can
	// try to run them in parallel
	for (
		idx,
		WorkTable {
			kind,
			input_path,
			cas_id,
			output_path,
			metadata_res,
		},
	) in join_all(entries).await.into_iter().enumerate()
	{
		ctx_update_fn(idx + 1);
		match metadata_res {
			Ok(_) => {
				if !regenerate {
					trace!(
						"Thumbnail already exists, skipping generation for {}",
						input_path.display()
					);
					run_metadata.skipped += 1;
				} else {
					tracing::debug!(
						"Renegerating thumbnail {} to {}",
						input_path.display(),
						output_path.display()
					);
					process_single_thumbnail(
						cas_id,
						kind,
						&input_path,
						&output_path,
						&mut errors,
						&mut run_metadata,
						library,
					)
					.await;
				}
			}

			Err(e) if e.kind() == io::ErrorKind::NotFound => {
				trace!(
					"Writing {} to {}",
					input_path.display(),
					output_path.display()
				);

				process_single_thumbnail(
					cas_id,
					kind,
					&input_path,
					&output_path,
					&mut errors,
					&mut run_metadata,
					library,
				)
				.await;
			}
			Err(e) => {
				error!(
					"Error getting metadata for thumb: {:#?}",
					FileIOError::from((output_path, e))
				);
				errors.push(format!(
					"Had an error generating thumbnail for \"{}\"",
					input_path.display()
				));
			}
		}
	}

	Ok((run_metadata, errors.into()))
}

// Using &Path as this function if private only to this module, always being used with a &Path, so we
// don't pay the compile price for generics
async fn process_single_thumbnail(
	cas_id: &str,
	kind: ThumbnailerEntryKind,
	input_path: &Path,
	output_path: &Path,
	errors: &mut Vec<String>,
	run_metadata: &mut ThumbnailerMetadata,
	library: &Library,
) {
	match kind {
		ThumbnailerEntryKind::Image => {
			if let Err(e) = generate_image_thumbnail(&input_path, &output_path).await {
				error!(
					"Error generating thumb for image \"{}\": {e:#?}",
					input_path.display()
				);
				errors.push(format!(
					"Had an error generating thumbnail for \"{}\"",
					input_path.display()
				));

				return;
			}
		}
		#[cfg(feature = "ffmpeg")]
		ThumbnailerEntryKind::Video => {
			if let Err(e) = generate_video_thumbnail(&input_path, &output_path).await {
				error!(
					"Error generating thumb for video \"{}\": {e:#?}",
					input_path.display()
				);
				errors.push(format!(
					"Had an error generating thumbnail for \"{}\"",
					input_path.display()
				));

				return;
			}
		}
	}

	trace!("Emitting new thumbnail event");
	library.emit(CoreEvent::NewThumbnail {
		thumb_key: get_thumb_key(cas_id),
	});
	run_metadata.created += 1;
}

// TODO(fogodev): Unify how we generate thumbnails

#[derive(Debug)]
pub struct GenerateThumbnailArgs {
	pub extension: String,
	pub cas_id: String,
	pub path: PathBuf,
	pub node: Arc<Node>,
}

impl GenerateThumbnailArgs {
	pub fn new(extension: String, cas_id: String, path: PathBuf, node: Arc<Node>) -> Self {
		Self {
			extension,
			cas_id,
			path,
			node,
		}
	}
}

pub async fn generate_thumbnail(
	extension: &str,
	cas_id: String,
	path: impl AsRef<Path>,
	node: Arc<Node>,
	in_background: bool,
) -> Result<String, ThumbnailerError> {
	let path = path.as_ref();
	trace!("Generating thumbnail for {}", path.display());
	let output_path = get_thumbnail_path(&node, &cas_id);

	if let Err(e) = fs::metadata(&output_path).await {
		if e.kind() != io::ErrorKind::NotFound {
			error!(
				"Failed to check if thumbnail exists, but we will try to generate it anyway: {e}"
			);
		}
	// Otherwise we good, thumbnail doesn't exist so we can generate it
	} else {
		trace!(
			"Skipping thumbnail generation for {} because it already exists",
			path.display()
		);
		return Ok(cas_id);
	}

	if let Ok(extension) = ImageExtension::from_str(extension) {
		if can_generate_thumbnail_for_image(&extension) {
			generate_image_thumbnail(&path, &output_path).await?;
		}
	} else if let Ok(extension) = DocumentExtension::from_str(extension) {
		if can_generate_thumbnail_for_document(&extension) {
			generate_image_thumbnail(&path, &output_path).await?;
		}
	}

	#[cfg(feature = "ffmpeg")]
	{
		if let Ok(extension) = VideoExtension::from_str(extension) {
			if can_generate_thumbnail_for_video(&extension) {
				generate_video_thumbnail(&path, &output_path).await?;
			}
		}
	}

	if !in_background {
		trace!("Emitting new thumbnail event");
		node.emit(CoreEvent::NewThumbnail {
			thumb_key: get_thumb_key(&cas_id),
		});
	}

	trace!("Generated thumbnail for {}", path.display());

	Ok(cas_id)
}
