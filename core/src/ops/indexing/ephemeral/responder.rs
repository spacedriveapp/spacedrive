//! Ephemeral responder for updating in-memory indexes on filesystem changes.
//!
//! This module processes filesystem events against the ephemeral index cache.
//! When a user is browsing an ephemeral directory (external drive, network share)
//! and files change, the responder updates the in-memory index to reflect changes.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use sd_core::ops::indexing::ephemeral::responder;
//!
//! // Check if an event should be handled by the ephemeral system
//! if let Some(root) = responder::find_ephemeral_root(&path, &context) {
//!     responder::process_event(&context, &root, event_kind).await?;
//! }
//! ```

use crate::context::CoreContext;
use crate::infra::event::FsRawEventKind;
use crate::ops::indexing::change_detection::{self, ChangeConfig, EphemeralChangeHandler};
use crate::ops::indexing::rules::RuleToggles;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Check if a path falls under an ephemeral watched directory.
///
/// Returns the watched root path if found.
pub fn find_ephemeral_root(path: &Path, context: &CoreContext) -> Option<PathBuf> {
	context.ephemeral_cache().find_watched_root(path)
}

/// Check if any path in a batch of events falls under an ephemeral watched directory.
pub fn find_ephemeral_root_for_events(
	events: &[FsRawEventKind],
	context: &CoreContext,
) -> Option<PathBuf> {
	let paths: Vec<&Path> = events
		.iter()
		.flat_map(|e| match e {
			FsRawEventKind::Create { path } => vec![path.as_path()],
			FsRawEventKind::Modify { path } => vec![path.as_path()],
			FsRawEventKind::Remove { path } => vec![path.as_path()],
			FsRawEventKind::Rename { from, to } => vec![from.as_path(), to.as_path()],
		})
		.collect();

	context
		.ephemeral_cache()
		.find_watched_root_for_any(paths.into_iter())
}

/// Process a batch of filesystem events against the ephemeral index.
///
/// Creates an `EphemeralChangeHandler` and processes the events using shared
/// handler logic. The ephemeral index is updated in-place and ResourceChanged
/// events are emitted for UI updates.
pub async fn apply_batch(
	context: &Arc<CoreContext>,
	root_path: &Path,
	events: Vec<FsRawEventKind>,
	rule_toggles: RuleToggles,
) -> Result<()> {
	if events.is_empty() {
		return Ok(());
	}

	let index = context.ephemeral_cache().get_global_index();
	let event_bus = context.events.clone();

	let mut handler = EphemeralChangeHandler::new(index, event_bus, root_path.to_path_buf());

	let config = ChangeConfig {
		rule_toggles,
		location_root: root_path,
		volume_backend: None, // Ephemeral paths typically don't use volume backends
	};

	change_detection::apply_batch(&mut handler, events, &config).await
}

/// Process a single filesystem event against the ephemeral index.
pub async fn apply(
	context: &Arc<CoreContext>,
	root_path: &Path,
	event: FsRawEventKind,
	rule_toggles: RuleToggles,
) -> Result<()> {
	apply_batch(context, root_path, vec![event], rule_toggles).await
}

/// Register an ephemeral path for filesystem watching.
///
/// After calling this, filesystem events under the path will be detectable
/// via `find_ephemeral_root`. The path must already be indexed in the
/// ephemeral cache.
///
/// Returns true if registration succeeded, false if the path is not indexed.
pub fn register_for_watching(context: &CoreContext, path: PathBuf) -> bool {
	context.ephemeral_cache().register_for_watching(path)
}

/// Unregister an ephemeral path from filesystem watching.
pub fn unregister_from_watching(context: &CoreContext, path: &Path) {
	context.ephemeral_cache().unregister_from_watching(path)
}

/// Check if any ephemeral paths are being watched.
pub fn has_watched_paths(context: &CoreContext) -> bool {
	!context.ephemeral_cache().watched_paths().is_empty()
}

/// Get all currently watched ephemeral paths.
pub fn watched_paths(context: &CoreContext) -> Vec<PathBuf> {
	context.ephemeral_cache().watched_paths()
}

#[cfg(test)]
mod tests {
	use super::*;

	// Integration tests would require a full CoreContext setup
	// Unit tests for the helper functions are covered by index_cache tests
}
