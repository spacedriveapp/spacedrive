//! # Space export and import
//!
//! `spaces.export` and `spaces.import` carry a space to another library as one
//! `.sdspace` file. An export holds the space record, its change log, the
//! actors the log names, and its versions, snapshots and the chunks they
//! need, or with `current_only` only the newest version of each file. A space
//! with versioning off exports its current files.
//!
//! An import checks the whole file and writes its files in the data
//! directory before it creates anything, so a damaged file leaves the
//! library as it was. It then creates a workspace space at the empty
//! destination, moves the files in, and brings the log, versions and
//! snapshots across, so history and restore work as they did. A failure
//! after that removes the new space, its source and its files again.
//!
//! ## Example
//! ```rust,no_run
//! use std::{path::PathBuf, sync::Arc};
//!
//! use sd_core::{
//!     context::CoreContext,
//!     infra::action::LibraryAction,
//!     library::Library,
//!     ops::spaces::{SpaceExportAction, SpaceExportInput, SpaceImportAction, SpaceImportInput},
//! };
//!
//! async fn copy(
//!     library: Arc<Library>,
//!     context: Arc<CoreContext>,
//!     id: uuid::Uuid,
//! ) -> Result<(), Box<dyn std::error::Error>> {
//!     let file = PathBuf::from("/tmp/project.sdspace");
//!     SpaceExportAction::from_input(SpaceExportInput {
//!         id,
//!         path: file.clone(),
//!         current_only: false,
//!     })?
//!     .execute(library.clone(), context.clone())
//!     .await?;
//!     let imported = SpaceImportAction::from_input(SpaceImportInput {
//!         path: file,
//!         destination: PathBuf::from("/tmp/project-copy"),
//!         name: None,
//!     })?
//!     .execute(library, context)
//!     .await?;
//!     tracing::info!(space = %imported.space.id, "imported");
//!     Ok(())
//! }
//! ```

use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	sync::Arc,
};

use chrono::Utc;
use sd_store::{
	space_file::{SpaceFile, SpaceFileCounts},
	EventQuery, SpaceVersions, VersionRow,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::warn;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	domain::{Space, SpaceKind},
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
	ops::{
		actors::{self, Actor},
		sources::{DeleteSourceAction, DeleteSourceInput},
		spaces::{
			create, events, store, versioning, SpaceCreateAction, SpaceCreateInput,
			SpaceDeleteAction, SpaceDeleteInput, SpaceUpdateAction, SpaceUpdateInput,
		},
	},
};

/// Where an import stages its files, under the data directory.
const IMPORTS_DIR: &str = "imports";

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

	// A space without versions exports its files as they are now, staged in
	// a version store in the space's own store directory that is gone when
	// this returns. The export itself is left out when it sits in the space.
	let staging;
	let (versions, current_only) = if space.versioning {
		let versions = versioning::open(context, space.source_id, space.id)
			.await
			.ok_or("the space's versions are not on this device")?;
		(versions, current_only)
	} else {
		let dir = versioning::space_dir(context, space.source_id, space.id)
			.ok_or("the space's store is not on this device")?;
		tokio::fs::create_dir_all(&dir)
			.await
			.map_err(|e| e.to_string())?;
		staging = tempfile::Builder::new()
			.prefix("export-")
			.tempdir_in(&dir)
			.map_err(|e| e.to_string())?;
		let versions = Arc::new(
			SpaceVersions::open(staging.path())
				.await
				.map_err(|e| e.to_string())?,
		);
		let person = actors::this_person(library, context).await?;
		let template =
			VersionRow::attributed(&Utc::now().to_rfc3339(), person, versioning::BASELINE, None);
		// The export may be spelled through a link or a firmlink to the
		// space's folder, so it is recognized as the same file on disk, not by
		// its spelling.
		for (source, below) in versioning::current_files(context, space).await {
			if source.file_name() == path.file_name() {
				let (source, output) = (source.clone(), path.to_path_buf());
				let own = tokio::task::spawn_blocking(move || {
					same_file::is_same_file(&source, &output).unwrap_or(false)
				})
				.await
				.unwrap_or(false);
				if own {
					continue;
				}
			}
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
		file.check()
			.await
			.map_err(|e| ActionError::InvalidInput(e.to_string()))?;
		let current = file
			.current()
			.await
			.map_err(|e| ActionError::InvalidInput(e.to_string()))?;

		// The files are written and checked in the data directory first, which
		// no space watches, so a damaged file fails before any space exists
		// and no enclosing space sees the staged copies.
		let destination = &self.input.destination;
		let imports = context.data_dir.join(IMPORTS_DIR);
		tokio::fs::create_dir_all(&imports)
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?;
		let staging = tempfile::Builder::new()
			.prefix("import-")
			.tempdir_in(&imports)
			.map_err(|e| ActionError::Internal(e.to_string()))?;
		for row in &current {
			let target = staging.path().join(&row.path);
			if let Some(parent) = target.parent() {
				tokio::fs::create_dir_all(parent)
					.await
					.map_err(|e| ActionError::Internal(e.to_string()))?;
			}
			let hash = row.blob_hash.as_deref().unwrap_or_default();
			file.read_blob_to(hash, &target)
				.await
				.map_err(|e| ActionError::InvalidInput(format!("{}: {e}", row.path)))?;
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
		let existed = tokio::fs::symlink_metadata(destination).await.is_ok();
		// A source already registered stays registered on failure, and a
		// catalog adopted from an earlier registration stays on disk.
		let registered = store::locate(&library, &context, destination)
			.await
			.map(|located| located.source_id);
		let created = create::create(
			library.clone(),
			context.clone(),
			SpaceCreateAction::from_input(SpaceCreateInput {
				name: Some(self.input.name.clone().unwrap_or(record.name.clone())),
				kind: SpaceKind::Workspace,
				path: Some(destination.clone()),
				content_indexing: Some(record.content_indexing),
			})
			.map_err(ActionError::InvalidInput)?,
		)
		.await?;
		let space = created.space;
		let claim = if registered == Some(space.source_id) {
			Claim::Registered
		} else if created.catalog_reused {
			Claim::Adopted
		} else {
			Claim::Fresh
		};
		let result = publish(
			&library,
			&context,
			&file,
			&record,
			space.clone(),
			staging.path(),
		)
		.await;
		match result {
			Ok((space, changes, counts)) => Ok(SpaceImportOutput {
				summary: summary(current.len() as u64, changes, &counts),
				space,
			}),
			Err(error) => {
				unwind(&library, &context, &space, claim, destination, existed).await;
				Err(error)
			}
		}
	}

	fn action_kind(&self) -> &'static str {
		"spaces.import"
	}
}

crate::register_library_action!(SpaceImportAction, "spaces.import");

/// Move the checked files into the new space and bring its change log and
/// versions across. Answers the space as it ends up and what came across.
async fn publish(
	library: &Arc<Library>,
	context: &Arc<CoreContext>,
	file: &SpaceFile,
	record: &Record,
	space: Space,
	staged: &Path,
) -> Result<(Space, u64, SpaceFileCounts), ActionError> {
	let internal = |e: &dyn std::fmt::Display| ActionError::Internal(e.to_string());
	let folder = space
		.path
		.clone()
		.ok_or_else(|| ActionError::Internal("the new space has no folder".to_string()))?;
	// The files land as part of the import, not as changes made outside
	// Spacedrive.
	let held = events::hold(&[folder.as_path()]);
	let mut entries = tokio::fs::read_dir(staged)
		.await
		.map_err(|e| internal(&e))?;
	while let Some(entry) = entries.next_entry().await.map_err(|e| internal(&e))? {
		let (from, to) = (entry.path(), folder.join(entry.file_name()));
		tokio::task::spawn_blocking(move || move_tree(&from, &to))
			.await
			.map_err(|e| internal(&e))?
			.map_err(|e| internal(&e))?;
	}
	drop(held);

	if let Some(json) = file.meta("actors").await.map_err(|e| internal(&e))? {
		for actor in serde_json::from_str::<Vec<Actor>>(&json).unwrap_or_default() {
			actors::remember(library, &actor)
				.await
				.map_err(ActionError::Internal)?;
		}
	}
	let imported = file.events().await.map_err(|e| internal(&e))?;
	let mut seqs = HashMap::new();
	let log = events::log_of(context, space.source_id, space.id)
		.await
		.ok_or_else(|| {
			ActionError::Internal("could not open the new space's change log".to_string())
		})?;
	for event in &imported {
		let seq = log
			.append(&sd_store::SpaceEvent {
				space_id: space.id,
				undo_id: None,
				..event.clone()
			})
			.await
			.map_err(|e| internal(&e))?;
		seqs.insert(event.seq, seq);
	}

	if !record.versioning {
		return Ok((space, imported.len() as u64, SpaceFileCounts::default()));
	}
	let versions = versioning::open(context, space.source_id, space.id)
		.await
		.ok_or_else(|| {
			ActionError::Internal("could not open the new space's versions".to_string())
		})?;
	let counts = file
		.restore_versions(&versions, &seqs)
		.await
		.map_err(|e| internal(&e))?;
	let space = SpaceUpdateAction::from_input(SpaceUpdateInput {
		id: space.id,
		name: None,
		content_indexing: None,
		versioning: Some(true),
	})
	.map_err(ActionError::Internal)?
	.execute(library.clone(), context.clone())
	.await?
	.space;
	Ok((space, imported.len() as u64, counts))
}

/// How a failed import's space came by its source, which decides how much
/// of that source the import may take back.
#[derive(Clone, Copy)]
enum Claim {
	/// The source was registered before the import, so only the space goes.
	Registered,
	/// The import registered the source over a catalog already on disk, so
	/// the registration goes and the catalog stays.
	Adopted,
	/// The import made the source and its catalog, so both go.
	Fresh,
}

/// Take back a failed import: the space, its versions, what the import
/// added to the source, and the files it moved in. A destination that
/// existed before stays, empty, as it was.
async fn unwind(
	library: &Arc<Library>,
	context: &Arc<CoreContext>,
	space: &Space,
	claim: Claim,
	destination: &Path,
	existed: bool,
) {
	if matches!(claim, Claim::Fresh) {
		versioning::close(space.id).await;
	} else if let Err(error) = versioning::discard(context, space.source_id, space.id).await {
		warn!(space = %space.id, %error, "could not delete the versions of a failed import");
	}
	// A catalog that stays must not keep the space's row, so the space is
	// deleted from it before the registration goes.
	let mut removed = Ok(());
	if !matches!(claim, Claim::Fresh) {
		removed = match SpaceDeleteAction::from_input(SpaceDeleteInput { id: space.id }) {
			Ok(action) => action
				.execute(library.clone(), context.clone())
				.await
				.map(drop),
			Err(error) => Err(ActionError::Internal(error)),
		};
	}
	if !matches!(claim, Claim::Registered) && removed.is_ok() {
		removed = match DeleteSourceAction::from_input(DeleteSourceInput {
			source_id: space.source_id.to_string(),
			delete_catalog: matches!(claim, Claim::Fresh),
		}) {
			Ok(action) => action
				.execute(library.clone(), context.clone())
				.await
				.map(drop),
			Err(error) => Err(ActionError::Internal(error)),
		};
	}
	if let Err(error) = removed {
		warn!(space = %space.id, ?error, "could not remove the space of a failed import");
	}
	let cleared = if existed {
		clear(destination).await
	} else {
		tokio::fs::remove_dir_all(destination).await
	};
	if let Err(error) = cleared {
		warn!(path = %destination.display(), %error, "could not remove the files of a failed import");
	}
}

/// Move a staged file or folder into place. The data directory and the
/// destination can sit on different disks, where a rename cannot reach, so
/// that case copies and then removes the staged copy.
fn move_tree(from: &Path, to: &Path) -> std::io::Result<()> {
	match std::fs::rename(from, to) {
		Err(error) if error.kind() == std::io::ErrorKind::CrossesDevices => {}
		other => return other,
	}
	if std::fs::symlink_metadata(from)?.is_dir() {
		std::fs::create_dir(to)?;
		for entry in std::fs::read_dir(from)? {
			let entry = entry?;
			move_tree(&entry.path(), &to.join(entry.file_name()))?;
		}
		std::fs::remove_dir(from)
	} else {
		std::fs::copy(from, to)?;
		std::fs::remove_file(from)
	}
}

/// Remove everything inside `folder`, leaving the folder.
async fn clear(folder: &Path) -> std::io::Result<()> {
	let mut entries = tokio::fs::read_dir(folder).await?;
	while let Some(entry) = entries.next_entry().await? {
		if entry.file_type().await?.is_dir() {
			tokio::fs::remove_dir_all(entry.path()).await?;
		} else {
			tokio::fs::remove_file(entry.path()).await?;
		}
	}
	Ok(())
}
