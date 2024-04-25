use crate::{utils::sub_path::maybe_get_iso_file_path_from_sub_path, Error, NonCriticalJobError};

use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_prisma_helpers::file_path_for_file_identifier;
use sd_core_sync::Manager as SyncManager;

use sd_prisma::prisma::{file_path, location, PrismaClient, SortOrder};
use sd_task_system::{
	BaseTaskDispatcher, CancelTaskOnDrop, TaskDispatcher, TaskOutput, TaskStatus,
};
use sd_utils::db::maybe_missing;

use std::{
	path::{Path, PathBuf},
	sync::Arc,
};

use futures_concurrency::future::FutureGroup;
use lending_stream::{LendingStream, StreamExt};
use prisma_client_rust::or;
use tracing::{debug, warn};

use super::{
	tasks::{ExtractFileMetadataTask, ExtractFileMetadataTaskOutput, ObjectProcessorTask},
	FileIdentifierError, CHUNK_SIZE,
};

pub async fn shallow(
	location: location::Data,
	sub_path: impl AsRef<Path> + Send,
	dispatcher: BaseTaskDispatcher<Error>,
	db: Arc<PrismaClient>,
	sync: Arc<SyncManager>,
	invalidate_query: impl Fn(&'static str) + Send + Sync,
) -> Result<Vec<NonCriticalJobError>, Error> {
	let sub_path = sub_path.as_ref();

	let location_path = maybe_missing(&location.path, "location.path")
		.map(PathBuf::from)
		.map(Arc::new)
		.map_err(FileIdentifierError::from)?;

	let location = Arc::new(location);

	let sub_iso_file_path =
		maybe_get_iso_file_path_from_sub_path(location.id, &Some(sub_path), &*location_path, &db)
			.await
			.map_err(FileIdentifierError::from)?
			.map_or_else(
				|| {
					IsolatedFilePathData::new(location.id, &*location_path, &*location_path, true)
						.map_err(FileIdentifierError::from)
				},
				Ok,
			)?;

	let mut orphans_count = 0;
	let mut last_orphan_file_path_id = None;

	let mut pending_running_tasks = FutureGroup::new();

	loop {
		#[allow(clippy::cast_possible_wrap)]
		// SAFETY: we know that CHUNK_SIZE is a valid i64
		let orphan_paths = db
			.file_path()
			.find_many(orphan_path_filters(
				location.id,
				last_orphan_file_path_id,
				&sub_iso_file_path,
			))
			.order_by(file_path::id::order(SortOrder::Asc))
			.take(CHUNK_SIZE as i64)
			.select(file_path_for_file_identifier::select())
			.exec()
			.await
			.map_err(FileIdentifierError::from)?;

		let Some(last_orphan) = orphan_paths.last() else {
			// No orphans here!
			break;
		};

		orphans_count += orphan_paths.len() as u64;
		last_orphan_file_path_id = Some(last_orphan.id);

		pending_running_tasks.insert(CancelTaskOnDrop(
			dispatcher
				.dispatch(ExtractFileMetadataTask::new_shallow(
					Arc::clone(&location),
					Arc::clone(&location_path),
					orphan_paths,
				))
				.await,
		));
	}

	if orphans_count == 0 {
		debug!(
			"No orphans found on <location_id={}, sub_path='{}'>",
			location.id,
			sub_path.display()
		);
		return Ok(vec![]);
	}

	let errors = process_tasks(pending_running_tasks, dispatcher, db, sync).await?;

	invalidate_query("search.paths");
	invalidate_query("search.objects");

	Ok(errors)
}

async fn process_tasks(
	pending_running_tasks: FutureGroup<CancelTaskOnDrop<Error>>,
	dispatcher: BaseTaskDispatcher<Error>,
	db: Arc<PrismaClient>,
	sync: Arc<SyncManager>,
) -> Result<Vec<NonCriticalJobError>, Error> {
	let mut pending_running_tasks = pending_running_tasks.lend_mut();

	let mut errors = vec![];

	while let Some((pending_running_tasks, task_result)) = pending_running_tasks.next().await {
		match task_result {
			Ok(TaskStatus::Done((_, TaskOutput::Out(any_task_output)))) => {
				// We only care about ExtractFileMetadataTaskOutput because we need to dispatch further tasks
				// and the ObjectProcessorTask only gives back some metrics not much important for
				// shallow file identifier
				if any_task_output.is::<ExtractFileMetadataTaskOutput>() {
					let ExtractFileMetadataTaskOutput {
						identified_files,
						errors: more_errors,
						..
					} = *any_task_output
						.downcast::<ExtractFileMetadataTaskOutput>()
						.expect("just checked");

					errors.extend(more_errors);

					if !identified_files.is_empty() {
						pending_running_tasks.insert(CancelTaskOnDrop(
							dispatcher
								.dispatch(ObjectProcessorTask::new_shallow(
									identified_files,
									Arc::clone(&db),
									Arc::clone(&sync),
								))
								.await,
						));
					}
				}
			}

			Ok(TaskStatus::Done((task_id, TaskOutput::Empty))) => {
				warn!("Task <id='{task_id}'> returned an empty output");
			}

			Ok(TaskStatus::Shutdown(_)) => {
				debug!(
					"Spacedrive is shutting down while a shallow file identifier was in progress"
				);
				return Ok(vec![]);
			}

			Ok(TaskStatus::Error(e)) => {
				return Err(e);
			}

			Ok(TaskStatus::Canceled | TaskStatus::ForcedAbortion) => {
				warn!("Task was cancelled or aborted on shallow file identifier");
				return Ok(vec![]);
			}

			Err(e) => {
				return Err(e.into());
			}
		}
	}

	Ok(errors)
}

fn orphan_path_filters(
	location_id: location::id::Type,
	file_path_id: Option<file_path::id::Type>,
	sub_iso_file_path: &IsolatedFilePathData<'_>,
) -> Vec<file_path::WhereParam> {
	sd_utils::chain_optional_iter(
		[
			or!(
				file_path::object_id::equals(None),
				file_path::cas_id::equals(None)
			),
			file_path::is_dir::equals(Some(false)),
			file_path::location_id::equals(Some(location_id)),
			file_path::materialized_path::equals(Some(
				sub_iso_file_path
					.materialized_path_for_children()
					.expect("sub path for shallow identifier must be a directory"),
			)),
			file_path::size_in_bytes_bytes::not(Some(0u64.to_be_bytes().to_vec())),
		],
		[file_path_id.map(file_path::id::gte)],
	)
}
