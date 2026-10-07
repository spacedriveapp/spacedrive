//! # Backup verification
//!
//! Hashes every file a backup's manifest lists and reports what differs,
//! without restoring anything. The same check runs at the start of a
//! restore; this query exists so a release gate or a nightly job can prove
//! a backup is intact where it sits.

use super::input::LibraryBackupVerifyInput;
use super::manifest::{BackupManifest, MANIFEST_FILE};
use super::output::{LibraryBackupVerifyOutput, VerifyFailure};
use super::snapshot;
use crate::{
	context::CoreContext,
	infra::query::{CoreQuery, QueryError, QueryResult},
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryBackupVerifyQuery {
	input: LibraryBackupVerifyInput,
}

/// A backup opened for reading: its root directory, and the directory to
/// remove afterwards when the backup was unpacked from an archive.
pub struct OpenedBackup {
	pub root: PathBuf,
	pub manifest: BackupManifest,
	pub unpacked: Option<PathBuf>,
}

impl OpenedBackup {
	/// Resolve `source` to a backup root: a directory, its manifest, or an
	/// archive unpacked under `scratch`.
	pub async fn open(source: &Path, scratch: &Path) -> Result<Self, String> {
		if snapshot::is_archive(source) {
			let dir = scratch.join(format!("unpack-{}", Uuid::now_v7().simple()));
			snapshot::unpack(source, &dir).await?;
			let manifest = BackupManifest::load(&dir.join(MANIFEST_FILE)).await?;
			return Ok(Self {
				root: dir.clone(),
				manifest,
				unpacked: Some(dir),
			});
		}
		let root = if source.is_file() {
			source
				.parent()
				.ok_or_else(|| format!("{} has no parent", source.display()))?
				.to_path_buf()
		} else {
			source.to_path_buf()
		};
		let manifest = BackupManifest::load(&root.join(MANIFEST_FILE)).await?;
		Ok(Self {
			root,
			manifest,
			unpacked: None,
		})
	}

	pub async fn discard(self) {
		if let Some(dir) = self.unpacked {
			let _ = tokio::fs::remove_dir_all(dir).await;
		}
	}
}

impl CoreQuery for LibraryBackupVerifyQuery {
	type Input = LibraryBackupVerifyInput;
	type Output = LibraryBackupVerifyOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let scratch = context.data_dir.join("restore-staging");
		let opened = OpenedBackup::open(&self.input.source, &scratch)
			.await
			.map_err(QueryError::Internal)?;
		let failures = opened.manifest.verify(&opened.root).await;
		let manifest = opened.manifest.clone();
		opened.discard().await;

		Ok(LibraryBackupVerifyOutput {
			library_id: manifest.library.id,
			library_name: manifest.library.name.clone(),
			created_at: manifest.created_at,
			build_sha: manifest.build_sha.clone(),
			files: manifest.files.len() as u32,
			bytes: manifest.total_bytes(),
			sources: manifest.sources.len() as u32,
			failures: failures
				.into_iter()
				.map(|(path, reason)| VerifyFailure { path, reason })
				.collect(),
			unknown_migrations: manifest.unknown_migrations(),
		})
	}
}

crate::register_core_query!(LibraryBackupVerifyQuery, "libraries.backup.verify");
