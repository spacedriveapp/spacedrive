//! Offline copies: the library's replica of a catalog that lives on its
//! source.
//!
//! A store placed on its drive leaves the library with nothing to answer
//! from when the drive is away. With `keep_offline_copy` set, the library
//! keeps a replica of that store under its own data directory, under the
//! same rules a peer's replica follows: the on-source store is the origin
//! and the only writer; the copy is a validated `VACUUM INTO` export named
//! by the origin's revision, published by rename only after it opens and
//! counts; a copy is never thrown away because the origin stopped answering.
//! What differs from a peer replica is only the transport: the origin is a
//! file on this machine, so there is no byterange stream, no partial
//! transfer to resume, and the bandwidth cap and pause switch, which exist
//! for the link, do not apply.
//!
//! The copy is refreshed on the peer replicas' cadence ([`super::REFRESH_INTERVAL`])
//! by comparing the origin's revision with the copy's, and at once when a
//! walk lands. A revision that moved between two polls is a store still
//! being written, and the copy waits for it to settle rather than
//! exporting a store every thirty seconds through a walk, up to
//! [`MAX_LAG_SECS`], after which a copy is taken regardless.
//!
//! While the drive is away, [`crate::ops::indexing::volume_index::VolumeIndex::read_store`]
//! answers from the copy, so listings and search are served from it and
//! `sources.list` says so. Turning the setting off removes the copy, but
//! only once the origin has been confirmed reachable: the copy may be the
//! only catalog this machine has, and a setting flipped while the drive is
//! in a drawer must not destroy it.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::context::CoreContext;
use crate::infra::event::{Event, EventBus};
use crate::infra::source_dirs::SourceDirs;
use crate::ops::indexing::sources::StorePlacement;
use crate::ops::indexing::volume_index::VolumeIndex;
use crate::service::network::protocol::byterange::revision_generation;

/// How long a store may keep moving before a copy is taken anyway.
pub const MAX_LAG_SECS: u64 = 600;

/// What the offline copy was taken from and when, written beside it so a
/// listing can describe the copy without opening it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfflineCopyManifest {
	pub source_id: Uuid,
	/// The origin store's identity and revision the copy was exported at.
	pub store_id: Uuid,
	pub revision: i64,
	/// The revision folded into the generation word peer replicas use.
	pub generation: u64,
	pub synced_at_secs: u64,
	pub record_count: u64,
	pub bytes: u64,
}

/// A source's offline copy as `sources.list` reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct OfflineCopyInfo {
	/// A validated copy is on disk.
	pub present: bool,
	/// Listings and search answer from the copy right now, because the
	/// origin is away.
	pub serving: bool,
	/// When the copy was taken, RFC 3339.
	pub last_synced: Option<String>,
	/// Changes the origin has committed since the copy was taken. Absent
	/// while the origin is away, when there is no copy, and when the origin
	/// is a different store than the copy came from.
	pub behind_by: Option<u64>,
	/// Records the copy holds.
	pub record_count: Option<u64>,
	pub bytes: Option<u64>,
}

/// Whether a pass copies at once or waits for the origin to settle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pace {
	/// Copy now if the origin moved.
	Now,
	/// Copy only once the origin's revision has held for one full poll, or
	/// the copy is older than [`MAX_LAG_SECS`].
	Settled,
}

/// What one pass over a source did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncOutcome {
	/// The source keeps no offline copy.
	NotKept,
	/// The origin is away or has no store yet; the copy, if any, stands.
	OriginAway,
	/// The copy already carries the origin's revision.
	Current,
	/// The origin is still moving; the copy waits.
	Settling,
	/// A new copy was published.
	Copied { revision: i64, record_count: u64 },
}

fn now_secs() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map(|d| d.as_secs())
		.unwrap_or(0)
}

/// One pass per source at a time: the cadence and a walk landing must not
/// export the same store twice into the same part file.
fn sync_lock(source_id: Uuid) -> Arc<tokio::sync::Mutex<()>> {
	static LOCKS: OnceLock<Mutex<HashMap<Uuid, Arc<tokio::sync::Mutex<()>>>>> = OnceLock::new();
	LOCKS
		.get_or_init(|| Mutex::new(HashMap::new()))
		.lock()
		.unwrap()
		.entry(source_id)
		.or_default()
		.clone()
}

/// The generation each source showed on the previous settled poll.
fn last_seen() -> &'static Mutex<HashMap<Uuid, u64>> {
	static SEEN: OnceLock<Mutex<HashMap<Uuid, u64>>> = OnceLock::new();
	SEEN.get_or_init(|| Mutex::new(HashMap::new()))
}

pub async fn read_manifest(dirs: &SourceDirs, source_id: Uuid) -> Option<OfflineCopyManifest> {
	let bytes = tokio::fs::read(dirs.offline_copy_manifest(source_id))
		.await
		.ok()?;
	let manifest: OfflineCopyManifest = serde_json::from_slice(&bytes).ok()?;
	(manifest.source_id == source_id && dirs.offline_copy_file(source_id).is_file())
		.then_some(manifest)
}

/// The copy's state for one source, or `None` for a source whose store is
/// not placed on it.
pub async fn info(cache: &VolumeIndex, source_id: Uuid) -> Option<OfflineCopyInfo> {
	let config = cache.source_config(source_id)?;
	if config.placement != StorePlacement::OnSource {
		return None;
	}
	let dirs = cache.source_dirs()?;
	let manifest = read_manifest(dirs, source_id).await;
	let serving = cache.offline_copy_wanted(source_id);
	// The origin's revision comes off the cached read handle, which is the
	// origin's whenever the copy is not being served.
	let behind_by = match &manifest {
		Some(manifest) if !serving && cache.origin_store_file(source_id).is_some() => {
			match cache.read_store(source_id).await {
				Some(db) => db
					.revision()
					.await
					.ok()
					.filter(|revision| revision.store_id == manifest.store_id)
					.map(|revision| revision.value.saturating_sub(manifest.revision).max(0) as u64),
				None => None,
			}
		}
		_ => None,
	};
	Some(OfflineCopyInfo {
		present: manifest.is_some(),
		serving,
		last_synced: manifest.as_ref().and_then(|m| {
			chrono::DateTime::<chrono::Utc>::from_timestamp(m.synced_at_secs as i64, 0)
				.map(|at| at.to_rfc3339())
		}),
		behind_by,
		record_count: manifest.as_ref().map(|m| m.record_count),
		bytes: manifest.as_ref().map(|m| m.bytes),
	})
}

/// Bring one source's offline copy up to the origin's revision.
///
/// The origin is flushed first when this machine holds its writer, so the
/// copy carries what has been committed rather than what is queued. The
/// export is validated by opening it read-only and counting its records
/// before it replaces the previous copy; a failed export leaves the previous
/// copy and its manifest untouched.
pub async fn sync_source(
	cache: &VolumeIndex,
	events: Option<&EventBus>,
	source_id: Uuid,
	pace: Pace,
) -> anyhow::Result<SyncOutcome> {
	let lock = sync_lock(source_id);
	let _guard = lock.lock().await;

	let Some(config) = cache.source_config(source_id) else {
		anyhow::bail!("source {source_id} is not registered on this machine");
	};
	if config.placement != StorePlacement::OnSource || !config.keep_offline_copy {
		return Ok(SyncOutcome::NotKept);
	}
	let Some(dirs) = cache.source_dirs() else {
		anyhow::bail!("no persistent source layout");
	};
	let Some(origin) = cache.origin_store_file(source_id) else {
		return Ok(SyncOutcome::OriginAway);
	};

	if let Some(store) = cache
		.open_stores()
		.into_iter()
		.find(|store| store.id() == source_id)
	{
		store
			.flush()
			.await
			.map_err(|e| anyhow::anyhow!("origin store did not commit cleanly: {e}"))?;
	}

	let db = sd_store::SourceManager::open_file_read_only(&origin)
		.await
		.map_err(|e| anyhow::anyhow!("origin store would not open: {e}"))?;
	let revision = match db.revision().await {
		Ok(revision) => revision,
		Err(e) => {
			db.pool().close().await;
			anyhow::bail!("origin revision unreadable: {e}");
		}
	};
	let generation = revision_generation(revision);
	let previous = read_manifest(dirs, source_id).await;
	if previous
		.as_ref()
		.is_some_and(|m| m.generation == generation)
	{
		db.pool().close().await;
		last_seen().lock().unwrap().insert(source_id, generation);
		return Ok(SyncOutcome::Current);
	}
	if pace == Pace::Settled {
		let seen = last_seen().lock().unwrap().insert(source_id, generation);
		let lagging = previous
			.as_ref()
			.is_none_or(|m| now_secs() >= m.synced_at_secs.saturating_add(MAX_LAG_SECS));
		if seen != Some(generation) && !lagging {
			db.pool().close().await;
			return Ok(SyncOutcome::Settling);
		}
	}

	let part = dirs.offline_copy_part(source_id);
	let _ = tokio::fs::remove_file(&part).await;
	if let Err(e) = tokio::fs::create_dir_all(dirs.source_dir(source_id)).await {
		db.pool().close().await;
		anyhow::bail!("create {}: {e}", dirs.source_dir(source_id).display());
	}
	let exported = sqlx::query("VACUUM INTO ?")
		.bind(part.to_string_lossy().into_owned())
		.execute(db.pool())
		.await;
	db.pool().close().await;
	if let Err(e) = exported {
		let _ = tokio::fs::remove_file(&part).await;
		anyhow::bail!("export failed: {e}");
	}

	// The open and the count are the gate: a copy that does not open as a
	// store never replaces one that does.
	let (record_count, bytes) = match validate(&part).await {
		Ok(validated) => validated,
		Err(e) => {
			let _ = tokio::fs::remove_file(&part).await;
			return Err(e);
		}
	};

	// Whatever reads the previous copy lets go before the file moves; a
	// handle on the origin stays, since other readers share it.
	cache.retire_offline_copy_reader(source_id).await;
	tokio::fs::rename(&part, dirs.offline_copy_file(source_id)).await?;
	let manifest = OfflineCopyManifest {
		source_id,
		store_id: revision.store_id,
		revision: revision.value,
		generation,
		synced_at_secs: now_secs(),
		record_count,
		bytes,
	};
	tokio::fs::write(
		dirs.offline_copy_manifest(source_id),
		serde_json::to_vec_pretty(&manifest)?,
	)
	.await?;
	last_seen().lock().unwrap().insert(source_id, generation);

	tracing::info!(
		source = %source_id,
		revision = revision.value,
		records = record_count,
		bytes,
		"offline copy published"
	);
	if let Some(events) = events {
		events.emit(Event::Custom {
			event_type: "sources.offline_copy".to_string(),
			data: serde_json::json!({
				"source_id": source_id,
				"revision": revision.value,
				"record_count": record_count,
				"bytes": bytes,
			}),
		});
	}
	Ok(SyncOutcome::Copied {
		revision: revision.value,
		record_count,
	})
}

async fn validate(part: &std::path::Path) -> anyhow::Result<(u64, u64)> {
	let copy = sd_store::SourceManager::open_file_read_only(part)
		.await
		.map_err(|e| anyhow::anyhow!("exported copy would not open: {e}"))?;
	let counted: Result<(i64,), _> = sqlx::query_as("SELECT COUNT(*) FROM record")
		.fetch_one(copy.pool())
		.await;
	copy.pool().close().await;
	let record_count = counted
		.map_err(|e| anyhow::anyhow!("exported copy would not count: {e}"))?
		.0
		.max(0) as u64;
	let bytes = tokio::fs::metadata(part).await?.len();
	Ok((record_count, bytes))
}

/// Remove a source's offline copy, once the origin is confirmed reachable.
///
/// The copy is removed only after the origin store opens on this machine,
/// so turning the setting off while the drive is away keeps the one
/// catalog the library can still answer from. The error names why the copy
/// was kept.
pub async fn remove(cache: &VolumeIndex, source_id: Uuid) -> anyhow::Result<()> {
	let lock = sync_lock(source_id);
	let _guard = lock.lock().await;
	let Some(dirs) = cache.source_dirs() else {
		return Ok(());
	};
	let copy = dirs.offline_copy_file(source_id);
	if !copy.exists() {
		let _ = tokio::fs::remove_file(dirs.offline_copy_manifest(source_id)).await;
		let _ = tokio::fs::remove_file(dirs.offline_copy_part(source_id)).await;
		return Ok(());
	}
	let Some(origin) = cache.origin_store_file(source_id) else {
		anyhow::bail!(
			"the source's own catalog is not reachable right now; the offline copy is kept until it is"
		);
	};
	match sd_store::SourceManager::open_file_read_only(&origin).await {
		Ok(db) => db.pool().close().await,
		Err(e) => anyhow::bail!(
			"the source's own catalog at {} would not open ({e}); the offline copy is kept",
			origin.display()
		),
	}

	cache.retire_offline_copy_reader(source_id).await;
	tokio::fs::remove_file(&copy).await?;
	let _ = tokio::fs::remove_file(dirs.offline_copy_manifest(source_id)).await;
	let _ = tokio::fs::remove_file(dirs.offline_copy_part(source_id)).await;
	last_seen().lock().unwrap().remove(&source_id);
	tracing::info!(source = %source_id, "offline copy removed");
	Ok(())
}

/// Delete a source's offline copy with its catalog, no origin check: the
/// caller is deleting the catalog itself.
pub async fn delete_with_catalog(cache: &VolumeIndex, source_id: Uuid) {
	let Some(dirs) = cache.source_dirs() else {
		return;
	};
	cache.retire_offline_copy_reader(source_id).await;
	let _ = tokio::fs::remove_file(dirs.offline_copy_file(source_id)).await;
	let _ = tokio::fs::remove_file(dirs.offline_copy_manifest(source_id)).await;
	let _ = tokio::fs::remove_file(dirs.offline_copy_part(source_id)).await;
}

/// Copy a source now, off the caller's path.
pub fn sync_soon(context: Arc<CoreContext>, source_id: Uuid) {
	tokio::spawn(async move {
		match sync_source(
			context.volume_index(),
			Some(&context.events),
			source_id,
			Pace::Now,
		)
		.await
		{
			Ok(_) => {}
			Err(e) => tracing::warn!(source = %source_id, %e, "offline copy not refreshed"),
		}
	});
}

/// Every source keeping an offline copy, polled on the replica cadence.
pub async fn watch(context: Arc<CoreContext>) {
	loop {
		tokio::time::sleep(super::REFRESH_INTERVAL).await;
		let cache = context.volume_index();
		for source in cache.sources() {
			let keeps = cache
				.source_config(source.id)
				.is_some_and(|c| c.placement == StorePlacement::OnSource && c.keep_offline_copy);
			if !keeps || !source.attached {
				continue;
			}
			match sync_source(cache, Some(&context.events), source.id, Pace::Settled).await {
				Ok(_) => {}
				Err(e) => tracing::warn!(source = %source.id, %e, "offline copy not refreshed"),
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::ops::indexing::metadata::EntryMetadata;
	use crate::ops::indexing::sources::SourceConfig;
	use crate::ops::indexing::state::EntryKind;
	use crate::ops::indexing::volume_index::tests::{test_library, tracked_volume};
	use crate::volume::VolumeState;

	const LIBRARY: Uuid = Uuid::from_u128(0x42);

	/// An on-source source over a temporary root with a written store, its
	/// cache rooted at `data`.
	async fn on_source_fixture(
		data: &std::path::Path,
		root: &std::path::Path,
	) -> (VolumeIndex, Uuid, Uuid) {
		let library = test_library(data).await;
		let anchor = tracked_volume(&library, root).await;
		let cache = VolumeIndex::with_sources_dir(Some(data.to_path_buf())).expect("cache");
		cache
			.attach_library(LIBRARY, library)
			.await
			.expect("attach");
		cache.track_volume(anchor.uuid, root.to_path_buf());
		let id = cache
			.register_source(root, Some(anchor.clone()))
			.await
			.unwrap();
		cache
			.set_source_config(
				id,
				SourceConfig {
					placement: StorePlacement::OnSource,
					keep_offline_copy: true,
					..SourceConfig::default()
				},
			)
			.await
			.unwrap();
		std::fs::write(root.join("a.txt"), "a").unwrap();
		let store = cache.store_for(&root.join("a.txt")).await.expect("store");
		store
			.identify_one(
				&EntryMetadata {
					path: root.join("a.txt"),
					kind: EntryKind::File,
					size: 1,
					modified: None,
					accessed: None,
					created: None,
					inode: None,
					permissions: None,
					uid: None,
					gid: None,
					link_target: None,
					is_hidden: false,
				},
				None,
			)
			.await
			.expect("identified");
		store.flush().await.unwrap();
		(cache, id, anchor.uuid)
	}

	/// The off switch removes the copy only once the origin opens; with the
	/// drive away the copy stays, the error says why, and the copy is still
	/// what reads answer from.
	#[tokio::test]
	async fn the_off_switch_keeps_the_copy_while_the_origin_is_away() {
		let data = tempfile::tempdir().unwrap();
		let drive = tempfile::tempdir().unwrap();
		let root = drive.path().to_path_buf();
		let (cache, id, volume) = on_source_fixture(data.path(), &root).await;

		let copied = sync_source(&cache, None, id, Pace::Now).await.unwrap();
		assert!(
			matches!(
				copied,
				SyncOutcome::Copied {
					record_count: 1,
					..
				}
			),
			"{copied:?}"
		);
		let dirs = cache.source_dirs().unwrap();
		assert!(dirs.offline_copy_file(id).is_file());
		let manifest = read_manifest(dirs, id).await.expect("manifest");
		assert_eq!(manifest.record_count, 1);
		assert_eq!(
			sync_source(&cache, None, id, Pace::Now).await.unwrap(),
			SyncOutcome::Current
		);
		assert!(!cache.offline_copy_wanted(id), "the origin answers");

		cache
			.volume_state_changed(volume, &root, VolumeState::Unmounted)
			.await;
		assert!(cache.origin_store_file(id).is_none());
		assert!(cache.offline_copy_wanted(id), "reads fall back to the copy");
		assert_eq!(
			sync_source(&cache, None, id, Pace::Now).await.unwrap(),
			SyncOutcome::OriginAway
		);
		let refused = remove(&cache, id).await.expect_err("the drive is away");
		assert!(refused.to_string().contains("kept"), "{refused}");
		assert!(dirs.offline_copy_file(id).is_file(), "the copy survives");
		assert!(read_manifest(dirs, id).await.is_some());
		let served = cache.read_store(id).await.expect("the copy opens");
		let counted: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM record")
			.fetch_one(served.pool())
			.await
			.unwrap();
		assert_eq!(counted.0, 1);
		let state = info(&cache, id).await.expect("on-source state");
		assert!(state.present && state.serving);
		assert_eq!(state.behind_by, None, "no origin to measure against");

		cache
			.volume_state_changed(volume, &root, VolumeState::Mounted)
			.await;
		assert!(!cache.offline_copy_wanted(id));
		remove(&cache, id).await.expect("the origin is back");
		assert!(!dirs.offline_copy_file(id).is_file());
		assert!(!dirs.offline_copy_manifest(id).is_file());
		assert!(info(&cache, id).await.is_some_and(|s| !s.present));
	}

	/// A settled pass copies only once the origin's revision has held for a
	/// poll, so a store under a walk is not exported on every tick; a
	/// moved generation always copies on a `Now` pass.
	#[tokio::test]
	async fn a_settled_pass_waits_for_the_origin_to_stop_moving() {
		let data = tempfile::tempdir().unwrap();
		let drive = tempfile::tempdir().unwrap();
		let root = drive.path().to_path_buf();
		let (cache, id, _) = on_source_fixture(data.path(), &root).await;

		assert!(
			matches!(
				sync_source(&cache, None, id, Pace::Settled).await.unwrap(),
				SyncOutcome::Copied { .. }
			),
			"a source with no copy yet is copied on first sight"
		);

		std::fs::write(root.join("b.txt"), "b").unwrap();
		let store = cache.store_for(&root.join("b.txt")).await.unwrap();
		store
			.identify_one(
				&EntryMetadata {
					path: root.join("b.txt"),
					kind: EntryKind::File,
					size: 1,
					modified: None,
					accessed: None,
					created: None,
					inode: None,
					permissions: None,
					uid: None,
					gid: None,
					link_target: None,
					is_hidden: false,
				},
				None,
			)
			.await
			.expect("identified");
		store.flush().await.unwrap();
		let state = info(&cache, id).await.unwrap();
		assert!(state.behind_by.is_some_and(|n| n > 0), "{state:?}");
		assert_eq!(
			sync_source(&cache, None, id, Pace::Settled).await.unwrap(),
			SyncOutcome::Settling,
			"a generation first seen this poll has not settled"
		);
		assert!(
			matches!(
				sync_source(&cache, None, id, Pace::Settled).await.unwrap(),
				SyncOutcome::Copied {
					record_count: 2,
					..
				}
			),
			"the same generation on the next poll has"
		);
		std::fs::write(root.join("c.txt"), "c").unwrap();
		store
			.identify_one(
				&EntryMetadata {
					path: root.join("c.txt"),
					kind: EntryKind::File,
					size: 1,
					modified: None,
					accessed: None,
					created: None,
					inode: None,
					permissions: None,
					uid: None,
					gid: None,
					link_target: None,
					is_hidden: false,
				},
				None,
			)
			.await
			.expect("identified");
		store.flush().await.unwrap();
		assert_eq!(
			sync_source(&cache, None, id, Pace::Settled).await.unwrap(),
			SyncOutcome::Settling
		);
		// A reader holding the origin's pool across a publish keeps working:
		// the publish retires only a handle on the copy.
		let held = cache.read_store(id).await.expect("origin handle");
		let copied = sync_source(&cache, None, id, Pace::Now).await.unwrap();
		assert!(
			matches!(
				copied,
				SyncOutcome::Copied {
					record_count: 3,
					..
				}
			),
			"{copied:?}"
		);
		let counted: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM record")
			.fetch_one(held.pool())
			.await
			.expect("the origin pool is still open");
		assert_eq!(counted.0, 3);
		assert_eq!(info(&cache, id).await.unwrap().behind_by, Some(0));
	}
}
