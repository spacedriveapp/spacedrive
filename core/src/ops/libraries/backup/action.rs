//! # Library backup
//!
//! Writes a library and every source store this device holds for it to one
//! directory or `.tar.zst` archive while the daemon keeps running. Each
//! database is copied with `VACUUM INTO`, so the copy is consistent and the
//! daemon's writers are never held longer than one copy's read transaction.
//! Every file is hashed and listed in `manifest.json`, which is what a
//! verify or a restore checks before trusting the copy.
//!
//! The backup is assembled in a staging directory beside the destination
//! and renamed into place last, so a destination that exists is complete.

use super::input::LibraryBackupInput;
use super::manifest::{
	BackupManifest, FileEntry, LibraryEntry, SourceEntry, MANIFEST_FILE, MANIFEST_FORMAT,
};
use super::output::LibraryBackupOutput;
use super::snapshot;
use crate::{
	context::CoreContext,
	infra::{
		action::{error::ActionError, LibraryAction},
		db::entities::source,
		event::Event,
	},
	library::Library,
};
use sea_orm::EntityTrait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryBackupAction {
	input: LibraryBackupInput,
}

/// One file the backup will hold.
struct Planned {
	relative: String,
	from: PathBuf,
	/// Replica manifests are plain JSON the peer sync writes atomically;
	/// everything else is SQLite and needs a consistent copy.
	sqlite: bool,
}

impl LibraryAction for LibraryBackupAction {
	type Input = LibraryBackupInput;
	type Output = LibraryBackupOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		if input.destination.as_os_str().is_empty() {
			return Err("destination cannot be empty".to_string());
		}
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let started = Instant::now();
		let destination = self.input.destination.clone();
		let archive = snapshot::is_archive(&destination);
		if destination.exists() && !(destination.is_dir() && dir_is_empty(&destination).await) {
			return Err(ActionError::Validation {
				field: "destination".to_string(),
				message: format!("{} already exists", destination.display()),
			});
		}
		let parent = destination.parent().filter(|p| !p.as_os_str().is_empty());
		let Some(parent) = parent else {
			return Err(ActionError::Validation {
				field: "destination".to_string(),
				message: "destination needs a parent directory".to_string(),
			});
		};
		tokio::fs::create_dir_all(parent)
			.await
			.map_err(|e| ActionError::Internal(format!("create {}: {e}", parent.display())))?;
		let name = destination
			.file_name()
			.map(|n| n.to_string_lossy().to_string())
			.unwrap_or_else(|| "backup".to_string());
		let staging = parent.join(format!(".{name}.staging-{}", Uuid::now_v7().simple()));

		let result = match self.write(&library, &context, &staging, started).await {
			Ok(done) => place(
				archive,
				&staging,
				&parent.join(format!(".{name}.partial")),
				&destination,
			)
			.await
			.map(|()| done),
			Err(error) => Err(error),
		};

		let (manifest, files) = match result {
			Ok(done) => done,
			Err(error) => {
				let _ = tokio::fs::remove_dir_all(&staging).await;
				return Err(ActionError::Internal(error));
			}
		};

		let manifest_path = if archive {
			PathBuf::from(MANIFEST_FILE)
		} else {
			destination.join(MANIFEST_FILE)
		};
		let output = LibraryBackupOutput {
			library_id: library.id(),
			destination,
			manifest_path,
			files,
			bytes: manifest.total_bytes(),
			duration_ms: started.elapsed().as_millis() as u64,
			sources_without_store: manifest
				.sources
				.iter()
				.filter(|s| s.store.is_none())
				.map(|s| s.id)
				.collect(),
		};
		tracing::info!(
			library = %output.library_id,
			destination = %output.destination.display(),
			files = output.files,
			bytes = output.bytes,
			duration_ms = output.duration_ms,
			"library backed up"
		);
		Ok(output)
	}

	fn action_kind(&self) -> &'static str {
		"library.backup"
	}
}

impl LibraryBackupAction {
	async fn write(
		&self,
		library: &Arc<Library>,
		context: &Arc<CoreContext>,
		staging: &Path,
		started: Instant,
	) -> Result<(BackupManifest, u32), String> {
		tokio::fs::create_dir_all(staging)
			.await
			.map_err(|e| format!("create staging {}: {e}", staging.display()))?;

		let library_id = library.id();
		let library_path = library.path().to_path_buf();
		let rows = source::Entity::find()
			.all(library.db().conn())
			.await
			.map_err(|e| format!("list sources: {e}"))?;
		let source_dirs =
			crate::infra::source_dirs::SourceDirs::new(context.data_dir.join("sources"))
				.map_err(|e| e.to_string())?;

		// What the copy holds is what has committed. A store with queued
		// writes would otherwise be copied from before the caller's last
		// observation landed.
		for store in context.volume_index().stores().await {
			if let Err(error) = store.flush().await {
				return Err(format!(
					"source store {} did not commit cleanly; refusing to back it up: {error}",
					store.id()
				));
			}
		}

		// The daemon rewrites library.json in place whenever statistics
		// change, so a byte copy can read a truncated file. The open
		// library's config is the authoritative state; serialize that.
		let config_copy = staging.join("library").join("library.json");
		tokio::fs::create_dir_all(staging.join("library"))
			.await
			.map_err(|e| format!("create staging library dir: {e}"))?;
		let config = library.config().await;
		tokio::fs::write(
			&config_copy,
			serde_json::to_vec_pretty(&config).map_err(|e| format!("serialize config: {e}"))?,
		)
		.await
		.map_err(|e| format!("write {}: {e}", config_copy.display()))?;
		let (bytes, blake3) = snapshot::hash_file(&config_copy).await?;
		let mut files = vec![FileEntry {
			path: "library/library.json".to_string(),
			bytes,
			blake3,
		}];

		let mut plan = vec![Planned {
			relative: "library/library.db".to_string(),
			from: library_path.join(crate::library::LIBRARY_DB_FILENAME),
			sqlite: true,
		}];
		let has_sync_db = library_path.join("sync.db").exists();
		if has_sync_db {
			plan.push(Planned {
				relative: "library/sync.db".to_string(),
				from: library_path.join("sync.db"),
				sqlite: true,
			});
		}

		let mut sources = Vec::with_capacity(rows.len());
		for row in &rows {
			let dir = source_dirs.source_dir(row.uuid);
			let store = dir.join("data.db");
			let sidecars = source_dirs.sidecars_file(row.uuid);
			let has_store = store.is_file();
			let has_sidecars = self.input.include_sidecars && sidecars.is_file();
			if has_store {
				plan.push(Planned {
					relative: format!("sources/{}/data.db", row.uuid.simple()),
					from: store,
					sqlite: true,
				});
			}
			if has_sidecars {
				plan.push(Planned {
					relative: format!("sources/{}/sidecars.db", row.uuid.simple()),
					from: sidecars,
					sqlite: true,
				});
			}
			sources.push(SourceEntry {
				id: row.uuid,
				name: row.name.clone(),
				data_type: row.data_type.clone(),
				root: row.root.clone(),
				store: None,
				has_sidecars,
			});
		}

		if self.input.include_replicas {
			let replicas = context.data_dir.join("mounts-remote");
			if replicas.is_dir() {
				for relative in snapshot::files_under(&replicas)? {
					let name = relative.to_string_lossy();
					if name.ends_with("-wal") || name.ends_with("-shm") || name.contains(".part") {
						continue;
					}
					let relative_str = relative
						.components()
						.map(|c| c.as_os_str().to_string_lossy().to_string())
						.collect::<Vec<_>>()
						.join("/");
					plan.push(Planned {
						relative: format!("replicas/{relative_str}"),
						from: replicas.join(&relative),
						sqlite: name.ends_with(".db"),
					});
				}
			}
		}

		let total = plan.len() as u32 + 1;
		for (index, planned) in plan.iter().enumerate() {
			context.events.emit(Event::Custom {
				event_type: "library.backup.progress".to_string(),
				data: serde_json::json!({
					"library_id": library_id,
					"phase": "copy",
					"file": planned.relative,
					"done": index as u32 + 1,
					"total": total,
					"elapsed_ms": started.elapsed().as_millis() as u64,
				}),
			});
			let to = BackupManifest::file_path(staging, &planned.relative);
			if planned.sqlite {
				snapshot::sqlite_copy(&planned.from, &to).await?;
			} else {
				snapshot::copy_file(&planned.from, &to).await?;
			}
			let (bytes, blake3) = snapshot::hash_file(&to).await?;
			files.push(FileEntry {
				path: planned.relative.clone(),
				bytes,
				blake3,
			});
		}

		for entry in &mut sources {
			let copy = staging
				.join("sources")
				.join(entry.id.simple().to_string())
				.join("data.db");
			if copy.is_file() {
				entry.store = Some(snapshot::store_identity(&copy).await?);
			}
		}

		let applied_migrations =
			snapshot::applied_migrations(&staging.join("library").join("library.db")).await?;
		let manifest = BackupManifest {
			format: MANIFEST_FORMAT,
			created_at: chrono::Utc::now(),
			device_id: context
				.device_manager
				.device_id()
				.map_err(|e| e.to_string())?,
			build_sha: option_env!("VERGEN_GIT_SHA")
				.unwrap_or("unknown")
				.to_string(),
			core_version: env!("CARGO_PKG_VERSION").to_string(),
			library: LibraryEntry {
				id: library_id,
				name: library.name().await,
				applied_migrations,
				has_sync_db,
			},
			include_sidecars: self.input.include_sidecars,
			include_replicas: self.input.include_replicas,
			sources,
			files,
		};
		manifest.save(&staging.join(MANIFEST_FILE)).await?;

		context.events.emit(Event::Custom {
			event_type: "library.backup.progress".to_string(),
			data: serde_json::json!({
				"library_id": library_id,
				"phase": "done",
				"done": total,
				"total": total,
				"elapsed_ms": started.elapsed().as_millis() as u64,
			}),
		});
		Ok((manifest, total))
	}
}

/// Move a finished staging directory to its destination: packed into an
/// archive, or renamed as the directory itself.
async fn place(
	archive: bool,
	staging: &Path,
	partial: &Path,
	destination: &Path,
) -> Result<(), String> {
	if archive {
		snapshot::pack(staging, partial).await?;
		tokio::fs::rename(partial, destination)
			.await
			.map_err(|e| format!("rename {} into place: {e}", partial.display()))?;
		tokio::fs::remove_dir_all(staging)
			.await
			.map_err(|e| format!("remove staging {}: {e}", staging.display()))?;
		return Ok(());
	}
	if destination.exists() {
		tokio::fs::remove_dir(destination)
			.await
			.map_err(|e| format!("replace empty {}: {e}", destination.display()))?;
	}
	tokio::fs::rename(staging, destination)
		.await
		.map_err(|e| format!("rename {} into place: {e}", staging.display()))
}

async fn dir_is_empty(dir: &Path) -> bool {
	match tokio::fs::read_dir(dir).await {
		Ok(mut entries) => matches!(entries.next_entry().await, Ok(None)),
		Err(_) => false,
	}
}

crate::register_library_action!(LibraryBackupAction, "libraries.backup");
