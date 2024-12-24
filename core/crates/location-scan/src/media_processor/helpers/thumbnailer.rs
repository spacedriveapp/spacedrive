use sd_core_shared_errors::job::media_processor::NonCriticalThumbnailerError;
use sd_core_shared_types::cas_id::CasId;
use sd_core_shared_types::thumbnail::{
	get_shard_hex, ThumbKey, ThumbnailKind, EPHEMERAL_DIR, WEBP_EXTENSION,
};
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
	sync::LazyLock,
	time::Duration,
};

use image::{imageops, DynamicImage, GenericImageView};
use serde::{Deserialize, Serialize};

use tokio::{
	fs::{self, File},
	io::{self, AsyncWriteExt},
	sync::{oneshot, Mutex},
	task::spawn_blocking,
	time::{sleep, Instant},
};
use tracing::{error, instrument, trace};

use webp::{Encoder, WebPConfig};

/// This is the target pixel count for all thumbnails to be resized to, and it is eventually downscaled
/// to [`TARGET_QUALITY`].
pub const TARGET_PX: f32 = 1_048_576.0; // 1024x1024

/// This is the target quality that we render thumbnails at, it is a float between 0-100
/// and is treated as a percentage (so 60% in this case, or it's the same as multiplying by `0.6`).
pub const TARGET_QUALITY: f32 = 60.0;

/// How much time we allow for the thumbnailer task to complete before we give up.
pub const THUMBNAILER_TASK_TIMEOUT: Duration = Duration::from_secs(60 * 5);

#[cfg(feature = "ffmpeg")]
pub static THUMBNAILABLE_VIDEO_EXTENSIONS: LazyLock<Vec<Extension>> = LazyLock::new(|| {
	ALL_VIDEO_EXTENSIONS
		.iter()
		.copied()
		.filter(|&ext| can_generate_thumbnail_for_video(ext))
		.map(Extension::Video)
		.collect()
});

pub static THUMBNAILABLE_EXTENSIONS: LazyLock<Vec<Extension>> = LazyLock::new(|| {
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

pub static ALL_THUMBNAILABLE_EXTENSIONS: LazyLock<Vec<Extension>> = LazyLock::new(|| {
	#[cfg(feature = "ffmpeg")]
	return THUMBNAILABLE_EXTENSIONS
		.iter()
		.cloned()
		.chain(THUMBNAILABLE_VIDEO_EXTENSIONS.iter().cloned())
		.collect();

	#[cfg(not(feature = "ffmpeg"))]
	THUMBNAILABLE_EXTENSIONS.clone()
});

static WEBP_CONFIG: LazyLock<WebPConfig> = LazyLock::new(|| {
	let mut config = WebPConfig::new().expect("failed to instantiate global webp config");
	config.lossless = 0;
	config.alpha_compression = 1;
	config.quality = TARGET_QUALITY;

	config
});

const HALF_SEC: Duration = Duration::from_millis(500);

static LAST_SINGLE_THUMB_GENERATED_LOCK: LazyLock<Mutex<Instant>> =
	LazyLock::new(|| Mutex::new(Instant::now()));

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
	Result<(ThumbKey, GenerationStatus), NonCriticalThumbnailerError>,
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
) -> Result<Vec<u8>, NonCriticalThumbnailerError> {
	let mut img = format_image(file_path)
		.map_err(|e| NonCriticalThumbnailerError::FormatImage(file_path.clone(), e.to_string()))?;

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
		NonCriticalThumbnailerError::WebPEncoding(file_path.clone(), reason.to_string())
	})?;

	let thumb = encoder.encode_advanced(&WEBP_CONFIG).map_err(|reason| {
		NonCriticalThumbnailerError::WebPEncoding(file_path.clone(), format!("{reason:?}"))
	})?;

	// Type `WebPMemory` is !Send, which makes the `Future` in this function `!Send`,
	// this make us `deref` to have a `&[u8]` and then `to_owned` to make a `Vec<u8>`
	// which implies on a unwanted clone...
	Ok(thumb.deref().to_owned())
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
) -> Result<(), NonCriticalThumbnailerError> {
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
						Err(NonCriticalThumbnailerError::PanicWhileGeneratingThumbnail(
							file_path,
							"Internal panic on third party crate".to_string(),
						))
					},
				),
			);
		}
	});

	let webp = if let Ok(res) = rx.await {
		res?
	} else {
		error!("Failed to generate thumbnail");
		return Err(NonCriticalThumbnailerError::PanicWhileGeneratingThumbnail(
			file_path,
			handle
				.await
				.expect_err("as the channel was closed, then the spawned task panicked")
				.to_string(),
		));
	};

	trace!("Generated thumbnail bytes");

	let output_path = output_path.as_ref();

	if let Some(shard_dir) = output_path.parent() {
		fs::create_dir_all(shard_dir).await.map_err(|e| {
			NonCriticalThumbnailerError::CreateShardDirectory(
				FileIOError::from((shard_dir, e)).to_string(),
			)
		})?;
	} else {
		error!("Failed to get parent directory for sharding parent directory");
	}

	trace!("Created shard directory and writing it to disk");

	let mut file = File::create(output_path).await.map_err(|e| {
		NonCriticalThumbnailerError::SaveThumbnail(
			file_path.clone(),
			FileIOError::from((output_path, e)).to_string(),
		)
	})?;

	file.write_all(&webp).await.map_err(|e| {
		NonCriticalThumbnailerError::SaveThumbnail(
			file_path.clone(),
			FileIOError::from((output_path, e)).to_string(),
		)
	})?;

	file.sync_all().await.map_err(|e| {
		NonCriticalThumbnailerError::SaveThumbnail(
			file_path,
			FileIOError::from((output_path, e)).to_string(),
		)
	})?;

	trace!("Wrote thumbnail to disk");
	return Ok(());
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
) -> Result<(), NonCriticalThumbnailerError> {
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
		NonCriticalThumbnailerError::VideoThumbnailGenerationFailed(
			file_path.to_path_buf(),
			e.to_string(),
		)
	})
}

/// WARNING!!!! DON'T USE THIS FUNCTION IN A LOOP!!!!!!!!!!!!! It will be pretty slow on purpose!
pub async fn generate_single_thumbnail(
	thumbnails_directory: impl AsRef<Path> + Send,
	extension: String,
	cas_id: CasId<'static>,
	path: impl AsRef<Path> + Send,
	kind: ThumbnailKind,
) -> Result<(), NonCriticalThumbnailerError> {
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
