//! # Warm-start reconcile
//!
//! A restart restores each source's map from its snapshot or its store, and the
//! watcher only hears about changes from that moment on. Whatever happened
//! while the daemon was stopped is invisible to both. This pass closes that
//! gap: it reads every directory of an attached source, compares size and
//! modification time with what the map holds, and applies the differences
//! through the same `ArenaWriter` the watcher uses, so the arena, the store and
//! the clients hear one description of each change.
//!
//! Unchanged files cost a `stat` and nothing else. Identity comes from the
//! store's ledger, so a file renamed while the daemon was stopped keeps its
//! record (and its tags) when its inode, size and modification time agree,
//! and only new or modified files are queued for hashing. One case resolves
//! fresh: two paths of different kinds swapped through a temporary name. Each
//! path still holds the other's old binding when the arrivals resolve, so
//! the swapped entries get new records and are hashed again.
//!
//! Removals follow the sweep rules every walk follows. Nothing is removed
//! unless the pass read every directory it needed to, a pass that would remove
//! more than half of what it compared removes nothing, and each removal is
//! checked against the disk once more before it is applied, since the watcher
//! is live while this runs.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::context::CoreContext;
use crate::ops::indexing::change_detection::handler::should_filter_path;
use crate::ops::indexing::metadata::{extract_metadata, EntryMetadata};
use crate::ops::indexing::state::EntryKind;
use crate::ops::indexing::{ArenaWriter, Notify, Seen};

/// Observations per writer batch.
const BATCH: usize = 1_000;

/// How long to wait for a map still filling from its store.
const FILL_WAIT: Duration = Duration::from_secs(600);

/// What one reconcile found and applied.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReconcileReport {
	pub directories: u64,
	pub compared: u64,
	pub added: u64,
	pub modified: u64,
	pub moved: u64,
	pub removed: u64,
	/// Removals withheld because the pass could not trust its own view.
	pub removals_refused: bool,
}

/// One entry a fresh read of a directory found.
struct OnDisk {
	path: PathBuf,
	kind: Option<EntryKind>,
	size: u64,
	modified: Option<SystemTime>,
}

fn seconds(time: Option<SystemTime>) -> Option<u64> {
	time.and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
		.map(|since| since.as_secs())
}

/// Read one directory. An entry whose metadata cannot be read is still
/// reported, with no kind, so it is never mistaken for a deletion.
fn read_directory(dir: &Path) -> std::io::Result<Vec<OnDisk>> {
	let mut entries = Vec::new();
	for entry in std::fs::read_dir(dir)? {
		let Ok(entry) = entry else {
			continue;
		};
		let path = entry.path();
		match std::fs::symlink_metadata(&path) {
			Ok(metadata) => entries.push(OnDisk {
				kind: Some(if metadata.is_dir() {
					EntryKind::Directory
				} else if metadata.is_symlink() {
					EntryKind::Symlink
				} else {
					EntryKind::File
				}),
				size: metadata.len(),
				modified: metadata.modified().ok(),
				path,
			}),
			Err(_) => entries.push(OnDisk {
				path,
				kind: None,
				size: 0,
				modified: None,
			}),
		}
	}
	Ok(entries)
}

/// Hand paths to the writer in batches and note the identities they resolve
/// to. Returns how many were still there to observe.
async fn arrive(writer: &ArenaWriter, paths: &[PathBuf], arrived: &mut HashSet<Uuid>) -> u64 {
	let mut count = 0;
	for chunk in paths.chunks(BATCH) {
		let mut batch: Vec<EntryMetadata> = Vec::with_capacity(chunk.len());
		for path in chunk {
			// Gone again since the read; the watcher answers for it.
			if let Ok(metadata) = extract_metadata(path, None).await {
				batch.push(metadata);
			}
		}
		count += batch.len() as u64;
		for change in writer.apply(Seen::Entries(batch)).await {
			arrived.insert(change.uuid);
		}
	}
	count
}

/// Reconcile every attached source with the disk, one after another.
///
/// Runs at startup once the maps are restored. A source with no map coverage
/// is left for the coverage heal, and one already being walked is left to its
/// walk.
pub async fn reconcile_attached_sources(context: Arc<CoreContext>) {
	let cache = context.volume_index();
	for source in cache.sources() {
		if !source.attached {
			continue;
		}
		reconcile_source(
			&context,
			source.library,
			&source.root,
			source.entry_count == Some(0),
		)
		.await;
	}
}

/// Bring one source's map and store in line with its disk.
///
/// `walked_empty` says a completed walk found the source empty, so a map
/// with nothing under the root is coverage rather than a gap.
pub async fn reconcile_source(
	context: &Arc<CoreContext>,
	library: Option<Uuid>,
	root: &Path,
	walked_empty: bool,
) -> Option<ReconcileReport> {
	let cache = context.volume_index();
	cache.ensure_restored(root).await;

	// A store rebuild fills the map in the background, and a restore can
	// start a browse of a root it holds nothing under; comparing against a
	// half-built map would read the rest as new.
	let busy = || cache.is_filling_from_store(root) || cache.is_indexing(root);
	let waited = Instant::now();
	while busy() && waited.elapsed() < FILL_WAIT {
		tokio::time::sleep(Duration::from_millis(200)).await;
	}
	if busy() {
		debug!(root = %root.display(), "not reconciling: the map is still being built");
		return None;
	}

	let index = cache.resolve_index(root);
	let covered = index
		.read()
		.await
		.known_children(root)
		.is_some_and(|children| !children.is_empty());
	if !covered && !walked_empty {
		debug!(root = %root.display(), "not reconciling: no map coverage, left to the heal");
		return None;
	}

	let started = Instant::now();
	let rule_toggles = cache.rule_toggles_for(root);
	let mut report = ReconcileReport::default();
	let mut changed: Vec<PathBuf> = Vec::new();
	let mut added: Vec<PathBuf> = Vec::new();
	let mut missing: Vec<(PathBuf, bool, u64)> = Vec::new();
	let mut replaced: Vec<(PathBuf, bool, u64)> = Vec::new();
	let mut unreadable = false;
	let mut known_entries: u64 = 0;

	let mut pending = VecDeque::from([root.to_path_buf()]);
	while let Some(dir) = pending.pop_front() {
		let read = {
			let dir = dir.clone();
			tokio::task::spawn_blocking(move || read_directory(&dir)).await
		};
		let on_disk = match read {
			Ok(Ok(entries)) => entries,
			Ok(Err(error)) => {
				warn!(dir = %dir.display(), %error, "reconcile could not read a directory");
				unreadable = true;
				continue;
			}
			Err(error) => {
				warn!(dir = %dir.display(), %error, "reconcile read task failed");
				unreadable = true;
				continue;
			}
		};
		report.directories += 1;

		let known = index.read().await.known_children(&dir);
		let mut known: std::collections::HashMap<PathBuf, crate::ops::indexing::arena::KnownChild> =
			known
				.unwrap_or_default()
				.into_iter()
				.map(|child| (child.path.clone(), child))
				.collect();
		known_entries += known.len() as u64;

		for entry in on_disk {
			if crate::config::is_managed(&entry.path) {
				known.remove(&entry.path);
				continue;
			}
			let Some(kind) = entry.kind else {
				known.remove(&entry.path);
				continue;
			};
			match known.remove(&entry.path) {
				Some(held) if held.kind == kind => {
					report.compared += 1;
					if kind == EntryKind::Directory {
						if !held.summarised {
							pending.push_back(entry.path);
						}
					} else if held.size != entry.size
						|| seconds(held.modified) != seconds(entry.modified)
					{
						changed.push(entry.path);
					}
				}
				held => {
					if let Some(held) = held {
						// A file replaced by a directory, or the reverse. The
						// path is there, so this is no absence for the fuse to
						// weigh, but the old entry and anything under it go.
						replaced.push((held.path, held.kind == EntryKind::Directory, held.entries));
					}
					if should_filter_path(&entry.path, rule_toggles, root, None)
						.await
						.unwrap_or(false)
					{
						continue;
					}
					if kind == EntryKind::Directory {
						pending.push_back(entry.path.clone());
					}
					added.push(entry.path);
				}
			}
		}

		for (path, held) in known {
			missing.push((path, held.kind == EntryKind::Directory, held.entries));
		}
	}

	let store = cache.store_for(root).await;
	let writer = ArenaWriter::new(index.clone(), context.events.clone(), store.clone())
		.notifying(Notify::Batched);
	let quiet = ArenaWriter::new(index.clone(), context.events.clone(), store.clone())
		.notifying(Notify::Silent);
	let loud = ArenaWriter::new(index.clone(), context.events.clone(), store.clone());

	// Parents come before their children in breadth-first order, so a new
	// directory's record exists before the files under it resolve. Arrivals
	// at or under a path whose kind changed wait until the old entry is
	// cleared, or a directory's old children would stay under a path that is
	// now a file. The rest arrive first, so a file that moved out of a
	// replaced directory is rebound before that directory is forgotten.
	let (after_replacement, added): (Vec<PathBuf>, Vec<PathBuf>) = added
		.into_iter()
		.partition(|path| replaced.iter().any(|(old, ..)| path.starts_with(old)));
	let mut arrived: HashSet<Uuid> = HashSet::new();
	report.added += arrive(&writer, &added, &mut arrived).await;
	report.modified += arrive(&writer, &changed, &mut arrived).await;

	// What leaves with each old path: every identity at or under it that did
	// not arrive somewhere else. An old path all of whose identities arrived
	// moved, and dropping it loses nothing. For a replaced path the path
	// itself is affirmatively gone, so only what was under it counts. A
	// missing directory was never descended into, so its subtree counts
	// toward what the map held only here.
	let mut doomed = 0u64;
	let mut moved = Vec::new();
	let mut leaving = Vec::new();
	let mut replacing = Vec::new();
	{
		let index = index.read().await;
		let not_arrived = |path: &Path| -> (bool, u64) {
			let identities = index.subtree_identities(path);
			let own = identities
				.first()
				.copied()
				.flatten()
				.is_some_and(|uuid| arrived.contains(&uuid));
			let beneath = identities
				.iter()
				.skip(1)
				.filter(|uuid| !uuid.is_some_and(|uuid| arrived.contains(&uuid)))
				.count() as u64;
			(own, beneath)
		};
		for (path, is_directory, entries) in missing {
			known_entries += entries - 1;
			let (own, beneath) = not_arrived(&path);
			let lost = beneath + u64::from(!own);
			doomed += lost;
			if lost == 0 {
				moved.push((path, is_directory));
			} else {
				leaving.push((path, is_directory, own));
			}
		}
		for (path, is_directory, entries) in replaced {
			known_entries += entries - 1;
			let (own, beneath) = not_arrived(&path);
			doomed += beneath;
			replacing.push((path, is_directory, own, beneath));
		}
	}

	let still_gone = |path: PathBuf| async move {
		matches!(
			tokio::fs::symlink_metadata(&path).await,
			Err(ref error) if error.kind() == std::io::ErrorKind::NotFound
		)
	};

	let destructive = !leaving.is_empty() || replacing.iter().any(|(.., beneath)| *beneath > 0);
	let walk_owns_removals = cache.is_indexing(root);
	let refused = if walk_owns_removals {
		debug!(root = %root.display(), "a walk started under the reconcile; its sweep owns removals");
		true
	} else if unreadable && destructive {
		warn!(
			root = %root.display(),
			withheld = leaving.len() + replacing.len(),
			"reconcile left removals alone: part of the source could not be read"
		);
		true
	} else if known_entries > 100 && doomed * 2 > known_entries {
		error!(
			root = %root.display(),
			doomed,
			known = known_entries,
			"reconcile refused its removals: it would remove more than half of the source"
		);
		true
	} else {
		false
	};

	// The record of a clean move lives on at its new path, so the old entry
	// is dropped without announcing it as deleted, whatever the safeguards
	// say, unless a walk now owns every removal.
	if !walk_owns_removals {
		for (path, is_directory) in moved {
			if still_gone(path.clone()).await {
				quiet.apply(Seen::Lost { path, is_directory }).await;
				report.moved += 1;
			}
		}
	}

	if refused {
		report.removals_refused = !leaving.is_empty() || !replacing.is_empty();
	} else {
		for (path, is_directory, own_moved) in leaving {
			if still_gone(path.clone()).await {
				let writer = if own_moved { &quiet } else { &loud };
				writer.apply(Seen::Lost { path, is_directory }).await;
				if own_moved {
					report.moved += 1;
				} else {
					report.removed += 1;
				}
			}
		}
		for (path, is_directory, own_moved, _) in replacing {
			let writer = if own_moved { &quiet } else { &loud };
			writer.apply(Seen::Lost { path, is_directory }).await;
			report.removed += 1;
		}
		report.added += arrive(&writer, &after_replacement, &mut arrived).await;
	}

	if let Some(store) = &store {
		if let Err(error) = store.flush().await {
			warn!(root = %root.display(), %error, "reconcile writes did not all land");
		}
	}

	info!(
		root = %root.display(),
		directories = report.directories,
		compared = report.compared,
		added = report.added,
		modified = report.modified,
		moved = report.moved,
		removed = report.removed,
		removals_refused = report.removals_refused,
		took_ms = started.elapsed().as_millis() as u64,
		"reconciled source with the disk"
	);

	if report.added + report.modified > 0 {
		queue_hashing(context, library, root).await;
	}
	Some(report)
}

/// Hash what the reconcile added or changed, as the watcher's nudge does.
async fn queue_hashing(context: &Arc<CoreContext>, library: Option<Uuid>, root: &Path) {
	let Some(library) = library else {
		return;
	};
	let Some(library) = context.libraries().await.get_library(library).await else {
		return;
	};
	let Some(store) = context.volume_index().store_for(root).await else {
		return;
	};
	if store.files_needing_content_count().await.unwrap_or(1) == 0 {
		return;
	}
	if let Err(error) = library
		.jobs()
		.dispatch_with_priority(
			crate::ops::indexing::content_identity::ContentIdentityJob::background(
				root.to_path_buf(),
			),
			crate::infra::job::types::JobPriority::LOW,
			None,
		)
		.await
	{
		warn!(root = %root.display(), %error, "could not queue hashing after the reconcile");
	}
}
