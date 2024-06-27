use crate::media_processor::thumbnailer;

use sd_core_prisma_helpers::CasId;

use sd_file_ext::extensions::{
	DocumentExtension, Extension, ImageExtension, ALL_DOCUMENT_EXTENSIONS, ALL_IMAGE_EXTENSIONS,
};
use sd_images::{format_image, scale_dimensions, ConvertibleExtension};
use sd_media_metadata::exif::Orientation;
use sd_utils::error::FileIOError;

#[cfg(feature = "ffmpeg")]
use sd_file_ext::extensions::{VideoExtension, ALL_VIDEO_EXTENSIONS};

use std::{
	ops::Deref,
	panic,
	path::{Path, PathBuf},
	str::FromStr,
	time::Duration,
};

use image::{imageops, DynamicImage, GenericImageView};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::{
	fs, io,
	sync::{oneshot, Mutex},
	task::spawn_blocking,
	time::{sleep, Instant},
};
use tracing::{error, instrument, trace};
use uuid::Uuid;
use webp::Encoder;

// Files names constants
pub const THUMBNAIL_CACHE_DIR_NAME: &str = "thumbnails";
pub const WEBP_EXTENSION: &str = "webp";
pub const EPHEMERAL_DIR: &str = "ephemeral";

/// This is the target pixel count for all thumbnails to be resized to, and it is eventually downscaled
/// to [`TARGET_QUALITY`].
pub const TARGET_PX: f32 = 1_048_576.0; // 1024x1024

/// This is the target quality that we render thumbnails at, it is a float between 0-100
/// and is treated as a percentage (so 60% in this case, or it's the same as multiplying by `0.6`).
pub const TARGET_QUALITY: f32 = 60.0;

/// How much time we allow for the thumbnailer task to complete before we give up.
pub const THUMBNAILER_TASK_TIMEOUT: Duration = Duration::from_secs(60 * 5);

pub fn get_thumbnails_directory(data_directory: impl AsRef<Path>) -> PathBuf {
	data_directory.as_ref().join(THUMBNAIL_CACHE_DIR_NAME)
}

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
#[derive(Debug, Serialize, Deserialize, Type, Clone)]
pub struct ThumbKey {
	pub shard_hex: String,
	pub cas_id: CasId<'static>,
	pub base_directory_str: String,
}

impl ThumbKey {
	#[must_use]
	pub fn new(cas_id: CasId<'static>, kind: &ThumbnailKind) -> Self {
		Self {
			shard_hex: get_shard_hex(&cas_id).to_string(),
			cas_id,
			base_directory_str: match kind {
				ThumbnailKind::Ephemeral => String::from(EPHEMERAL_DIR),
				ThumbnailKind::Indexed(library_id) => library_id.to_string(),
			},
		}
	}

	#[must_use]
	pub fn new_indexed(cas_id: CasId<'static>, library_id: Uuid) -> Self {
		Self {
			shard_hex: get_shard_hex(&cas_id).to_string(),
			cas_id,
			base_directory_str: library_id.to_string(),
		}
	}

	#[must_use]
	pub fn new_ephemeral(cas_id: CasId<'static>) -> Self {
		Self {
			shard_hex: get_shard_hex(&cas_id).to_string(),
			cas_id,
			base_directory_str: String::from(EPHEMERAL_DIR),
		}
	}
}

#[derive(Debug, Serialize, Deserialize, Type, Clone, Copy)]
pub enum ThumbnailKind {
	Ephemeral,
	Indexed(Uuid),
}

impl ThumbnailKind {
	pub fn compute_path(&self, data_directory: impl AsRef<Path>, cas_id: &CasId<'_>) -> PathBuf {
		let mut thumb_path = get_thumbnails_directory(data_directory);
		match self {
			Self::Ephemeral => thumb_path.push(EPHEMERAL_DIR),
			Self::Indexed(library_id) => {
				thumb_path.push(library_id.to_string());
			}
		}
		thumb_path.push(get_shard_hex(cas_id));
		thumb_path.push(cas_id.as_str());
		thumb_path.set_extension(WEBP_EXTENSION);

		thumb_path
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateThumbnailArgs<'cas_id> {
	pub extension: String,
	pub cas_id: CasId<'cas_id>,
	pub path: PathBuf,
}

impl<'cas_id> GenerateThumbnailArgs<'cas_id> {
	#[must_use]
	pub const fn new(extension: String, cas_id: CasId<'cas_id>, path: PathBuf) -> Self {
		Self {
			extension,
			cas_id,
			path,
		}
	}
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
#[must_use]
pub fn get_shard_hex<'cas_id>(cas_id: &'cas_id CasId<'cas_id>) -> &'cas_id str {
	// Use the first three characters of the hash as the directory name
	&cas_id.as_str()[0..3]
}

#[cfg(feature = "ffmpeg")]
#[must_use]
pub const fn can_generate_thumbnail_for_video(video_extension: VideoExtension) -> bool {
	use VideoExtension::{Hevc, M2ts, M2v, Mpg, Mts, Swf, Ts};
	// File extensions that are specifically not supported by the thumbnailer
	!matches!(video_extension, Mpg | Swf | M2v | Hevc | M2ts | Mts | Ts)
}

#[must_use]
pub const fn can_generate_thumbnail_for_image(image_extension: ImageExtension) -> bool {
	use ImageExtension::{
		Avif, Bmp, Gif, Heic, Heics, Heif, Heifs, Ico, Jpeg, Jpg, Png, Svg, Webp,
	};

	matches!(
		image_extension,
		Jpg | Jpeg | Png | Webp | Gif | Svg | Heic | Heics | Heif | Heifs | Avif | Bmp | Ico
	)
}

#[must_use]
pub const fn can_generate_thumbnail_for_document(document_extension: DocumentExtension) -> bool {
	use DocumentExtension::Pdf;

	matches!(document_extension, Pdf)
}

#[derive(Debug)]
pub enum GenerationStatus {
	Generated,
	Skipped,
}

#[instrument(skip(thumbnails_directory, cas_id, should_regenerate, kind))]
pub async fn generate_thumbnail(
	thumbnails_directory: &Path,
	GenerateThumbnailArgs {
		extension,
		cas_id,
		path,
	}: &GenerateThumbnailArgs<'_>,
	kind: &ThumbnailKind,
	should_regenerate: bool,
) -> (
	Duration,
	Result<(ThumbKey, GenerationStatus), thumbnailer::NonCriticalThumbnailerError>,
) {
	trace!("Generating thumbnail");
	let start = Instant::now();

	let mut output_path = match kind {
		ThumbnailKind::Ephemeral => thumbnails_directory.join(EPHEMERAL_DIR),
		ThumbnailKind::Indexed(library_id) => thumbnails_directory.join(library_id.to_string()),
	};

	output_path.push(get_shard_hex(cas_id));
	output_path.push(cas_id.as_str());
	output_path.set_extension(WEBP_EXTENSION);

	if let Err(e) = fs::metadata(&*output_path).await {
		if e.kind() != io::ErrorKind::NotFound {
			error!(
				?e,
				"Failed to check if thumbnail exists, but we will try to generate it anyway;"
			);
		}
	// Otherwise we good, thumbnail doesn't exist so we can generate it
	} else if !should_regenerate {
		trace!("Skipping thumbnail generation because it already exists");
		return (
			start.elapsed(),
			Ok((
				ThumbKey::new(cas_id.to_owned(), kind),
				GenerationStatus::Skipped,
			)),
		);
	}

	if let Ok(extension) = ImageExtension::from_str(extension) {
		if can_generate_thumbnail_for_image(extension) {
			trace!("Generating image thumbnail");
			if let Err(e) = generate_image_thumbnail(&path, &output_path).await {
				return (start.elapsed(), Err(e));
			}
			trace!("Generated image thumbnail");
		}
	} else if let Ok(extension) = DocumentExtension::from_str(extension) {
		if can_generate_thumbnail_for_document(extension) {
			trace!("Generating document thumbnail");
			if let Err(e) = generate_image_thumbnail(&path, &output_path).await {
				return (start.elapsed(), Err(e));
			}
			trace!("Generating document thumbnail");
		}
	}

	#[cfg(feature = "ffmpeg")]
	{
		use crate::media_processor::helpers::thumbnailer::can_generate_thumbnail_for_video;
		use sd_file_ext::extensions::VideoExtension;

		if let Ok(extension) = VideoExtension::from_str(extension) {
			if can_generate_thumbnail_for_video(extension) {
				trace!("Generating video thumbnail");
				if let Err(e) = generate_video_thumbnail(&path, &output_path).await {
					return (start.elapsed(), Err(e));
				}
				trace!("Generated video thumbnail");
			}
		}
	}

	trace!("Generated thumbnail");

	(
		start.elapsed(),
		Ok((
			ThumbKey::new(cas_id.to_owned(), kind),
			GenerationStatus::Generated,
		)),
	)
}

fn inner_generate_image_thumbnail(
	file_path: &PathBuf,
) -> Result<Vec<u8>, thumbnailer::NonCriticalThumbnailerError> {
	let mut img = format_image(file_path).map_err(|e| {
		thumbnailer::NonCriticalThumbnailerError::FormatImage(file_path.clone(), e.to_string())
	})?;

	let (w, h) = img.dimensions();

	#[allow(clippy::cast_precision_loss)]
	let (w_scaled, h_scaled) = scale_dimensions(w as f32, h as f32, TARGET_PX);

	// Optionally, resize the existing photo and convert back into DynamicImage
	if w != w_scaled && h != h_scaled {
		img = DynamicImage::ImageRgba8(imageops::resize(
			&img,
			w_scaled,
			h_scaled,
			imageops::FilterType::Triangle,
		));
	}

	// this corrects the rotation/flip of the image based on the *available* exif data
	// not all images have exif data, so we don't error. we also don't rotate HEIF as that's against the spec
	if let Some(orientation) = Orientation::from_path(file_path) {
		if ConvertibleExtension::try_from(file_path.as_ref())
			.expect("we already checked if the image was convertible")
			.should_rotate()
		{
			img = orientation.correct_thumbnail(img);
		}
	}

	// Create the WebP encoder for the above image
	let encoder = Encoder::from_image(&img).map_err(|reason| {
		thumbnailer::NonCriticalThumbnailerError::WebPEncoding(
			file_path.clone(),
			reason.to_string(),
		)
	})?;

	// Type `WebPMemory` is !Send, which makes the `Future` in this function `!Send`,
	// this make us `deref` to have a `&[u8]` and then `to_owned` to make a `Vec<u8>`
	// which implies on a unwanted clone...
	Ok(encoder.encode(TARGET_QUALITY).deref().to_owned())
}

#[instrument(
	skip_all,
	fields(
		input_path = %file_path.as_ref().display(),
		output_path = %output_path.as_ref().display()
	)
)]
async fn generate_image_thumbnail(
	file_path: impl AsRef<Path> + Send,
	output_path: impl AsRef<Path> + Send,
) -> Result<(), thumbnailer::NonCriticalThumbnailerError> {
	let file_path = file_path.as_ref().to_path_buf();

	let (tx, rx) = oneshot::channel();

	// Using channel instead of waiting the JoinHandle as for some reason
	// the JoinHandle can take some extra time to complete
	let handle = spawn_blocking({
		let file_path = file_path.clone();

		move || {
			// Handling error on receiver side

			let _ = tx.send(
				panic::catch_unwind(|| inner_generate_image_thumbnail(&file_path)).unwrap_or_else(
					move |_| {
						Err(
							thumbnailer::NonCriticalThumbnailerError::PanicWhileGeneratingThumbnail(
								file_path,
								"Internal panic on third party crate".to_string(),
							),
						)
					},
				),
			);
		}
	});

	let webp = if let Ok(res) = rx.await {
		res?
	} else {
		error!("Failed to generate thumbnail");
		return Err(
			thumbnailer::NonCriticalThumbnailerError::PanicWhileGeneratingThumbnail(
				file_path,
				handle
					.await
					.expect_err("as the channel was closed, then the spawned task panicked")
					.to_string(),
			),
		);
	};

	trace!("Generated thumbnail bytes");

	let output_path = output_path.as_ref();

	if let Some(shard_dir) = output_path.parent() {
		fs::create_dir_all(shard_dir).await.map_err(|e| {
			thumbnailer::NonCriticalThumbnailerError::CreateShardDirectory(
				FileIOError::from((shard_dir, e)).to_string(),
			)
		})?;
	} else {
		error!("Failed to get parent directory for sharding parent directory");
	}

	trace!("Created shard directory and writing it to disk");

	let res = fs::write(output_path, &webp).await.map_err(|e| {
		thumbnailer::NonCriticalThumbnailerError::SaveThumbnail(
			file_path,
			FileIOError::from((output_path, e)).to_string(),
		)
	});

	trace!("Wrote thumbnail to disk");
	res
}

#[instrument(
	skip_all,
	fields(
		input_path = %file_path.as_ref().display(),
		output_path = %output_path.as_ref().display()
	)
)]
#[cfg(feature = "ffmpeg")]
async fn generate_video_thumbnail(
	file_path: impl AsRef<Path> + Send,
	output_path: impl AsRef<Path> + Send,
) -> Result<(), thumbnailer::NonCriticalThumbnailerError> {
	use sd_ffmpeg::{to_thumbnail, ThumbnailSize};

	let file_path = file_path.as_ref();

	to_thumbnail(
		file_path,
		output_path,
		ThumbnailSize::Scale(1024),
		TARGET_QUALITY,
	)
	.await
	.map_err(|e| {
		thumbnailer::NonCriticalThumbnailerError::VideoThumbnailGenerationFailed(
			file_path.to_path_buf(),
			e.to_string(),
		)
	})
}

const HALF_SEC: Duration = Duration::from_millis(500);
static LAST_SINGLE_THUMB_GENERATED_LOCK: Lazy<Mutex<Instant>> =
	Lazy::new(|| Mutex::new(Instant::now()));

/// WARNING!!!! DON'T USE THIS FUNCTION IN A LOOP!!!!!!!!!!!!! It will be pretty slow on purpose!
pub async fn generate_single_thumbnail(
	thumbnails_directory: impl AsRef<Path> + Send,
	extension: String,
	cas_id: CasId<'static>,
	path: impl AsRef<Path> + Send,
	kind: ThumbnailKind,
) -> Result<(), thumbnailer::NonCriticalThumbnailerError> {
	let mut last_single_thumb_generated_guard = LAST_SINGLE_THUMB_GENERATED_LOCK.lock().await;

	let elapsed = Instant::now() - *last_single_thumb_generated_guard;
	if elapsed < HALF_SEC {
		// This will choke up in case someone try to use this method in a loop, otherwise
		// it will consume all the machine resources like a gluton monster from hell
		sleep(HALF_SEC - elapsed).await;
	}

	let (_duration, res) = generate_thumbnail(
		thumbnails_directory.as_ref(),
		&GenerateThumbnailArgs {
			extension,
			cas_id,
			path: path.as_ref().to_path_buf(),
		},
		&kind,
		false,
	)
	.await;

	let (_thumb_key, status) = res?;

	if matches!(status, GenerationStatus::Generated) {
		*last_single_thumb_generated_guard = Instant::now();
		drop(last_single_thumb_generated_guard); // Clippy was weirdly complaining about not doing an "early" drop here
	}

	Ok(())
}
