use crate::{
	library::LibraryContext,
	location::{
		fetch_location,
		indexer::indexer_job::indexer_job_location,
		manager::{LocationId, LocationManagerError},
	},
};

use std::{future::Future, time::Duration};

use async_trait::async_trait;
use notify::{
	event::{CreateKind, DataChange, ModifyKind, RenameMode},
	Event, EventKind,
};
use tokio::{fs, select, spawn, sync::oneshot, time::sleep};
use tracing::{debug, trace, warn};

use super::{
	utils::{check_location_online, create_dir, create_file, remove_event, rename, update_file},
	EventHandler,
};

#[derive(Debug, Default)]
pub(super) struct MacOsEventHandler {
	maybe_rename_sender: Option<oneshot::Sender<Event>>,
}

#[async_trait]
impl EventHandler for MacOsEventHandler {
	fn new() -> Self
	where
		Self: Sized,
	{
		Self {
			..Default::default()
		}
	}

	async fn handle_event(
		&mut self,
		location_id: LocationId,
		library_ctx: &LibraryContext,
		event: Event,
	) -> Result<(), LocationManagerError> {
		debug!("Received event: {:#?}", event);
		if let Some(location) = fetch_location(library_ctx, location_id)
			.include(indexer_job_location::include())
			.exec()
			.await?
		{
			if !check_location_online(&location) {
				return Ok(());
			}

			match event.kind {
				EventKind::Create(create_kind) => match create_kind {
					CreateKind::File => {
						let (maybe_rename_tx, maybe_rename_rx) = oneshot::channel();
						spawn(wait_to_create(
							location,
							event,
							library_ctx.clone(),
							create_file,
							maybe_rename_rx,
						));
						self.maybe_rename_sender = Some(maybe_rename_tx);
					}
					CreateKind::Folder => {
						let (maybe_rename_tx, maybe_rename_rx) = oneshot::channel();
						spawn(wait_to_create(
							location,
							event,
							library_ctx.clone(),
							create_dir,
							maybe_rename_rx,
						));
						self.maybe_rename_sender = Some(maybe_rename_tx);
					}
					other => {
						trace!("Ignoring other create event: {:#?}", other);
					}
				},
				EventKind::Modify(ref modify_kind) => match modify_kind {
					ModifyKind::Data(DataChange::Any) => {
						let metadata = fs::metadata(&event.paths[0]).await?;
						if metadata.is_file() {
							update_file(location, event, library_ctx).await?;
						}
						// We ignore EventKind::Modify(ModifyKind::Data(DataChange::Any)) for directories
						// as they're also used for removing files and directories, being emitted
						// on the parent directory in this case
					}
					ModifyKind::Name(RenameMode::Any) => {
						if let Some(rename_sender) = self.maybe_rename_sender.take() {
							if !rename_sender.is_closed() && rename_sender.send(event).is_err() {
								warn!("Failed to send rename event");
							}
						}
					}
					other => {
						trace!("Ignoring other modify event: {:#?}", other);
					}
				},
				EventKind::Remove(remove_kind) => {
					remove_event(location, event, remove_kind, library_ctx).await?;
					// An EventKind::Modify(ModifyKind::Data(DataChange::Any)) - On parent directory
					// is also emitted, but we can ignore it.
				}
				other_event_kind => {
					debug!("Other event that we don't handle for now: {other_event_kind:#?}");
				}
			}
		}

		Ok(())
	}
}

// FIX-ME: Had some troubles with borrowck, to receive a
// impl FnOnce(indexer_job_location::Data, Event, &LibraryContext) -> Fut
// as a parameter, had to move LibraryContext into the functions
async fn wait_to_create<Fut>(
	location: indexer_job_location::Data,
	event: Event,
	library_ctx: LibraryContext,
	create_fn: impl FnOnce(indexer_job_location::Data, Event, LibraryContext) -> Fut,
	maybe_rename_rx: oneshot::Receiver<Event>,
) -> Result<(), LocationManagerError>
where
	Fut: for<'r> Future<Output = Result<(), LocationManagerError>>,
{
	select! {
		() = sleep(Duration::from_secs(1)) => {
			create_fn(location, event, library_ctx).await
		},
		Ok(rename_event) = maybe_rename_rx => {
			debug!("Renaming file or directory instead of creating a new one");
			rename(&event.paths[0], &rename_event.paths[0], location, &library_ctx).await
		}
	}
}
