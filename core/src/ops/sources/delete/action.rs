//! Remove a source from the library.
//!
//! Removal and deletion are two different acts. Removing a source drops its
//! registration: the library stops indexing, watching and listing it. The
//! catalog stays where it is, because it holds what no walk can rebuild, the
//! assertions, and because a store placed on a drive is that drive's to
//! keep. Re-adding the scope reopens the catalog through its descriptor.
//! Deleting the catalog is a second, explicit choice the caller makes.
//!
//! The containing volume stays tracked. Removing the last folder on a drive
//! does not make the library forget the hardware it knows.

use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
	ops::sources::registry,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{path::PathBuf, sync::Arc};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeleteSourceInput {
	pub source_id: String,
	/// Also delete the source's store, assertions included. Off by default:
	/// removing a source from the library keeps its catalog.
	#[serde(default)]
	pub delete_catalog: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeleteSourceOutput {
	pub deleted: bool,
	/// Where the catalog remains, when it was kept and this machine had it.
	pub catalog_path: Option<PathBuf>,
	/// Whether the catalog was deleted along with the registration.
	pub catalog_deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteSourceAction {
	input: DeleteSourceInput,
}

impl LibraryAction for DeleteSourceAction {
	type Input = DeleteSourceInput;
	type Output = DeleteSourceOutput;

	fn from_input(input: DeleteSourceInput) -> Result<Self, String> {
		if input.source_id.trim().is_empty() {
			return Err("Source ID cannot be empty".to_string());
		}
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let source_id = uuid::Uuid::parse_str(&self.input.source_id)
			.map_err(|e| ActionError::Internal(format!("Invalid source ID: {e}")))?;

		// A filesystem source is retired from the volume index first, so no
		// writer is left open over a directory about to be deleted and a
		// later add reopens the store under its own registration.
		let filesystem = context.volume_index().forget_source(source_id).await;
		let catalog_path = match &filesystem {
			Some((_, dir)) => dir.clone(),
			None => None,
		};

		let mut catalog_deleted = false;
		if self.input.delete_catalog {
			// The library's offline copy is part of the catalog being
			// deleted; without the flag it stays beside the sidecars.
			if filesystem.is_some() {
				crate::service::mounts::offline::delete_with_catalog(
					context.volume_index(),
					source_id,
				)
				.await;
			}
			match &filesystem {
				Some((_, Some(dir))) => match tokio::fs::remove_dir_all(dir).await {
					Ok(()) => catalog_deleted = true,
					Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
					Err(e) => {
						return Err(ActionError::Internal(format!(
							"removed the source but could not delete its catalog at {}: {e}",
							dir.display()
						)))
					}
				},
				Some((_, None)) => {}
				None => {
					if library.source_manager().is_none() {
						library.init_source_manager().await.map_err(|e| {
							ActionError::Internal(format!("Failed to init source manager: {e}"))
						})?;
					}
					let source_manager = library.source_manager().ok_or_else(|| {
						ActionError::Internal("Source manager not available".to_string())
					})?;
					source_manager
						.delete_source(&registry::store_id(source_id))
						.await
						.map_err(ActionError::Internal)?;
					catalog_deleted = true;
				}
			}
		}

		registry::unregister(library.db().conn(), source_id)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to unregister source: {e}")))?;

		Ok(DeleteSourceOutput {
			deleted: true,
			catalog_path: if catalog_deleted { None } else { catalog_path },
			catalog_deleted,
		})
	}

	fn action_kind(&self) -> &'static str {
		"sources.delete"
	}
}

crate::register_library_action!(DeleteSourceAction, "sources.delete");

#[cfg(test)]
mod tests {
	use super::*;

	/// The wire shape a caller sent before the flag existed removes the
	/// source and keeps the catalog, which is the safe reading of a request
	/// that did not ask for deletion.
	#[test]
	fn deletion_of_the_catalog_is_opt_in() {
		let input: DeleteSourceInput = serde_json::from_str(r#"{"source_id":"abc"}"#).unwrap();
		assert!(!input.delete_catalog);
		let input: DeleteSourceInput =
			serde_json::from_str(r#"{"source_id":"abc","delete_catalog":true}"#).unwrap();
		assert!(input.delete_catalog);
		assert!(DeleteSourceAction::from_input(DeleteSourceInput {
			source_id: " ".to_string(),
			delete_catalog: false,
		})
		.is_err());
	}
}
