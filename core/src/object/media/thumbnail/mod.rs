use crate::{
	api::CoreEvent,
	job::JobRunErrors,
	library::Library,
	location::file_path_helper::{file_path_for_media_processor, IsolatedFilePathData},
	prisma::location,
	util::{error::FileIOError, version_manager::VersionManagerError},
	Node,
};

use sd_file_ext::extensions::{Extension, ImageExtension, ALL_IMAGE_EXTENSIONS};
use sd_media_data::image::Orientation;

#[cfg(feature = "ffmpeg")]
use sd_file_ext::extensions::{VideoExtension, ALL_VIDEO_EXTENSIONS};

use std::{
	collections::HashMap,
	error::Error,
	ops::Deref,
	path::{Path, PathBuf},
};

use futures_concurrency::future::{Join, TryJoin};
use image::{self, imageops, DynamicImage, GenericImageView};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{fs, io, task::block_in_place};
use tracing::{error, trace, warn};
use webp::Encoder;

mod directory;
mod shard;

pub use directory::init_thumbnail_dir;
pub use shard::get_shard_hex;

const THUMBNAIL_SIZE_FACTOR: f32 = 0.2;
const THUMBNAIL_QUALITY: f32 = 30.0;
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
pub(super) static FILTERED_VIDEO_EXTENSIONS: Lazy<Vec<Extension>> = Lazy::new(|| {
	ALL_VIDEO_EXTENSIONS
		.iter()
		.cloned()
		.filter(can_generate_thumbnail_for_video)
		.map(Extension::Video)
		.collect()
});

pub(super) static FILTERED_IMAGE_EXTENSIONS: Lazy<Vec<Extension>> = Lazy::new(|| {
	ALL_IMAGE_EXTENSIONS
		.iter()
		.cloned()
		.filter(can_generate_thumbnail_for_image)
		.map(Extension::Image)
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

// TOOD(brxken128): validate avci and avcs
#[cfg(all(feature = "heif", not(target_os = "linux")))]
const HEIF_EXTENSIONS: [&str; 7] = ["heif", "heifs", "heic", "heics", "avif", "avci", "avcs"];

pub async fn generate_image_thumbnail<P: AsRef<Path>>(
	file_path: P,
	output_path: P,
) -> Result<(), Box<dyn Error>> {
	// Webp creation has blocking code
	let webp = block_in_place(|| -> Result<Vec<u8>, Box<dyn Error>> {
		#[cfg(all(feature = "heif", not(target_os = "linux")))]
		let img = {
			let ext = file_path
				.as_ref()
				.extension()
				.unwrap_or_default()
				.to_ascii_lowercase();
			if HEIF_EXTENSIONS
				.iter()
				.any(|e| ext == std::ffi::OsStr::new(e))
			{
				sd_heif::heif_to_dynamic_image(file_path.as_ref())?
			} else {
				image::open(file_path.as_ref())?
			}
		};

		#[cfg(not(all(feature = "heif", not(target_os = "linux"))))]
		let img = image::open(file_path.as_ref())?;

		let orientation = Orientation::source_orientation(&file_path);

		let (w, h) = img.dimensions();
		// Optionally, resize the existing photo and convert back into DynamicImage
		let mut img = DynamicImage::ImageRgba8(imageops::resize(
			&img,
			// FIXME : Think of a better heuristic to get the thumbnail size
			(w as f32 * THUMBNAIL_SIZE_FACTOR) as u32,
			(h as f32 * THUMBNAIL_SIZE_FACTOR) as u32,
			imageops::FilterType::Triangle,
		));

		// this corrects the rotation/flip of the image based on the available exif data
		if let Some(x) = orientation {
			img = x.correct_thumbnail(img);
		}

		// Create the WebP encoder for the above image
		let encoder = Encoder::from_image(&img)?;

		// Encode the image at a specified quality 0-100

		// Type WebPMemory is !Send, which makes the Future in this function !Send,
		// this make us `deref` to have a `&[u8]` and then `to_owned` to make a Vec<u8>
		// which implies on a unwanted clone...
		Ok(encoder.encode(THUMBNAIL_QUALITY).deref().to_owned())
	})?;

	let output_path = output_path.as_ref();

	if let Some(shard_dir) = output_path.parent() {
		fs::create_dir_all(shard_dir)
			.await
			.map_err(|e| FileIOError::from((shard_dir, e)))?;
	} else {
		return Err(io::Error::new(
			io::ErrorKind::InvalidInput,
			"Cannot determine parent shard directory for thumbnail",
		)
		.into());
	}

	fs::write(output_path, &webp)
		.await
		.map_err(|e| FileIOError::from((output_path, e)))
		.map_err(Into::into)
}

#[cfg(feature = "ffmpeg")]
pub async fn generate_video_thumbnail<P: AsRef<Path>>(
	file_path: P,
	output_path: P,
) -> Result<(), Box<dyn Error>> {
	use sd_ffmpeg::to_thumbnail;

	to_thumbnail(file_path, output_path, 256, THUMBNAIL_QUALITY).await?;

	Ok(())
}

#[cfg(feature = "ffmpeg")]
pub const fn can_generate_thumbnail_for_video(video_extension: &VideoExtension) -> bool {
	use VideoExtension::*;
	// File extensions that are specifically not supported by the thumbnailer
	!matches!(video_extension, Mpg | Swf | M2v | Hevc | M2ts | Mts | Ts)
}

pub const fn can_generate_thumbnail_for_image(image_extension: &ImageExtension) -> bool {
	use ImageExtension::*;

	#[cfg(all(feature = "heif", not(target_os = "linux")))]
	let res = matches!(
		image_extension,
		Jpg | Jpeg | Png | Webp | Gif | Heic | Heics | Heif | Heifs | Avif
	);

	#[cfg(not(all(feature = "heif", not(target_os = "linux"))))]
	let res = matches!(image_extension, Jpg | Jpeg | Png | Webp | Gif);

	res
}

pub(super) async fn process(
	entries: impl IntoIterator<Item = (&file_path_for_media_processor::Data, ThumbnailerEntryKind)>,
	location_id: location::id::Type,
	location_path: impl AsRef<Path>,
	thumbnails_base_dir: impl AsRef<Path>,
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
	to_create_dirs
		.into_values()
		.collect::<Vec<_>>()
		.try_join()
		.await?;

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
	) in entries.join().await.into_iter().enumerate()
	{
		ctx_update_fn(idx + 1);
		match metadata_res {
			Ok(_) => {
				trace!(
					"Thumb already exists, skipping generation for {}",
					output_path.display()
				);
				run_metadata.skipped += 1;
				continue;
			}

			Err(e) if e.kind() == io::ErrorKind::NotFound => {
				trace!(
					"Writing {} to {}",
					input_path.display(),
					output_path.display()
				);

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
							continue;
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
							continue;
						}
					}
				}

				trace!("Emitting new thumbnail event");
				library.emit(CoreEvent::NewThumbnail {
					thumb_key: get_thumb_key(cas_id),
				});
				run_metadata.created += 1;
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
