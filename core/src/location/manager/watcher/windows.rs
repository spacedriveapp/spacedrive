use crate::{
	library::LibraryContext,
	location::{indexer::indexer_job::indexer_job_location, manager::LocationManagerError},
};

use async_trait::async_trait;
use notify::{
	event::{CreateKind, ModifyKind, RenameMode},
	Event, EventKind,
};
use tokio::fs;
use tracing::{debug, warn};

use super::{
	utils::{create_dir, create_file, remove_event, rename, update_file},
	EventHandler,
};

#[derive(Debug, Default)]
pub(super) struct WindowsEventHandler {
	rename_stack: Option<Event>,
	create_file_stack: Option<Event>,
}

#[async_trait]
impl EventHandler for WindowsEventHandler {
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
		debug!("Received Windows event: {:#?}", event);

		match event.kind {
			EventKind::Create(CreateKind::Any) => {
				let metadata = fs::metadata(&event.paths[0]).await?;
				if metadata.is_file() {
					self.create_file_stack = Some(event);
				} else {
					create_dir(location, event, library_ctx.clone()).await?;
				}
			}
			EventKind::Modify(ModifyKind::Any) => {
				let metadata = fs::metadata(&event.paths[0]).await?;
				if metadata.is_file() {
					if let Some(create_file_event) = self.create_file_stack.take() {
						create_file(location, create_file_event, library_ctx.clone()).await?;
					} else {
						update_file(location, event, library_ctx).await?;
					}
				} else {
					warn!("Unexpected Windows modify event on a directory");
				}
			}
			EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
				self.rename_stack = Some(event);
			}
			EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
				let from_event = self
					.rename_stack
					.take()
					.expect("Unexpectedly missing rename from windows event");
				rename(&event.paths[0], &from_event.paths[0], location, library_ctx).await?;
			}
			EventKind::Remove(remove_kind) => {
				remove_event(location, event, remove_kind, library_ctx).await?;
			}

			other_event_kind => {
				debug!("Other Windows event that we don't handle for now: {other_event_kind:#?}");
			}
		}

		Ok(())
	}
}
