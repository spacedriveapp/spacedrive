use crate::{
	media_processor::{
		self,
		helpers::{exif_media_data, ffmpeg_media_data},
	},
	Error,
};

use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_prisma_helpers::{file_path_for_media_processor, ObjectPubId};
use sd_core_sync::SyncManager;

use sd_media_metadata::{ExifMetadata, FFmpegMetadata};
use sd_prisma::prisma::{exif_data, ffmpeg_data, file_path, location, object, PrismaClient};
use sd_task_system::{
	check_interruption, ExecStatus, Interrupter, InterruptionKind, IntoAnyTaskOutput,
	SerializableTask, Task, TaskId,
};

use std::{
	collections::{HashMap, HashSet},
	future::{Future, IntoFuture},
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
use tracing::{debug, instrument, trace, Level};

#[derive(thiserror::Error, Debug, Serialize, Deserialize, Type, Clone)]
pub enum NonCriticalMediaDataExtractorError {
	#[error("failed to extract media data from <file='{}'>: {1}", .0.display())]
	FailedToExtractImageMediaData(PathBuf, String),
	#[error("file path missing object id: <file_path_id='{0}'>")]
	FilePathMissingObjectId(file_path::id::Type),
	#[error("failed to construct isolated file path data: <file_path_id='{0}'>: {1}")]
	FailedToConstructIsolatedFilePathData(file_path::id::Type, String),
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
enum Kind {
	Exif,
	FFmpeg,
}

#[derive(Debug)]
pub struct MediaDataExtractor {
	// Task control
	id: TaskId,
	kind: Kind,

	// Received input args
	file_paths: Vec<file_path_for_media_processor::Data>,
	location_id: location::id::Type,
	location_path: Arc<PathBuf>,

	// Inner state
	stage: Stage,

	// Out collector
	output: Output,

	// Dependencies
	db: Arc<PrismaClient>,
	sync: SyncManager,
}

#[derive(Debug, Serialize, Deserialize)]
enum Stage {
	Starting,
	FetchedObjectsAlreadyWithMediaData(Vec<object::id::Type>),
	ExtractingMediaData {
		paths_by_id: HashMap<file_path::id::Type, (PathBuf, object::id::Type, ObjectPubId)>,
		exif_media_datas: Vec<(ExifMetadata, object::id::Type, ObjectPubId)>,
		ffmpeg_media_datas: Vec<(FFmpegMetadata, object::id::Type)>,
		extract_ids_to_remove_from_map: Vec<file_path::id::Type>,
	},
	SaveMediaData {
		exif_media_datas: Vec<(ExifMetadata, object::id::Type, ObjectPubId)>,
		ffmpeg_media_datas: Vec<(FFmpegMetadata, object::id::Type)>,
	},
}

/// [`MediaDataExtractor`] task output
#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Output {
	/// How many files were successfully processed
	pub extracted: u64,
	/// How many files were skipped
	pub skipped: u64,
	/// Time spent reading data from database
	pub db_read_time: Duration,
	/// Time spent filtering files to extract media data and files to skip
	pub filtering_time: Duration,
	/// Time spent extracting media data
	pub extraction_time: Duration,
	/// Time spent writing media data to database
	pub db_write_time: Duration,
	/// Errors encountered during the task
	pub errors: Vec<crate::NonCriticalError>,
}

#[async_trait::async_trait]
impl Task<Error> for MediaDataExtractor {
	fn id(&self) -> TaskId {
		self.id
	}

	/// MediaDataExtractor never needs priority, as the data it generates are only accessed through
	/// the media inspector, so it isn't latency sensitive like other tasks, like FileIdentifier or
	/// the Thumbnailer
	fn with_priority(&self) -> bool {
		false
	}

	#[instrument(
		skip_all,
		fields(
			task_id = %self.id,
			kind = ?self.kind,
			location_id = %self.location_id,
			location_path = %self.location_path.display(),
			file_paths_count = %self.file_paths.len(),
		),
		ret(level = Level::TRACE),
		err,
	)]
	#[allow(clippy::blocks_in_conditions)] // Due to `err` on `instrument` macro above
	#[allow(clippy::too_many_lines)]
	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, Error> {
		loop {
			match &mut self.stage {
				Stage::Starting => {
					let db_read_start = Instant::now();
					let object_ids = fetch_objects_already_with_media_data(
						self.kind,
						&self.file_paths,
						&self.db,
					)
					.await?;
					self.output.db_read_time = db_read_start.elapsed();
					trace!(
						object_ids_count = object_ids.len(),
						"Fetched objects already with media data;",
					);

					self.stage = Stage::FetchedObjectsAlreadyWithMediaData(object_ids);
				}

				Stage::FetchedObjectsAlreadyWithMediaData(objects_already_with_media_data) => {
					if self.file_paths.len() == objects_already_with_media_data.len() {
						self.output.skipped = self.file_paths.len() as u64; // Files already have media data, skipping
						debug!("Skipped all files as they already have media data");
						break;
					}

					let filtering_start = Instant::now();
					let paths_by_id = filter_files_to_extract_media_data(
						mem::take(objects_already_with_media_data),
						self.location_id,
						&self.location_path,
						&mut self.file_paths,
						&mut self.output,
					);
					self.output.filtering_time = filtering_start.elapsed();

					trace!(
						paths_needing_media_data_extraction_count = paths_by_id.len(),
						"Filtered files to extract media data;",
					);

					self.stage = Stage::ExtractingMediaData {
						extract_ids_to_remove_from_map: Vec::with_capacity(paths_by_id.len()),
						exif_media_datas: if self.kind == Kind::Exif {
							Vec::with_capacity(paths_by_id.len())
						} else {
							Vec::new()
						},
						ffmpeg_media_datas: if self.kind == Kind::FFmpeg {
							Vec::with_capacity(paths_by_id.len())
						} else {
							Vec::new()
						},
						paths_by_id,
					};
				}

				Stage::ExtractingMediaData {
					paths_by_id,
					exif_media_datas,
					ffmpeg_media_datas,
					extract_ids_to_remove_from_map,
				} => {
					{
						// This inner scope is necessary to appease the mighty borrowck
						let extraction_start = Instant::now();
						for id in extract_ids_to_remove_from_map.drain(..) {
							paths_by_id.remove(&id);
						}

						let mut futures = pin!(prepare_extraction_futures(
							self.kind,
							paths_by_id,
							interrupter
						));

						while let Some(race_output) = futures.next().await {
							match race_output {
								InterruptRace::Processed(out) => {
									process_output(
										out,
										exif_media_datas,
										ffmpeg_media_datas,
										extract_ids_to_remove_from_map,
										&mut self.output,
									);
								}

								InterruptRace::Interrupted(kind) => {
									self.output.extraction_time += extraction_start.elapsed();
									return Ok(match kind {
										InterruptionKind::Pause => ExecStatus::Paused,
										InterruptionKind::Cancel => ExecStatus::Canceled,
									});
								}
							}
						}
					}

					self.stage = Stage::SaveMediaData {
						exif_media_datas: mem::take(exif_media_datas),
						ffmpeg_media_datas: mem::take(ffmpeg_media_datas),
					};
				}

				Stage::SaveMediaData {
					exif_media_datas,
					ffmpeg_media_datas,
				} => {
					let db_write_start = Instant::now();
					self.output.extracted = save(
						self.kind,
						exif_media_datas,
						ffmpeg_media_datas,
						&self.db,
						&self.sync,
					)
					.await?;
					self.output.db_write_time = db_write_start.elapsed();

					self.output.skipped += self.output.errors.len() as u64;

					break;
				}
			}

			check_interruption!(interrupter);
		}

		Ok(ExecStatus::Done(mem::take(&mut self.output).into_output()))
	}
}

impl MediaDataExtractor {
	fn new(
		kind: Kind,
		file_paths: &[file_path_for_media_processor::Data],
		location_id: location::id::Type,
		location_path: Arc<PathBuf>,
		db: Arc<PrismaClient>,
		sync: SyncManager,
	) -> Self {
		let mut output = Output::default();

		Self {
			id: TaskId::new_v4(),
			kind,
			file_paths: file_paths
				.iter()
				.filter(|file_path| {
					if file_path.object.is_some() {
						true
					} else {
						output.errors.push(
							media_processor::NonCriticalMediaProcessorError::from(
								NonCriticalMediaDataExtractorError::FilePathMissingObjectId(
									file_path.id,
								),
							)
							.into(),
						);
						false
					}
				})
				.cloned()
				.collect(),
			location_id,
			location_path,
			stage: Stage::Starting,
			db,
			sync,
			output,
		}
	}

	#[must_use]
	pub fn new_exif(
		file_paths: &[file_path_for_media_processor::Data],
		location_id: location::id::Type,
		location_path: Arc<PathBuf>,
		db: Arc<PrismaClient>,
		sync: SyncManager,
	) -> Self {
		Self::new(Kind::Exif, file_paths, location_id, location_path, db, sync)
	}

	#[must_use]
	pub fn new_ffmpeg(
		file_paths: &[file_path_for_media_processor::Data],
		location_id: location::id::Type,
		location_path: Arc<PathBuf>,
		db: Arc<PrismaClient>,
		sync: SyncManager,
	) -> Self {
		Self::new(
			Kind::FFmpeg,
			file_paths,
			location_id,
			location_path,
			db,
			sync,
		)
	}
}

#[inline]
async fn fetch_objects_already_with_media_data(
	kind: Kind,
	file_paths: &[file_path_for_media_processor::Data],
	db: &PrismaClient,
) -> Result<Vec<object::id::Type>, media_processor::Error> {
	let object_ids = file_paths
		.iter()
		.filter_map(|file_path| file_path.object.as_ref().map(|object| object.id))
		.collect();

	match kind {
		Kind::Exif => db
			.exif_data()
			.find_many(vec![exif_data::object_id::in_vec(object_ids)])
			.select(exif_data::select!({ object_id }))
			.exec()
			.await
			.map(|object_ids| object_ids.into_iter().map(|data| data.object_id).collect())
			.map_err(Into::into),

		Kind::FFmpeg => db
			.ffmpeg_data()
			.find_many(vec![ffmpeg_data::object_id::in_vec(object_ids)])
			.select(ffmpeg_data::select!({ object_id }))
			.exec()
			.await
			.map(|object_ids| object_ids.into_iter().map(|data| data.object_id).collect())
			.map_err(Into::into),
	}
}

#[inline]
fn filter_files_to_extract_media_data(
	objects_already_with_media_data: Vec<object::id::Type>,
	location_id: location::id::Type,
	location_path: &Path,
	file_paths: &mut Vec<file_path_for_media_processor::Data>,
	Output {
		skipped, errors, ..
	}: &mut Output,
) -> HashMap<file_path::id::Type, (PathBuf, object::id::Type, ObjectPubId)> {
	let unique_objects_already_with_media_data = objects_already_with_media_data
		.into_iter()
		.collect::<HashSet<_>>();

	*skipped = unique_objects_already_with_media_data.len() as u64;

	file_paths.retain(|file_path| {
		!unique_objects_already_with_media_data
			.contains(&file_path.object.as_ref().expect("already checked").id)
	});

	file_paths
		.iter()
		.filter_map(|file_path| {
			IsolatedFilePathData::try_from((location_id, file_path))
				.map_err(|e| {
					errors.push(
						media_processor::NonCriticalMediaProcessorError::from(
							NonCriticalMediaDataExtractorError::FailedToConstructIsolatedFilePathData(
								file_path.id,
								e.to_string(),
							),
						)
						.into(),
					);
				})
				.map(|iso_file_path| {
					let object = file_path.object.as_ref().expect("already checked");

					(
						file_path.id,
						(
							location_path.join(iso_file_path),
							object.id,
							object.pub_id.as_slice().into(),
						),
					)
				})
				.ok()
		})
		.collect()
}

enum ExtractionOutputKind {
	Exif(Result<Option<ExifMetadata>, media_processor::NonCriticalMediaProcessorError>),
	FFmpeg(Result<FFmpegMetadata, media_processor::NonCriticalMediaProcessorError>),
}

struct ExtractionOutput {
	file_path_id: file_path::id::Type,
	object_id: object::id::Type,
	object_pub_id: ObjectPubId,
	kind: ExtractionOutputKind,
}

#[allow(clippy::large_enum_variant)]
/*
 * NOTE(fogodev): Interrupts will be pretty rare, so paying the boxing price for
 * the Processed variant isn't worth it to avoid the enum size disparity between variants
 */
enum InterruptRace {
	Interrupted(InterruptionKind),
	Processed(ExtractionOutput),
}

#[inline]
fn prepare_extraction_futures<'a>(
	kind: Kind,
	paths_by_id: &'a HashMap<file_path::id::Type, (PathBuf, object::id::Type, ObjectPubId)>,
	interrupter: &'a Interrupter,
) -> FuturesUnordered<impl Future<Output = InterruptRace> + 'a> {
	paths_by_id
		.iter()
		.map(
			|(file_path_id, (path, object_id, object_pub_id))| async move {
				InterruptRace::Processed(ExtractionOutput {
					file_path_id: *file_path_id,
					object_id: *object_id,
					object_pub_id: object_pub_id.clone(),
					kind: match kind {
						Kind::Exif => {
							ExtractionOutputKind::Exif(exif_media_data::extract(path).await)
						}
						Kind::FFmpeg => {
							ExtractionOutputKind::FFmpeg(ffmpeg_media_data::extract(path).await)
						}
					},
				})
			},
		)
		.map(|fut| {
			(
				fut,
				interrupter.into_future().map(InterruptRace::Interrupted),
			)
				.race()
		})
		.collect::<FuturesUnordered<_>>()
}

#[instrument(skip_all, fields(%file_path_id, %object_id))]
#[inline]
fn process_output(
	ExtractionOutput {
		file_path_id,
		object_id,
		object_pub_id,
		kind,
	}: ExtractionOutput,
	exif_media_datas: &mut Vec<(ExifMetadata, object::id::Type, ObjectPubId)>,
	ffmpeg_media_datas: &mut Vec<(FFmpegMetadata, object::id::Type)>,
	extract_ids_to_remove_from_map: &mut Vec<file_path::id::Type>,
	output: &mut Output,
) {
	trace!("Processing extracted media data");

	match kind {
		ExtractionOutputKind::Exif(Ok(Some(exif_data))) => {
			exif_media_datas.push((exif_data, object_id, object_pub_id));
		}
		ExtractionOutputKind::Exif(Ok(None)) => {
			// No exif media data found
			output.skipped += 1;
		}
		ExtractionOutputKind::FFmpeg(Ok(ffmpeg_data)) => {
			ffmpeg_media_datas.push((ffmpeg_data, object_id));
		}
		ExtractionOutputKind::Exif(Err(e)) | ExtractionOutputKind::FFmpeg(Err(e)) => {
			output.errors.push(e.into());
		}
	}

	extract_ids_to_remove_from_map.push(file_path_id);
}

#[inline]
async fn save(
	kind: Kind,
	exif_media_datas: &mut Vec<(ExifMetadata, object::id::Type, ObjectPubId)>,
	ffmpeg_media_datas: &mut Vec<(FFmpegMetadata, object::id::Type)>,
	db: &PrismaClient,
	sync: &SyncManager,
) -> Result<u64, media_processor::Error> {
	trace!("Saving media data on database");

	match kind {
		Kind::Exif => exif_media_data::save(mem::take(exif_media_datas), db, sync).await,
		Kind::FFmpeg => ffmpeg_media_data::save(mem::take(ffmpeg_media_datas), db).await,
	}
	.map_err(Into::into)
}

#[derive(Debug, Serialize, Deserialize)]
struct SaveState {
	id: TaskId,
	kind: Kind,
	file_paths: Vec<file_path_for_media_processor::Data>,
	location_id: location::id::Type,
	location_path: Arc<PathBuf>,
	stage: Stage,
	output: Output,
}

impl SerializableTask<Error> for MediaDataExtractor {
	type SerializeError = rmp_serde::encode::Error;

	type DeserializeError = rmp_serde::decode::Error;

	type DeserializeCtx = (Arc<PrismaClient>, SyncManager);

	async fn serialize(self) -> Result<Vec<u8>, Self::SerializeError> {
		let Self {
			id,
			kind,
			file_paths,
			location_id,
			location_path,
			stage,
			output,
			..
		} = self;

		rmp_serde::to_vec_named(&SaveState {
			id,
			kind,
			file_paths,
			location_id,
			location_path,
			stage,
			output,
		})
	}

	async fn deserialize(
		data: &[u8],
		(db, sync): Self::DeserializeCtx,
	) -> Result<Self, Self::DeserializeError> {
		rmp_serde::from_slice(data).map(
			|SaveState {
			     id,
			     kind,
			     file_paths,
			     location_id,
			     location_path,
			     stage,
			     output,
			 }| Self {
				id,
				kind,
				file_paths,
				location_id,
				location_path,
				stage,
				output,
				db,
				sync,
			},
		)
	}
}
