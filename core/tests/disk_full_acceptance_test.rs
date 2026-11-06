//! # Disk full acceptance
//!
//! Acceptance for the disk-full finding in step 4 of the candidate
//! validation (docs/core/acceptance/candidate-validation.md, "Disk full hangs
//! the indexer"): a walk whose source store runs out of space fails, with an
//! error that says the disk is full and names the store, instead of hanging
//! as running. The store stays intact, and once there is room again the
//! coverage heal walks the source to completion.
//!
//! The tree and the source stores live on a small loop-backed ext4 volume,
//! filled until the store cannot hold the walk. The rest of the data
//! directory, with the library and jobs databases, stays on the regular disk:
//! the walk's job row is written before the walk starts, and on a full disk
//! that insert fails first, at a point that depends on how much slack SQLite
//! happens to hold. Freeing the filler is then all it takes to give the store
//! room again. The helper skips with a reason where there is no passwordless
//! sudo or no loop device.

#![cfg(target_os = "linux")]

mod helpers;

use std::{path::Path, sync::Arc, time::Duration};

use helpers::test_volumes::{TestVolumeBuilder, TestVolumeManager};
use helpers::TestConfigBuilder;
use sd_core::{
	infra::{action::LibraryAction, job::prelude::JobId, job::types::JobStatus},
	library::AddOverrides,
	ops::sources::track::{TrackSourceAction, TrackSourceInput},
	Core,
};
use tracing::warn;

const DIRECTORIES: usize = 80;
const FILES_PER_DIRECTORY: usize = 100;

/// Room left on the volume once it is filled. Enough for the walk to start
/// and land a few batches, not enough for the store to hold the tree.
const LEAVE_BYTES: u64 = 768 * 1024;

async fn skip_reason() -> Option<String> {
	TestVolumeManager::new()
		.check_privileges()
		.await
		.err()
		.map(|e| e.to_string())
}

async fn sudo(args: &[&str]) {
	let output = tokio::process::Command::new("sudo")
		.args(args)
		.output()
		.await
		.expect("sudo");
	assert!(
		output.status.success(),
		"sudo {args:?}: {}",
		String::from_utf8_lossy(&output.stderr)
	);
}

async fn available_bytes(path: &Path) -> u64 {
	let output = tokio::process::Command::new("df")
		.args(["-B1", "--output=avail", path.to_str().unwrap()])
		.output()
		.await
		.expect("df");
	String::from_utf8_lossy(&output.stdout)
		.lines()
		.last()
		.and_then(|line| line.trim().parse().ok())
		.expect("available bytes")
}

/// Allocate real blocks, so the space is gone rather than promised.
async fn fill(path: &Path, bytes: u64) {
	let output = tokio::process::Command::new("fallocate")
		.args(["-l", &bytes.to_string(), path.to_str().unwrap()])
		.output()
		.await
		.expect("fallocate");
	assert!(
		output.status.success(),
		"fallocate: {}",
		String::from_utf8_lossy(&output.stderr)
	);
}

/// Records the store holds, read without a flush. A flush answers for the
/// failed walk's writes and would hide whether the walk itself drained them
/// before the heal's walk ran.
async fn committed(pool: &sqlx::SqlitePool) -> u64 {
	sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM record")
		.fetch_one(pool)
		.await
		.expect("count") as u64
}

async fn boot(data_dir: &Path) -> Arc<Core> {
	let config = TestConfigBuilder::new(data_dir.to_path_buf())
		.build()
		.expect("config");
	config.save().expect("save config");
	Arc::new(Core::new(data_dir.to_path_buf()).await.expect("core"))
}

#[tokio::test]
async fn a_full_disk_fails_the_walk_and_the_heal_finishes_it_once_there_is_room() {
	let _ = tracing_subscriber::fmt::try_init();
	if let Some(reason) = skip_reason().await {
		warn!("Skipping: {reason}");
		return;
	}

	let _ = tokio::process::Command::new("sudo")
		.args(["umount", "-l", "/tmp/spacedrive_test_volumes/SdDiskFull"])
		.output()
		.await;
	let volume = TestVolumeBuilder::new("SdDiskFull")
		.size_mb(48)
		.build()
		.await
		.expect("loop volume");
	let mount = volume.path().clone();
	sudo(&["chown", "-R", &whoami::username(), mount.to_str().unwrap()]).await;

	let tree = mount.join("tree");
	let root = tree.clone();
	tokio::task::spawn_blocking(move || {
		for directory in 0..DIRECTORIES {
			let directory = root.join(format!("d{directory:03}"));
			std::fs::create_dir_all(&directory).expect("directory");
			for file in 0..FILES_PER_DIRECTORY {
				std::fs::write(directory.join(format!("f{file:03}.txt")), b"").expect("file");
			}
		}
	})
	.await
	.expect("tree");
	let expected_records = (DIRECTORIES * (FILES_PER_DIRECTORY + 1)) as u64;

	// Only the stores go on the volume, through a link where the daemon
	// looks for them.
	let data = tempfile::TempDir::new().expect("data dir");
	let data_dir = data.path().to_path_buf();
	let sources = mount.join("sources");
	tokio::fs::create_dir_all(&sources)
		.await
		.expect("sources dir");
	tokio::fs::symlink(&sources, data_dir.join("sources"))
		.await
		.expect("sources link");
	let core = boot(&data_dir).await;
	let library = core
		.libraries
		.list()
		.await
		.into_iter()
		.next()
		.expect("default library");
	let cache = core.context.volume_index();

	let filler = mount.join("filler");
	let available = available_bytes(&mount).await;
	assert!(available > LEAVE_BYTES, "{available} bytes free");
	fill(&filler, available - LEAVE_BYTES).await;

	// 1. The walk fails within bounded time, and says why.
	let output = TrackSourceAction::from_input(TrackSourceInput {
		path: tree.clone(),
		name: None,
		overrides: AddOverrides::default(),
	})
	.expect("input")
	.execute(library.clone(), core.context.clone())
	.await
	.expect("track");
	let job_id = output.job_id.expect("tracking dispatched a walk");
	let walk = library
		.jobs()
		.get_job(JobId(job_id))
		.await
		.expect("the walk is running");
	let outcome = tokio::time::timeout(Duration::from_secs(120), walk.wait())
		.await
		.expect("the walk ends instead of hanging");
	outcome.expect_err("a walk into a full disk cannot succeed");

	// The job reads as failed, in memory and in the jobs database.
	let info = library
		.jobs()
		.get_job_info(job_id)
		.await
		.expect("job info")
		.expect("the job is known");
	assert_eq!(info.status, JobStatus::Failed);
	let message = info.error_message.unwrap_or_default();
	assert!(
		message.contains("disk full") && message.contains("data.db"),
		"the failure says disk full and names the store: {message}"
	);
	let listed = library.jobs().list_jobs(None).await.expect("jobs");
	assert!(listed
		.iter()
		.any(|job| job.id == job_id && job.status == JobStatus::Failed));

	// 2. The store is intact, holds part of the tree at most, and is marked
	// for the heal.
	let store = cache.store_for(&tree).await.expect("store");
	let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
		.fetch_one(store.db().pool())
		.await
		.expect("integrity check");
	assert_eq!(integrity, "ok");
	let partial = committed(store.db().pool()).await;
	assert!(partial < expected_records, "{partial} records");
	assert!(
		store.walk_unfinished().await,
		"the store remembers the walk"
	);
	let reopened = sd_store::SourceManager::open_file_read_only(
		&data_dir
			.join("sources")
			.join(output.id.simple().to_string())
			.join("data.db"),
	)
	.await
	.expect("the store opens on its own");
	reopened.pool().close().await;

	// 3. Free the space; the coverage heal walks the source again and this
	// time it lands whole.
	std::fs::remove_file(&filler).expect("free space");
	core.volumes.refresh_volumes().await.expect("refresh");
	sd_core::ops::volumes::index::map_attached_volumes(&library, &core.context, false).await;

	let mut records = 0;
	for _ in 0..1200 {
		records = committed(store.db().pool()).await;
		if records == expected_records && !store.walk_unfinished().await {
			break;
		}
		tokio::time::sleep(Duration::from_millis(100)).await;
	}
	assert_eq!(records, expected_records, "the heal lands the whole tree");
	assert!(
		!store.walk_unfinished().await,
		"the healed walk clears its mark"
	);
	assert!(store.disk_full().is_none());
	let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
		.fetch_one(store.db().pool())
		.await
		.expect("integrity check");
	assert_eq!(integrity, "ok");

	drop(store);
	drop(library);
	core.shutdown().await.expect("shutdown");
	drop(core);
	// The stores are on the volume, and pools that outlive shutdown keep it
	// busy for the helper's plain unmount; a lazy one detaches it now so the
	// next suite's volume does not mount over it.
	tokio::time::sleep(Duration::from_secs(2)).await;
	sudo(&["umount", "-l", mount.to_str().unwrap()]).await;
	drop(volume);
}
