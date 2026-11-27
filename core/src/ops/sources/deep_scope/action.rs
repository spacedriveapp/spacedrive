//! `sources.deep_scope.update`: keep a folder's contents read, or stop.
//!
//! The "Keep indexed" gesture of `docs/core/design/zero-onboarding-startup.md`.
//! A source's records always cover its whole root; this decides where its
//! file contents are read (content identity, the text index and what follows
//! them). Keeping a folder hashes and reads what it holds; releasing one
//! stops that and drops its text rows. Hashes already taken stay, because
//! they are what duplicate and integrity answers stand on, and a file that
//! changes outside the scope simply is not read again.

use std::{path::PathBuf, sync::Arc};

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	infra::{
		action::{error::ActionError, LibraryAction},
		job::types::JobPriority,
	},
	library::Library,
	ops::indexing::{
		content_identity::ContentIdentityJob, content_text::ContentTextJob, deep_scope,
	},
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateDeepScopeInput {
	/// A folder inside a source tracked on this device, or its root. Only a
	/// folder can be kept; releasing takes any path.
	pub path: PathBuf,
	/// Keep the folder's contents read, or stop reading them.
	pub keep: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateDeepScopeOutput {
	pub source_id: Uuid,
	/// The kept subtrees after the change, relative to the source root. An
	/// empty path is the whole source.
	pub subtrees: Vec<String>,
	/// The pass that reads a newly kept folder or drops a released one's
	/// text. None when nothing changed or the source does not read contents.
	pub job_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDeepScopeAction {
	input: UpdateDeepScopeInput,
}

impl LibraryAction for UpdateDeepScopeAction {
	type Input = UpdateDeepScopeInput;
	type Output = UpdateDeepScopeOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let index = context.volume_index();
		let path = &self.input.path;
		let not_tracked = || {
			ActionError::InvalidInput(format!(
				"{} is not under a source tracked on this device",
				path.display()
			))
		};
		let source_id = index.source_id_for(path).ok_or_else(not_tracked)?;
		let root = index.source_root(source_id).ok_or_else(not_tracked)?;
		let relative = path
			.strip_prefix(&root)
			.map(deep_scope::slash_path)
			.map_err(|_| not_tracked())?;
		let store = index.store_for(&root).await.ok_or_else(|| {
			ActionError::Internal("the source has no store on this device".into())
		})?;

		let current = store
			.deep_subtrees()
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?;
		let subtrees = if self.input.keep {
			// The scope is a list of folders; a file is read when its folder is.
			if !tokio::fs::metadata(path)
				.await
				.is_ok_and(|meta| meta.is_dir())
			{
				return Err(ActionError::InvalidInput(format!(
					"{} is not a folder",
					path.display()
				)));
			}
			deep_scope::keep(&current, &relative)
		} else {
			deep_scope::release(&current, &relative).map_err(ActionError::InvalidInput)?
		};
		if subtrees == current {
			return Ok(UpdateDeepScopeOutput {
				source_id,
				subtrees,
				job_id: None,
			});
		}

		let device = context
			.device_manager
			.device_id()
			.map_err(|e| ActionError::Internal(format!("no device identity: {e}")))?;
		store
			.db()
			.set_deep_scope(&subtrees, &crate::ops::tags::stamp::assertion_stamp(device))
			.await
			.map_err(|e| ActionError::Internal(format!("could not save the deep scope: {e}")))?;

		let reads_contents = index
			.source_config(source_id)
			.is_some_and(|config| config.identify_content);
		let job_id = if !reads_contents || index.dispatch_refusal(&root).is_some() {
			None
		} else if self.input.keep {
			// Identification queues the text and image passes behind it.
			let handle = library
				.jobs()
				.dispatch_with_priority(ContentIdentityJob::new(root), JobPriority::LOW, None)
				.await?;
			Some(handle.id().0)
		} else {
			let handle = library
				.jobs()
				.dispatch_with_priority(ContentTextJob::new(root, false), JobPriority::LOW, None)
				.await?;
			Some(handle.id().0)
		};

		Ok(UpdateDeepScopeOutput {
			source_id,
			subtrees,
			job_id,
		})
	}

	fn action_kind(&self) -> &'static str {
		"sources.deep_scope.update"
	}
}

crate::register_library_action!(UpdateDeepScopeAction, "sources.deep_scope.update");
