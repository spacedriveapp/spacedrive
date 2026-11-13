//! # Tracking a source whose first walk is refused
//!
//! The index refuses to walk a drive whose mount point is not a mount point,
//! because the directory an unmounted drive leaves behind must never be walked
//! as the drive. `sources.track` used to register such a source, skip the
//! walk with a log line, and report success. It has to fail visibly instead,
//! keep the source registered and marked unfinished, and let the heal walk it
//! once the refusal clears.
//!
//! On macOS a second case tracks a temporary directory on the Data volume and
//! asserts its walk is not refused: APFS reports the same device for `/` and
//! `/System/Volumes/Data`, which once made every folder under `/Users` read
//! as sitting on an unmounted drive.

#![cfg(unix)]

mod helpers;

use std::{
	path::{Path, PathBuf},
	sync::Arc,
	time::Duration,
};

use helpers::TestConfigBuilder;
use sd_core::{
	domain::volume::{Volume, VolumeFingerprint},
	infra::action::LibraryAction,
	library::{AddOverrides, Library},
	ops::{
		indexing::store::SourceStore,
		sources::track::{TrackSourceAction, TrackSourceInput},
	},
	Core,
};
use uuid::Uuid;

const FILES: usize = 50;

async fn boot(data_dir: &Path) -> (Arc<Core>, Arc<Library>) {
	let config = TestConfigBuilder::new(data_dir.to_path_buf())
		.build()
		.expect("config");
	config.save().expect("save config");
	let core = Arc::new(Core::new(data_dir.to_path_buf()).await.expect("core"));
	let library = core
		.libraries
		.list()
		.await
		.into_iter()
		.next()
		.expect("default library");
	(core, library)
}

fn write_tree(tree: &Path) {
	std::fs::create_dir_all(tree).expect("tree");
	for file in 0..FILES {
		std::fs::write(tree.join(format!("f{file:03}.txt")), b"").expect("file");
	}
}

fn track(tree: &Path) -> TrackSourceAction {
	TrackSourceAction::from_input(TrackSourceInput {
		path: tree.to_path_buf(),
		name: None,
		overrides: AddOverrides::default(),
	})
	.expect("input")
}

async fn committed(store: &SourceStore) -> u64 {
	sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM record")
		.fetch_one(store.db().pool())
		.await
		.expect("count") as u64
}

/// A detected drive mounted at `mount_point`, which is a plain directory.
fn drive_at(mount_point: PathBuf) -> Volume {
	Volume::new(
		Uuid::nil(),
		VolumeFingerprint("refused-walk-drive".into()),
		"Away".into(),
		mount_point,
	)
}

#[tokio::test]
async fn a_refused_first_walk_fails_the_track_and_the_heal_walks_it_later() {
	let _ = tracing_subscriber::fmt::try_init();
	let temp = tempfile::tempdir().expect("tempdir");
	let temp = std::fs::canonicalize(temp.path()).expect("canonical tempdir");
	let (core, library) = boot(&temp.join("data")).await;
	let cache = core.context.volume_index();

	// A drive detection reports at a directory that is not a mount point,
	// the way it does for a drive in the refresh interval after it went away.
	let drive = temp.join("drive");
	let tree = drive.join("tree");
	write_tree(&tree);
	cache.track_detected_volume(&drive_at(drive.clone()));
	assert!(cache.dispatch_refusal(&tree).is_some());

	// 1. The add says the walk did not start, why, and that it will retry.
	let message = track(&tree)
		.execute(library.clone(), core.context.clone())
		.await
		.expect_err("a refused walk fails the add")
		.to_string();
	assert!(
		message.contains("first walk could not start")
			&& message.contains("is not a mount point")
			&& message.contains("next time it starts"),
		"the error names the refusal and the retry: {message}"
	);

	// 2. The source stays registered and marked unfinished, and nothing
	// reads it as being indexed.
	let source = cache
		.sources_of(library.id())
		.into_iter()
		.find(|source| source.root == tree)
		.expect("the source stays registered");
	assert_eq!(source.entry_count, None);
	assert!(!cache.is_indexing(&tree));
	let store = cache.store_for(&tree).await.expect("store");
	assert_eq!(store.id(), source.id);
	assert!(
		store.walk_unfinished().await,
		"a refused walk leaves the store marked for the heal"
	);

	// 3. Once the drive no longer claims the tree, the heal walks it.
	cache.track_detected_volume(&drive_at(temp.join("elsewhere")));
	assert!(cache.dispatch_refusal(&tree).is_none());
	sd_core::ops::volumes::index::map_attached_volumes(&library, &core.context, false).await;
	let expected = FILES as u64 + 1;
	let mut records = 0;
	for _ in 0..600 {
		records = committed(&store).await;
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

	drop(store);
	drop(library);
	core.shutdown().await.expect("shutdown");
}

/// A folder on the APFS Data volume, reached through `$TMPDIR` the way a
/// home folder is reached through `/Users`, is walked rather than refused.
#[cfg(target_os = "macos")]
#[tokio::test]
async fn a_folder_on_the_macos_data_volume_is_walked() {
	let _ = tracing_subscriber::fmt::try_init();
	let temp = tempfile::tempdir().expect("tempdir");
	let (core, library) = boot(&temp.path().join("data")).await;
	let cache = core.context.volume_index();
	let tree = temp.path().join("tree");
	write_tree(&tree);

	let output = track(&tree)
		.execute(library.clone(), core.context.clone())
		.await
		.expect("the walk of a Data volume folder is not refused");
	assert!(output.job_id.is_some(), "a walk was queued");
	assert!(
		cache.dispatch_refusal(&output.root).is_none(),
		"{} is not refused",
		output.root.display()
	);

	let store = cache.store_for(&output.root).await.expect("store");
	let expected = FILES as u64 + 1;
	let mut records = 0;
	for _ in 0..600 {
		records = committed(&store).await;
		if records == expected && !store.walk_unfinished().await {
			break;
		}
		tokio::time::sleep(Duration::from_millis(100)).await;
	}
	assert_eq!(records, expected, "the walk lands the whole tree");

	drop(store);
	drop(library);
	core.shutdown().await.expect("shutdown");
}
