//! Persistent location responder.
//!
//! Thin adapter over `DatabaseAdapter` that translates filesystem
//! events into database mutations. The watcher calls `apply_batch` with events;
//! this module delegates to the unified change handling infrastructure.

use crate::context::CoreContext;
use crate::ops::indexing::change_detection::{self, ChangeConfig, DatabaseAdapter};
use crate::ops::indexing::rules::RuleToggles;
use anyhow::Result;
use sd_fs_watcher::FsEvent;
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

/// Translates a single filesystem event into database mutations.
///
/// Creates a `DatabaseAdapter` and delegates to the unified change
/// handling infrastructure in `change_detection`.
pub async fn apply(
	context: &Arc<CoreContext>,
	library_id: Uuid,
	location_id: Uuid,
	event: FsEvent,
	rule_toggles: RuleToggles,
	location_root: &Path,
	volume_backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
) -> Result<()> {
	apply_batch(
		context,
		library_id,
		location_id,
		vec![event],
		rule_toggles,
		location_root,
		volume_backend,
	)
	.await
}

/// Processes multiple filesystem events as a batch.
///
/// Creates a `DatabaseAdapter` and delegates to the unified
/// `change_detection::apply_batch` which handles deduplication, ordering,
/// and correct processing sequence (removes, renames, creates, modifies).
pub async fn apply_batch(
	context: &Arc<CoreContext>,
	library_id: Uuid,
	location_id: Uuid,
	events: Vec<FsEvent>,
	rule_toggles: RuleToggles,
	location_root: &Path,
	volume_backend: Option<&Arc<dyn crate::volume::VolumeBackend>>,
) -> Result<()> {
	if events.is_empty() {
		return Ok(());
	}

	tracing::debug!(
		"Responder received batch of {} events for location {}",
		events.len(),
		location_id
	);

	let mut handler = DatabaseAdapter::new(
		context.clone(),
		library_id,
		location_id,
		location_root,
		volume_backend.cloned(),
	)
	.await?;

	let config = ChangeConfig {
		rule_toggles,
		location_root,
		volume_backend,
	};

	change_detection::apply_batch(&mut handler, events, &config).await
}
