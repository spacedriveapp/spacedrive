//! # Tracking on a full data disk
//!
//! When the disk holding the data directory is full, `sources.track` can
//! register the source and then fail to queue its first walk, because the
//! job row does not fit in `jobs.db`. The action has to say so instead of
//! reporting a tracked source with nothing indexing it, and the source has to
//! stay registered and marked unfinished, so the coverage heal walks it once
//! there is room again.
//!
//! The whole data directory and the tree live on a small loop-backed ext4
//! volume, filled until only a few blocks are left. The helper skips with a
//! reason where there is no passwordless sudo or no loop device. A second
//! case refuses the job row directly, so the store opens and the walk-unfinished
//! mark written before the dispatch can be checked.

#![cfg(target_os = "linux")]

mod helpers;

use std::{
	path::{Path, PathBuf},
	sync::Arc,
	time::Duration,
};

use helpers::test_volumes::{TestVolume, TestVolumeBuilder, TestVolumeManager};
use helpers::TestConfigBuilder;
use sd_core::{
	infra::action::{error::ActionError, LibraryAction},
	library::{AddOverrides, Library},
	ops::{
		indexing::store::SourceStore,
		sources::track::{TrackSourceAction, TrackSourceInput},
	},
	Core,
};
use tracing::warn;

const DIRECTORIES: usize = 80;
const FILES_PER_DIRECTORY: usize = 100;
const EXPECTED_RECORDS: u64 = (DIRECTORIES * (FILES_PER_DIRECTORY + 1)) as u64;

/// Room left on the volume once it is filled. Enough for the registration to
/// land, not enough for the walk's job row.
const LEAVE_BYTES: u64 = 64 * 1024;

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

async fn write_tree(tree: &Path) {
	let root = tree.to_path_buf();
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
}

fn track(tree: &Path) -> TrackSourceAction {
	TrackSourceAction::from_input(TrackSourceInput {
		path: tree.to_path_buf(),
		name: None,
		overrides: AddOverrides::default(),
	})
	.expect("input")
}

/// The failure a track on the full disk returns, checked for what the caller
/// needs to hear: the walk did not start, and it will be retried.
fn assert_retry_promised(error: ActionError) {
	let message = error.to_string();
	assert!(
		message.contains("first walk could not start") && message.contains("next time it starts"),
		"the error says the walk did not start and will retry: {message}"
	);
}

/// Wait for the heal's walk to land the whole tree and clear its mark.
async fn assert_healed(store: &SourceStore, expected: u64) {
	let mut records = 0;
	for _ in 0..1200 {
		records = committed(store.db().pool()).await;
		if records == expected && !store.walk_unfinished().await {
			break;
		}
		tokio::time::sleep(Duration::from_millis(100)).await;
	}
	assert_eq!(records, expected, "the heal lands the whole tree");
	assert!(
		!store.walk_unfinished().await,
		"the healed walk clears its mark"
	);
}

/// A loop volume holding a booted data directory and the tree to track.
struct Disk {
	volume: TestVolume,
	mount: PathBuf,
	tree: PathBuf,
	filler: PathBuf,
	core: Arc<Core>,
	library: Arc<Library>,
}

impl Disk {
	async fn new(name: &str) -> Option<Self> {
		let _ = tracing_subscriber::fmt::try_init();
		if let Some(reason) = skip_reason().await {
			warn!("Skipping: {reason}");
			return None;
		}
		let _ = tokio::process::Command::new("sudo")
			.args([
				"umount",
				"-l",
				&format!("/tmp/spacedrive_test_volumes/{name}"),
			])
			.output()
			.await;
		let volume = TestVolumeBuilder::new(name)
			.size_mb(48)
			.build()
			.await
			.expect("loop volume");
		let mount = volume.path().clone();
		sudo(&["chown", "-R", &whoami::username(), mount.to_str().unwrap()]).await;

		let tree = mount.join("tree");
		write_tree(&tree).await;

		let core = boot(&mount.join("data")).await;
		let library = core
			.libraries
			.list()
			.await
			.into_iter()
			.next()
			.expect("default library");
		Some(Self {
			filler: mount.join("filler"),
			volume,
			mount,
			tree,
			core,
			library,
		})
	}

	async fn fill(&self) {
		let available = available_bytes(&self.mount).await;
		assert!(available > LEAVE_BYTES, "{available} bytes free");
		fill(&self.filler, available - LEAVE_BYTES).await;
	}

	/// Free the space and run the heal startup runs.
	async fn free_and_heal(&self) {
		std::fs::remove_file(&self.filler).expect("free space");
		self.core.volumes.refresh_volumes().await.expect("refresh");
		sd_core::ops::volumes::index::map_attached_volumes(
			&self.library,
			&self.core.context,
			false,
		)
		.await;
	}

	async fn teardown(self) {
		drop(self.library);
		self.core.shutdown().await.expect("shutdown");
		drop(self.core);
		// Pools that outlive shutdown keep the data directory busy for the
		// helper's plain unmount; a lazy one detaches it now.
		tokio::time::sleep(Duration::from_secs(2)).await;
		sudo(&["umount", "-l", self.mount.to_str().unwrap()]).await;
		drop(self.volume);
	}
}

/// A first track on a disk too full to open the new source's store.
#[tokio::test]
async fn a_first_track_on_a_full_data_disk_fails_visibly_and_the_heal_walks_the_source() {
	let Some(disk) = Disk::new("SdFullDataDisk").await else {
		return;
	};
	let cache = disk.core.context.volume_index();
	disk.fill().await;

	// 1. The add reports that indexing did not start, and says it will retry.
	let error = track(&disk.tree)
		.execute(disk.library.clone(), disk.core.context.clone())
		.await
		.expect_err("a walk that cannot start fails the add");
	assert_retry_promised(error);

	// 2. The source stays registered, with no record count, which is the
	// heal's evidence that it never finished a walk, and nothing reads it
	// as being indexed.
	let source = cache
		.sources_of(disk.library.id())
		.into_iter()
		.find(|source| source.root == disk.tree)
		.expect("the source stays registered");
	assert_eq!(source.entry_count, None);
	assert!(
		!cache.is_indexing(&disk.tree),
		"no walk is running for the source"
	);

	// 3. Once there is room, the heal walks the source to completion.
	disk.free_and_heal().await;
	let store = cache.store_for(&disk.tree).await.expect("store");
	assert_eq!(store.id(), source.id);
	assert_healed(&store, EXPECTED_RECORDS).await;

	drop(store);
	disk.teardown().await;
}

/// A first track whose store opens but whose walk cannot be queued, because
/// `jobs.db` refuses the job row the way a full disk did. Which write a
/// nearly full disk refuses first depends on how much room SQLite already
/// holds, so a trigger stands in for the full disk here and makes the order
/// fixed: the store is marked before the dispatch, so the mark is there when
/// the dispatch fails.
#[tokio::test]
async fn a_walk_that_cannot_be_queued_leaves_its_store_marked_for_the_heal() {
	let _ = tracing_subscriber::fmt::try_init();
	let temp = tempfile::tempdir().expect("tempdir");
	let tree = temp.path().join("tree");
	write_tree(&tree).await;
	let core = boot(&temp.path().join("data")).await;
	let library = core
		.libraries
		.list()
		.await
		.into_iter()
		.next()
		.expect("default library");
	let cache = core.context.volume_index();

	let jobs = sqlx::SqlitePool::connect(&format!(
		"sqlite://{}",
		library.path().join("jobs.db").display()
	))
	.await
	.expect("jobs.db");
	sqlx::query(
		"CREATE TRIGGER refuse_jobs BEFORE INSERT ON jobs BEGIN SELECT RAISE(ABORT, 'database or disk is full'); END",
	)
	.execute(&jobs)
	.await
	.expect("trigger");

	let error = track(&tree)
		.execute(library.clone(), core.context.clone())
		.await
		.expect_err("a walk that cannot be queued fails the add");
	assert_retry_promised(error);
	let store = cache.store_for(&tree).await.expect("store");
	assert!(
		store.walk_unfinished().await,
		"the store is marked before the walk is queued"
	);
	assert!(
		!cache.is_indexing(&tree),
		"no walk is running for the source"
	);

	sqlx::query("DROP TRIGGER refuse_jobs")
		.execute(&jobs)
		.await
		.expect("drop trigger");
	jobs.close().await;
	sd_core::ops::volumes::index::map_attached_volumes(&library, &core.context, false).await;
	assert_healed(&store, EXPECTED_RECORDS).await;

	drop(store);
	drop(library);
	core.shutdown().await.expect("shutdown");
}
