//! List ephemeral sidecars query
//!
//! Returns all sidecars (thumbnails, previews, etc.) for a specific ephemeral
//! entry. Scans the temp directory to find what derivatives exist.

use crate::{
	context::CoreContext,
	infra::query::{CoreQuery, QueryResult},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

/// Input for listing ephemeral sidecars
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListEphemeralSidecarsInput {
	/// Entry UUID to list sidecars for
	pub entry_uuid: Uuid,
	/// Library ID
	pub library_id: Uuid,
}

/// Information about a single ephemeral sidecar
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EphemeralSidecarInfo {
	/// Sidecar kind (e.g., "thumb", "preview", "transcript")
	pub kind: String,
	/// Sidecar variant (e.g., "grid@1x", "detail@2x")
	pub variant: String,
	/// File format (e.g., "webp", "mp4", "txt")
	pub format: String,
	/// File size in bytes
	pub size: u64,
	/// Relative path within temp directory (for debugging)
	pub path: String,
}

/// Output containing ephemeral sidecar information
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListEphemeralSidecarsOutput {
	/// List of sidecars found for this entry
	pub sidecars: Vec<EphemeralSidecarInfo>,
	/// Total number of sidecars
	pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListEphemeralSidecarsQuery {
	input: ListEphemeralSidecarsInput,
}

impl CoreQuery for ListEphemeralSidecarsQuery {
	type Input = ListEphemeralSidecarsInput;
	type Output = ListEphemeralSidecarsOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let cache = context.ephemeral_cache();
		let sidecar_cache = cache.get_sidecar_cache(self.input.library_id);

		// Get the entry directory
		let entry_dir = sidecar_cache.compute_entry_dir(&self.input.entry_uuid);

		if !tokio::fs::try_exists(&entry_dir).await? {
			return Ok(ListEphemeralSidecarsOutput {
				sidecars: Vec::new(),
				total: 0,
			});
		}

		let mut sidecars = Vec::new();

		// Scan the entry directory for sidecar kind directories
		let mut read_dir = tokio::fs::read_dir(&entry_dir).await?;
		while let Some(kind_entry) = read_dir.next_entry().await? {
			let kind_name = kind_entry.file_name().to_string_lossy().to_string();

			// Convert plural back to singular (thumbs -> thumb, etc.)
			let kind = if kind_name == "transcript" {
				kind_name.clone()
			} else {
				kind_name.trim_end_matches('s').to_string()
			};

			// Scan files within the kind directory
			let mut files_dir = tokio::fs::read_dir(kind_entry.path()).await?;
			while let Some(file_entry) = files_dir.next_entry().await? {
				let filename = file_entry.file_name().to_string_lossy().to_string();

				// Parse filename as "variant.format"
				if let Some((variant, format)) = filename.rsplit_once('.') {
					let metadata = file_entry.metadata().await?;
					let relative_path = file_entry
						.path()
						.strip_prefix(sidecar_cache.temp_root())
						.unwrap_or(&file_entry.path())
						.to_string_lossy()
						.to_string();

					sidecars.push(EphemeralSidecarInfo {
						kind: kind.clone(),
						variant: variant.to_string(),
						format: format.to_string(),
						size: metadata.len(),
						path: relative_path,
					});
				}
			}
		}

		let total = sidecars.len();

		Ok(ListEphemeralSidecarsOutput { sidecars, total })
	}
}

crate::register_core_query!(ListEphemeralSidecarsQuery, "core.ephemeral_sidecars.list");
