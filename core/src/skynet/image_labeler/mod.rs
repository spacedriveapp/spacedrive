use crate::{
	invalidate_query,
	library::Library,
	util::{db::MissingFieldError, error::FileIOError},
};

use sd_prisma::prisma::{file_path, label, label_on_object, object};

use std::{collections::HashSet, path::Path};

use chrono::{DateTime, FixedOffset, Utc};
use thiserror::Error;
use tracing::error;
use uuid::Uuid;

mod actor;
mod model;

pub use actor::ImageLabeler;
pub use model::{Model, YoloV8};

#[derive(Debug)]
pub struct LabelerOutput {
	pub file_path_id: file_path::id::Type,
	pub labels_result: Result<HashSet<String>, ImageLabelerError>,
}

#[derive(Debug, Error)]
pub enum ImageLabelerError {
	#[error("model executor failed: {0}")]
	ModelExecutorFailed(#[from] ort::Error),
	#[error("image load failed: {0}")]
	ImageLoadFailed(#[from] image::ImageError),
	#[error("failed to get isolated file path data: {0}")]
	IsolateFilePathData(#[from] MissingFieldError),
	#[error("file_path with unsupported extension: <id='{0}', extension='{1}'>")]
	UnsupportedExtension(file_path::id::Type, String),
	#[error("file_path too big: <id='{0}', size='{1}'>")]
	FileTooBig(file_path::id::Type, usize),
	#[error("model file not found: {}", .0.display())]
	ModelFileNotFound(Box<Path>),
	#[error("no model available for inference")]
	NoModelAvailable,

	#[error(transparent)]
	FileIO(#[from] FileIOError),
}

pub async fn assign_labels(
	object_id: object::id::Type,
	mut labels: HashSet<String>,
	library @ Library { db, .. }: &Library,
) -> Result<(), prisma_client_rust::QueryError> {
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
