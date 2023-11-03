use crate::{
	invalidate_query,
	job::{
		CurrentStep, JobError, JobInitOutput, JobReportUpdate, JobResult, JobStepOutput,
		StatefulJob, WorkerContext,
	},
	library::Library,
	location::file_path_helper::{
		ensure_file_path_exists, ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
		file_path_for_media_processor, IsolatedFilePathData,
	},
	prisma::{location, PrismaClient},
	util::db::{maybe_missing, MissingFieldError},
	Node,
};

use std::{
	hash::Hash,
	path::{Path, PathBuf},
	pin::pin,
	time::Duration,
};

use async_channel as chan;
use futures::StreamExt;
use itertools::Itertools;
use prisma_client_rust::{raw, PrismaValue};
use sd_file_ext::extensions::Extension;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::time::sleep;
use tracing::{debug, error, info, trace, warn};

use super::{
	media_data_extractor, process,
	thumbnail::{self, GenerateThumbnailArgs},
	BatchToProcess, MediaProcessorError, MediaProcessorMetadata,
};

const BATCH_SIZE: usize = 10;

#[derive(Serialize, Deserialize, Debug)]
pub struct MediaProcessorJobInit {
	pub location: location::Data,
	pub sub_path: Option<PathBuf>,
	pub regenerate_thumbnails: bool,
}

impl Hash for MediaProcessorJobInit {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.location.id.hash(state);
		if let Some(ref sub_path) = self.sub_path {
			sub_path.hash(state);
		}
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MediaProcessorJobData {
	location_path: PathBuf,
	to_process_path: PathBuf,
	#[serde(skip, default)]
	maybe_thumbnailer_progress_rx: Option<chan::Receiver<(u32, u32)>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MediaProcessorJobStep {
	ExtractMediaData(Vec<file_path_for_media_processor::Data>),
	WaitThumbnails(usize),
}

#[async_trait::async_trait]
impl StatefulJob for MediaProcessorJobInit {
	type Data = MediaProcessorJobData;
	type Step = MediaProcessorJobStep;
	type RunMetadata = MediaProcessorMetadata;

	const NAME: &'static str = "media_processor";
	const IS_BATCHED: bool = true;

	fn target_location(&self) -> location::id::Type {
		self.location.id
	}

	async fn init(
		&self,
		ctx: &WorkerContext,
		data: &mut Option<Self::Data>,
	) -> Result<JobInitOutput<Self::RunMetadata, Self::Step>, JobError> {
		let Library { db, .. } = ctx.library.as_ref();

		let location_id = self.location.id;
		let location_path =
			maybe_missing(&self.location.path, "location.path").map(PathBuf::from)?;

		let (to_process_path, iso_file_path) = match &self.sub_path {
			Some(sub_path) if sub_path != Path::new("") => {
				let full_path = ensure_sub_path_is_in_location(&location_path, sub_path)
					.await
					.map_err(MediaProcessorError::from)?;
				ensure_sub_path_is_directory(&location_path, sub_path)
					.await
					.map_err(MediaProcessorError::from)?;

				let sub_iso_file_path =
					IsolatedFilePathData::new(location_id, &location_path, &full_path, true)
						.map_err(MediaProcessorError::from)?;

				ensure_file_path_exists(
					sub_path,
					&sub_iso_file_path,
					db,
					MediaProcessorError::SubPathNotFound,
				)
				.await?;

				(full_path, sub_iso_file_path)
			}
			_ => (
				location_path.to_path_buf(),
				IsolatedFilePathData::new(location_id, &location_path, &location_path, true)
					.map_err(MediaProcessorError::from)?,
			),
		};

		debug!(
			"Searching for media files in location {location_id} at directory \"{iso_file_path}\""
		);

		let thumbs_to_process_count = dispatch_thumbnails_for_processing(
			location_id,
			&location_path,
			&iso_file_path,
			&ctx.library,
			&ctx.node,
			false,
		)
		.await?;

		let maybe_thumbnailer_progress_rx = if thumbs_to_process_count > 0 {
			let (progress_tx, progress_rx) = chan::unbounded();

			ctx.node
				.thumbnailer
				.register_reporter(location_id, progress_tx)
				.await;

			Some(progress_rx)
		} else {
			None
		};

		let file_paths = get_files_for_media_data_extraction(db, &iso_file_path).await?;

		let total_files = file_paths.len();

		let chunked_files =
			file_paths
				.into_iter()
				.chunks(BATCH_SIZE)
				.into_iter()
				.map(|chunk| chunk.collect::<Vec<_>>())
				.map(MediaProcessorJobStep::ExtractMediaData)
				.chain(
					[(thumbs_to_process_count > 0).then_some(
						MediaProcessorJobStep::WaitThumbnails(thumbs_to_process_count as usize),
					)]
					.into_iter()
					.flatten(),
				)
				.collect::<Vec<_>>();

		ctx.progress(vec![
			JobReportUpdate::TaskCount(total_files),
			JobReportUpdate::Phase("media_data".to_string()),
			JobReportUpdate::Message(format!(
				"Preparing to process {total_files} files in {} chunks",
				chunked_files.len()
			)),
		]);

		*data = Some(MediaProcessorJobData {
			location_path,
			to_process_path,
			maybe_thumbnailer_progress_rx,
		});

		Ok((
			Self::RunMetadata {
				thumbs_processed: thumbs_to_process_count,
				..Default::default()
			},
			chunked_files,
		)
			.into())
	}

	async fn execute_step(
		&self,
		ctx: &WorkerContext,
		CurrentStep { step, step_number }: CurrentStep<'_, Self::Step>,
		data: &Self::Data,
		_: &Self::RunMetadata,
	) -> Result<JobStepOutput<Self::Step, Self::RunMetadata>, JobError> {
		match step {
			MediaProcessorJobStep::ExtractMediaData(file_paths) => process(
				file_paths,
				self.location.id,
				&data.location_path,
				&ctx.library.db,
				&|completed_count| {
					ctx.progress(vec![JobReportUpdate::CompletedTaskCount(
						step_number * BATCH_SIZE + completed_count,
					)]);
				},
			)
			.await
			.map(Into::into)
			.map_err(Into::into),
			MediaProcessorJobStep::WaitThumbnails(total_thumbs) => {
				ctx.progress(vec![
					JobReportUpdate::TaskCount(*total_thumbs),
					JobReportUpdate::Phase("thumbnails".to_string()),
					JobReportUpdate::Message(format!(
						"Waiting for processing of {total_thumbs} thumbnails",
					)),
				]);

				let mut progress_rx = pin!(if let Some(progress_rx) =
					data.maybe_thumbnailer_progress_rx.clone()
				{
					progress_rx
				} else {
					let (progress_tx, progress_rx) = chan::unbounded();

					ctx.node
						.thumbnailer
						.register_reporter(self.location.id, progress_tx)
						.await;

					progress_rx
				});

				let mut total_completed = 0;

				while let Some((completed, total)) = progress_rx.next().await {
					trace!("Received progress update from thumbnailer: {completed}/{total}",);
					ctx.progress(vec![JobReportUpdate::CompletedTaskCount(
						completed as usize,
					)]);
					total_completed = completed;
				}

				if progress_rx.is_closed() && total_completed < *total_thumbs as u32 {
					warn!(
						"Thumbnailer progress reporter channel closed before all thumbnails were
						processed, job will wait a bit waiting for a shutdown signal from manager"
					);
					sleep(Duration::from_secs(5)).await;
				}

				Ok(None.into())
			}
		}
	}

	async fn finalize(
		&self,
		ctx: &WorkerContext,
		data: &Option<Self::Data>,
		run_metadata: &Self::RunMetadata,
	) -> JobResult {
		info!(
			"Finished media processing for location {} at {}",
			self.location.id,
			data.as_ref()
				.expect("critical error: missing data on job state")
				.to_process_path
				.display()
		);

		if run_metadata.media_data.extracted > 0 {
			invalidate_query!(ctx.library, "search.paths");
		}

		Ok(Some(json!({"init: ": self, "run_metadata": run_metadata})))
	}
}

async fn dispatch_thumbnails_for_processing(
	location_id: location::id::Type,
	location_path: impl AsRef<Path>,
	parent_iso_file_path: &IsolatedFilePathData<'_>,
	library: &Library,
	node: &Node,
	should_regenerate: bool,
) -> Result<u32, MediaProcessorError> {
	let Library { db, .. } = library;

	let location_path = location_path.as_ref();

	let file_paths = get_all_children_files_by_extensions(
		db,
		parent_iso_file_path,
		&thumbnail::ALL_THUMBNAILABLE_EXTENSIONS,
	)
	.await?;

	if file_paths.is_empty() {
		return Ok(0);
	}

	let mut current_batch = Vec::with_capacity(16);

	// PDF thumbnails are currently way slower so we process them by last
	let mut pdf_thumbs = Vec::with_capacity(16);

	let mut current_materialized_path = None;

	let mut in_background = false;

	let mut thumbs_count = 0;

	for file_path in file_paths {
		// Initializing current_materialized_path with the first file_path materialized_path
		if current_materialized_path.is_none() {
			current_materialized_path = file_path.materialized_path.clone();
		}

		if file_path.materialized_path != current_materialized_path
			&& (!current_batch.is_empty() || !pdf_thumbs.is_empty())
		{
			// Now we found a different materialized_path so we dispatch the current batch and start a new one

			thumbs_count += current_batch.len() as u32;

			node.thumbnailer
				.new_indexed_thumbnails_batch_with_ticket(
					BatchToProcess::new(current_batch, should_regenerate, in_background),
					library.id,
					location_id,
				)
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
		thumbs_count += current_batch.len() as u32;
		node.thumbnailer
			.new_indexed_thumbnails_batch_with_ticket(
				BatchToProcess::new(current_batch, should_regenerate, in_background),
				library.id,
				location_id,
			)
			.await;
	}

	// We now put the pdf_thumbs to be processed by last
	if !pdf_thumbs.is_empty() {
		thumbs_count += pdf_thumbs.len() as u32;
		node.thumbnailer
			.new_indexed_thumbnails_batch_with_ticket(
				BatchToProcess::new(pdf_thumbs, should_regenerate, in_background),
				library.id,
				location_id,
			)
			.await;
	}

	Ok(thumbs_count)
}

async fn get_files_for_media_data_extraction(
	db: &PrismaClient,
	parent_iso_file_path: &IsolatedFilePathData<'_>,
) -> Result<Vec<file_path_for_media_processor::Data>, MediaProcessorError> {
	get_all_children_files_by_extensions(
		db,
		parent_iso_file_path,
		&media_data_extractor::FILTERED_IMAGE_EXTENSIONS,
	)
	.await
	.map_err(Into::into)
}

async fn get_all_children_files_by_extensions(
	db: &PrismaClient,
	parent_iso_file_path: &IsolatedFilePathData<'_>,
	extensions: &[Extension],
) -> Result<Vec<file_path_for_media_processor::Data>, MediaProcessorError> {
	// FIXME: Had to use format! macro because PCR doesn't support IN with Vec for SQLite
	// We have no data coming from the user, so this is sql injection safe
	db._query_raw(raw!(
		&format!(
			"SELECT id, materialized_path, is_dir, name, extension, cas_id, object_id
			FROM file_path
			WHERE
				location_id={{}}
				AND cas_id IS NOT NULL
				AND LOWER(extension) IN ({})
				AND materialized_path LIKE {{}}
			ORDER BY materialized_path ASC",
			// Orderind by materialized_path so we can prioritize processing the first files
			// in the above part of the directories tree
			extensions
				.iter()
				.map(|ext| format!("LOWER('{ext}')"))
				.collect::<Vec<_>>()
				.join(",")
		),
		PrismaValue::Int(parent_iso_file_path.location_id() as i64),
		PrismaValue::String(format!(
			"{}%",
			parent_iso_file_path
				.materialized_path_for_children()
				.expect("sub path iso_file_path must be a directory")
		))
	))
	.exec()
	.await
	.map_err(Into::into)
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
