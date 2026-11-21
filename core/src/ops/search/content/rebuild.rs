//! `search.content.rebuild`: drop a source's content text index and read
//! every file again.

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
	ops::{indexing::content_text::ContentTextJob, sources::registry},
};

/// Which source's index to rebuild.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RebuildContentIndexInput {
	/// The root of a source tracked on this device, or any path inside it.
	pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RebuildContentIndexOutput {
	/// Files the index held before it was dropped.
	pub cleared_files: u64,
	/// The pass that reads the source again.
	pub job_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebuildContentIndexAction {
	input: RebuildContentIndexInput,
}

impl LibraryAction for RebuildContentIndexAction {
	type Input = RebuildContentIndexInput;
	type Output = RebuildContentIndexOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		Ok(Self { input })
	}

	/// The index is generation: every row came from bytes the origin holds,
	/// so dropping it loses nothing. The pass refuses a source whose volume
	/// is away, which leaves the index empty until it returns.
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
		registry::get(library.db().conn(), source_id)
			.await
			.map_err(|_| not_tracked())?;
		let root = index.source_root(source_id).ok_or_else(not_tracked)?;
		if !index
			.source_config(source_id)
			.is_some_and(|config| config.identify_content)
		{
			return Err(ActionError::InvalidInput(
				"this source does not read file contents, so it has no text index".into(),
			));
		}
		if let Some(reason) = index.dispatch_refusal(&root) {
			return Err(ActionError::InvalidInput(reason));
		}
		let store = index.store_for(&root).await.ok_or_else(|| {
			ActionError::Internal("the source has no store on this device".into())
		})?;
		let text = store
			.text_index()
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?;
		let cleared_files = text
			.counts()
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?
			.files;
		text.clear()
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?;

		let handle = library
			.jobs()
			.dispatch_with_priority(ContentTextJob::new(root, false), JobPriority::LOW, None)
			.await?;
		Ok(RebuildContentIndexOutput {
			cleared_files,
			job_id: Some(handle.id().0),
		})
	}

	fn action_kind(&self) -> &'static str {
		"search.content.rebuild"
	}
}

crate::register_library_action!(RebuildContentIndexAction, "search.content.rebuild");
