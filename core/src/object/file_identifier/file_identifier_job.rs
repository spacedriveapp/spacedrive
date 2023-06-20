use crate::{
	extract_job_data, extract_job_data_mut,
	job::{
		JobError, JobInitData, JobReportUpdate, JobResult, JobState, StatefulJob, WorkerContext,
	},
	library::Library,
	location::file_path_helper::{
		ensure_file_path_exists, ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
		file_path_for_file_identifier, IsolatedFilePathData,
	},
	prisma::{file_path, location, PrismaClient, SortOrder},
	util::db::{chain_optional_iter, maybe_missing},
};

use std::{
	hash::{Hash, Hasher},
	path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tracing::info;

use super::{
	process_identifier_file_paths, FileIdentifierJobError, FileIdentifierReport, CHUNK_SIZE,
};

pub struct FileIdentifierJob {}

/// `FileIdentifierJobInit` takes file_paths without an object_id from a location
/// or starting from a `sub_path` (getting every descendent from this `sub_path`
/// and uniquely identifies them:
/// - first: generating the cas_id and extracting metadata
/// - finally: creating unique object records, and linking them to their file_paths
#[derive(Serialize, Deserialize, Clone)]
pub struct FileIdentifierJobInit {
	pub location: location::Data,
	pub sub_path: Option<PathBuf>, // subpath to start from
}

impl Hash for FileIdentifierJobInit {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.location.id.hash(state);
		if let Some(ref sub_path) = self.sub_path {
			sub_path.hash(state);
		}
	}
}

#[derive(Serialize, Deserialize)]
pub struct FileIdentifierJobState {
	cursor: file_path::id::Type,
	report: FileIdentifierReport,
	maybe_sub_iso_file_path: Option<IsolatedFilePathData<'static>>,
}

impl JobInitData for FileIdentifierJobInit {
	type Job = FileIdentifierJob;
}

#[async_trait::async_trait]
impl StatefulJob for FileIdentifierJob {
	type Init = FileIdentifierJobInit;
	type Data = FileIdentifierJobState;
	type Step = ();

	const NAME: &'static str = "file_identifier";

	fn new() -> Self {
		Self {}
	}

	async fn init(
		&self,
		ctx: &mut WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let Library { db, .. } = &ctx.library;

		info!("Identifying orphan File Paths...");

		let location_id = state.init.location.id;

		let location_path =
			maybe_missing(&state.init.location.path, "location.path").map(Path::new)?;

		let maybe_sub_iso_file_path = if let Some(ref sub_path) = state.init.sub_path {
			let full_path = ensure_sub_path_is_in_location(location_path, sub_path)
				.await
				.map_err(FileIdentifierJobError::from)?;
			ensure_sub_path_is_directory(location_path, sub_path)
				.await
				.map_err(FileIdentifierJobError::from)?;

			let sub_iso_file_path =
				IsolatedFilePathData::new(location_id, location_path, &full_path, true)
					.map_err(FileIdentifierJobError::from)?;

			ensure_file_path_exists(
				sub_path,
				&sub_iso_file_path,
				db,
				FileIdentifierJobError::SubPathNotFound,
			)
			.await?;

			Some(sub_iso_file_path)
		} else {
			None
		};

		let orphan_count =
			count_orphan_file_paths(db, location_id, &maybe_sub_iso_file_path).await?;

		// Initializing `state.data` here because we need a complete state in case of early finish
		state.data = Some(FileIdentifierJobState {
			report: FileIdentifierReport {
				location_path: location_path.to_path_buf(),
				total_orphan_paths: orphan_count,
				..Default::default()
			},
			cursor: 0,
			maybe_sub_iso_file_path,
		});

		let data = extract_job_data_mut!(state);

		if orphan_count == 0 {
			return Err(JobError::EarlyFinish {
				name: <Self as StatefulJob>::NAME.to_string(),
				reason: "Found no orphan file paths to process".to_string(),
			});
		}

		info!("Found {} orphan file paths", orphan_count);

		let task_count = (orphan_count as f64 / CHUNK_SIZE as f64).ceil() as usize;
		info!(
			"Found {} orphan Paths. Will execute {} tasks...",
			orphan_count, task_count
		);

		// update job with total task count based on orphan file_paths count
		ctx.progress(vec![JobReportUpdate::TaskCount(task_count)]);

		let first_path = db
			.file_path()
			.find_first(orphan_path_filters(
				location_id,
				None,
				&data.maybe_sub_iso_file_path,
			))
			.select(file_path::select!({ id }))
			.exec()
			.await?
			.expect("We already validated before that there are orphans `file_path`s"); // SAFETY: We already validated before that there are orphans `file_path`s

		data.cursor = first_path.id;

		state.steps.extend((0..task_count).map(|_| ()));

		Ok(())
	}

	async fn execute_step(
		&self,
		ctx: &mut WorkerContext,
		state: &mut JobState<Self>,
	) -> Result<(), JobError> {
		let FileIdentifierJobState {
			ref mut cursor,
			ref mut report,
			ref maybe_sub_iso_file_path,
		} = extract_job_data_mut!(state);

		let step_number = state.step_number;
		let location = &state.init.location;

		// get chunk of orphans to process
		let file_paths = get_orphan_file_paths(
			&ctx.library.db,
			location.id,
			*cursor,
			maybe_sub_iso_file_path,
		)
		.await?;

		// if no file paths found, abort entire job early, there is nothing to do
		// if we hit this error, there is something wrong with the data/query
		if file_paths.is_empty() {
			return Err(JobError::EarlyFinish {
				name: <Self as StatefulJob>::NAME.to_string(),
				reason: "Expected orphan Paths not returned from database query for this chunk"
					.to_string(),
			});
		}

		let (total_objects_created, total_objects_linked) = process_identifier_file_paths(
			location,
			&file_paths,
			step_number,
			cursor,
			&ctx.library,
			report.total_orphan_paths,
		)
		.await?;

		report.total_objects_created += total_objects_created;
		report.total_objects_linked += total_objects_linked;

		ctx.progress(vec![
			JobReportUpdate::CompletedTaskCount(step_number),
			JobReportUpdate::Message(format!(
				"Processed {} of {} orphan Paths",
				step_number * CHUNK_SIZE,
				report.total_orphan_paths
			)),
		]);

		Ok(())
	}

	async fn finalize(&mut self, _: &mut WorkerContext, state: &mut JobState<Self>) -> JobResult {
		let report = &extract_job_data!(state).report;

		info!("Finalizing identifier job: {report:?}");

		Ok(Some(serde_json::to_value(report)?))
	}
}

fn orphan_path_filters(
	location_id: location::id::Type,
	file_path_id: Option<file_path::id::Type>,
	maybe_sub_iso_file_path: &Option<IsolatedFilePathData<'_>>,
) -> Vec<file_path::WhereParam> {
	chain_optional_iter(
		[
			file_path::object_id::equals(None),
			file_path::is_dir::equals(Some(false)),
			file_path::location_id::equals(Some(location_id)),
		],
		[
			// this is a workaround for the cursor not working properly
			file_path_id.map(file_path::id::gte),
			maybe_sub_iso_file_path.as_ref().map(|sub_iso_file_path| {
				file_path::materialized_path::starts_with(
					sub_iso_file_path
						.materialized_path_for_children()
						.expect("sub path iso_file_path must be a directory"),
				)
			}),
		],
	)
}

async fn count_orphan_file_paths(
	db: &PrismaClient,
	location_id: location::id::Type,
	maybe_sub_materialized_path: &Option<IsolatedFilePathData<'_>>,
) -> Result<usize, prisma_client_rust::QueryError> {
	db.file_path()
		.count(orphan_path_filters(
			location_id,
			None,
			maybe_sub_materialized_path,
		))
		.exec()
		.await
		.map(|c| c as usize)
}

async fn get_orphan_file_paths(
	db: &PrismaClient,
	location_id: location::id::Type,
	file_path_id: file_path::id::Type,
	maybe_sub_materialized_path: &Option<IsolatedFilePathData<'_>>,
) -> Result<Vec<file_path_for_file_identifier::Data>, prisma_client_rust::QueryError> {
	info!(
		"Querying {} orphan Paths at cursor: {:?}",
		CHUNK_SIZE, file_path_id
	);
	db.file_path()
		.find_many(orphan_path_filters(
			location_id,
			Some(file_path_id),
			maybe_sub_materialized_path,
		))
		.order_by(file_path::id::order(SortOrder::Asc))
		.take(CHUNK_SIZE as i64)
		// .skip(1)
		.select(file_path_for_file_identifier::select())
		.exec()
		.await
}
