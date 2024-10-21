use crate::{
	file_identifier, utils::sub_path::maybe_get_iso_file_path_from_sub_path, Error,
	NonCriticalError, OuterContext, UpdateEvent,
};

use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_prisma_helpers::file_path_for_file_identifier;

use sd_prisma::prisma::{device, file_path, location, SortOrder};
use sd_task_system::{
	BaseTaskDispatcher, CancelTaskOnDrop, TaskDispatcher, TaskHandle, TaskOutput, TaskStatus,
};
use sd_utils::db::maybe_missing;

use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	sync::Arc,
};

use futures::{stream::FuturesUnordered, StreamExt};
use tracing::{debug, instrument, trace, warn};

use super::{
	accumulate_file_paths_by_cas_id, dispatch_object_processor_tasks, orphan_path_filters_shallow,
	tasks::{self, identifier, object_processor},
	CHUNK_SIZE,
};

#[instrument(
	skip_all,
	fields(
		location_id = location.id,
		location_path = ?location.path,
		sub_path = %sub_path.as_ref().display()
	)
	err,
)]
pub async fn shallow(
	location: location::Data,
	sub_path: impl AsRef<Path> + Send,
	dispatcher: &BaseTaskDispatcher<Error>,
	ctx: &impl OuterContext,
) -> Result<Vec<NonCriticalError>, Error> {
	let db = ctx.db();

	let location_path = maybe_missing(&location.path, "location.path")
		.map(PathBuf::from)
		.map(Arc::new)
		.map_err(file_identifier::Error::from)?;

	let location = Arc::new(location);

	let sub_iso_file_path = maybe_get_iso_file_path_from_sub_path::<file_identifier::Error>(
		location.id,
		Some(sub_path.as_ref()),
		&*location_path,
		db,
	)
	.await?
	.map_or_else(
		|| {
			IsolatedFilePathData::new(location.id, &*location_path, &*location_path, true)
				.map_err(file_identifier::Error::from)
		},
		Ok,
	)?;

	let device_pub_id = &ctx.sync().device_pub_id;
	let device_id = ctx
		.db()
		.device()
		.find_unique(device::pub_id::equals(device_pub_id.to_db()))
		.exec()
		.await
		.map_err(file_identifier::Error::from)?
		.ok_or(file_identifier::Error::DeviceNotFound(
			device_pub_id.clone(),
		))?
		.id;

	let mut orphans_count = 0;
	let mut last_orphan_file_path_id = None;

	let mut identifier_tasks = vec![];

	loop {
		#[allow(clippy::cast_possible_wrap)]
		// SAFETY: we know that CHUNK_SIZE is a valid i64
		let orphan_paths = db
			.file_path()
			.find_many(orphan_path_filters_shallow(
				location.id,
				last_orphan_file_path_id,
				&sub_iso_file_path,
			))
			.order_by(file_path::id::order(SortOrder::Asc))
			.take(CHUNK_SIZE as i64)
			.select(file_path_for_file_identifier::select())
			.exec()
			.await
			.map_err(file_identifier::Error::from)?;

		let Some(last_orphan) = orphan_paths.last() else {
			// No orphans here!
			break;
		};

		orphans_count += orphan_paths.len() as u64;
		last_orphan_file_path_id = Some(last_orphan.id);

		let Ok(tasks) = dispatcher
			.dispatch(tasks::Identifier::new(
				Arc::clone(&location),
				Arc::clone(&location_path),
				orphan_paths,
				true,
				Arc::clone(ctx.db()),
				ctx.sync().clone(),
				device_id,
			))
			.await
		else {
			debug!("Task system is shutting down while a shallow file identifier was in progress");
			return Ok(vec![]);
		};

		identifier_tasks.push(tasks);
	}

	if orphans_count == 0 {
		trace!("No orphans found");
		return Ok(vec![]);
	}

	process_tasks(identifier_tasks, dispatcher, ctx, device_id).await
}

async fn process_tasks(
	identifier_tasks: Vec<TaskHandle<Error>>,
	dispatcher: &BaseTaskDispatcher<Error>,
	ctx: &impl OuterContext,
	device_id: device::id::Type,
) -> Result<Vec<NonCriticalError>, Error> {
	let total_identifier_tasks = identifier_tasks.len();

	let mut pending_running_tasks = identifier_tasks
		.into_iter()
		.map(CancelTaskOnDrop::new)
		.collect::<FuturesUnordered<_>>();

	let mut errors = vec![];
	let mut completed_identifier_tasks = 0;
	let mut file_paths_accumulator = HashMap::new();

	while let Some(task_result) = pending_running_tasks.next().await {
		match task_result {
			Ok(TaskStatus::Done((_, TaskOutput::Out(any_task_output)))) => {
				// We only care about ExtractFileMetadataTaskOutput because we need to dispatch further tasks
				// and the ObjectProcessorTask only gives back some metrics not much important for
				// shallow file identifier
				if any_task_output.is::<identifier::Output>() {
					let identifier::Output {
						file_path_ids_with_new_object,
						file_paths_by_cas_id,
						errors: more_errors,
						..
					} = *any_task_output.downcast().expect("just checked");

					completed_identifier_tasks += 1;

					ctx.report_update(UpdateEvent::NewIdentifiedObjects {
						file_path_ids: file_path_ids_with_new_object,
					});

					accumulate_file_paths_by_cas_id(
						file_paths_by_cas_id,
						&mut file_paths_accumulator,
					);

					errors.extend(more_errors);

					if total_identifier_tasks == completed_identifier_tasks {
						let Ok(tasks) = dispatch_object_processor_tasks(
							file_paths_accumulator.drain(),
							ctx,
							device_id,
							dispatcher,
							true,
						)
						.await
						else {
							debug!("Task system is shutting down while a shallow file identifier was in progress");
							continue;
						};

						pending_running_tasks.extend(tasks.into_iter().map(CancelTaskOnDrop::new));
					}
				} else {
					let object_processor::Output {
						file_path_ids_with_new_object,
						..
					} = *any_task_output.downcast().expect("just checked");

					ctx.report_update(UpdateEvent::NewIdentifiedObjects {
						file_path_ids: file_path_ids_with_new_object,
					});
				}
			}

			Ok(TaskStatus::Done((task_id, TaskOutput::Empty))) => {
				warn!(%task_id, "Task returned an empty output");
			}

			Ok(TaskStatus::Shutdown(_)) => {
				debug!(
					"Spacedrive is shutting down while a shallow file identifier was in progress"
				);
				continue;
			}

			Ok(TaskStatus::Error(e)) => {
				return Err(e);
			}

			Ok(TaskStatus::Canceled | TaskStatus::ForcedAbortion) => {
				warn!("Task was cancelled or aborted on shallow file identifier");
				return Ok(errors);
			}

			Err(e) => {
				return Err(e.into());
			}
		}
	}

	Ok(errors)
}
