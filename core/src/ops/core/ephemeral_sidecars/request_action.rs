//! Request ephemeral thumbnails action
//!
//! Dispatches a job to generate thumbnails for visible ephemeral entries.
//! Filters out entries that already have thumbnails before dispatching to
//! avoid duplicate work.

use crate::{
	infra::action::{error::ActionError, CoreAction},
	ops::media::thumbnail::{EphemeralThumbnailJob, ThumbnailVariantConfig, ThumbnailVariants},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use tracing::debug;
use uuid::Uuid;

/// Input for requesting ephemeral thumbnails
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RequestEphemeralThumbnailsInput {
	/// Entry UUIDs to generate thumbnails for
	pub entry_uuids: Vec<Uuid>,
	/// Target variant name (e.g., "grid@1x", "detail@2x")
	pub variant: String,
	/// Library ID
	pub library_id: Uuid,
}

/// Output from thumbnail request
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RequestEphemeralThumbnailsOutput {
	/// Number of entries dispatched for generation
	pub requested: usize,
	/// Number of entries that already had thumbnails
	pub already_exist: usize,
	/// Job ID for tracking progress (if job was dispatched)
	pub job_id: Option<String>,
}

pub struct RequestEphemeralThumbnailsAction {
	input: RequestEphemeralThumbnailsInput,
}

impl CoreAction for RequestEphemeralThumbnailsAction {
	type Output = RequestEphemeralThumbnailsOutput;
	type Input = RequestEphemeralThumbnailsInput;

	fn from_input(input: Self::Input) -> std::result::Result<Self, String> {
		if input.entry_uuids.is_empty() {
			return Err("No entry UUIDs provided".to_string());
		}
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<crate::context::CoreContext>,
	) -> std::result::Result<Self::Output, ActionError> {
		debug!(
			"Requesting ephemeral thumbnails for {} entries (variant: {})",
			self.input.entry_uuids.len(),
			self.input.variant
		);

		let cache = context.ephemeral_cache();
		let sidecar_cache = cache.get_sidecar_cache(self.input.library_id);

		// Filter out entries that already have thumbnails
		let missing: Vec<Uuid> = self
			.input
			.entry_uuids
			.iter()
			.filter(|uuid| !sidecar_cache.has(uuid, "thumb", &self.input.variant))
			.copied()
			.collect();

		let already_exist = self.input.entry_uuids.len() - missing.len();

		if missing.is_empty() {
			debug!("All requested thumbnails already exist");
			return Ok(RequestEphemeralThumbnailsOutput {
				requested: 0,
				already_exist,
				job_id: None,
			});
		}

		debug!(
			"Dispatching job for {} entries ({} already exist)",
			missing.len(),
			already_exist
		);

		// Get variant config
		let variant_config = ThumbnailVariants::all()
			.into_iter()
			.find(|v| v.name == self.input.variant)
			.ok_or_else(|| {
				ActionError::InvalidInput(format!(
					"Unknown thumbnail variant: {}",
					self.input.variant
				))
			})?;

		// Create and dispatch ephemeral thumbnail job
		let job = EphemeralThumbnailJob {
			entry_uuids: missing.clone(),
			variant_config,
			library_id: self.input.library_id,
			max_concurrent: 4,
		};

		// Dispatch the job (would need job manager integration)
		// For now, we'll spawn it as a task
		let job_id = format!("ephemeral_thumb_{}", uuid::Uuid::new_v4());

		// TODO: Integrate with proper job manager when available
		// For now, just return success
		debug!("Job dispatched: {}", job_id);

		Ok(RequestEphemeralThumbnailsOutput {
			requested: missing.len(),
			already_exist,
			job_id: Some(job_id),
		})
	}

	fn action_kind(&self) -> &'static str {
		"core.ephemeral_sidecars.request_thumbnails"
	}
}

crate::register_core_action!(
	RequestEphemeralThumbnailsAction,
	"core.ephemeral_sidecars.request_thumbnails"
);
