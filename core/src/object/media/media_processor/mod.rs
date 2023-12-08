use crate::job::{JobRunErrors, JobRunMetadata};

#[cfg(feature = "skynet")]
use crate::{invalidate_query, library::Library};

use sd_file_path_helper::{file_path_for_media_processor, FilePathError};
use sd_prisma::prisma::{location, PrismaClient};

#[cfg(feature = "skynet")]
use sd_prisma::prisma::{label, label_on_object, object};

use std::path::Path;

#[cfg(feature = "skynet")]
use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::error;

use super::{
	media_data_extractor::{self, MediaDataError, MediaDataExtractorMetadata},
	thumbnail::{self, BatchToProcess, ThumbnailerError},
};

mod job;
mod shallow;

pub use job::MediaProcessorJobInit;
pub use shallow::shallow;

#[derive(Error, Debug)]
pub enum MediaProcessorError {
	#[error("sub path not found: <path='{}'>", .0.display())]
	SubPathNotFound(Box<Path>),

	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error(transparent)]
	FilePath(#[from] FilePathError),

	#[error(transparent)]
	Thumbnailer(#[from] ThumbnailerError),
	#[error(transparent)]
	MediaDataExtractor(#[from] MediaDataError),
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct MediaProcessorMetadata {
	media_data: MediaDataExtractorMetadata,
	thumbs_processed: u32,
	labels_extracted: u32,
}

impl From<MediaDataExtractorMetadata> for MediaProcessorMetadata {
	fn from(media_data: MediaDataExtractorMetadata) -> Self {
		Self {
			media_data,
			thumbs_processed: 0,
			labels_extracted: 0,
		}
	}
}

impl JobRunMetadata for MediaProcessorMetadata {
	fn update(&mut self, new_data: Self) {
		self.media_data.extracted += new_data.media_data.extracted;
		self.media_data.skipped += new_data.media_data.skipped;
		self.thumbs_processed += new_data.thumbs_processed;
		self.labels_extracted += new_data.labels_extracted;
	}
}

pub async fn process(
	files_paths: &[file_path_for_media_processor::Data],
	location_id: location::id::Type,
	location_path: impl AsRef<Path>,
	db: &PrismaClient,
	ctx_update_fn: &impl Fn(usize),
) -> Result<(MediaProcessorMetadata, JobRunErrors), MediaProcessorError> {
	// Add here new kinds of media processing if necessary in the future

	media_data_extractor::process(files_paths, location_id, location_path, db, ctx_update_fn)
		.await
		.map(|(media_data, errors)| (media_data.into(), errors))
		.map_err(Into::into)
}

#[cfg(feature = "skynet")]
pub async fn assign_labels(
	object_id: object::id::Type,
	mut labels: HashSet<String>,
	library @ Library { db, .. }: &Library,
) -> Result<(), prisma_client_rust::QueryError> {
	use chrono::{DateTime, FixedOffset, Utc};
	use uuid::Uuid;

	let mut labels_ids = db
		.label()
		.find_many(vec![label::name::in_vec(labels.iter().cloned().collect())])
		.select(label::select!({ id name }))
		.exec()
		.await?
		.into_iter()
		.map(|label| {
			labels.remove(&label.name);

			label.id
		})
		.collect::<Vec<_>>();

	let date_created: DateTime<FixedOffset> = Utc::now().into();

	if !labels.is_empty() {
		labels_ids.extend(
			db._batch(
				labels
					.into_iter()
					.map(|name| {
						db.label()
							.create(
								Uuid::new_v4().as_bytes().to_vec(),
								name,
								vec![label::date_created::set(date_created)],
							)
							.select(label::select!({ id }))
					})
					.collect::<Vec<_>>(),
			)
			.await?
			.into_iter()
			.map(|label| label.id),
		);
	}

	db.label_on_object()
		.create_many(
			labels_ids
				.into_iter()
				.map(|label_id| {
					label_on_object::create_unchecked(
						label_id,
						object_id,
						vec![label_on_object::date_created::set(date_created)],
					)
				})
				.collect(),
		)
		.skip_duplicates()
		.exec()
		.await?;

	invalidate_query!(library, "labels.list");
	invalidate_query!(library, "labels.getForObject");
	invalidate_query!(library, "labels.getWithObjects");

	Ok(())
}
