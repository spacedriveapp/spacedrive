use crate::{
	library::LibraryContext,
	location::{indexer::indexer_job::indexer_job_location, manager::LocationManagerError},
};

use async_trait::async_trait;
use notify::{
	event::{CreateKind, DataChange, ModifyKind, RenameMode},
	Event, EventKind,
};
use tracing::trace;

use super::{
	utils::{create_dir, file_creation_or_update, remove_event, rename},
	EventHandler,
};

#[derive(Debug, Default)]
pub(super) struct MacOsEventHandler {
	rename_stack: Option<Event>,
}

#[async_trait]
impl EventHandler for MacOsEventHandler {
	fn new() -> Self
	where
		Self: Sized,
	{
		Default::default()
	}

	async fn handle_event(
		&mut self,
		location: indexer_job_location::Data,
		library_ctx: &LibraryContext,
		event: Event,
	) -> Result<(), LocationManagerError> {
		trace!("Received MacOS event: {:#?}", event);

		match event.kind {
			EventKind::Create(CreateKind::Folder) => {
				create_dir(location, event, library_ctx).await?;
			}
			EventKind::Modify(ModifyKind::Data(DataChange::Content)) => {
				// If a file had its content modified, then it was updated or created
				file_creation_or_update(location, event, library_ctx).await?;
			}
			EventKind::Modify(ModifyKind::Name(RenameMode::Any)) => {
				match self.rename_stack.take() {
					None => {
						self.rename_stack = Some(event);
					}
					Some(from_event) => {
						rename(&event.paths[0], &from_event.paths[0], location, library_ctx)
							.await?;
					}
				}
			}

			EventKind::Remove(remove_kind) => {
				remove_event(location, event, remove_kind, library_ctx).await?;
			}
			other_event_kind => {
				trace!("Other MacOS event that we don't handle for now: {other_event_kind:#?}");
			}
		}

		Ok(())
	}
}
