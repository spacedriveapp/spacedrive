//! Thumbnails directory have the following structure:
//! thumbnails/
//! ├── version.txt
//! ├── ephemeral/ # ephemeral ones have it's own directory
//! │  └── <`cas_id`>[0..3]/ # sharding
//! │     └── <`cas_id`>.webp
//! └── <`library_id`>/ # we segregate thumbnails by library
//!    └── <`cas_id`>[0..3]/ # sharding
//!       └── <`cas_id`>.webp

use crate::{
	media_processor::{
		self,
		helpers::thumbnailer::{
			can_generate_thumbnail_for_document, can_generate_thumbnail_for_image, get_shard_hex,
			EPHEMERAL_DIR, TARGET_PX, TARGET_QUALITY, THUMBNAIL_GENERATION_TIMEOUT, WEBP_EXTENSION,
		},
		ThumbKey, ThumbnailKind,
	},
	Error,
};

use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_prisma_helpers::file_path_for_media_processor;

use sd_file_ext::extensions::{DocumentExtension, ImageExtension};
use sd_images::{format_image, scale_dimensions, ConvertibleExtension};
use sd_media_metadata::image::Orientation;
use sd_prisma::prisma::{file_path, location};
use sd_task_system::{
	check_interruption, ExecStatus, Interrupter, IntoAnyTaskOutput, SerializableTask, Task, TaskId,
};
use sd_utils::error::FileIOError;

use std::{
	collections::HashMap,
	fmt, mem,
	ops::Deref,
	path::{Path, PathBuf},
	pin::pin,
	str::FromStr,
	sync::Arc,
	time::Duration,
};

use futures::{FutureExt, StreamExt};
use futures_concurrency::future::{FutureGroup, Race};
use image::{imageops, DynamicImage, GenericImageView};
use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::{fs, io, task::spawn_blocking, time::sleep};
use tracing::{error, trace};
use uuid::Uuid;
use webp::Encoder;

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateThumbnailArgs {
	pub extension: String,
	pub cas_id: String,
	pub path: PathBuf,
}

impl GenerateThumbnailArgs {
	#[must_use]
	pub const fn new(extension: String, cas_id: String, path: PathBuf) -> Self {
		Self {
			extension,
			cas_id,
			path,
		}
	}
}

pub type ThumbnailId = u32;

pub trait NewThumbnailReporter: Send + Sync + fmt::Debug + 'static {
	fn new_thumbnail(&self, thumb_key: ThumbKey);
}

#[derive(Debug)]
pub struct Thumbnailer<Reporter: NewThumbnailReporter> {
	id: TaskId,
	reporter: Arc<Reporter>,
	thumbs_kind: ThumbnailKind,
	thumbnails_directory_path: Arc<PathBuf>,
	thumbnails_to_generate: HashMap<ThumbnailId, GenerateThumbnailArgs>,
	already_processed_ids: Vec<ThumbnailId>,
	should_regenerate: bool,
	with_priority: bool,
	output: Output,
}

#[async_trait::async_trait]
impl<Reporter: NewThumbnailReporter> Task<Error> for Thumbnailer<Reporter> {
	fn id(&self) -> TaskId {
		self.id
	}

	fn with_priority(&self) -> bool {
		self.with_priority
	}

	fn with_timeout(&self) -> Option<Duration> {
		Some(Duration::from_secs(60 * 5)) // The entire task must not take more than 5 minutes
	}

	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, Error> {
		let Self {
			thumbs_kind,
			thumbnails_directory_path,
			thumbnails_to_generate,
			already_processed_ids,
			should_regenerate,
			with_priority,
			reporter,
			output,
			..
		} = self;

		// Removing already processed thumbnails from a possible previous run
		already_processed_ids.drain(..).for_each(|id| {
			thumbnails_to_generate.remove(&id);
		});

		let mut futures = pin!(thumbnails_to_generate
			.iter()
			.map(|(id, generate_args)| {
				let path = generate_args.path.clone();

				(
					generate_thumbnail(
						thumbnails_directory_path,
						generate_args,
						thumbs_kind,
						*should_regenerate,
					)
					.map(|res| (*id, res)),
					sleep(THUMBNAIL_GENERATION_TIMEOUT)
						.map(|()| (*id, Err(NonCriticalError::ThumbnailGenerationTimeout(path)))),
				)
					.race()
			})
			.collect::<FutureGroup<_>>());

		while let Some((id, res)) = futures.next().await {
			match res {
				Ok((thumb_key, status)) => {
					match status {
						GenerationStatus::Generated => {
							output.generated += 1;
						}
						GenerationStatus::Skipped => {
							output.skipped += 1;
						}
					}

					// This if is REALLY needed, due to the sheer performance of the thumbnailer,
					// I restricted to only send events notifying for thumbnails in the current
					// opened directory, sending events for the entire location turns into a
					// humongous bottleneck in the frontend lol, since it doesn't even knows
					// what to do with thumbnails for inner directories lol
					// - fogodev
					if *with_priority {
						reporter.new_thumbnail(thumb_key);
					}
				}
				Err(e) => {
					output
						.errors
						.push(media_processor::NonCriticalError::from(e).into());
				}
			}

			already_processed_ids.push(id);
			check_interruption!(interrupter);
		}

		Ok(ExecStatus::Done(mem::take(output).into_output()))
	}
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Output {
	generated: u32,
	skipped: u32,
	errors: Vec<crate::NonCriticalError>,
}

#[derive(thiserror::Error, Debug, Serialize, Deserialize, Type)]
pub enum NonCriticalError {
	#[error("file path <id='{0}'> has no cas_id")]
	MissingCasId(file_path::id::Type),
	#[error("failed to extract isolated file path data from file path <id='{0}'>: {1}")]
	FailedToExtractIsolatedFilePathData(file_path::id::Type, String),
	#[error("failed to generate video file thumbnail <path='{}'>: {1}", .0.display())]
	VideoThumbnailGenerationFailed(PathBuf, String),
	#[error("failed to format image <path='{}'>: {1}", .0.display())]
	FormatImage(PathBuf, String),
	#[error("failed to encode webp image <path='{}'>: {1}", .0.display())]
	WebPEncoding(PathBuf, String),
	#[error("processing thread panicked while generating thumbnail from <path='{}'>: {1}", .0.display())]
	PanicWhileGeneratingThumbnail(PathBuf, String),
	#[error("failed to create shard directory for thumbnail: {0}")]
	CreateShardDirectory(String),
	#[error("failed to save thumbnail <path='{}'>: {1}", .0.display())]
	SaveThumbnail(PathBuf, String),
	#[error("thumbnail generation timed out <path='{}'>", .0.display())]
	ThumbnailGenerationTimeout(PathBuf),
}

impl<Reporter: NewThumbnailReporter> Thumbnailer<Reporter> {
	fn new(
		thumbs_kind: ThumbnailKind,
		thumbnails_directory_path: Arc<PathBuf>,
		thumbnails_to_generate: HashMap<ThumbnailId, GenerateThumbnailArgs>,
		errors: Vec<crate::NonCriticalError>,
		should_regenerate: bool,
		with_priority: bool,
		reporter: Arc<Reporter>,
	) -> Self {
		Self {
			id: TaskId::new_v4(),
			thumbs_kind,
			thumbnails_directory_path,
			already_processed_ids: Vec::with_capacity(thumbnails_to_generate.len()),
			thumbnails_to_generate,
			should_regenerate,
			with_priority,
			output: Output {
				errors,
				..Default::default()
			},
			reporter,
		}
	}

	#[must_use]
	pub fn new_ephemeral(
		thumbnails_directory_path: Arc<PathBuf>,
		thumbnails_to_generate: Vec<GenerateThumbnailArgs>,
		reporter: Arc<Reporter>,
	) -> Self {
		Self::new(
			ThumbnailKind::Ephemeral,
			thumbnails_directory_path,
			thumbnails_to_generate
				.into_iter()
				.enumerate()
				.map(|(i, args)| {
					#[allow(clippy::cast_possible_truncation)]
					{
						// SAFETY: it's fine, we will never process more than 4 billion thumbnails
						// on a single task LMAO
						(i as ThumbnailId, args)
					}
				})
				.collect(),
			Vec::new(),
			false,
			true,
			reporter,
		)
	}

	#[must_use]
	pub fn new_indexed(
		thumbnails_directory_path: Arc<PathBuf>,
		file_paths: &[file_path_for_media_processor::Data],
		(location_id, location_path): (location::id::Type, &Path),
		library_id: Uuid,
		should_regenerate: bool,
		with_priority: bool,
		reporter: Arc<Reporter>,
	) -> Self {
		let mut errors = Vec::new();

		Self::new(
			ThumbnailKind::Indexed(library_id),
			thumbnails_directory_path,
			file_paths
				.iter()
				.filter_map(|file_path| {
					if let Some(cas_id) = file_path.cas_id.as_ref() {
						let file_path_id = file_path.id;
						IsolatedFilePathData::try_from((location_id, file_path))
							.map_err(|e| {
								errors.push(
									media_processor::NonCriticalError::from(
										NonCriticalError::FailedToExtractIsolatedFilePathData(
											file_path_id,
											e.to_string(),
										),
									)
									.into(),
								);
							})
							.ok()
							.map(|iso_file_path| (file_path_id, cas_id, iso_file_path))
					} else {
						errors.push(
							media_processor::NonCriticalError::from(
								NonCriticalError::MissingCasId(file_path.id),
							)
							.into(),
						);
						None
					}
				})
				.map(|(file_path_id, cas_id, iso_file_path)| {
					let full_path = location_path.join(&iso_file_path);

					#[allow(clippy::cast_sign_loss)]
					{
						(
							// SAFETY: db doesn't have negative indexes
							file_path_id as u32,
							GenerateThumbnailArgs::new(
								iso_file_path.extension().to_string(),
								cas_id.clone(),
								full_path,
							),
						)
					}
				})
				.collect::<HashMap<_, _>>(),
			errors,
			should_regenerate,
			with_priority,
			reporter,
		)
	}
}

#[derive(Debug, Serialize, Deserialize)]
struct SaveState {
	id: TaskId,
	thumbs_kind: ThumbnailKind,
	thumbnails_directory_path: Arc<PathBuf>,
	thumbnails_to_generate: HashMap<ThumbnailId, GenerateThumbnailArgs>,
	should_regenerate: bool,
	with_priority: bool,
	output: Output,
}

impl<Reporter: NewThumbnailReporter> SerializableTask<Error> for Thumbnailer<Reporter> {
	type SerializeError = rmp_serde::encode::Error;

	type DeserializeError = rmp_serde::decode::Error;

	type DeserializeCtx = Arc<Reporter>;

	async fn serialize(self) -> Result<Vec<u8>, Self::SerializeError> {
		let Self {
			id,
			thumbs_kind,
			thumbnails_directory_path,
			mut thumbnails_to_generate,
			already_processed_ids,
			should_regenerate,
			with_priority,
			output,
			..
		} = self;

		for id in already_processed_ids {
			thumbnails_to_generate.remove(&id);
		}

		rmp_serde::to_vec_named(&SaveState {
			id,
			thumbs_kind,
			thumbnails_directory_path,
			thumbnails_to_generate,
			should_regenerate,
			with_priority,
			output,
		})
	}

	async fn deserialize(
		data: &[u8],
		reporter: Self::DeserializeCtx,
	) -> Result<Self, Self::DeserializeError> {
		rmp_serde::from_slice(data).map(
			|SaveState {
			     id,
			     thumbs_kind,
			     thumbnails_directory_path,
			     thumbnails_to_generate,
			     should_regenerate,
			     with_priority,
			     output,
			 }| Self {
				id,
				reporter,
				thumbs_kind,
				thumbnails_directory_path,
				thumbnails_to_generate,
				already_processed_ids: Vec::new(),
				should_regenerate,
				with_priority,
				output,
			},
		)
	}
}

enum GenerationStatus {
	Generated,
	Skipped,
}

async fn generate_thumbnail(
	thumbnails_directory: &Path,
	GenerateThumbnailArgs {
		extension,
		cas_id,
		path,
	}: &GenerateThumbnailArgs,
	kind: &ThumbnailKind,
	should_regenerate: bool,
) -> Result<(ThumbKey, GenerationStatus), NonCriticalError> {
	trace!("Generating thumbnail for {}", path.display());

	let mut output_path = match kind {
		ThumbnailKind::Ephemeral => thumbnails_directory.join(EPHEMERAL_DIR),
		ThumbnailKind::Indexed(library_id) => thumbnails_directory.join(library_id.to_string()),
	};

	output_path.push(get_shard_hex(cas_id));
	output_path.push(cas_id);
	output_path.set_extension(WEBP_EXTENSION);

	if let Err(e) = fs::metadata(&*output_path).await {
		if e.kind() != io::ErrorKind::NotFound {
			error!(
				"Failed to check if thumbnail exists, but we will try to generate it anyway: {e:#?}"
			);
		}
	// Otherwise we good, thumbnail doesn't exist so we can generate it
	} else if !should_regenerate {
		trace!(
			"Skipping thumbnail generation for {} because it already exists",
			path.display()
		);
		return Ok((ThumbKey::new(cas_id, kind), GenerationStatus::Skipped));
	}

	if let Ok(extension) = ImageExtension::from_str(extension) {
		if can_generate_thumbnail_for_image(extension) {
			generate_image_thumbnail(&path, &output_path).await?;
		}
	} else if let Ok(extension) = DocumentExtension::from_str(extension) {
		if can_generate_thumbnail_for_document(extension) {
			generate_image_thumbnail(&path, &output_path).await?;
		}
	}

	#[cfg(feature = "ffmpeg")]
	{
		use crate::media_processor::helpers::thumbnailer::can_generate_thumbnail_for_video;
		use sd_file_ext::extensions::VideoExtension;

		if let Ok(extension) = VideoExtension::from_str(extension) {
			if can_generate_thumbnail_for_video(extension) {
				generate_video_thumbnail(&path, &output_path).await?;
			}
		}
	}

	trace!("Generated thumbnail for {}", path.display());

	Ok((ThumbKey::new(cas_id, kind), GenerationStatus::Generated))
}

async fn generate_image_thumbnail(
	file_path: impl AsRef<Path> + Send,
	output_path: impl AsRef<Path> + Send,
) -> Result<(), NonCriticalError> {
	let file_path = file_path.as_ref().to_path_buf();

	let webp = spawn_blocking({
		let file_path = file_path.clone();

		move || -> Result<_, NonCriticalError> {
			let mut img = format_image(&file_path)
				.map_err(|e| NonCriticalError::FormatImage(file_path.clone(), e.to_string()))?;

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
			if let Some(orientation) = Orientation::from_path(&file_path) {
				if ConvertibleExtension::try_from(file_path.as_ref())
					.expect("we already checked if the image was convertible")
					.should_rotate()
				{
					img = orientation.correct_thumbnail(img);
				}
			}

			// Create the WebP encoder for the above image
			let encoder = Encoder::from_image(&img)
				.map_err(|reason| NonCriticalError::WebPEncoding(file_path, reason.to_string()))?;

			// Type `WebPMemory` is !Send, which makes the `Future` in this function `!Send`,
			// this make us `deref` to have a `&[u8]` and then `to_owned` to make a `Vec<u8>`
			// which implies on a unwanted clone...
			Ok(encoder.encode(TARGET_QUALITY).deref().to_owned())
		}
	})
	.await
	.map_err(|e| {
		NonCriticalError::PanicWhileGeneratingThumbnail(file_path.clone(), e.to_string())
	})??;

	let output_path = output_path.as_ref();

	if let Some(shard_dir) = output_path.parent() {
		fs::create_dir_all(shard_dir).await.map_err(|e| {
			NonCriticalError::CreateShardDirectory(FileIOError::from((shard_dir, e)).to_string())
		})?;
	} else {
		error!(
			"Failed to get parent directory of '{}' for sharding parent directory",
			output_path.display()
		);
	}

	fs::write(output_path, &webp).await.map_err(|e| {
		NonCriticalError::SaveThumbnail(file_path, FileIOError::from((output_path, e)).to_string())
	})
}

#[cfg(feature = "ffmpeg")]
async fn generate_video_thumbnail(
	file_path: impl AsRef<Path> + Send,
	output_path: impl AsRef<Path> + Send,
) -> Result<(), NonCriticalError> {
	use sd_ffmpeg::to_thumbnail;

	let file_path = file_path.as_ref();

	to_thumbnail(file_path, output_path, 256, TARGET_QUALITY)
		.await
		.map_err(|e| {
			NonCriticalError::VideoThumbnailGenerationFailed(file_path.to_path_buf(), e.to_string())
		})
}
