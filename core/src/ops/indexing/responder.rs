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

	apply_to_index(
		context.volume_index(),
		context.events.clone(),
		root_path,
		events,
		rule_toggles,
	)
	.await
}

async fn apply_to_index(
	cache: &crate::ops::indexing::VolumeIndex,
	event_bus: Arc<crate::infra::event::EventBus>,
	root_path: &Path,
	events: Vec<FsEvent>,
	default_rules: RuleToggles,
) -> Result<()> {
	let mut groups: Vec<(Option<uuid::Uuid>, Vec<FsEvent>)> = Vec::new();
	for event in events {
		if let FsEventKind::Rename { from, to } = &event.kind {
			if cache.source_id_for(from) != cache.source_id_for(to) {
				for (_, group) in groups.drain(..) {
					apply_group(cache, &event_bus, root_path, group, default_rules).await?;
				}
				apply_group(
					cache,
					&event_bus,
					root_path,
					vec![FsEvent::remove(from.clone())],
					default_rules,
				)
				.await?;
				apply_group(cache, &event_bus, root_path, vec![event], default_rules).await?;
				continue;
			}
		}
		let source_id = cache.source_id_for(&event.path);
		if let Some((_, group)) = groups.iter_mut().find(|(id, _)| *id == source_id) {
			group.push(event);
		} else {
			groups.push((source_id, vec![event]));
		}
	}
	for (_, group) in groups {
		apply_group(cache, &event_bus, root_path, group, default_rules).await?;
	}
	Ok(())
}

async fn apply_group(
	cache: &crate::ops::indexing::VolumeIndex,
	event_bus: &Arc<crate::infra::event::EventBus>,
	root_path: &Path,
	events: Vec<FsEvent>,
	default_rules: RuleToggles,
) -> Result<()> {
	let path = &events[0].path;
	let source_root = cache.source_root_for(path);
	let rules = source_root
		.as_ref()
		.map(|_| cache.rule_toggles_for(path))
		.unwrap_or(default_rules);
	let index = cache.resolve_index(path);
	let store = cache.store_for(path).await;
	let mut writer = ArenaWriter::new(index, event_bus.clone(), store);
	let config = ChangeConfig {
		rule_toggles: rules,
		root: source_root.as_deref().unwrap_or(root_path),
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

#[cfg(test)]
mod tests {
	use super::*;
	use crate::infra::event::EventBus;
	use crate::ops::indexing::VolumeIndex;

	#[tokio::test]
	async fn ancestor_watch_routes_changes_to_nested_source_store() {
		let data = tempfile::tempdir().unwrap();
		let drive = tempfile::tempdir().unwrap();
		let source = drive.path().join("source");
		std::fs::create_dir(&source).unwrap();
		let cache = VolumeIndex::with_sources_dir(Some(data.path().to_path_buf())).unwrap();
		let id = cache.register_source(&source, None).await.unwrap();
		let events = Arc::new(EventBus::new(128));
		let original = source.join("before.txt");
		std::fs::write(&original, b"fixture").unwrap();
		apply_to_index(
			&cache,
			events.clone(),
			drive.path(),
			vec![FsEvent::create(original.clone())],
			RuleToggles::none(),
		)
		.await
		.unwrap();
		let store = cache
			.open_stores()
			.into_iter()
			.find(|store| store.id() == id)
			.expect("source writer");
		store.read_barrier().await.unwrap();
		let original_id = store
			.db()
			.resolve_path("before.txt")
			.await
			.unwrap()
			.expect("original identity");

		let renamed = source.join("after.txt");
		std::fs::rename(&original, &renamed).unwrap();
		apply_to_index(
			&cache,
			events.clone(),
			drive.path(),
			vec![
				FsEvent::remove(original.clone()),
				FsEvent::rename(original, renamed.clone()),
			],
			RuleToggles::none(),
		)
		.await
		.unwrap();
		store.read_barrier().await.unwrap();
		assert!(store
			.db()
			.resolve_path("before.txt")
			.await
			.unwrap()
			.is_none());
		assert_eq!(
			store.db().resolve_path("after.txt").await.unwrap(),
			Some(original_id)
		);

		let outside = drive.path().join("outside.txt");
		std::fs::rename(&renamed, &outside).unwrap();
		apply_to_index(
			&cache,
			events.clone(),
			drive.path(),
			vec![FsEvent::rename(renamed, outside.clone())],
			RuleToggles::none(),
		)
		.await
		.unwrap();
		store.read_barrier().await.unwrap();
		assert!(store
			.db()
			.resolve_path("after.txt")
			.await
			.unwrap()
			.is_none());

		let other_root = drive.path().join("other");
		std::fs::create_dir(&other_root).unwrap();
		let other_id = cache.register_source(&other_root, None).await.unwrap();
		let other_file = other_root.join("transferred.txt");
		std::fs::rename(&outside, &other_file).unwrap();
		apply_to_index(
			&cache,
			events.clone(),
			drive.path(),
			vec![FsEvent::rename(outside, other_file.clone())],
			RuleToggles::none(),
		)
		.await
		.unwrap();
		let other_store = cache
			.open_stores()
			.into_iter()
			.find(|store| store.id() == other_id)
			.expect("other writer");
		other_store.read_barrier().await.unwrap();
		assert!(other_store
			.db()
			.resolve_path("transferred.txt")
			.await
			.unwrap()
			.is_some());

		let returned = source.join("returned.txt");
		std::fs::rename(&other_file, &returned).unwrap();
		apply_to_index(
			&cache,
			events.clone(),
			drive.path(),
			vec![FsEvent::rename(other_file, returned)],
			RuleToggles::none(),
		)
		.await
		.unwrap();
		other_store.read_barrier().await.unwrap();
		store.read_barrier().await.unwrap();
		assert!(other_store
			.db()
			.resolve_path("transferred.txt")
			.await
			.unwrap()
			.is_none());
		assert!(store
			.db()
			.resolve_path("returned.txt")
			.await
			.unwrap()
			.is_some());

		let old_dir = source.join("tree");
		std::fs::create_dir_all(old_dir.join("deep/level")).unwrap();
		std::fs::write(old_dir.join("deep/level/file.txt"), b"fixture").unwrap();
		apply_to_index(
			&cache,
			events.clone(),
			drive.path(),
			vec![
				FsEvent::create(old_dir.clone()),
				FsEvent::create(old_dir.join("deep")),
				FsEvent::create(old_dir.join("deep/level")),
				FsEvent::create(old_dir.join("deep/level/file.txt")),
			],
			RuleToggles::none(),
		)
		.await
		.unwrap();
		let new_dir = other_root.join("tree");
		std::fs::rename(&old_dir, &new_dir).unwrap();
		apply_to_index(
			&cache,
			events,
			drive.path(),
			vec![FsEvent::rename(old_dir, new_dir)],
			RuleToggles::none(),
		)
		.await
		.unwrap();
		other_store.read_barrier().await.unwrap();
		store.read_barrier().await.unwrap();
		assert!(other_store
			.db()
			.resolve_path("tree/deep/level/file.txt")
			.await
			.unwrap()
			.is_some());
		assert!(store
			.db()
			.resolve_path("tree/deep/level/file.txt")
			.await
			.unwrap()
			.is_none());
	}
}
