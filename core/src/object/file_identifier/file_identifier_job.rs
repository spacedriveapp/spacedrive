use crate::{
	job::{
		CurrentStep, JobError, JobInitOutput, JobResult, JobRunMetadata, JobStepOutput,
		StatefulJob, WorkerContext,
	},
	library::Library,
	location::file_path_helper::{
		ensure_file_path_exists, ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
		file_path_for_file_identifier, IsolatedFilePathData,
	},
	prisma::{file_path, location, PrismaClient, SortOrder},
	util::db::maybe_missing,
};

use std::{
	hash::{Hash, Hasher},
	path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, info, trace};

use super::{process_identifier_file_paths, FileIdentifierJobError, CHUNK_SIZE};

/// `FileIdentifierJobInit` takes file_paths without an object_id from a location
/// or starting from a `sub_path` (getting every descendent from this `sub_path`
/// and uniquely identifies them:
/// - first: generating the cas_id and extracting metadata
/// - finally: creating unique object records, and linking them to their file_paths
#[derive(Serialize, Deserialize, Clone, Debug)]
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

#[derive(Serialize, Deserialize, Debug)]
pub struct FileIdentifierJobData {
	location_path: PathBuf,
	maybe_sub_iso_file_path: Option<IsolatedFilePathData<'static>>,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct FileIdentifierJobRunMetadata {
	cursor: file_path::id::Type,
	total_orphan_paths: usize,
	total_objects_created: usize,
	total_objects_linked: usize,
	total_objects_ignored: usize,
}

impl JobRunMetadata for FileIdentifierJobRunMetadata {
	fn update(&mut self, new_data: Self) {
		self.total_orphan_paths += new_data.total_orphan_paths;
		self.total_objects_created += new_data.total_objects_created;
		self.total_objects_linked += new_data.total_objects_linked;
		self.total_objects_ignored += new_data.total_objects_ignored;
		self.cursor = new_data.cursor;
	}
}

#[async_trait::async_trait]
impl StatefulJob for FileIdentifierJobInit {
	type Data = FileIdentifierJobData;
	type Step = ();
	type RunMetadata = FileIdentifierJobRunMetadata;

	const NAME: &'static str = "file_identifier";

	async fn init(
		&self,
		ctx: &WorkerContext,
		data: &mut Option<Self::Data>,
	) -> Result<JobInitOutput<Self::RunMetadata, Self::Step>, JobError> {
		let init = self;
		let Library { db, .. } = &*ctx.library;

		debug!("Identifying orphan File Paths...");

		let location_id = init.location.id;

		let location_path = maybe_missing(&init.location.path, "location.path").map(Path::new)?;

		let maybe_sub_iso_file_path = match &init.sub_path {
			Some(sub_path) if sub_path != Path::new("") => {
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
			}
			_ => None,
		};

		let orphan_count =
			count_orphan_file_paths(db, location_id, &maybe_sub_iso_file_path).await?;

		// Initializing `state.data` here because we need a complete state in case of early finish
		*data = Some(FileIdentifierJobData {
			location_path: location_path.to_path_buf(),
			maybe_sub_iso_file_path,
		});

		let data = data.as_ref().expect("we just set it");

		if orphan_count == 0 {
			return Err(JobError::EarlyFinish {
				name: <Self as StatefulJob>::NAME.to_string(),
				reason: "Found no orphan file paths to process".to_string(),
			});
		}

		debug!("Found {} orphan file paths", orphan_count);

		let task_count = (orphan_count as f64 / CHUNK_SIZE as f64).ceil() as usize;
		debug!(
			"Found {} orphan Paths. Will execute {} tasks...",
			orphan_count, task_count
		);

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

		Ok((
			FileIdentifierJobRunMetadata {
				total_orphan_paths: orphan_count,
				cursor: first_path.id,
				..Default::default()
			},
			vec![(); task_count],
		)
			.into())
	}

	async fn execute_step(
		&self,
		ctx: &WorkerContext,
		CurrentStep { step_number, .. }: CurrentStep<'_, Self::Step>,
		data: &Self::Data,
		run_metadata: &Self::RunMetadata,
	) -> Result<JobStepOutput<Self::Step, Self::RunMetadata>, JobError> {
		let init = self;
		let location = &init.location;

		let mut new_metadata = Self::RunMetadata::default();

		// get chunk of orphans to process
		let file_paths = get_orphan_file_paths(
			&ctx.library.db,
			location.id,
			run_metadata.cursor,
			&data.maybe_sub_iso_file_path,
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

		let (total_objects_created, total_objects_linked, new_cursor) =
			process_identifier_file_paths(
				location,
				&file_paths,
				step_number,
				run_metadata.cursor,
				&ctx.library,
				run_metadata.total_orphan_paths,
			)
			.await?;

		new_metadata.total_objects_created = total_objects_created;
		new_metadata.total_objects_linked = total_objects_linked;
		new_metadata.cursor = new_cursor;

		ctx.progress_msg(format!(
			"Processed {} of {} orphan Paths",
			step_number * CHUNK_SIZE,
			run_metadata.total_orphan_paths
		));

		Ok(new_metadata.into())
	}

	async fn finalize(
		&self,
		_: &WorkerContext,
		_data: &Option<Self::Data>,
		run_metadata: &Self::RunMetadata,
	) -> JobResult {
		let init = self;
		info!("Finalizing identifier job: {:?}", &run_metadata);

		Ok(Some(json!({"init: ": init, "run_metadata": run_metadata})))
	}
}

fn orphan_path_filters(
	location_id: location::id::Type,
	file_path_id: Option<file_path::id::Type>,
	maybe_sub_iso_file_path: &Option<IsolatedFilePathData<'_>>,
) -> Vec<file_path::WhereParam> {
	sd_utils::chain_optional_iter(
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
	trace!(
		"Querying {} orphan Paths at cursor: {:?}",
		CHUNK_SIZE,
		file_path_id
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
