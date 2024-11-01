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
			generate_thumbnail, GenerateThumbnailArgs, GenerationStatus, THUMBNAILER_TASK_TIMEOUT,
		},
		ThumbKey, ThumbnailKind,
	},
	Error,
};

use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_prisma_helpers::{file_path_for_media_processor, CasId};

use sd_prisma::prisma::{file_path, location};
use sd_task_system::{
	ExecStatus, Interrupter, InterruptionKind, IntoAnyTaskOutput, SerializableTask, Task, TaskId,
};

use std::{
	collections::HashMap,
	fmt,
	future::IntoFuture,
	mem,
	path::{Path, PathBuf},
	pin::pin,
	sync::Arc,
	time::Duration,
};

use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use futures_concurrency::future::Race;
use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::time::Instant;
use tracing::{error, instrument, trace, Level};
use uuid::Uuid;

pub type ThumbnailId = u32;

pub trait NewThumbnailReporter: Send + Sync + fmt::Debug + 'static {
	fn new_thumbnail(&self, thumb_key: ThumbKey);
}

#[derive(Debug)]
pub struct Thumbnailer {
	// Task control
	id: TaskId,
	with_priority: bool,

	// Received input args
	thumbs_kind: ThumbnailKind,
	thumbnails_directory_path: Arc<PathBuf>,
	thumbnails_to_generate: HashMap<ThumbnailId, GenerateThumbnailArgs<'static>>,
	should_regenerate: bool,

	// Inner state
	already_processed_ids: Vec<ThumbnailId>,

	// Out collector
	output: Output,

	// Dependencies
	reporter: Arc<dyn NewThumbnailReporter>,
}

#[async_trait::async_trait]
impl Task<Error> for Thumbnailer {
	fn id(&self) -> TaskId {
		self.id
	}

	fn with_priority(&self) -> bool {
		self.with_priority
	}

	fn with_timeout(&self) -> Option<Duration> {
		Some(THUMBNAILER_TASK_TIMEOUT) // The entire task must not take more than this constant
	}

	#[instrument(
		skip_all,
		fields(
			task_id = %self.id,
			thumbs_kind = ?self.thumbs_kind,
			should_regenerate = self.should_regenerate,
			thumbnails_to_generate_count = self.thumbnails_to_generate.len(),
			already_processed_ids_count = self.already_processed_ids.len(),
			with_priority = self.with_priority,
		),
		ret(level = Level::TRACE),
		err,
	)]
	#[allow(clippy::blocks_in_conditions)] // Due to `err` on `instrument` macro above
	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, Error> {
		enum InterruptRace {
			Interrupted(InterruptionKind),
			Processed(ThumbnailGenerationOutput),
		}

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

		let start = Instant::now();

		let futures = thumbnails_to_generate
			.iter()
			.map(|(id, generate_args)| {
				generate_thumbnail(
					thumbnails_directory_path,
					generate_args,
					thumbs_kind,
					*should_regenerate,
				)
				.map(|res| InterruptRace::Processed((*id, res)))
			})
			.map(|fut| {
				(
					fut,
					interrupter.into_future().map(InterruptRace::Interrupted),
				)
					.race()
			})
			.collect::<FuturesUnordered<_>>();

		let mut futures = pin!(futures);

		while let Some(race_output) = futures.next().await {
			match race_output {
				InterruptRace::Processed(out) => process_thumbnail_generation_output(
					out,
					*with_priority,
					reporter.as_ref(),
					already_processed_ids,
					output,
				),

				InterruptRace::Interrupted(kind) => {
					output.total_time += start.elapsed();
					return Ok(match kind {
						InterruptionKind::Pause => ExecStatus::Paused,
						InterruptionKind::Cancel => ExecStatus::Canceled,
					});
				}
			}
		}

		output.total_time += start.elapsed();

		if output.generated > 1 {
			#[allow(clippy::cast_precision_loss)]
			// SAFETY: we're probably won't have 2^52 thumbnails being generated on a single task for this cast to have
			// a precision loss issue
			let total = (output.generated + output.skipped) as f64;
			let mean_generation_time_f64 = output.mean_time_acc / total;

			trace!(
				generated = output.generated,
				skipped = output.skipped,
				"mean generation time: {mean_generation_time:?} ± {generation_time_std_dev:?};",
				mean_generation_time = Duration::from_secs_f64(mean_generation_time_f64),
				generation_time_std_dev = Duration::from_secs_f64(
					(mean_generation_time_f64
						.mul_add(-mean_generation_time_f64, output.std_dev_acc / total))
					.sqrt(),
				)
			);
		}

		Ok(ExecStatus::Done(mem::take(output).into_output()))
	}
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Output {
	pub generated: u64,
	pub skipped: u64,
	pub errors: Vec<crate::NonCriticalError>,
	pub total_time: Duration,
	pub mean_time_acc: f64,
	pub std_dev_acc: f64,
}

#[derive(thiserror::Error, Debug, Serialize, Deserialize, Type, Clone)]
pub enum NonCriticalThumbnailerError {
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
	#[error("task timed out: {0}")]
	TaskTimeout(TaskId),
}

impl Thumbnailer {
	fn new(
		thumbs_kind: ThumbnailKind,
		thumbnails_directory_path: Arc<PathBuf>,
		thumbnails_to_generate: HashMap<ThumbnailId, GenerateThumbnailArgs<'static>>,
		errors: Vec<crate::NonCriticalError>,
		should_regenerate: bool,
		with_priority: bool,
		reporter: Arc<dyn NewThumbnailReporter>,
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
		thumbnails_to_generate: Vec<GenerateThumbnailArgs<'static>>,
		reporter: Arc<dyn NewThumbnailReporter>,
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
		reporter: Arc<dyn NewThumbnailReporter>,
	) -> Self {
		let mut errors = Vec::new();

		Self::new(
			ThumbnailKind::Indexed(library_id),
			thumbnails_directory_path,
			file_paths
				.iter()
				.filter_map(|file_path| {
					if let Some(cas_id) = file_path
						.cas_id
						.as_ref()
						.map(CasId::from)
						.map(CasId::into_owned)
					{
						let file_path_id = file_path.id;
						IsolatedFilePathData::try_from((location_id, file_path))
							.map_err(|e| {
								errors.push(
									media_processor::NonCriticalMediaProcessorError::from(
										NonCriticalThumbnailerError::FailedToExtractIsolatedFilePathData(
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
							media_processor::NonCriticalMediaProcessorError::from(
								NonCriticalThumbnailerError::MissingCasId(file_path.id),
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
								cas_id,
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

#[instrument(skip_all, fields(thumb_id = id, %generated, %skipped, ?elapsed_time, ?res))]
fn process_thumbnail_generation_output(
	(id, (elapsed_time, res)): ThumbnailGenerationOutput,
	with_priority: bool,
	reporter: &dyn NewThumbnailReporter,
	already_processed_ids: &mut Vec<ThumbnailId>,
	Output {
		generated,
		skipped,
		errors,
		mean_time_acc: mean_generation_time_accumulator,
		std_dev_acc: std_dev_accumulator,
		..
	}: &mut Output,
) {
	let elapsed_time = elapsed_time.as_secs_f64();
	*mean_generation_time_accumulator += elapsed_time;
	*std_dev_accumulator += elapsed_time * elapsed_time;

	match res {
		Ok((thumb_key, status)) => {
			match status {
				GenerationStatus::Generated => {
					*generated += 1;
					// This if is REALLY needed, due to the sheer performance of the thumbnailer,
					// I restricted to only send events notifying for thumbnails in the current
					// opened directory, sending events for the entire location turns into a
					// humongous bottleneck in the frontend lol, since it doesn't even knows
					// what to do with thumbnails for inner directories lol
					// - fogodev
					if with_priority {
						reporter.new_thumbnail(thumb_key);
					}
				}
				GenerationStatus::Skipped => {
					*skipped += 1;
				}
			}
		}
		Err(e) => {
			errors.push(media_processor::NonCriticalMediaProcessorError::from(e).into());
			*skipped += 1;
		}
	}

	already_processed_ids.push(id);

	trace!("Thumbnail processed");
}

#[derive(Debug, Serialize, Deserialize)]
struct SaveState {
	id: TaskId,
	thumbs_kind: ThumbnailKind,
	thumbnails_directory_path: Arc<PathBuf>,
	thumbnails_to_generate: HashMap<ThumbnailId, GenerateThumbnailArgs<'static>>,
	should_regenerate: bool,
	with_priority: bool,
	output: Output,
}

impl SerializableTask<Error> for Thumbnailer {
	type SerializeError = rmp_serde::encode::Error;

	type DeserializeError = rmp_serde::decode::Error;

	type DeserializeCtx = Arc<dyn NewThumbnailReporter>;

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
			     thumbnails_to_generate,
			     thumbnails_directory_path,
			     should_regenerate,
			     with_priority,
			     output,
			 }| Self {
				id,
				reporter,
				thumbs_kind,
				thumbnails_to_generate,
				thumbnails_directory_path,
				already_processed_ids: Vec::new(),
				should_regenerate,
				with_priority,
				output,
			},
		)
	}
}

type ThumbnailGenerationOutput = (
	ThumbnailId,
	(
		Duration,
		Result<(ThumbKey, GenerationStatus), NonCriticalThumbnailerError>,
	),
);
