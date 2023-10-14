use crate::{
	job::{JobRunErrors, JobRunMetadata},
	library::Library,
	location::file_path_helper::{
		file_path_for_media_processor, FilePathError, IsolatedFilePathData,
	},
	util::db::{maybe_missing, MissingFieldError},
	Node,
};

use sd_file_ext::extensions::Extension;
use sd_prisma::prisma::{location, PrismaClient};

use std::{future::Future, path::Path};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::error;

use super::{
	media_data_extractor::{self, MediaDataError, MediaDataExtractorMetadata},
	thumbnail::{
		self,
		actor::{BatchToProcess, GenerateThumbnailArgs},
		ThumbnailerError,
	},
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
}

impl From<MediaDataExtractorMetadata> for MediaProcessorMetadata {
	fn from(media_data: MediaDataExtractorMetadata) -> Self {
		Self { media_data }
	}
}

impl JobRunMetadata for MediaProcessorMetadata {
	fn update(&mut self, new_data: Self) {
		self.media_data.extracted += new_data.media_data.extracted;
		self.media_data.skipped += new_data.media_data.skipped;
	}
}

// `thumbs_fetcher_fn` MUST return file_paths ordered by `materialized_path` for optimal results
async fn dispatch_thumbnails_for_processing<'d, 'p, 'e, 'ret, F>(
	location_id: location::id::Type,
	location_path: impl AsRef<Path>,
	parent_iso_file_path: &'p IsolatedFilePathData<'_>,
	library: &'d Library,
	node: &Node,
	should_regenerate: bool,
	thumbs_fetcher_fn: impl Fn(&'d PrismaClient, &'p IsolatedFilePathData<'_>, &'e [Extension]) -> F,
) -> Result<(), MediaProcessorError>
where
	'd: 'ret,
	'p: 'ret,
	'e: 'ret,
	F: Future<Output = Result<Vec<file_path_for_media_processor::Data>, MediaProcessorError>>
		+ 'ret,
{
	let Library { db, .. } = library;

	let location_path = location_path.as_ref();

	let file_paths = thumbs_fetcher_fn(
		db,
		parent_iso_file_path,
		&thumbnail::ALL_THUMBNAILABLE_EXTENSIONS,
	)
	.await?;

	tracing::debug!("Found {} file_paths to process", file_paths.len());

	let mut current_batch = Vec::with_capacity(16);

	// PDF thumbnails are currently way slower so we process them by last
	let mut pdf_thumbs = Vec::with_capacity(16);

	let mut current_materialized_path = None;

	let mut in_background = false;

	for file_path in file_paths {
		// Initializing current_materialized_path with the first file_path materialized_path
		if current_materialized_path.is_none() {
			current_materialized_path = file_path.materialized_path.clone();
		}

		if file_path.materialized_path != current_materialized_path
			&& (!current_batch.is_empty() || !pdf_thumbs.is_empty())
		{
			// Now we found a different materialized_path so we dispatch the current batch and start a new one

			// We starting by appending all pdfs and leaving the vec clean to be reused
			current_batch.append(&mut pdf_thumbs);

			tracing::debug!(
				"Found {} file_paths to process in {}",
				current_batch.len(),
				if in_background {
					"background"
				} else {
					"foreground"
				}
			);

			node.thumbnailer
				.new_indexed_thumbnails_batch(BatchToProcess {
					batch: current_batch,
					should_regenerate,
					in_background,
				})
				.await;

			// We moved our vec so we need a new
			current_batch = Vec::with_capacity(16);
			in_background = true; // Only the first batch should be processed in foreground

			// Exchaging for the first different materialized_path
			current_materialized_path = file_path.materialized_path.clone();
		}

		let file_path_id = file_path.id;
		if let Err(e) = add_to_batch(
			location_id,
			location_path,
			file_path,
			&mut current_batch,
			&mut pdf_thumbs,
		) {
			error!("Error adding file_path <id='{file_path_id}'> to thumbnail batch: {e:#?}");
		}
	}

	// Dispatching the last batch
	if !current_batch.is_empty() {
		tracing::debug!(
			"Found {} file_paths to process in {}",
			current_batch.len(),
			if in_background {
				"background"
			} else {
				"foreground"
			}
		);
		node.thumbnailer
			.new_indexed_thumbnails_batch(BatchToProcess {
				batch: current_batch,
				should_regenerate,
				in_background,
			})
			.await;
	}

	Ok(())
}

fn add_to_batch(
	location_id: location::id::Type,
	location_path: &Path, // This function is only used internally once, so we can pass &Path as a parameter
	file_path: file_path_for_media_processor::Data,
	current_batch: &mut Vec<GenerateThumbnailArgs>,
	pdf_thumbs: &mut Vec<GenerateThumbnailArgs>,
) -> Result<(), MissingFieldError> {
	let cas_id = maybe_missing(&file_path.cas_id, "file_path.cas_id")?.clone();

	let iso_file_path = IsolatedFilePathData::try_from((location_id, file_path))?;
	let full_path = location_path.join(&iso_file_path);

	let extension = iso_file_path.extension();
	let args = GenerateThumbnailArgs::new(extension.to_string(), cas_id, full_path);

	if extension != "pdf" {
		current_batch.push(args);
	} else {
		pdf_thumbs.push(args);
	}

	Ok(())
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
