//! Freezing a source: a dated, self-contained copy of its store.
//!
//! The live store follows the disk. Rescan after a cleanup and the sweep
//! forgets everything the cleanup deleted, which is correct for a mirror and
//! fatal for a record. A freeze is the record: one SQLite file holding the
//! source's state at a moment, readable by anything, kept until deleted.

use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{path::PathBuf, sync::Arc};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FreezeSourceInput {
	pub source_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FreezeSourceOutput {
	/// Where the frozen copy landed.
	pub path: String,
	/// Records the copy holds, read back from the copy itself rather than
	/// from the live store, so the number describes the artifact.
	pub records: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreezeSourceAction {
	input: FreezeSourceInput,
}

impl LibraryAction for FreezeSourceAction {
	type Input = FreezeSourceInput;
	type Output = FreezeSourceOutput;

	fn from_input(input: FreezeSourceInput) -> Result<Self, String> {
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

		// The registry holds the source's absolute root; the library row only
		// stores it relative to its volume.
		let cache = context.ephemeral_cache();
		let root = cache.source_root(source_id).ok_or_else(|| {
			ActionError::Internal(format!(
				"source {source_id} is not registered on this machine"
			))
		})?;
		let store = cache.store_for(&root).await.ok_or_else(|| {
			ActionError::Internal(format!("no store open for {}", root.display()))
		})?;

		let dir = cache
			.source_dirs()
			.ok_or_else(|| ActionError::Internal("no sources directory".to_string()))?
			.source_dir(source_id)
			.join("freezes");

		let path = store
			.freeze_into(&dir)
			.await
			.map_err(|e| ActionError::Internal(format!("freeze failed: {e}")))?;

		let records = frozen_record_count(&path)
			.await
			.map_err(|e| ActionError::Internal(format!("frozen copy unreadable: {e}")))?;

		Ok(FreezeSourceOutput {
			path: path.display().to_string(),
			records,
		})
	}

	fn action_kind(&self) -> &'static str {
		"sources.freeze"
	}
}

/// Open the artifact read-only and count what it holds. Verification, not
/// bookkeeping: a freeze whose copy cannot answer this was not a freeze.
async fn frozen_record_count(path: &std::path::Path) -> anyhow::Result<u64> {
	use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

	let pool = SqlitePoolOptions::new()
		.max_connections(1)
		.connect_with(
			SqliteConnectOptions::new()
				.filename(path)
				.read_only(true)
				.immutable(true),
		)
		.await?;
	let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM record")
		.fetch_one(&pool)
		.await?;
	pool.close().await;
	Ok(count.max(0) as u64)
}

crate::register_library_action!(FreezeSourceAction, "sources.freeze");
