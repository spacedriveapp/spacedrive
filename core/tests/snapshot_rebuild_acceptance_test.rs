//! Missing or unusable restart snapshot (R8 "Missing or invalid restart
//! snapshot", `docs/core/acceptance/source-runtime.md`): a source whose
//! drive snapshot is gone, corrupt, or from an older format lists from a
//! map rebuilt out of its store, with the store's uuids, before any walk
//! runs. The unreadable artifact stays beside the slot for diagnosis.
//!
//! One `Core` over temporary directories, restarted between the fixture and
//! the check. No loop device, no network.

mod helpers;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use helpers::TestConfigBuilder;
use sd_core::{
	domain::addressing::SdPath,
	infra::{
		action::LibraryAction,
		api::SessionContext,
		query::{CoreQuery, LibraryQuery},
	},
	library::Library,
	ops::{
		core::index_status::query::{IndexStatusInput, IndexStatusQuery},
		files::query::directory_listing::{
			DirectoryListingInput, DirectoryListingQuery, DirectorySortBy,
		},
		sources::track::action::{TrackSourceAction, TrackSourceInput},
	},
	Core,
};
use tempfile::TempDir;
use uuid::Uuid;

/// A daemon over `data_dir`: watcher on, so the restart restores every
/// registered source the way a real launch does; networking off.
async fn boot(data_dir: &Path) -> Arc<Core> {
	let mut config = TestConfigBuilder::new(data_dir.to_path_buf())
		.build()
		.expect("config");
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

async fn job_count(library: &Library) -> usize {
	library.jobs().list_jobs(None).await.expect("jobs").len()
}

/// A walked and hashed source whose daemon shut down cleanly, so its
/// snapshot and store are both on disk.
struct Walked {
	_root_dir: TempDir,
	root: PathBuf,
	data_dir: TempDir,
	library_path: PathBuf,
	snapshot_path: PathBuf,
	/// Every file's path and the uuid its store assigned.
	identities: Vec<(PathBuf, Uuid)>,
}

const FILES: [&str; 6] = [
	"a.txt",
	"b.txt",
	"photos/c.jpg",
	"photos/d.jpg",
	"photos/trip/e.jpg",
	"notes/f.md",
];

async fn walked_source(name: &str) -> Walked {
	let root_dir = tempfile::Builder::new()
		.prefix(name)
		.tempdir()
		.expect("root dir");
	let root = root_dir.path().to_path_buf();
	for file in FILES {
		let path = root.join(file);
		std::fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
		std::fs::write(&path, file).expect("write");
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
	let library_path = library.path().to_path_buf();

	let output = TrackSourceAction::from_input(TrackSourceInput {
		path: root.clone(),
		name: None,
		overrides: Default::default(),
	})
	.expect("input")
	.execute(library.clone(), core.context.clone())
	.await
	.expect("track");
	let root = output.root.clone();
	let job_id = output.job_id.expect("tracking dispatched a walk");
	if let Some(walk) = library
		.jobs()
		.get_job(sd_core::infra::job::prelude::JobId(job_id))
		.await
	{
		walk.wait().await.expect("walk");
	}

	let cache = core.context.volume_index();
	let store = cache.store_for(&root).await.expect("store");
	// The hashing pass the walk queued is part of the fixture; wait on its
	// effect so the restart is not about job resumption.
	for _ in 0..300 {
		if store
			.files_needing_content_count()
			.await
			.expect("pending count")
			== 0
		{
			break;
		}
		tokio::time::sleep(Duration::from_millis(100)).await;
	}
	store.flush().await.expect("flush");
	let db = store.db();
	let mut identities = Vec::new();
	for file in FILES {
		let uuid = db
			.resolve_path(file)
			.await
			.expect("query")
			.unwrap_or_else(|| panic!("{file} is in the store"));
		identities.push((root.join(file), uuid));
	}
	let snapshot_path = cache.snapshot_path_for(&root).expect("snapshot path");

	drop(library);
	core.shutdown().await.expect("shutdown");
	drop(core);
	// Spawned service tasks let go of the key store after shutdown returns.
	tokio::time::sleep(Duration::from_secs(2)).await;

	assert!(
		snapshot_path.exists(),
		"the fixture left a snapshot at {}",
		snapshot_path.display()
	);

	Walked {
		_root_dir: root_dir,
		root,
		data_dir,
		library_path,
		snapshot_path,
		identities,
	}
}

/// Restart over the fixture and check the source lists from a map the
/// store rebuilt: no walk dispatched, the listing served, the store's uuids
/// kept, the status surface reporting the source restored with nothing in
/// progress.
async fn restart_and_check(walked: &Walked, retained_prefix: Option<&str>) {
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

	let listing = DirectoryListingQuery::from_input(DirectoryListingInput {
		path: SdPath::Physical {
			device_slug: sd_core::device::get_current_device_slug(),
			path: walked.root.clone(),
		},
		limit: None,
		include_hidden: Some(false),
		sort_by: DirectorySortBy::Name,
		folders_first: Some(false),
		overlay: None,
	})
	.expect("input")
	.execute(core.context.clone(), session(&library))
	.await
	.expect("listing");
	// `File::name` is the stem; the extension travels separately.
	let mut names: Vec<String> = listing.files.iter().map(|file| file.name.clone()).collect();
	names.sort();
	assert_eq!(
		names,
		vec!["a", "b", "notes", "photos"],
		"the listing is served from the rebuilt map"
	);
	assert!(
		cache.arena_answers(&walked.root),
		"the arena answers for the source"
	);
	assert!(
		!cache.restored_from_snapshot(&walked.root),
		"nothing came from the snapshot file"
	);
	assert!(
		!cache.is_indexing(&walked.root),
		"no walk is in progress over the source"
	);
	assert_eq!(
		job_count(&library).await,
		jobs_before,
		"the listing dispatched no walk"
	);

	let index = cache.get_for_search(&walked.root).expect("rebuilt index");
	let index = index.read().await;
	for (path, uuid) in &walked.identities {
		assert_eq!(
			index.get_entry_uuid(path),
			Some(*uuid),
			"{} keeps the uuid its store holds",
			path.display()
		);
	}
	drop(index);

	let status = IndexStatusQuery::from_input(IndexStatusInput::default())
		.expect("input")
		.execute(core.context.clone(), session(&library))
		.await
		.expect("index status");
	let source = status
		.sources
		.iter()
		.find(|source| source.root == walked.root)
		.expect("index status lists the source");
	assert!(source.attached, "the source is attached");
	assert!(
		source.restored,
		"core.index_status reports the map restored"
	);
	assert_eq!(
		status.indexing_in_progress_count, 0,
		"core.index_status reports no walk in progress: {:?}",
		status.paths_in_progress
	);

	assert!(
		!walked.snapshot_path.exists(),
		"the slot is clear for the next save"
	);
	if let Some(prefix) = retained_prefix {
		let retained = std::fs::read_dir(walked.snapshot_path.parent().unwrap())
			.expect("snapshot dir")
			.filter_map(|entry| entry.ok().map(|entry| entry.path()))
			.filter(|path| {
				path.file_name()
					.map(|name| name.to_string_lossy().starts_with(prefix))
					.unwrap_or(false)
			})
			.count();
		assert_eq!(retained, 1, "the unusable artifact is kept beside the slot");
	}

	drop(library);
	core.shutdown().await.expect("shutdown");
}

/// The snapshot is gone: deleted by hand, lost with the cache directory, or
/// never written. The store refills the map.
#[tokio::test]
async fn a_missing_snapshot_rebuilds_the_map_from_the_store() {
	let _ = tracing_subscriber::fmt::try_init();
	let walked = walked_source("sd-snapshot-missing").await;
	std::fs::remove_file(&walked.snapshot_path).expect("delete snapshot");
	restart_and_check(&walked, None).await;
}

/// The snapshot will not parse. It is quarantined for diagnosis and the
/// store refills the map.
#[tokio::test]
async fn a_corrupt_snapshot_is_quarantined_and_the_map_rebuilt_from_the_store() {
	let _ = tracing_subscriber::fmt::try_init();
	let walked = walked_source("sd-snapshot-corrupt").await;
	std::fs::write(&walked.snapshot_path, b"this is not a snapshot").expect("corrupt");
	let name = walked
		.snapshot_path
		.file_name()
		.unwrap()
		.to_string_lossy()
		.into_owned();
	restart_and_check(&walked, Some(&format!("{name}.corrupt-"))).await;
}
