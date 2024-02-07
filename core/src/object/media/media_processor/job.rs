use crate::{
	invalidate_query,
	job::{
		CurrentStep, JobError, JobInitOutput, JobReportUpdate, JobResult, JobStepOutput,
		StatefulJob, WorkerContext,
	},
	library::Library,
	Node,
};

#[cfg(feature = "ai")]
use crate::job::JobRunErrors;

use sd_file_ext::extensions::Extension;
use sd_file_path_helper::{
	ensure_file_path_exists, ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
	file_path_for_media_processor, IsolatedFilePathData,
};
use sd_prisma::prisma::{location, PrismaClient};
use sd_utils::db::maybe_missing;

#[cfg(feature = "ai")]
use sd_ai::image_labeler::{BatchToken as ImageLabelerBatchToken, LabelerOutput};

#[cfg(feature = "ai")]
use std::sync::Arc;

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
	pub regenerate_labels: bool,
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
	#[cfg(feature = "ai")]
	labeler_batch_token: ImageLabelerBatchToken,
	#[cfg(feature = "ai")]
	#[serde(skip, default)]
	maybe_labels_rx: Option<chan::Receiver<LabelerOutput>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MediaProcessorJobStep {
	ExtractMediaData(Vec<file_path_for_media_processor::Data>),
	WaitThumbnails(usize),
	#[cfg(feature = "ai")]
	WaitLabels(usize),
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
			self.regenerate_thumbnails,
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

		#[cfg(feature = "ai")]
		let file_paths_for_labeling =
			get_files_for_labeling(db, &iso_file_path, self.regenerate_labels).await?;

		#[cfg(feature = "ai")]
		let total_files_for_labeling = file_paths_for_labeling.len();

		#[cfg(feature = "ai")]
		let (labeler_batch_token, labels_rx) =
			if let Some(image_labeller) = ctx.node.image_labeller.as_ref() {
				let (labeler_batch_token, labels_rx) = image_labeller
					.new_resumable_batch(
						location_id,
						location_path.clone(),
						file_paths_for_labeling,
						Arc::clone(db),
						sync.clone(),
					)
					.await;
				(labeler_batch_token, Some(labels_rx))
			} else {
				(uuid::Uuid::new_v4(), None)
			};

		let total_files = file_paths.len();

		let chunked_files = file_paths
			.into_iter()
			.chunks(BATCH_SIZE)
			.into_iter()
			.map(|chunk| chunk.collect::<Vec<_>>())
			.map(MediaProcessorJobStep::ExtractMediaData)
			.chain(
				[
					(thumbs_to_process_count > 0).then_some(MediaProcessorJobStep::WaitThumbnails(
						thumbs_to_process_count as usize,
					)),
				]
				.into_iter()
				.flatten(),
			)
			.chain(
				[
					#[cfg(feature = "ai")]
					{
						(total_files_for_labeling > 0)
							.then_some(MediaProcessorJobStep::WaitLabels(total_files_for_labeling))
					},
					#[cfg(not(feature = "ai"))]
					{
						None
					},
				]
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
			#[cfg(feature = "ai")]
			labeler_batch_token,
			#[cfg(feature = "ai")]
			maybe_labels_rx: labels_rx,
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
						"Thumbnailer progress reporter channel closed before all thumbnails were \
						processed, job will wait a bit waiting for a shutdown signal from manager"
					);
					sleep(Duration::from_secs(5)).await;
				}

				Ok(None.into())
			}

			#[cfg(feature = "ai")]
			MediaProcessorJobStep::WaitLabels(total_labels) => {
				let Some(image_labeller) = ctx.node.image_labeller.as_ref() else {
					let err = "AI system is disabled due to a previous error, skipping labels job";
					error!(err);
					return Ok(JobRunErrors(vec![err.to_string()]).into());
				};

				ctx.progress(vec![
					JobReportUpdate::TaskCount(*total_labels),
					JobReportUpdate::Phase("labels".to_string()),
					JobReportUpdate::Message(
						format!("Extracting labels for {total_labels} files",),
					),
				]);

				let mut labels_rx = pin!(if let Some(labels_rx) = data.maybe_labels_rx.clone() {
					labels_rx
				} else {
					match image_labeller
						.resume_batch(
							data.labeler_batch_token,
							Arc::clone(&ctx.library.db),
							ctx.library.sync.clone(),
						)
						.await
					{
						Ok(labels_rx) => labels_rx,
						Err(e) => return Ok(JobRunErrors(vec![e.to_string()]).into()),
					}
				});

				let mut total_labeled = 0;

				let mut errors = Vec::new();

				while let Some(LabelerOutput {
					file_path_id,
					has_new_labels,
					result,
				}) = labels_rx.next().await
				{
					total_labeled += 1;
					ctx.progress(vec![JobReportUpdate::CompletedTaskCount(total_labeled)]);

					if let Err(e) = result {
						error!(
							"Failed to generate labels <file_path_id='{}'>: {e:#?}",
							file_path_id
						);

						errors.push(e.to_string());
					} else if has_new_labels {
						// invalidate_query!(&ctx.library, "labels.count"); // TODO: This query doesn't exist on main yet
					}
				}

				invalidate_query!(&ctx.library, "labels.list");
				invalidate_query!(&ctx.library, "labels.getForObject");
				invalidate_query!(&ctx.library, "labels.getWithObjects");

				if !errors.is_empty() {
					Ok(JobRunErrors(errors).into())
				} else {
					Ok(None.into())
				}
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

	let mut file_paths = get_all_children_files_by_extensions(
		db,
		parent_iso_file_path,
		&thumbnail::ALL_THUMBNAILABLE_EXTENSIONS,
	)
	.await?;

	if file_paths.is_empty() {
		return Ok(0);
	}

	let first_materialized_path = file_paths[0].materialized_path.clone();

	// Only the first materialized_path should be processed in foreground
	let different_materialized_path_idx = file_paths
		.iter()
		.position(|file_path| file_path.materialized_path != first_materialized_path);

	let background_thumbs_args = different_materialized_path_idx
		.map(|idx| {
			file_paths
				.split_off(idx)
				.into_iter()
				.filter_map(|file_path| prepare_args(location_id, location_path, file_path))
				.collect::<Vec<_>>()
		})
		.unwrap_or_default();

	let foreground_thumbs_args = file_paths
		.into_iter()
		.filter_map(|file_path| prepare_args(location_id, location_path, file_path))
		.collect::<Vec<_>>();

	let thumbs_count = background_thumbs_args.len() + foreground_thumbs_args.len();

	debug!(
		"Dispatching {thumbs_count} thumbnails to be processed, {} in foreground and {} in background",
		foreground_thumbs_args.len(),
		background_thumbs_args.len()
	);

	if !foreground_thumbs_args.is_empty() {
		node.thumbnailer
			.new_indexed_thumbnails_tracked_batch(
				BatchToProcess::new(foreground_thumbs_args, should_regenerate, false),
				library.id,
				location_id,
			)
			.await;
	}

	if !background_thumbs_args.is_empty() {
		node.thumbnailer
			.new_indexed_thumbnails_tracked_batch(
				BatchToProcess::new(background_thumbs_args, should_regenerate, true),
				library.id,
				location_id,
			)
			.await;
	}

	Ok(thumbs_count as u32)
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

#[cfg(feature = "ai")]
async fn get_files_for_labeling(
	db: &PrismaClient,
	parent_iso_file_path: &IsolatedFilePathData<'_>,
	regenerate_labels: bool,
) -> Result<Vec<file_path_for_media_processor::Data>, MediaProcessorError> {
	// FIXME: Had to use format! macro because PCR doesn't support IN with Vec for SQLite
	// We have no data coming from the user, so this is sql injection safe
	db._query_raw(raw!(
		&format!(
			"SELECT id, materialized_path, is_dir, name, extension, cas_id, object_id
			FROM file_path f
			WHERE
				location_id={{}}
				AND cas_id IS NOT NULL
				AND LOWER(extension) IN ({})
				AND materialized_path LIKE {{}}
				{}
			ORDER BY materialized_path ASC",
			// Orderind by materialized_path so we can prioritize processing the first files
			// in the above part of the directories tree
			&media_data_extractor::FILTERED_IMAGE_EXTENSIONS
				.iter()
				.map(|ext| format!("LOWER('{ext}')"))
				.collect::<Vec<_>>()
				.join(","),
			if !regenerate_labels {
				"AND NOT EXISTS (SELECT 1 FROM label_on_object WHERE object_id = f.object_id)"
			} else {
				""
			}
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

fn prepare_args(
	location_id: location::id::Type,
	location_path: &Path, // This function is only used internally once, so we can pass &Path as a parameter
	file_path: file_path_for_media_processor::Data,
) -> Option<GenerateThumbnailArgs> {
	let file_path_id = file_path.id;

	let Ok(cas_id) = maybe_missing(&file_path.cas_id, "file_path.cas_id").cloned() else {
		error!("Missing cas_id for file_path <id='{file_path_id}'>");
		return None;
	};

	let Ok(iso_file_path) = IsolatedFilePathData::try_from((location_id, file_path)).map_err(|e| {
		error!("Failed to extract isolated file path data from file path <id='{file_path_id}'>: {e:#?}");
	}) else {
		return None;
	};

	Some(GenerateThumbnailArgs::new(
		iso_file_path.extension().to_string(),
		cas_id,
		location_path.join(&iso_file_path),
	))
}
