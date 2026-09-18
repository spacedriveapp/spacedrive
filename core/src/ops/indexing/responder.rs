//! Applies filesystem changes under a watched root to the volume index.
//!
//! The watcher's event handler decides which root an event belongs to; this
//! module turns the event into arena and store writes through an
//! [`ArenaWriter`], and recounts summarised directories a change fell under.

use crate::context::CoreContext;
use crate::ops::indexing::change_detection::{self, ChangeConfig};
use crate::ops::indexing::rules::RuleToggles;
use anyhow::Result;
use sd_fs_watcher::{FsEvent, FsEventKind};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{ArenaWriter, Seen};

/// Process a batch of filesystem events against the volume index.
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
		tracing::debug!("responder::apply_batch() called with empty events");
		return Ok(());
	}

	tracing::debug!(
		"responder::apply_batch() processing {} events for root: {}",
		events.len(),
		root_path.display()
	);

	let index = context.volume_index().resolve_index(root_path);
	let store = context.volume_index().store_for(root_path).await;
	let event_bus = context.events.clone();

	let mut writer = ArenaWriter::new(index, event_bus, store);

	let config = ChangeConfig {
		rule_toggles,
		root: root_path,
		volume_backend: None,
	};

	change_detection::apply_batch(&mut writer, events, &config).await
}

/// Process a single filesystem event against the volume index.
pub async fn apply(
	context: &Arc<CoreContext>,
	root_path: &Path,
	event: FsEvent,
	rule_toggles: RuleToggles,
) -> Result<()> {
	tracing::debug!(
		"responder::apply() called for root: {}, event: {:?}",
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

	let cache = context.volume_index();
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
