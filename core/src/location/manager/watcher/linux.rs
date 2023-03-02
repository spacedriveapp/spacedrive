use crate::{
	library::LibraryContext,
	location::{indexer::indexer_job_location, manager::LocationManagerError},
};

use async_trait::async_trait;
use notify::{
	event::{AccessKind, AccessMode, CreateKind, ModifyKind, RenameMode},
	Event, EventKind,
};
use tracing::trace;

use super::{
	utils::{create_dir, file_creation_or_update, remove_event, rename_both_event},
	EventHandler,
};

#[derive(Debug)]
pub(super) struct LinuxEventHandler {}

#[async_trait]
impl EventHandler for LinuxEventHandler {
	fn new() -> Self {
		Self {}
	}

	async fn handle_event(
		&mut self,
		location: indexer_job_location::Data,
		library_ctx: &LibraryContext,
		event: Event,
	) -> Result<(), LocationManagerError> {
		trace!("Received Linux event: {:#?}", event);

		match event.kind {
			EventKind::Access(AccessKind::Close(AccessMode::Write)) => {
				// If a file was closed with write mode, then it was updated or created
				file_creation_or_update(&location, &event, library_ctx).await?;
			}
			EventKind::Create(CreateKind::Folder) => {
				create_dir(&location, &event, library_ctx).await?;
			}
			EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
				rename_both_event(&location, &event, library_ctx).await?;
			}
			EventKind::Remove(remove_kind) => {
				remove_event(&location, &event, remove_kind, library_ctx).await?;
			}
			other_event_kind => {
				trace!("Other Linux event that we don't handle for now: {other_event_kind:#?}");
			}
		}

		Ok(())
	}
}
