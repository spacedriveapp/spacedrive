//! Acceptance for step 4 of the Add to Library plan
//! (`docs/plans/2026-09-16-add-to-library.md`, "Keep an offline copy";
//! `docs/core/acceptance/add-to-library.md` row 10): a source whose catalog
//! lives on its drive keeps a replica of that store in the library, the
//! replica matches the origin, listings and search answer from it while the
//! drive is away and say so, it catches up once the drive is back and the
//! walk lands, a backup taken while the drive is away carries it, and the
//! off switch removes it only once the origin is reachable.
//!
//! The drive is a loop-backed ext4 image through the shared test volume
//! helper, unmounted with its mount point left behind, the shape of an
//! unplugged drive. The helper skips with a reason where there is no
//! passwordless sudo or no loop device.

mod helpers;

use std::{path::Path, sync::Arc};

use helpers::test_volumes::{TestVolume, TestVolumeBuilder, TestVolumeManager};
use helpers::TestConfigBuilder;
use sd_core::{
	infra::{action::LibraryAction, api::SessionContext, query::LibraryQuery},
	library::{AddOverrides, Library},
	ops::{
		indexing::sources::StorePlacement,
		libraries::backup::{manifest::BackupManifest, LibraryBackupAction, LibraryBackupInput},
		sources::{
			list::query::ListSourcesQuery,
			list_records::query::{ListSourceRecordsInput, ListSourceRecordsQuery},
			track::{TrackSourceAction, TrackSourceInput},
			update::action::{UpdateSourceAction, UpdateSourceInput},
		},
	},
	service::{
		mounts::offline::{self, Pace, SyncOutcome},
		volume_monitor::VolumeMonitorService,
	},
	Core,
};
use tracing::warn;

async fn skip_reason() -> Option<String> {
	TestVolumeManager::new()
		.check_privileges()
		.await
		.err()
		.map(|e| e.to_string())
}

/// Detach the drive under the running daemon. The daemon holds the store
/// on the drive open, which a plain `umount` refuses with "target is
/// busy"; a lazy unmount detaches the mount the way a pulled cable does,
/// as far as detection and the mount table can see.
async fn unmount(volume: &TestVolume) {
	let output = tokio::process::Command::new("sudo")
		.args(["umount", "-l", volume.path().to_str().unwrap()])
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

/// Track `root` and wait for its walk and hashing pass, so the store is
/// at rest when the copy is compared against it.
async fn track(
	core: &Arc<Core>,
	library: &Arc<Library>,
	root: &Path,
	overrides: AddOverrides,
) -> anyhow::Result<uuid::Uuid> {
	let output = TrackSourceAction::from_input(TrackSourceInput {
		path: root.to_path_buf(),
		name: None,
		overrides,
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
	let store = core
		.context
		.volume_index()
		.store_for(root)
		.await
		.expect("store");
	for _ in 0..300 {
		if store.files_needing_content_count().await? == 0 {
			break;
		}
		tokio::time::sleep(std::time::Duration::from_millis(100)).await;
	}
	assert_eq!(store.files_needing_content_count().await?, 0);
	Ok(output.id)
}

async fn reconcile(core: &Arc<Core>, library: &Arc<Library>) {
	core.volumes.refresh_volumes().await.expect("refresh");
	VolumeMonitorService::reconcile_tracked_volumes(&core.volumes, library)
		.await
		.expect("reconcile");
}

async fn listed(
	core: &Arc<Core>,
	library: &Arc<Library>,
	id: uuid::Uuid,
) -> sd_core::ops::sources::list::output::SourceInfo {
	ListSourcesQuery::all()
		.execute(core.context.clone(), session(library))
		.await
		.expect("sources.list")
		.into_iter()
		.find(|info| info.id == id)
		.expect("the source is listed")
}

async fn records(core: &Arc<Core>, library: &Arc<Library>, id: uuid::Uuid) -> usize {
	ListSourceRecordsQuery::from_input(ListSourceRecordsInput {
		source_id: id.to_string(),
		limit: 1000,
		offset: 0,
	})
	.unwrap()
	.execute(core.context.clone(), session(library))
	.await
	.expect("sources.list_records")
	.len()
}

#[tokio::test]
async fn an_on_source_catalog_keeps_a_library_copy_that_answers_while_the_drive_is_away() {
	let _ = tracing_subscriber::fmt::try_init();
	if let Some(reason) = skip_reason().await {
		warn!("Skipping: {reason}");
		return;
	}

	// A previous run whose cleanup found the mount busy leaves it in the
	// table, and a fresh image mounted over it would unmount to the stale
	// one instead of to an empty directory.
	let _ = tokio::process::Command::new("sudo")
		.args(["umount", "-l", "/tmp/spacedrive_test_volumes/SdOfflineCopy"])
		.output()
		.await;
	let volume = TestVolumeBuilder::new("SdOfflineCopy")
		.size_mb(32)
		.build()
		.await
		.expect("loop volume");
	let root = volume.path().clone();
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
	let library = core
		.libraries
		.list()
		.await
		.into_iter()
		.next()
		.expect("default library");
	let cache = core.context.volume_index();

	// 1. Track with the catalog on the drive and an offline copy kept.
	let id = track(
		&core,
		&library,
		&root,
		AddOverrides {
			placement: Some(StorePlacement::OnSource),
			keep_offline_copy: Some(true),
			..AddOverrides::default()
		},
	)
	.await
	.expect("track");
	let origin = root
		.join(".spacedrive")
		.join("sources")
		.join(id.simple().to_string())
		.join("data.db");
	assert!(origin.is_file(), "the catalog is on the drive");
	let origin_records = cache
		.store_for(&root)
		.await
		.expect("store")
		.counts()
		.await
		.expect("counts")
		.records;
	assert!(origin_records >= 5, "{origin_records}");

	// 2. The replica appears in the library and matches the origin.
	let copied = offline::sync_source(cache, Some(&core.context.events), id, Pace::Now)
		.await
		.expect("copy");
	assert!(
		matches!(copied, SyncOutcome::Copied { .. }),
		"a moved origin is copied: {copied:?}"
	);
	let dirs = cache.source_dirs().expect("layout");
	let copy = dirs.offline_copy_file(id);
	assert!(copy.is_file(), "the copy lives in the library's layout");
	assert!(copy.starts_with(data_dir.path()));
	let manifest = offline::read_manifest(dirs, id).await.expect("manifest");
	assert_eq!(manifest.record_count, origin_records);
	let info = listed(&core, &library, id).await;
	assert!(info.attached);
	let state = info.offline_copy.clone().expect("on-source state");
	assert!(state.present && !state.serving, "{state:?}");
	assert_eq!(state.behind_by, Some(0));
	assert_eq!(state.record_count, Some(origin_records));
	assert!(!cache.offline_copy_wanted(id));
	let listed_from_origin = records(&core, &library, id).await;
	assert!(listed_from_origin >= 5);

	// 3. Unmount: the drive leaves its mount point behind, the library's
	// listing and search answer from the copy, and the listing says so.
	unmount(&volume).await;
	reconcile(&core, &library).await;
	let info = listed(&core, &library, id).await;
	assert!(!info.attached, "the source reads as detached");
	let state = info.offline_copy.clone().expect("on-source state");
	assert!(state.serving, "{state:?}");
	assert_eq!(state.behind_by, None, "no origin to compare against");
	assert!(cache.offline_copy_wanted(id));
	assert!(cache.origin_store_file(id).is_none());
	assert_eq!(
		records(&core, &library, id).await,
		listed_from_origin,
		"listings answer from the copy"
	);
	let db = cache.read_store(id).await.expect("the copy opens");
	let found = sd_core::ops::search::store_search::search_source_store(
		&db,
		&root,
		&sd_core::device::get_current_device_slug(),
		"file-3",
		None,
		&Default::default(),
		&core.context.file_type_registry(),
	)
	.await
	.expect("search");
	assert_eq!(found.results.len(), 1, "search answers from the copy");
	assert_eq!(
		offline::sync_source(cache, None, id, Pace::Now)
			.await
			.expect("pass"),
		SyncOutcome::OriginAway,
		"nothing is fetched while the origin is away"
	);

	// The off switch refuses while the origin is away and leaves the
	// setting on.
	let refused = UpdateSourceAction::from_input(UpdateSourceInput {
		source_id: id.to_string(),
		name: None,
		unfiltered: None,
		keep_offline_copy: Some(false),
	})
	.unwrap()
	.execute(library.clone(), core.context.clone())
	.await
	.expect_err("the copy cannot be dropped while the drive is away");
	assert!(refused.to_string().contains("kept"), "{refused}");
	assert!(copy.is_file());
	assert!(listed(&core, &library, id)
		.await
		.settings
		.is_some_and(|settings| settings.keep_offline_copy));

	// A backup taken while the drive is away carries the copy as the
	// source's store and says where it came from.
	let backup_dir = data_dir.path().join("backup-away");
	let backup = LibraryBackupAction::from_input(LibraryBackupInput {
		library_id: library.id(),
		destination: backup_dir.clone(),
		include_sidecars: false,
		include_replicas: false,
	})
	.unwrap()
	.execute(library.clone(), core.context.clone())
	.await
	.expect("backup");
	assert!(
		!backup.sources_without_store.contains(&id),
		"the copy stands in for the store"
	);
	let manifest = BackupManifest::load(&backup_dir.join("manifest.json"))
		.await
		.expect("manifest");
	let entry = manifest
		.sources
		.iter()
		.find(|entry| entry.id == id)
		.expect("source entry");
	assert!(entry.from_offline_copy);
	assert_eq!(entry.placement, StorePlacement::OnSource);
	assert!(backup_dir
		.join("sources")
		.join(id.simple().to_string())
		.join("data.db")
		.is_file());

	// 4. Remount, add a file, walk, and the copy catches up.
	remount(&volume).await;
	reconcile(&core, &library).await;
	assert!(listed(&core, &library, id).await.attached);
	assert!(
		!cache.offline_copy_wanted(id),
		"reads go back to the origin"
	);
	std::fs::write(root.join("file-5.txt"), "5").expect("write");
	let readded = track(
		&core,
		&library,
		&root,
		AddOverrides {
			placement: Some(StorePlacement::OnSource),
			..AddOverrides::default()
		},
	)
	.await
	.expect("re-track");
	assert_eq!(readded, id, "the catalog on the drive is reopened");
	let origin_records = cache
		.store_for(&root)
		.await
		.expect("store")
		.counts()
		.await
		.expect("counts")
		.records;
	assert!(origin_records >= 6, "{origin_records}");
	let state = listed(&core, &library, id)
		.await
		.offline_copy
		.expect("state");
	assert!(
		state.behind_by.is_some_and(|behind| behind > 0)
			|| state.record_count == Some(origin_records),
		"the listing reports the copy behind until the next pass: {state:?}"
	);
	let caught_up = offline::sync_source(cache, Some(&core.context.events), id, Pace::Now)
		.await
		.expect("copy");
	assert!(
		matches!(caught_up, SyncOutcome::Copied { .. } | SyncOutcome::Current),
		"{caught_up:?}"
	);
	let manifest = offline::read_manifest(dirs, id).await.expect("manifest");
	assert_eq!(manifest.record_count, origin_records);
	let state = listed(&core, &library, id)
		.await
		.offline_copy
		.expect("state");
	assert_eq!(state.behind_by, Some(0));

	// 5. With the origin reachable, the off switch removes the copy.
	let updated = UpdateSourceAction::from_input(UpdateSourceInput {
		source_id: id.to_string(),
		name: None,
		unfiltered: None,
		keep_offline_copy: Some(false),
	})
	.unwrap()
	.execute(library.clone(), core.context.clone())
	.await
	.expect("update");
	assert!(!updated.keep_offline_copy);
	assert!(!copy.is_file(), "the copy is gone");
	assert!(!dirs.offline_copy_manifest(id).is_file());
	let state = listed(&core, &library, id)
		.await
		.offline_copy
		.expect("state");
	assert!(!state.present);

	drop(library);
	core.shutdown().await.expect("shutdown");
	drop(core);
	// Spawned service tasks let go of the drive after shutdown returns, and
	// the helper's cleanup unmounts it without `-l`.
	tokio::time::sleep(std::time::Duration::from_secs(2)).await;
	drop(volume);
}
