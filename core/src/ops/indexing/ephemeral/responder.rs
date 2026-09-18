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
//!     responder::process_event(&context, &root, event).await?;
//! }
//! ```

use crate::context::CoreContext;
use crate::ops::indexing::change_detection::{self, ChangeConfig};
use crate::ops::indexing::rules::RuleToggles;
use anyhow::Result;
use sd_fs_watcher::{FsEvent, FsEventKind};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{ArenaWriter, Seen};

/// Process a batch of filesystem events against the ephemeral index.
///
/// Creates an `ArenaWriter` and processes the events using shared handler
/// logic. The arena is updated in place, the source store hears the same
/// change, and clients are told.
pub async fn apply_batch(
	context: &Arc<CoreContext>,
	root_path: &Path,
	events: Vec<FsEvent>,
	rule_toggles: RuleToggles,
) -> Result<()> {
	if events.is_empty() {
		tracing::debug!("ephemeral::responder::apply_batch() called with empty events");
		return Ok(());
	}

	tracing::debug!(
		"ephemeral::responder::apply_batch() processing {} events for root: {}",
		events.len(),
		root_path.display()
	);

	let index = context.ephemeral_cache().resolve_index(root_path);
	let store = context.ephemeral_cache().store_for(root_path).await;
	let event_bus = context.events.clone();

	let mut writer = ArenaWriter::new(index, event_bus, store);

	let config = ChangeConfig {
		rule_toggles,
		location_root: root_path,
		volume_backend: None, // Ephemeral paths typically don't use volume backends
	};

	change_detection::apply_batch(&mut writer, events, &config).await
}

/// Process a single filesystem event against the ephemeral index.
pub async fn apply(
	context: &Arc<CoreContext>,
	root_path: &Path,
	event: FsEvent,
	rule_toggles: RuleToggles,
) -> Result<()> {
	tracing::debug!(
		"ephemeral::responder::apply() called for root: {}, event: {:?}",
		root_path.display(),
		event
	);
	apply_batch(context, root_path, vec![event], rule_toggles).await
}

/// Recount every summarised directory something has changed under.
///
/// A summary keeps no children, so a change beneath one has nothing to update
/// and the event carries no size to adjust the total by. What it does say is
/// that the count is wrong, and that is enough: the directory is marked, and
/// this is where it gets counted again. Debounced by its caller, so a file
/// being written in a loop costs one count rather than one per write.
///
/// Returns how many were recounted.
pub async fn recount_summaries(context: &Arc<CoreContext>) -> usize {
	use crate::ops::indexing::summary::count_subtree;

	let cache = context.ephemeral_cache();
	let mut recounted = 0;

	for path in cache.take_dirty_stubs() {
		let index = cache.resolve_index(&path);

		// A directory that has since been walked is no longer one a count can
		// speak for: it has children now, and they answer for themselves.
		if !index.read().await.is_summarised(&path) {
			continue;
		}

		let writer = ArenaWriter::new(index, context.events.clone(), cache.store_for(&path).await);

		// The change that marked it may have been the directory itself going
		// away, which is a deletion like any other.
		if !path.is_dir() {
			writer
				.apply(Seen::Lost {
					path,
					is_directory: true,
				})
				.await;
			continue;
		}

		let totals = count_subtree(path.clone()).await;
		writer.apply(Seen::Counted { path, totals }).await;
		recounted += 1;
	}

	recounted
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
