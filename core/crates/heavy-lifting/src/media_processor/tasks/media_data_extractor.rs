use crate::{media_processor, Error, NonCriticalError};

use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_prisma_helpers::file_path_for_media_processor;

use sd_file_ext::extensions::{Extension, ImageExtension, ALL_IMAGE_EXTENSIONS};
use sd_media_metadata::ImageMetadata;
use sd_prisma::prisma::{file_path, location, media_data, object, PrismaClient};
use sd_task_system::{
	check_interruption, ExecStatus, Interrupter, IntoAnyTaskOutput, Task, TaskId,
};

use std::{
	collections::{HashMap, HashSet},
	mem,
	path::{Path, PathBuf},
	pin::pin,
	sync::Arc,
	time::Duration,
};

use futures::StreamExt;
use futures_concurrency::future::FutureGroup;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tokio::{task::spawn_blocking, time::Instant};

use super::media_data_image_to_query;

#[derive(Debug)]
pub struct MediaDataExtractor {
	id: TaskId,
	file_paths: Vec<file_path_for_media_processor::Data>,
	location_id: location::id::Type,
	location_path: Arc<PathBuf>,
	stage: Stage,
	errors: Vec<NonCriticalError>,
	db: Arc<PrismaClient>,
	output: Output,
	is_shallow: bool,
}

#[derive(Debug, Serialize, Deserialize)]
enum Stage {
	Starting,
	FetchedObjectsAlreadyWithMediaData(Vec<object::id::Type>),
	ExtractingMediaData {
		paths_by_id: HashMap<file_path::id::Type, (Arc<PathBuf>, object::id::Type)>,
		// TODO: Change to support any kind of media data, not only images
		media_datas: Vec<(ImageMetadata, object::id::Type)>,
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
		let mut errors = Vec::new();

		Self {
			id: TaskId::new_v4(),
			file_paths: file_paths
				.iter()
				.filter(|file_path| {
					if file_path.object_id.is_some() {
						true
					} else {
						errors.push(
							media_processor::NonCriticalError::FilePathMissingObjectId(
								file_path.id,
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
			errors,
			db,
			output: Output::default(),
			is_shallow,
		}
	}

	pub fn new_deep(
		file_paths: &[file_path_for_media_processor::Data],
		location_id: location::id::Type,
		location_path: Arc<PathBuf>,
		db: Arc<PrismaClient>,
	) -> Self {
		Self::new(file_paths, location_id, location_path, db, false)
	}

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
		let Self {
			file_paths,
			location_id,
			location_path,
			stage,
			errors,
			db,
			output,
			..
		} = self;

		loop {
			match stage {
				Stage::Starting => {
					let db_read_start = Instant::now();
					let object_ids = fetch_objects_already_with_media_data(file_paths, db).await?;
					output.db_read_time = db_read_start.elapsed();

					*stage = Stage::FetchedObjectsAlreadyWithMediaData(object_ids);
				}

				Stage::FetchedObjectsAlreadyWithMediaData(objects_already_with_media_data) => {
					let filtering_start = Instant::now();
					if file_paths.len() == objects_already_with_media_data.len() {
						// All files already have media data, skipping
						#[allow(clippy::cast_possible_truncation)]
						{
							// SAFETY: we shouldn't have more than 4 billion unique objects already with media data
							output.skipped = file_paths.len() as u32;
						}
						break;
					}

					let unique_objects_already_with_media_data =
						mem::take(objects_already_with_media_data)
							.into_iter()
							.collect::<HashSet<_>>();

					#[allow(clippy::cast_possible_truncation)]
					{
						// SAFETY: we shouldn't have more than 4 billion unique objects already with media data
						output.skipped = unique_objects_already_with_media_data.len() as u32;
					}

					file_paths.retain(|file_path| {
						!unique_objects_already_with_media_data
							.contains(&file_path.object_id.expect("already checked"))
					});

					let paths_by_id = file_paths.iter().filter_map(|file_path| {
						IsolatedFilePathData::try_from((*location_id, file_path))
							.map_err(|e| errors.push(media_processor::NonCriticalError::FailedToConstructIsolatedFilePathData(file_path.id, e.to_string()).into()))
							.map(|iso_file_path| {
								(file_path.id, (Arc::new(location_path.join(iso_file_path)), file_path.object_id.expect("already checked")))
							}).ok()
					}).collect();

					output.filtering_time = filtering_start.elapsed();

					*stage = Stage::ExtractingMediaData {
						paths_by_id,
						media_datas: Vec::new(),
					};
				}

				Stage::ExtractingMediaData {
					paths_by_id,
					media_datas,
				} => {
					let extraction_start = Instant::now();

					let mut futures = pin!(paths_by_id
						.iter()
						.map(|(file_path_id, (path, object_id))| {
							// Copy the values to make them owned and make the borrowck happy
							let file_path_id = *file_path_id;
							let path = Arc::clone(path);
							let object_id = *object_id;

							async move { (extract_media_data(&*path).await, file_path_id, object_id) }
						})
						.collect::<FutureGroup<_>>());

					while let Some((res, file_path_id, object_id)) = futures.next().await {
						match res {
							Ok(Some(media_data)) => {
								media_datas.push((media_data, object_id));
							}
							Ok(None) => {
								// No media data found
								output.skipped += 1;
							}
							Err(e) => errors.push(e.into()),
						}

						paths_by_id.remove(&file_path_id);

						let extraction_time = &mut output.extraction_time;

						check_interruption!(interrupter, extraction_start, extraction_time);
					}

					*stage = Stage::SaveMediaData {
						media_datas: mem::take(media_datas),
					};
				}

				Stage::SaveMediaData { media_datas } => {
					let db_write_start = Instant::now();
					output.extracted = save_media_data(mem::take(media_datas), db).await?;

					output.db_write_time = db_write_start.elapsed();

					#[allow(clippy::cast_possible_truncation)]
					{
						// SAFETY: we shouldn't have more than 4 billion errors LMAO
						output.skipped += errors.len() as u32;
					}

					break;
				}
			}

			check_interruption!(interrupter);
		}

		Ok(ExecStatus::Done(mem::take(output).into_output()))
	}
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Output {
	pub extracted: u32,
	pub skipped: u32,
	pub db_read_time: Duration,
	pub filtering_time: Duration,
	pub extraction_time: Duration,
	pub db_write_time: Duration,
}

pub(super) static FILTERED_IMAGE_EXTENSIONS: Lazy<Vec<Extension>> = Lazy::new(|| {
	ALL_IMAGE_EXTENSIONS
		.iter()
		.copied()
		.filter(can_extract_media_data_for_image)
		.map(Extension::Image)
		.collect()
});

pub const fn can_extract_media_data_for_image(image_extension: &ImageExtension) -> bool {
	use ImageExtension::{
		Avci, Avcs, Avif, Dng, Heic, Heif, Heifs, Hif, Jpeg, Jpg, Png, Tiff, Webp,
	};
	matches!(
		image_extension,
		Tiff | Dng | Jpeg | Jpg | Heif | Heifs | Heic | Avif | Avcs | Avci | Hif | Png | Webp
	)
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
			Err(e) => Err(
				media_processor::NonCriticalError::FailedToExtractImageMediaData(
					path,
					e.to_string(),
				),
			),
		}
	})
	.await
	.map_err(|e| {
		media_processor::NonCriticalError::PanicWhileExtractingImageMediaData(path, e.to_string())
	})?
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
) -> Result<u32, media_processor::Error> {
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
			#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
			{
				// SAFETY: we can't create a negative amount of media_data and we won't create more than
				// 4 billion media_data entries
				created as u32
			}
		})
		.map_err(Into::into)
}
