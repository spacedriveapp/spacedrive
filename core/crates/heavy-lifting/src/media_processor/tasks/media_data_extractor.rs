use crate::{
	media_processor::{self, helpers::media_data_extractor::media_data_image_to_query},
	Error,
};

use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_prisma_helpers::file_path_for_media_processor;

use sd_media_metadata::ImageMetadata;
use sd_prisma::prisma::{file_path, location, media_data, object, PrismaClient};
use sd_task_system::{
	check_interruption, ExecStatus, Interrupter, InterruptionKind, IntoAnyTaskOutput,
	SerializableTask, Task, TaskId,
};

use std::{
	collections::{HashMap, HashSet},
	future::IntoFuture,
	mem,
	path::{Path, PathBuf},
	pin::pin,
	sync::Arc,
	time::Duration,
};

use futures::{FutureExt, StreamExt};
use futures_concurrency::future::{FutureGroup, Race};
use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::{task::spawn_blocking, time::Instant};

#[derive(Debug)]
pub struct MediaDataExtractor {
	id: TaskId,
	file_paths: Vec<file_path_for_media_processor::Data>,
	location_id: location::id::Type,
	location_path: Arc<PathBuf>,
	stage: Stage,
	db: Arc<PrismaClient>,
	output: Output,
	is_shallow: bool,
}

#[derive(Debug, Serialize, Deserialize)]
enum Stage {
	Starting,
	FetchedObjectsAlreadyWithMediaData(Vec<object::id::Type>),
	ExtractingMediaData {
		paths_by_id: HashMap<file_path::id::Type, (PathBuf, object::id::Type)>,
		// TODO: Change to support any kind of media data, not only images
		media_datas: Vec<(ImageMetadata, object::id::Type)>,
		extract_ids_to_remove_from_map: Vec<file_path::id::Type>,
	},
	SaveMediaData {
		media_datas: Vec<(ImageMetadata, object::id::Type)>,
	},
}

impl MediaDataExtractor {
	fn new(
		file_paths: &[file_path_for_media_processor::Data],
		location_id: location::id::Type,
		location_path: Arc<PathBuf>,
		db: Arc<PrismaClient>,
		is_shallow: bool,
	) -> Self {
		let mut output = Output::default();

		Self {
			id: TaskId::new_v4(),
			file_paths: file_paths
				.iter()
				.filter(|file_path| {
					if file_path.object_id.is_some() {
						true
					} else {
						output.errors.push(
							media_processor::NonCriticalError::from(
								NonCriticalError::FilePathMissingObjectId(file_path.id),
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
			output,
			is_shallow,
		}
	}

	#[must_use]
	pub fn new_deep(
		file_paths: &[file_path_for_media_processor::Data],
		location_id: location::id::Type,
		location_path: Arc<PathBuf>,
		db: Arc<PrismaClient>,
	) -> Self {
		Self::new(file_paths, location_id, location_path, db, false)
	}

	#[must_use]
	pub fn new_shallow(
		file_paths: &[file_path_for_media_processor::Data],
		location_id: location::id::Type,
		location_path: Arc<PathBuf>,
		db: Arc<PrismaClient>,
	) -> Self {
		Self::new(file_paths, location_id, location_path, db, true)
	}
}

#[async_trait::async_trait]
impl Task<Error> for MediaDataExtractor {
	fn id(&self) -> TaskId {
		self.id
	}

	fn with_priority(&self) -> bool {
		self.is_shallow
	}

	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, Error> {
		loop {
			match &mut self.stage {
				Stage::Starting => {
					let db_read_start = Instant::now();
					let object_ids =
						fetch_objects_already_with_media_data(&self.file_paths, &self.db).await?;
					self.output.db_read_time = db_read_start.elapsed();

					self.stage = Stage::FetchedObjectsAlreadyWithMediaData(object_ids);
				}

				Stage::FetchedObjectsAlreadyWithMediaData(objects_already_with_media_data) => {
					let filtering_start = Instant::now();
					if self.file_paths.len() == objects_already_with_media_data.len() {
						// All files already have media data, skipping
						self.output.skipped = self.file_paths.len() as u64;

						break;
					}
					let paths_by_id = filter_files_to_extract_media_data(
						mem::take(objects_already_with_media_data),
						self.location_id,
						&self.location_path,
						&mut self.file_paths,
						&mut self.output,
					);

					self.output.filtering_time = filtering_start.elapsed();

					self.stage = Stage::ExtractingMediaData {
						extract_ids_to_remove_from_map: Vec::with_capacity(paths_by_id.len()),
						media_datas: Vec::with_capacity(paths_by_id.len()),
						paths_by_id,
					};
				}

				Stage::ExtractingMediaData {
					paths_by_id,
					media_datas,
					extract_ids_to_remove_from_map,
				} => {
					{
						// This inner scope is necessary to appease the mighty borrowck
						let extraction_start = Instant::now();
						for id in extract_ids_to_remove_from_map.drain(..) {
							paths_by_id.remove(&id);
						}

						let futures = paths_by_id
							.iter()
							.map(|(file_path_id, (path, object_id))| {
								extract_media_data(path)
									.map(|res| (res, *file_path_id, *object_id))
									.map(InterruptRace::Processed)
							})
							.map(|fut| {
								(
									fut,
									interrupter.into_future().map(InterruptRace::Interrupted),
								)
									.race()
							})
							.collect::<FutureGroup<_>>();

						let mut futures = pin!(futures);

						while let Some(race_output) = futures.next().await {
							match race_output {
								InterruptRace::Processed((res, file_path_id, object_id)) => {
									match res {
										Ok(Some(media_data)) => {
											media_datas.push((media_data, object_id));
										}
										Ok(None) => {
											// No media data found
											self.output.skipped += 1;
										}
										Err(e) => self.output.errors.push(e.into()),
									}

									extract_ids_to_remove_from_map.push(file_path_id);
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
						media_datas: mem::take(media_datas),
					};
				}

				Stage::SaveMediaData { media_datas } => {
					let db_write_start = Instant::now();
					self.output.extracted =
						save_media_data(mem::take(media_datas), &self.db).await?;
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

type ExtractionOutput = (
	Result<Option<ImageMetadata>, media_processor::NonCriticalError>,
	file_path::id::Type,
	object::id::Type,
);

#[allow(clippy::large_enum_variant)]
/*
 * NOTE(fogodev): Interrupts will be pretty rare, so paying the boxing price for
 * the Processed variant isn't worth it to avoid the enum size disparity between variants
 */
enum InterruptRace {
	Interrupted(InterruptionKind),
	Processed(ExtractionOutput),
}

#[derive(thiserror::Error, Debug, Serialize, Deserialize, Type)]
pub enum NonCriticalError {
	#[error("failed to extract media data from <image='{}'>: {1}", .0.display())]
	FailedToExtractImageMediaData(PathBuf, String),
	#[error("processing thread panicked while extracting media data from <image='{}'>: {1}", .0.display())]
	PanicWhileExtractingImageMediaData(PathBuf, String),
	#[error("file path missing object id: <file_path_id='{0}'>")]
	FilePathMissingObjectId(file_path::id::Type),
	#[error("failed to construct isolated file path data: <file_path_id='{0}'>: {1}")]
	FailedToConstructIsolatedFilePathData(file_path::id::Type, String),
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Output {
	pub extracted: u64,
	pub skipped: u64,
	pub db_read_time: Duration,
	pub filtering_time: Duration,
	pub extraction_time: Duration,
	pub db_write_time: Duration,
	pub errors: Vec<crate::NonCriticalError>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SaveState {
	id: TaskId,
	file_paths: Vec<file_path_for_media_processor::Data>,
	location_id: location::id::Type,
	location_path: Arc<PathBuf>,
	stage: Stage,
	output: Output,
	is_shallow: bool,
}

impl SerializableTask<Error> for MediaDataExtractor {
	type SerializeError = rmp_serde::encode::Error;

	type DeserializeError = rmp_serde::decode::Error;

	type DeserializeCtx = Arc<PrismaClient>;

	async fn serialize(self) -> Result<Vec<u8>, Self::SerializeError> {
		let Self {
			id,
			file_paths,
			location_id,
			location_path,
			stage,
			output,
			is_shallow,
			..
		} = self;

		rmp_serde::to_vec_named(&SaveState {
			id,
			file_paths,
			location_id,
			location_path,
			stage,
			output,
			is_shallow,
		})
	}

	async fn deserialize(
		data: &[u8],
		db: Self::DeserializeCtx,
	) -> Result<Self, Self::DeserializeError> {
		rmp_serde::from_slice(data).map(
			|SaveState {
			     id,
			     file_paths,
			     location_id,
			     location_path,
			     stage,
			     output,
			     is_shallow,
			 }| Self {
				id,
				file_paths,
				location_id,
				location_path,
				stage,
				db,
				output,
				is_shallow,
			},
		)
	}
}

pub async fn extract_media_data(
	path: impl AsRef<Path> + Send,
) -> Result<Option<ImageMetadata>, media_processor::NonCriticalError> {
	let path = path.as_ref().to_path_buf();

	// Running in a separated blocking thread due to MediaData blocking behavior (due to sync exif lib)
	spawn_blocking({
		let path = path.clone();
		|| match ImageMetadata::from_path(&path) {
			Ok(media_data) => Ok(Some(media_data)),
			Err(sd_media_metadata::Error::NoExifDataOnPath(_)) => Ok(None),
			Err(e) => {
				Err(NonCriticalError::FailedToExtractImageMediaData(path, e.to_string()).into())
			}
		}
	})
	.await
	.map_err(|e| NonCriticalError::PanicWhileExtractingImageMediaData(path, e.to_string()))?
}

async fn fetch_objects_already_with_media_data(
	file_paths: &[file_path_for_media_processor::Data],
	db: &PrismaClient,
) -> Result<Vec<object::id::Type>, media_processor::Error> {
	db.media_data()
		.find_many(vec![media_data::object_id::in_vec(
			file_paths
				.iter()
				.filter_map(|file_path| file_path.object_id)
				.collect(),
		)])
		.select(media_data::select!({ object_id }))
		.exec()
		.await
		.map(|object_ids| object_ids.into_iter().map(|data| data.object_id).collect())
		.map_err(Into::into)
}

async fn save_media_data(
	media_datas: Vec<(ImageMetadata, object::id::Type)>,
	db: &PrismaClient,
) -> Result<u64, media_processor::Error> {
	db.media_data()
		.create_many(
			media_datas
				.into_iter()
				.map(|(media_data, object_id)| media_data_image_to_query(media_data, object_id))
				.collect(),
		)
		.skip_duplicates()
		.exec()
		.await
		.map(|created| {
			#[allow(clippy::cast_sign_loss)]
			{
				created as u64
			}
		})
		.map_err(Into::into)
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
) -> HashMap<file_path::id::Type, (PathBuf, object::id::Type)> {
	let unique_objects_already_with_media_data = objects_already_with_media_data
		.into_iter()
		.collect::<HashSet<_>>();

	*skipped = unique_objects_already_with_media_data.len() as u64;

	file_paths.retain(|file_path| {
		!unique_objects_already_with_media_data
			.contains(&file_path.object_id.expect("already checked"))
	});

	file_paths
		.iter()
		.filter_map(|file_path| {
			IsolatedFilePathData::try_from((location_id, file_path))
				.map_err(|e| {
					errors.push(
						media_processor::NonCriticalError::from(
							NonCriticalError::FailedToConstructIsolatedFilePathData(
								file_path.id,
								e.to_string(),
							),
						)
						.into(),
					);
				})
				.map(|iso_file_path| {
					(
						file_path.id,
						(
							location_path.join(iso_file_path),
							file_path.object_id.expect("already checked"),
						),
					)
				})
				.ok()
		})
		.collect()
}
