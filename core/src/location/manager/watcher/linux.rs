use crate::{
	library::Library,
	location::{location_with_indexer_rules, manager::LocationManagerError},
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
		location: location_with_indexer_rules::Data,
		library: &Library,
		event: Event,
	) -> Result<(), LocationManagerError> {
		trace!("Received Linux event: {:#?}", event);

		match event.kind {
			EventKind::Access(AccessKind::Close(AccessMode::Write)) => {
				// If a file was closed with write mode, then it was updated or created
				file_creation_or_update(&location, &event, library).await?;
			}
			EventKind::Create(CreateKind::Folder) => {
				create_dir(&location, &event, library).await?;
			}
			EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
				rename_both_event(&location, &event, library).await?;
			}
			EventKind::Remove(remove_kind) => {
				remove_event(&location, &event, remove_kind, library).await?;
			}
			other_event_kind => {
				trace!("Other Linux event that we don't handle for now: {other_event_kind:#?}");
			}
		}

		Ok(())
	}
}
