//! # Library restore
//!
//! Puts a backup's library and source stores onto this data directory,
//! either in place of an existing library or as a new one. Nothing in the
//! data directory is touched until every file has matched its manifest hash
//! and the backup's schema is one this build knows. The files are staged
//! under the data directory first and renamed into place, so each swap is
//! one rename, and whatever a replace displaces is moved to
//! `restore-trash/` rather than deleted.
//!
//! Replacing a library other devices are members of is refused without
//! `force`. The library carries those devices' rows and the sync watermarks
//! recording what each peer has been sent; restoring an older copy rewinds
//! both, so peers would hold changes the library no longer remembers
//! sending and would never be offered the changes made since the backup.

use super::input::{LibraryRestoreInput, RestoreMode};
use super::manifest::BackupManifest;
use super::output::LibraryRestoreOutput;
use super::snapshot;
use super::verify::OpenedBackup;
use crate::{
	context::CoreContext,
	infra::{
		action::{error::ActionError, CoreAction},
		db::entities::device,
	},
	library::{LibraryConfig, LIBRARY_DB_FILENAME},
};
use sea_orm::EntityTrait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryRestoreAction {
	input: LibraryRestoreInput,
}

impl CoreAction for LibraryRestoreAction {
	type Input = LibraryRestoreInput;
	type Output = LibraryRestoreOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		if input.source.as_os_str().is_empty() {
			return Err("source cannot be empty".to_string());
		}
		Ok(Self { input })
	}

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
		let scratch = context.data_dir.join("restore-staging");
		tokio::fs::create_dir_all(&scratch)
			.await
			.map_err(|e| ActionError::Internal(format!("create {}: {e}", scratch.display())))?;
		let opened = OpenedBackup::open(&self.input.source, &scratch)
			.await
			.map_err(|e| ActionError::Validation {
				field: "source".to_string(),
				message: e,
			})?;

		// Staging is a copy of the backup, so the backup stays intact
		// whatever happens, and every move below is a rename inside the
		// data directory.
		let stage = match opened.unpacked.clone() {
			Some(dir) => dir,
			None => {
				let dir = scratch.join(format!("stage-{}", Uuid::now_v7().simple()));
				if let Err(error) = snapshot::copy_dir(&opened.root, &dir).await {
					let _ = tokio::fs::remove_dir_all(&dir).await;
					return Err(ActionError::Internal(error));
				}
				dir
			}
		};

		let result = self
			.restore(&context, &opened.manifest, &opened.root, &stage)
			.await;
		let _ = tokio::fs::remove_dir_all(&stage).await;
		result
	}

	fn action_kind(&self) -> &'static str {
		"library.restore"
	}
}

impl LibraryRestoreAction {
	async fn restore(
		&self,
		context: &Arc<CoreContext>,
		manifest: &BackupManifest,
		root: &Path,
		stage: &Path,
	) -> Result<LibraryRestoreOutput, ActionError> {
		let refuse = |message: String| ActionError::Validation {
			field: "source".to_string(),
			message,
		};

		let failures = manifest.verify(root).await;
		if !failures.is_empty() {
			let listed: Vec<String> = failures
				.iter()
				.map(|(path, reason)| format!("{path}: {reason}"))
				.collect();
			return Err(refuse(format!(
				"backup does not match its manifest; refusing to restore: {}",
				listed.join("; ")
			)));
		}
		let unknown = manifest.unknown_migrations();
		if !unknown.is_empty() {
			return Err(refuse(format!(
				"backup was written by a newer build: it applied migrations this build does not know ({}). Upgrade before restoring",
				unknown.join(", ")
			)));
		}

		let libraries = context.libraries().await;
		let libraries_dir = libraries
			.libraries_dir()
			.ok_or_else(|| ActionError::Internal("no libraries directory".to_string()))?
			.to_path_buf();
		let this_device = context
			.device_manager
			.device_id()
			.map_err(|e| ActionError::DeviceManager(e.to_string()))?;
		let target_id = self.input.library_id.unwrap_or(manifest.library.id);

		let open = libraries.get_library(target_id).await;
		let existing_path = match &open {
			Some(library) => Some(library.path().to_path_buf()),
			None => libraries
				.scan_for_libraries()
				.await
				.map_err(|e| ActionError::Internal(e.to_string()))?
				.into_iter()
				.find(|found| found.config.id == target_id)
				.map(|found| found.path),
		};

		let source_dirs =
			crate::infra::source_dirs::SourceDirs::new(context.data_dir.join("sources"))
				.map_err(|e| ActionError::Internal(e.to_string()))?;
		let restored_sources: Vec<Uuid> = manifest
			.sources
			.iter()
			.filter(|source| source.store.is_some() || source.has_sidecars)
			.map(|source| source.id)
			.collect();

		let final_path = match self.input.mode {
			RestoreMode::Replace => {
				let Some(path) = existing_path.clone() else {
					return Err(refuse(format!(
						"no library {target_id} to replace on this device; restore as new instead"
					)));
				};
				let members = match &open {
					Some(library) => device::Entity::find()
						.all(library.db().conn())
						.await
						.map_err(|e| ActionError::Database(e.to_string()))?
						.into_iter()
						.filter(|row| row.uuid != this_device)
						.map(|row| format!("{} ({})", row.name, row.uuid))
						.collect::<Vec<_>>(),
					None => {
						snapshot::other_member_devices(&path.join(LIBRARY_DB_FILENAME), this_device)
							.await
							.map_err(ActionError::Internal)?
					}
				};
				if !members.is_empty() && !self.input.force {
					return Err(refuse(format!(
						"library {target_id} has other member devices ({}); their device rows and sync watermarks live in it and would diverge from what those peers hold. Pass force to replace it anyway",
						members.join(", ")
					)));
				}
				path
			}
			RestoreMode::New => {
				if existing_path.is_some() {
					return Err(refuse(format!(
						"library {target_id} already exists on this device; replace it, or restore under a different library id"
					)));
				}
				let in_the_way: Vec<String> = restored_sources
					.iter()
					.filter(|id| source_dirs.source_dir(**id).join("data.db").exists())
					.map(|id| id.to_string())
					.collect();
				if !in_the_way.is_empty() {
					return Err(refuse(format!(
						"source stores already exist on this device for {}; a new library cannot adopt them. Restore in replace mode or onto a fresh data directory",
						in_the_way.join(", ")
					)));
				}
				let base = crate::library::sanitize_filename(&manifest.library.name);
				let mut path = libraries_dir.join(format!("{base}.sdlibrary"));
				if path.exists() {
					path = libraries_dir.join(format!("{base}-{}.sdlibrary", target_id.simple()));
				}
				path
			}
		};

		// The hash gate proves the copy is what was written; this proves
		// what was written is a config the reopen will accept, by loading it
		// the way the reopen does.
		let config_path = stage.join("library").join("library.json");
		let mut config = LibraryConfig::load(&config_path)
			.await
			.map_err(|e| refuse(format!("backup library.json will not load: {e}")))?;
		if target_id != manifest.library.id {
			config.id = target_id;
			let json = serde_json::to_vec_pretty(&config)?;
			tokio::fs::write(&config_path, json)
				.await
				.map_err(|e| ActionError::Internal(format!("write staged config: {e}")))?;
		}

		// From here on the data directory changes. Each step is a rename,
		// and what a replace displaces goes to the trash directory named in
		// the output rather than being deleted.
		let _creating = libraries.mark_creating(final_path.clone()).await;
		let trash = context.data_dir.join("restore-trash").join(format!(
			"{}-{}",
			target_id.simple(),
			chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
		));

		let mut quiesced: Vec<Uuid> = restored_sources.clone();
		if let Some(old_path) = &existing_path {
			quiesced.extend(
				snapshot::source_ids_of(&old_path.join(LIBRARY_DB_FILENAME))
					.await
					.unwrap_or_default(),
			);
		}
		// Resolved while the library is still open: closing it takes its
		// registrations out of the index, and the partitions and snapshots
		// to clear are found through them.
		let quiesce_targets = context.volume_index().quiesce_targets(&quiesced);
		if existing_path.is_some() {
			if open.is_some() {
				libraries
					.close_library(target_id)
					.await
					.map_err(|e| ActionError::Internal(format!("close library: {e}")))?;
			}
		}
		drop(open);
		// The hold keeps every store closed until the renames are done, so a
		// watcher event arriving mid-swap cannot reopen and cache the file
		// that is about to be moved to the trash.
		let (hold, snapshots_removed) = context
			.volume_index()
			.quiesce_stores(&quiesced, &quiesce_targets)
			.await;
		let sidecar_hold = context.thumbs.hold_sidecars(&quiesced).await;

		let swapped = swap_into_place(
			context,
			manifest,
			stage,
			existing_path.as_deref(),
			&final_path,
			&source_dirs,
			&trash,
		)
		.await;
		drop(sidecar_hold);
		drop(hold);
		let (sources, replaced) = swapped.map_err(|(error, displaced)| {
			ActionError::Internal(if displaced {
				format!(
					"{error}; the data directory is partly swapped and the displaced state is under {}",
					trash.display()
				)
			} else {
				error
			})
		})?;
		tracing::debug!(
			snapshots = ?snapshots_removed,
			"drive snapshots removed so the arena rebuilds from the restored stores"
		);

		let opened = libraries.open_library(&final_path, context.clone()).await;

		// The swap dropped the drive arenas these sources lived in and the
		// snapshots that would refill them, so a change under a source root
		// has nowhere to land until its map is rebuilt: the handler files an
		// event only where the arena holds the parent. Every source on those
		// drives gets its map back from its own store, another open
		// library's included, which also re-arms the watch over it, without
		// a walk re-hashing what the store knows. This runs whether or not
		// the reopen succeeded: the other libraries on the drive were never
		// part of the restore and must not lose their maps to its failure.
		let rebuilt = context
			.volume_index()
			.rebuild_quiesced(&quiesce_targets)
			.await;
		tracing::debug!(rebuilt, "maps rebuilt from their stores after the swap");

		let library = opened.map_err(|e| {
			ActionError::Internal(format!(
				"open restored library: {e}; the displaced state is under {}",
				trash.display()
			))
		})?;

		let output = LibraryRestoreOutput {
			library_id: library.id(),
			library_name: library.name().await,
			path: final_path,
			files: manifest.files.len() as u32,
			bytes: manifest.total_bytes(),
			sources,
			replaced_state: replaced.then_some(trash),
		};
		tracing::info!(
			library = %output.library_id,
			path = %output.path.display(),
			sources = output.sources,
			replaced = ?output.replaced_state,
			"library restored"
		);
		Ok(output)
	}
}

/// Every rename of the swap, in order: the old library out, the staged
/// library in, then each store, sidecar and replica file. Returns the
/// number of stores restored and whether anything was displaced; on error,
/// whether anything had been displaced by then.
async fn swap_into_place(
	context: &Arc<CoreContext>,
	manifest: &BackupManifest,
	stage: &Path,
	existing_path: Option<&Path>,
	final_path: &Path,
	source_dirs: &crate::infra::source_dirs::SourceDirs,
	trash: &Path,
) -> Result<(u32, bool), (String, bool)> {
	let mut replaced = false;
	if let Some(old_path) = existing_path {
		snapshot::move_path(old_path, &trash.join("library"))
			.await
			.map_err(|e| (e, false))?;
		replaced = true;
	}
	snapshot::move_path(&stage.join("library"), final_path)
		.await
		.map_err(|e| (e, replaced))?;

	let mut sources = 0u32;
	for source in &manifest.sources {
		let staged_dir = stage.join("sources").join(source.id.simple().to_string());
		let dest_dir = source_dirs
			.create_source_dir(source.id)
			.map_err(|e| (e.to_string(), replaced))?;
		let trash_dir = trash.join("sources").join(source.id.simple().to_string());
		if source.store.is_some() {
			replaced |= swap_db(&staged_dir, &dest_dir, &trash_dir, "data.db")
				.await
				.map_err(|e| (e, replaced))?;
			sources += 1;
		}
		if source.has_sidecars {
			replaced |= swap_db(&staged_dir, &dest_dir, &trash_dir, "sidecars.db")
				.await
				.map_err(|e| (e, replaced))?;
		}
	}

	let staged_replicas = stage.join("replicas");
	if staged_replicas.is_dir() {
		let replicas = context.data_dir.join("mounts-remote");
		for relative in snapshot::files_under(&staged_replicas).map_err(|e| (e, replaced))? {
			let dest = replicas.join(&relative);
			if dest.exists() {
				snapshot::move_path(&dest, &trash.join("replicas").join(&relative))
					.await
					.map_err(|e| (e, replaced))?;
				replaced = true;
			}
			for suffix in ["-wal", "-shm"] {
				let journal = sidecar_of(&dest, suffix);
				if journal.exists() {
					let _ = tokio::fs::remove_file(journal).await;
				}
			}
			snapshot::move_path(&staged_replicas.join(&relative), &dest)
				.await
				.map_err(|e| (e, replaced))?;
		}
	}
	Ok((sources, replaced))
}

/// Move `name` from the staged source directory into place, parking the
/// file it displaces (and dropping its journal) in the trash directory.
/// Returns whether anything was displaced.
async fn swap_db(
	staged_dir: &Path,
	dest_dir: &Path,
	trash_dir: &Path,
	name: &str,
) -> Result<bool, String> {
	let dest = dest_dir.join(name);
	let mut replaced = false;
	if dest.exists() {
		snapshot::move_path(&dest, &trash_dir.join(name)).await?;
		replaced = true;
	}
	// A journal left beside a replaced database would be replayed into the
	// restored file on next open.
	for suffix in ["-wal", "-shm"] {
		let journal = sidecar_of(&dest, suffix);
		if journal.exists() {
			tokio::fs::remove_file(&journal)
				.await
				.map_err(|e| format!("remove {}: {e}", journal.display()))?;
		}
	}
	snapshot::move_path(&staged_dir.join(name), &dest).await?;
	Ok(replaced)
}

fn sidecar_of(path: &Path, suffix: &str) -> PathBuf {
	let mut name = path.as_os_str().to_os_string();
	name.push(suffix);
	PathBuf::from(name)
}

crate::register_core_action!(LibraryRestoreAction, "libraries.restore");
