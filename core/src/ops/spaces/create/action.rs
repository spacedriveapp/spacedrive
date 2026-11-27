//! `spaces.create`: make a folder a space, or create an empty one.
//!
//! A folder space points at a folder that already exists and changes nothing
//! on disk. A workspace space creates an empty folder, by default under the
//! data directory's workspaces folder, for an agent to work in. Either way a
//! folder no source of this library covers is tracked as a source first, so
//! the space has a store to live in and its files are indexed.

use std::{path::PathBuf, sync::Arc};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{
	config::WORKSPACES_DIR,
	context::CoreContext,
	domain::{Space, SpaceKind},
	infra::action::{error::ActionError, LibraryAction},
	library::{config::AddOverrides, Library},
	ops::{
		sources::track::action::track_and_index,
		spaces::store::{self, Located},
	},
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceCreateInput {
	/// Display name, or the folder's own name.
	pub name: Option<String>,
	pub kind: SpaceKind,
	/// The folder. Required for a folder space. For a workspace space, the
	/// folder to create, which must not exist or must be empty; by default a
	/// new folder under the data directory.
	pub path: Option<PathBuf>,
	/// Whether file contents inside the space are hashed and read. On by
	/// default.
	pub content_indexing: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceCreateOutput {
	pub space: Space,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceCreateAction {
	input: SpaceCreateInput,
}

impl LibraryAction for SpaceCreateAction {
	type Input = SpaceCreateInput;
	type Output = SpaceCreateOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		if input.kind == SpaceKind::Folder && input.path.is_none() {
			return Err("a folder space needs a path".to_string());
		}
		if input.path.as_ref().is_some_and(|path| !path.is_absolute()) {
			return Err("the path must be absolute".to_string());
		}
		if input
			.name
			.as_ref()
			.is_some_and(|name| name.trim().is_empty())
		{
			return Err("the name cannot be empty".to_string());
		}
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let id = Uuid::now_v7();
		let input = self.input;
		let path = match input.kind {
			SpaceKind::Folder => {
				let path = input.path.expect("checked in from_input");
				if !tokio::fs::metadata(&path)
					.await
					.is_ok_and(|meta| meta.is_dir())
				{
					return Err(ActionError::InvalidInput(format!(
						"{} is not a folder",
						path.display()
					)));
				}
				path
			}
			SpaceKind::Workspace => {
				let path = input
					.path
					.unwrap_or_else(|| context.data_dir.join(WORKSPACES_DIR).join(id.to_string()));
				create_empty(&path).await?;
				path
			}
		};
		let name = input
			.name
			.map(|name| name.trim().to_string())
			.or_else(|| {
				path.file_name()
					.map(|name| name.to_string_lossy().into_owned())
					.filter(|_| input.kind == SpaceKind::Folder)
			})
			.unwrap_or_else(|| "Workspace".to_string());

		// A workspace is its own source, so its walk and its events stay its
		// own even when the folder sits inside another source.
		let located = match input.kind {
			SpaceKind::Folder => store::locate(&library, &context, &path).await,
			SpaceKind::Workspace => None,
		};
		let Located {
			source_id,
			root,
			relative,
			store,
		} = match located {
			Some(located) => located,
			None => {
				track_and_index(
					&library,
					&context,
					path.clone(),
					Some(name.clone()),
					&AddOverrides::default(),
				)
				.await?;
				store::locate(&library, &context, &path)
					.await
					.ok_or_else(|| {
						ActionError::Internal(format!(
							"{} was tracked but has no store on this device",
							path.display()
						))
					})?
			}
		};

		// One create at a time, so two requests for one folder cannot both
		// pass the check below before either writes.
		let _one_create = CREATES.lock().await;
		let existing = store
			.db()
			.spaces()
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?;
		if let Some(space) = existing.iter().find(|space| space.root == relative) {
			return Err(ActionError::InvalidInput(format!(
				"{} is already the space {}",
				path.display(),
				space.name
			)));
		}

		let stamp = store::stamp(&context).map_err(ActionError::Internal)?;
		let row = sd_store::SpaceRow {
			uuid: id,
			name,
			kind: input.kind.as_str().to_string(),
			root: relative,
			content_indexing: input.content_indexing.unwrap_or(true),
			created_at: Utc::now().to_rfc3339(),
			deleted: false,
			updated_hlc: stamp.hlc,
			origin_device: stamp.device_uuid,
		};
		store
			.db()
			.put_space(&row)
			.await
			.map_err(|e| ActionError::Internal(format!("could not save the space: {e}")))?;

		Ok(SpaceCreateOutput {
			space: Space::from_row(row, source_id, Some(&root)),
		})
	}

	fn action_kind(&self) -> &'static str {
		"spaces.create"
	}
}

crate::register_library_action!(SpaceCreateAction, "spaces.create");

static CREATES: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Create `path` as an empty folder, or accept one that already is.
async fn create_empty(path: &std::path::Path) -> Result<(), ActionError> {
	if let Ok(mut entries) = tokio::fs::read_dir(path).await {
		if entries
			.next_entry()
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?
			.is_some()
		{
			return Err(ActionError::InvalidInput(format!(
				"{} is not empty",
				path.display()
			)));
		}
		return Ok(());
	}
	tokio::fs::create_dir_all(path)
		.await
		.map_err(|e| ActionError::Internal(format!("could not create {}: {e}", path.display())))
}
