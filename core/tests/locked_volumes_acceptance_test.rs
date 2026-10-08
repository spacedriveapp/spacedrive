//! Acceptance for L1, L2 and the plain-unmount half of L4 of the locked
//! volumes plan (`docs/plans/2026-09-28-locked-volumes.md`,
//! `docs/core/acceptance/volumes.md`): a volume that detection cannot see
//! comes up detached whatever its row says, the monitor marks a vanished
//! volume offline under a running daemon, an unmounted volume's mount
//! point is never walked, hashed or thumbnailed as an empty source, and a
//! volume that unmounts and remounts under a running daemon takes its
//! source with it, watch included, through the volume manager's events.
//!
//! Every test mounts a loop-backed ext4 image through the shared test volume
//! helper and unmounts it with the mount point left in place, which is the
//! shape of a locked ZFS dataset or an unplugged drive. The helper skips with
//! a reason where there is no passwordless sudo or no loop device.

mod helpers;

use std::{path::Path, sync::Arc};

use helpers::test_volumes::{TestVolume, TestVolumeBuilder, TestVolumeManager};
use helpers::TestConfigBuilder;
use sd_core::{
	infra::{action::LibraryAction, api::SessionContext, db::entities, query::LibraryQuery},
	library::Library,
	ops::{
		indexing::content_identity::identify_every_source,
		sources::{
			list::query::ListSourcesQuery,
			track::{TrackSourceAction, TrackSourceInput},
		},
	},
	service::volume_monitor::VolumeMonitorService,
	Core,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use tempfile::TempDir;
use tracing::warn;

/// Why the loop helper cannot run here, or `None` when it can.
async fn skip_reason() -> Option<String> {
	TestVolumeManager::new()
		.check_privileges()
		.await
		.err()
		.map(|e| e.to_string())
}

async fn unmount(volume: &TestVolume) {
	let output = tokio::process::Command::new("sudo")
		.args(["umount", volume.path().to_str().unwrap()])
		.output()
		.await
		.expect("umount");
	assert!(
		output.status.success(),
		"umount: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	assert!(volume.path().is_dir(), "the mount point stays behind");
}

async fn remount(volume: &TestVolume) {
	let output = tokio::process::Command::new("sudo")
		.args([
			"mount",
			&volume.platform_id,
			volume.path().to_str().unwrap(),
		])
		.output()
		.await
		.expect("mount");
	assert!(
		output.status.success(),
		"mount: {}",
		String::from_utf8_lossy(&output.stderr)
	);
}

/// A daemon over `data_dir`: detection on, watcher on, networking off.
async fn boot(data_dir: &Path) -> Arc<Core> {
	let mut config = TestConfigBuilder::new(data_dir.to_path_buf())
		.build()
		.expect("config");
	config.services.volume_monitoring_enabled = true;
	config.services.fs_watcher_enabled = true;
	config.save().expect("save config");
	Arc::new(Core::new(data_dir.to_path_buf()).await.expect("core"))
}

fn session(library: &Library) -> SessionContext {
	let mut session = SessionContext::device_session(
		sd_core::device::get_current_device_id(),
		sd_core::device::get_current_device_slug(),
	);
	session.current_library_id = Some(library.id());
	session
}

async fn track(core: &Arc<Core>, library: &Arc<Library>, root: &Path) -> anyhow::Result<()> {
	let output = TrackSourceAction::from_input(TrackSourceInput {
		path: root.to_path_buf(),
		name: None,
		overrides: Default::default(),
	})
	.map_err(anyhow::Error::msg)?
	.execute(library.clone(), core.context.clone())
	.await?;
	let job_id = output.job_id.expect("tracking dispatched a walk");
	if let Some(walk) = library
		.jobs()
		.get_job(sd_core::infra::job::prelude::JobId(job_id))
		.await
	{
		walk.wait().await?;
	}
	Ok(())
}

async fn volume_row(library: &Library, mount_point: &Path) -> entities::volume::Model {
	entities::volume::Entity::find()
		.filter(entities::volume::Column::MountPoint.eq(mount_point.to_string_lossy().into_owned()))
		.one(library.db().conn())
		.await
		.expect("query")
		.expect("the source's volume has a row")
}

/// What the library and the index know about sources and volumes, for a
/// failure message that says why a source is missing rather than that it is.
async fn describe(core: &Core, library: &Library) -> String {
	let sources = entities::source::Entity::find()
		.all(library.db().conn())
		.await
		.map(|rows| {
			rows.iter()
				.map(|row| format!("{:?}@{:?}/{:?}", row.name, row.volume_uuid, row.root))
				.collect::<Vec<_>>()
		})
		.unwrap_or_else(|e| vec![format!("query failed: {e}")]);
	let volumes = entities::volume::Entity::find()
		.all(library.db().conn())
		.await
		.map(|rows| {
			rows.iter()
				.map(|row| {
					format!(
						"{} device={} online={} at {:?}",
						row.uuid, row.device_id, row.is_online, row.mount_point
					)
				})
				.collect::<Vec<_>>()
		})
		.unwrap_or_else(|e| vec![format!("query failed: {e}")]);
	let index: Vec<String> = core
		.context
		.volume_index()
		.sources()
		.iter()
		.map(|source| format!("{} attached={}", source.root.display(), source.attached))
		.collect();
	let open: Vec<String> = core
		.libraries
		.list()
		.await
		.iter()
		.map(|library| format!("{} at {}", library.id(), library.path().display()))
		.collect();
	format!(
		"source rows {sources:?}; volume rows {volumes:?}; index {index:?}; open libraries {open:?}; device {}",
		sd_core::device::get_current_device_id()
	)
}

async fn job_count(library: &Library) -> usize {
	library.jobs().list_jobs(None).await.expect("jobs").len()
}

/// Poll `check` until it holds or ten seconds pass, for state the daemon
/// reaches through its event bus rather than on the caller's thread.
async fn eventually(what: &str, mut check: impl AsyncFnMut() -> bool) {
	for _ in 0..100 {
		if check().await {
			return;
		}
		tokio::time::sleep(std::time::Duration::from_millis(100)).await;
	}
	panic!("{what}: did not happen within ten seconds");
}

/// A populated loop volume tracked as a whole-volume source by a daemon
/// that was then shut down cleanly, so its snapshot and store are on disk.
struct Walked {
	volume: TestVolume,
	data_dir: TempDir,
	library_path: std::path::PathBuf,
	records: u64,
}

async fn walked_volume(name: &str) -> Walked {
	let volume = TestVolumeBuilder::new(name)
		.size_mb(32)
		.build()
		.await
		.expect("loop volume");
	let root = volume.path().clone();
	// A fresh ext4 root is owned by root with a `lost+found` nobody else may
	// enter, which the recursive watch would refuse.
	let output = tokio::process::Command::new("sudo")
		.args(["chown", "-R", &whoami::username(), root.to_str().unwrap()])
		.output()
		.await
		.expect("chown");
	assert!(output.status.success());
	let output = tokio::process::Command::new("sudo")
		.args(["chmod", "755", root.join("lost+found").to_str().unwrap()])
		.output()
		.await
		.expect("chmod");
	assert!(output.status.success());
	for n in 0..5 {
		std::fs::write(root.join(format!("file-{n}.txt")), n.to_string()).expect("write");
	}

	let data_dir = tempfile::tempdir().expect("data dir");
	let core = boot(data_dir.path()).await;
	// The one library the daemon created is enough for a volume fixture;
	// `multi_library_acceptance_test` is where a second library matters.
	let library = core
		.libraries
		.list()
		.await
		.into_iter()
		.next()
		.expect("default library");
	let library_path = library.path().to_path_buf();
	track(&core, &library, &root).await.expect("track");
	let store = core
		.context
		.volume_index()
		.store_for(&root)
		.await
		.expect("store");
	let records = store.counts().await.expect("counts").records;
	assert!(records >= 5, "the walk recorded the files: {records}");
	// The hashing pass the walk queued is part of the fixture; a daemon
	// stopping mid-hash would make the restart about job resumption. The
	// pass is dispatched after the walk handle resolves, with no ordering
	// against this function, so what is waited on is its effect: every
	// record has a content identity.
	for _ in 0..300 {
		if store
			.files_needing_content_count()
			.await
			.expect("pending count")
			== 0
		{
			break;
		}
		tokio::time::sleep(std::time::Duration::from_millis(100)).await;
	}
	assert_eq!(
		store
			.files_needing_content_count()
			.await
			.expect("pending count"),
		0,
		"the fixture's hashing pass finished"
	);
	drop(library);
	core.shutdown().await.expect("shutdown");
	drop(core);
	// Spawned service tasks let go of the key store after shutdown returns.
	tokio::time::sleep(std::time::Duration::from_secs(2)).await;

	Walked {
		volume,
		data_dir,
		library_path,
		records,
	}
}

/// L1: a source whose volume is gone at startup comes up detached, with its
/// map restored read-only and no watch armed, and reattaches when the
/// volume returns.
#[tokio::test]
async fn a_source_whose_volume_is_gone_at_startup_comes_up_detached() {
	let _ = tracing_subscriber::fmt::try_init();
	if let Some(reason) = skip_reason().await {
		warn!("Skipping: {reason}");
		return;
	}

	let walked = walked_volume("SdLockedL1").await;
	let root = walked.volume.path().clone();
	unmount(&walked.volume).await;

	let core = boot(walked.data_dir.path()).await;
	let library = core
		.libraries
		.list()
		.await
		.into_iter()
		.find(|library| library.path() == walked.library_path)
		.expect("the library reloads");
	let jobs_before = job_count(&library).await;

	let cache = core.context.volume_index();
	let Some(source) = cache
		.sources()
		.into_iter()
		.find(|source| source.root == root)
	else {
		panic!(
			"the source is still registered: {}",
			describe(&core, &library).await
		);
	};
	assert!(
		!source.attached,
		"an empty directory at the mount point is not the volume"
	);
	assert!(
		!volume_row(&library, &root).await.is_online,
		"the stale online flag is corrected against live detection"
	);

	let listed = ListSourcesQuery::all()
		.execute(core.context.clone(), session(&library))
		.await
		.expect("sources.list");
	let listed = listed
		.iter()
		.find(|info| info.id == source.id)
		.expect("listed");
	assert!(!listed.attached, "sources.list shows the source offline");

	let child = root.join("file-0.txt");
	assert!(cache.ensure_restored(&child).await, "the map restores");
	assert!(cache.is_detached(&child), "and serves read-only");
	let index = cache.get_for_search(&child).expect("restored index");
	assert!(
		index.read().await.get_entry_uuid(&child).is_some(),
		"the restored map holds the walked files"
	);

	tokio::time::sleep(std::time::Duration::from_secs(1)).await;
	let watcher = core.context.get_fs_watcher().await.expect("watcher");
	assert!(
		!watcher.watched_paths().await.contains(&root),
		"no watch is armed on the directory left behind"
	);
	assert!(!cache.is_watched(&root));
	assert_eq!(
		job_count(&library).await,
		jobs_before,
		"nothing dispatched a walk at the empty mount point"
	);

	remount(&walked.volume).await;
	core.volumes.refresh_volumes().await.expect("refresh");
	VolumeMonitorService::reconcile_tracked_volumes(&core.volumes, &library)
		.await
		.expect("reconcile");
	let source = cache
		.sources()
		.into_iter()
		.find(|source| source.root == root)
		.expect("registered");
	assert!(
		source.attached,
		"the source reattaches when the volume returns"
	);
	assert!(!cache.is_detached(&child));
	assert!(volume_row(&library, &root).await.is_online);
	watcher
		.watch_root(root.clone())
		.await
		.expect("a reattached source is watchable");

	drop(library);
	core.shutdown().await.expect("shutdown");
}

/// L1: the monitor marks a tracked volume offline when detection stops
/// returning it, instead of trusting the row's last state.
#[tokio::test]
async fn the_monitor_marks_a_vanished_volume_offline() {
	let _ = tracing_subscriber::fmt::try_init();
	if let Some(reason) = skip_reason().await {
		warn!("Skipping: {reason}");
		return;
	}

	let walked = walked_volume("SdLockedMon").await;
	let root = walked.volume.path().clone();
	let core = boot(walked.data_dir.path()).await;
	let library = core
		.libraries
		.list()
		.await
		.into_iter()
		.find(|library| library.path() == walked.library_path)
		.expect("the library reloads");
	assert!(volume_row(&library, &root).await.is_online);
	let sources = core.context.volume_index().sources();
	assert!(
		sources.first().is_some_and(|source| source.attached),
		"{}",
		describe(&core, &library).await
	);

	unmount(&walked.volume).await;
	core.volumes.refresh_volumes().await.expect("refresh");
	VolumeMonitorService::reconcile_tracked_volumes(&core.volumes, &library)
		.await
		.expect("reconcile");

	assert!(
		!volume_row(&library, &root).await.is_online,
		"a volume detection stopped returning is offline"
	);
	let cache = core.context.volume_index();
	assert!(!cache.sources()[0].attached, "and its source detaches");
	assert!(cache.is_detached(&root));

	drop(library);
	core.shutdown().await.expect("shutdown");
}

/// L2: with the mount point left as an empty directory and the stored state
/// still saying mounted, no walk, hash or thumbnail job runs over the
/// source, a forced heal refuses instead of sweeping, and tracking the
/// directory refuses instead of registering a second source.
#[tokio::test]
async fn an_empty_mount_point_is_reported_unmounted_not_walked() {
	let _ = tracing_subscriber::fmt::try_init();
	if let Some(reason) = skip_reason().await {
		warn!("Skipping: {reason}");
		return;
	}

	let walked = walked_volume("SdLockedL2").await;
	let root = walked.volume.path().clone();
	let core = boot(walked.data_dir.path()).await;
	let library = core
		.libraries
		.list()
		.await
		.into_iter()
		.find(|library| library.path() == walked.library_path)
		.expect("the library reloads");
	let cache = core.context.volume_index();
	assert!(
		cache
			.sources()
			.first()
			.is_some_and(|source| source.attached),
		"{}",
		describe(&core, &library).await
	);

	// The volume goes away between refreshes: detection and the row both
	// still say mounted, and the directory is empty.
	unmount(&walked.volume).await;
	assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
	let refusal = cache
		.dispatch_refusal(&root)
		.expect("the mount point check refuses the empty directory");
	assert!(refusal.contains("not mounted"), "{refusal}");

	// The snapshot is gone, so the heal sees records with no map coverage
	// and wants a walk; that walk must not run over the empty directory.
	let snapshot = cache
		.source_snapshot_path(cache.sources()[0].id)
		.expect("snapshot path");
	std::fs::remove_file(&snapshot).expect("drop the snapshot");
	let jobs_before = job_count(&library).await;
	sd_core::ops::volumes::index::map_attached_volumes(&library, &core.context, false).await;
	identify_every_source(&library, &core.context).await;
	tokio::time::sleep(std::time::Duration::from_secs(1)).await;
	assert_eq!(
		job_count(&library).await,
		jobs_before,
		"no walk or hash job was dispatched at the empty mount point"
	);
	let records = cache
		.store_for(&root)
		.await
		.expect("store")
		.counts()
		.await
		.expect("counts")
		.records;
	assert_eq!(records, walked.records, "nothing swept the store");

	let refused = TrackSourceAction::from_input(TrackSourceInput {
		path: root.clone(),
		name: None,
		overrides: Default::default(),
	})
	.unwrap()
	.execute(library.clone(), core.context.clone())
	.await;
	let error = refused.expect_err("tracking an unmounted mount point refuses");
	assert!(error.to_string().contains("not mounted"), "{error}");
	assert_eq!(cache.sources().len(), 1, "no second source was registered");

	// Once detection catches up, the listing says unmounted rather than
	// showing an empty source.
	core.volumes.refresh_volumes().await.expect("refresh");
	VolumeMonitorService::reconcile_tracked_volumes(&core.volumes, &library)
		.await
		.expect("reconcile");
	let listed = ListSourcesQuery::all()
		.execute(core.context.clone(), session(&library))
		.await
		.expect("sources.list");
	assert!(!listed[0].attached, "the source reads as unmounted");
	assert_eq!(listed[0].item_count as u64, walked.records);

	drop(library);
	core.shutdown().await.expect("shutdown");
}

/// L4 on a plain filesystem: a volume unmounted and remounted under the
/// running daemon detaches and reattaches its source through the volume
/// manager's events alone, the watch is dropped and armed again on the
/// returned filesystem, a file created after the return reaches the map,
/// and identifications that failed while it was away are retried.
#[tokio::test]
async fn a_remounted_volume_reattaches_its_source_and_rearms_the_watch() {
	let _ = tracing_subscriber::fmt::try_init();
	if let Some(reason) = skip_reason().await {
		warn!("Skipping: {reason}");
		return;
	}

	let walked = walked_volume("SdLockedL4").await;
	let root = walked.volume.path().clone();
	let core = boot(walked.data_dir.path()).await;
	let library = core
		.libraries
		.list()
		.await
		.into_iter()
		.find(|library| library.path() == walked.library_path)
		.expect("the library reloads");
	let cache = core.context.volume_index();
	let watcher = core.context.get_fs_watcher().await.expect("watcher");
	eventually("the restored source is watched", async || {
		cache.is_watched(&root)
	})
	.await;
	assert!(watcher.watched_paths().await.contains(&root));

	// One record is left as if its read had failed while the volume was
	// away: no identity, and a reason recorded.
	let store = cache.store_for(&root).await.expect("store");
	let failed: (Vec<u8>,) =
		sqlx::query_as("SELECT uuid FROM record WHERE type = 'file' ORDER BY rowid LIMIT 1")
			.fetch_one(store.db().pool())
			.await
			.expect("a record");
	sqlx::query("UPDATE record SET content_id = NULL WHERE uuid = ?")
		.bind(&failed.0)
		.execute(store.db().pool())
		.await
		.expect("drop identity");
	sqlx::query("UPDATE facet_file SET content_error = 'volume away' WHERE record_uuid = ?")
		.bind(&failed.0)
		.execute(store.db().pool())
		.await
		.expect("record failure");
	assert_eq!(store.files_needing_content_count().await.unwrap(), 0);

	// The volume goes away. Only the refresh runs; the monitor's row
	// reconciliation is not what moves the index any more.
	unmount(&walked.volume).await;
	core.volumes.refresh_volumes().await.expect("refresh");
	eventually("the source detaches on the volume's event", async || {
		cache
			.sources()
			.first()
			.is_some_and(|source| !source.attached)
	})
	.await;
	assert_eq!(
		cache.sources()[0].volume_state,
		Some(sd_core::volume::VolumeState::Unmounted)
	);
	eventually("the watch is dropped with the mount", async || {
		!cache.is_watched(&root) && !watcher.watched_paths().await.contains(&root)
	})
	.await;
	let listed = ListSourcesQuery::all()
		.execute(core.context.clone(), session(&library))
		.await
		.expect("sources.list");
	assert!(!listed[0].attached);
	assert_eq!(
		listed[0].volume_state,
		Some(sd_core::volume::VolumeState::Unmounted)
	);
	assert_eq!(
		listed[0].item_count as u64, walked.records,
		"no record was lost"
	);

	// And comes back.
	remount(&walked.volume).await;
	core.volumes.refresh_volumes().await.expect("refresh");
	eventually("the source reattaches on the volume's event", async || {
		cache
			.sources()
			.first()
			.is_some_and(|source| source.attached)
	})
	.await;
	eventually(
		"the watch is armed on the returned filesystem",
		async || cache.is_watched(&root) && watcher.watched_paths().await.contains(&root),
	)
	.await;

	let created = root.join("after-remount.txt");
	std::fs::write(&created, "seen").expect("write");
	eventually(
		"a file created after the return reaches the map",
		async || match cache.get_for_search(&created) {
			Some(index) => index.read().await.get_entry_uuid(&created).is_some(),
			None => false,
		},
	)
	.await;

	eventually("the failed identification is retried", async || {
		let error: (Option<String>,) =
			sqlx::query_as("SELECT content_error FROM facet_file WHERE record_uuid = ?")
				.bind(&failed.0)
				.fetch_one(store.db().pool())
				.await
				.expect("content error");
		error.0.is_none() && store.files_needing_content_count().await.unwrap() == 0
	})
	.await;
	let identity: (Option<i64>,) = sqlx::query_as("SELECT content_id FROM record WHERE uuid = ?")
		.bind(&failed.0)
		.fetch_one(store.db().pool())
		.await
		.expect("content id");
	assert!(identity.0.is_some(), "the record was identified after all");

	drop(library);
	core.shutdown().await.expect("shutdown");
}
