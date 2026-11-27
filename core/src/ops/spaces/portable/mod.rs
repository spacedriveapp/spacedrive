//! `spaces.export` and `spaces.import`: carry a space to another library
//! as one `.sdspace` file.
//!
//! An export holds the space record, its change log, the actors the log
//! names, and its versions, snapshots and the chunks they need, or with
//! `current_only` only the newest version of each file. A space with
//! versioning off exports its current files. An import creates a new
//! workspace space at an empty folder, writes the current files into it,
//! and brings the log, versions and snapshots across, so history and
//! restore work in the new library as they did in the old one.

use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	sync::Arc,
};

use chrono::Utc;
use sd_store::{
	space_file::{is_plain_relative, SpaceFile, SpaceFileCounts},
	EventQuery, SpaceVersions, VersionRow,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	domain::{Space, SpaceKind},
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
	ops::{
		actors::{self, Actor},
		spaces::{
			events, store, versioning, SpaceCreateAction, SpaceCreateInput, SpaceUpdateAction,
			SpaceUpdateInput,
		},
	},
};

/// What an export or an import carried.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct SpaceFileSummary {
	/// Files in the space's current state.
	pub files: u64,
	pub changes: u64,
	pub versions: u64,
	pub snapshots: u64,
	/// Chunk bytes the file holds.
	pub bytes: u64,
}

/// The space record an export carries.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Record {
	id: Uuid,
	name: String,
	content_indexing: bool,
	versioning: bool,
	created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceExportInput {
	pub id: Uuid,
	/// The file to write. It must not exist.
	pub path: PathBuf,
	/// Only the newest version of each file, with no snapshots.
	#[serde(default)]
	pub current_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceExportOutput {
	pub path: PathBuf,
	pub summary: SpaceFileSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceExportAction {
	input: SpaceExportInput,
}

impl LibraryAction for SpaceExportAction {
	type Input = SpaceExportInput;
	type Output = SpaceExportOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		if !input.path.is_absolute() {
			return Err("give the export file as an absolute path".to_string());
		}
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let space = store::find(&library, &context, self.input.id)
			.await
			.map_err(ActionError::InvalidInput)?;
		let file = SpaceFile::create(&self.input.path)
			.await
			.map_err(|e| ActionError::InvalidInput(e.to_string()))?;
		// Only a file this export created is removed when it fails.
		let summary = export(
			&library,
			&context,
			&space,
			file,
			&self.input.path,
			self.input.current_only,
		)
		.await
		.map_err(ActionError::Internal);
		if summary.is_err() {
			let _ = tokio::fs::remove_file(&self.input.path).await;
		}
		Ok(SpaceExportOutput {
			path: self.input.path,
			summary: summary?,
		})
	}

	fn action_kind(&self) -> &'static str {
		"spaces.export"
	}
}

crate::register_library_action!(SpaceExportAction, "spaces.export");

async fn export(
	library: &Library,
	context: &CoreContext,
	space: &Space,
	file: SpaceFile,
	path: &Path,
	current_only: bool,
) -> Result<SpaceFileSummary, String> {
	let record = Record {
		id: space.id,
		name: space.name.clone(),
		content_indexing: space.content_indexing,
		versioning: space.versioning,
		created_at: space.created_at.to_rfc3339(),
	};
	let set = |key: &'static str, value: String| {
		let file = file.clone();
		async move { file.set_meta(key, &value).await.map_err(|e| e.to_string()) }
	};
	set(
		"space",
		serde_json::to_string(&record).map_err(|e| e.to_string())?,
	)
	.await?;
	set("exported_at", Utc::now().to_rfc3339()).await?;
	set("current_only", current_only.to_string()).await?;

	let events = match events::log_of(context, space.source_id, space.id).await {
		Some(log) => log
			.read(&EventQuery {
				cursor: None,
				newest: false,
				path: None,
				limit: u32::MAX,
			})
			.await
			.map_err(|e| e.to_string())?,
		None => Vec::new(),
	};
	file.put_events(&events).await.map_err(|e| e.to_string())?;

	// A space without versions exports its files as they are now, staged in a
	// version store beside the export that is gone when this returns.
	let staging;
	let (versions, current_only) = if space.versioning {
		let versions = versioning::open(context, space.source_id, space.id)
			.await
			.ok_or("the space's versions are not on this device")?;
		(versions, current_only)
	} else {
		let parent = path.parent().ok_or("the export path has no folder")?;
		staging = tempfile::Builder::new()
			.prefix(".sdspace-")
			.tempdir_in(parent)
			.map_err(|e| e.to_string())?;
		let versions = Arc::new(
			SpaceVersions::open(staging.path())
				.await
				.map_err(|e| e.to_string())?,
		);
		let person = actors::this_person(library, context).await?;
		let template =
			VersionRow::attributed(&Utc::now().to_rfc3339(), person, versioning::BASELINE, None);
		for (source, below) in versioning::current_files(context, space).await {
			versions
				.capture(
					&source,
					VersionRow {
						path: below,
						..template.clone()
					},
				)
				.await
				.map_err(|e| e.to_string())?;
		}
		(versions, true)
	};
	let counts = file
		.put_versions(&versions, current_only)
		.await
		.map_err(|e| e.to_string())?;

	let mut named: Vec<Uuid> = events.iter().map(|event| event.actor_id).collect();
	named.extend(
		versions
			.history(None, None, u32::MAX)
			.await
			.map_err(|e| e.to_string())?
			.iter()
			.map(|row| row.actor_id),
	);
	named.sort();
	named.dedup();
	let known: Vec<Actor> = actors::all(library)
		.await
		.map_err(|e| e.to_string())?
		.into_iter()
		.filter(|actor| named.binary_search(&actor.id).is_ok())
		.collect();
	set(
		"actors",
		serde_json::to_string(&known).map_err(|e| e.to_string())?,
	)
	.await?;
	let files = file.current().await.map_err(|e| e.to_string())?.len() as u64;
	file.close().await;
	Ok(summary(files, events.len() as u64, &counts))
}

fn summary(files: u64, changes: u64, counts: &SpaceFileCounts) -> SpaceFileSummary {
	SpaceFileSummary {
		files,
		changes,
		versions: counts.versions,
		snapshots: counts.snapshots,
		bytes: counts.chunk_bytes,
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceImportInput {
	/// The `.sdspace` file.
	pub path: PathBuf,
	/// The folder the space's files go in. It must not exist or must be
	/// empty.
	pub destination: PathBuf,
	/// The new space's name; the exported name by default.
	#[serde(default)]
	pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceImportOutput {
	pub space: Space,
	pub summary: SpaceFileSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceImportAction {
	input: SpaceImportInput,
}

impl LibraryAction for SpaceImportAction {
	type Input = SpaceImportInput;
	type Output = SpaceImportOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		if !input.path.is_absolute() || !input.destination.is_absolute() {
			return Err("give the file and the destination as absolute paths".to_string());
		}
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let file = SpaceFile::open(&self.input.path)
			.await
			.map_err(|e| ActionError::InvalidInput(e.to_string()))?;
		let record: Record = file
			.meta("space")
			.await
			.map_err(|e| ActionError::InvalidInput(e.to_string()))?
			.and_then(|json| serde_json::from_str(&json).ok())
			.ok_or_else(|| {
				ActionError::InvalidInput("the file holds no space record".to_string())
			})?;
		let current = file
			.current()
			.await
			.map_err(|e| ActionError::InvalidInput(e.to_string()))?;
		if let Some(bad) = current.iter().find(|row| !is_plain_relative(&row.path)) {
			return Err(ActionError::InvalidInput(format!(
				"the file names a path outside its space: {}",
				bad.path
			)));
		}

		let space = SpaceCreateAction::from_input(SpaceCreateInput {
			name: Some(self.input.name.unwrap_or(record.name.clone())),
			kind: SpaceKind::Workspace,
			path: Some(self.input.destination.clone()),
			content_indexing: Some(record.content_indexing),
		})
		.map_err(ActionError::InvalidInput)?
		.execute(library.clone(), context.clone())
		.await?
		.space;
		let folder = space
			.path
			.clone()
			.ok_or_else(|| ActionError::Internal("the new space has no folder".to_string()))?;

		// The files land as part of the import, not as changes made outside
		// Spacedrive.
		let held = events::hold(&[folder.as_path()]);
		for row in &current {
			let target = folder.join(&row.path);
			if let Some(parent) = target.parent() {
				tokio::fs::create_dir_all(parent)
					.await
					.map_err(|e| ActionError::Internal(e.to_string()))?;
			}
			let hash = row.blob_hash.as_deref().unwrap_or_default();
			file.read_blob_to(hash, &target)
				.await
				.map_err(|e| ActionError::Internal(format!("{}: {e}", row.path)))?;
			#[cfg(unix)]
			if let Some(mode) = row.mode {
				use std::os::unix::fs::PermissionsExt;
				let _ = tokio::fs::set_permissions(
					&target,
					std::fs::Permissions::from_mode(mode as u32 & 0o777),
				)
				.await;
			}
		}
		drop(held);

		if let Some(json) = file.meta("actors").await.ok().flatten() {
			for actor in serde_json::from_str::<Vec<Actor>>(&json).unwrap_or_default() {
				actors::remember(&library, &actor)
					.await
					.map_err(ActionError::Internal)?;
			}
		}
		let imported = file
			.events()
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?;
		let mut seqs = HashMap::new();
		if let Some(log) = events::log_of(&context, space.source_id, space.id).await {
			for event in &imported {
				let seq = log
					.append(&sd_store::SpaceEvent {
						space_id: space.id,
						undo_id: None,
						..event.clone()
					})
					.await
					.map_err(|e| ActionError::Internal(e.to_string()))?;
				seqs.insert(event.seq, seq);
			}
		}

		let mut counts = SpaceFileCounts::default();
		let space = if record.versioning {
			let versions = versioning::open(&context, space.source_id, space.id)
				.await
				.ok_or_else(|| {
					ActionError::Internal("could not open the new space's versions".to_string())
				})?;
			counts = file
				.restore_versions(&versions, &seqs)
				.await
				.map_err(|e| ActionError::Internal(e.to_string()))?;
			SpaceUpdateAction::from_input(SpaceUpdateInput {
				id: space.id,
				name: None,
				content_indexing: None,
				versioning: Some(true),
			})
			.map_err(ActionError::Internal)?
			.execute(library.clone(), context.clone())
			.await?
			.space
		} else {
			space
		};
		Ok(SpaceImportOutput {
			summary: summary(current.len() as u64, imported.len() as u64, &counts),
			space,
		})
	}

	fn action_kind(&self) -> &'static str {
		"spaces.import"
	}
}

crate::register_library_action!(SpaceImportAction, "spaces.import");
