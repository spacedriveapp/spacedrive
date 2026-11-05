//! A source whose first walk was killed heals on a volume the volume list
//! hides (\`docs/core/acceptance/candidate-validation.md\`, step 4).
//!
//! On Linux \`/\`, \`/home\` and \`/tmp\` are system mounts the volume list hides,
//! and a home folder source sits on one of them. The heal resolves registered
//! sources directly, so visibility does not decide whether a source is
//! repaired. The fixture lives in a temporary directory, which on Linux is on
//! one of those hidden mounts.
//!
//! One \`Core\` over temporary directories, stopped mid-walk and restarted. No
//! loop device, no network.

mod helpers;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use helpers::TestConfigBuilder;
use sd_core::{
	domain::addressing::SdPath,
	infra::{action::LibraryAction, api::SessionContext, query::LibraryQuery},
	library::Library,
	ops::{
		files::query::directory_listing::{
			DirectoryListingInput, DirectoryListingQuery, DirectorySortBy,
		},
		indexing::startup::{StartupIndexingAction, StartupIndexingInput},
		sources::track::action::{TrackSourceAction, TrackSourceInput},
	},
	Core,
};

const DIRECTORIES: usize = 300;
const FILES_PER_DIRECTORY: usize = 100;

async fn boot(data_dir: &Path) -> Arc<Core> {
	let mut config = TestConfigBuilder::new(data_dir.to_path_buf())
		.build()
		.expect("config");
	config.services.fs_watcher_enabled = true;
	// A real daemon detects volumes, hidden ones included; the scenario is
	// about a source the volume list hides.
	config.services.volume_monitoring_enabled = true;
	config.save().expect("save config");
	let core = Arc::new(Core::new(data_dir.to_path_buf()).await.expect("core"));
	// The launch pass then heals and maps registered sources only, without
	// adding the home folder or walking whole drives.
	core.context.startup_indexing.skip_default_sources();
	core
}

async fn library_of(core: &Core) -> Arc<Library> {
	core.libraries
		.list()
		.await
		.into_iter()
		.next()
		.expect("default library")
}

async fn list_dir(core: &Arc<Core>, library: &Arc<Library>, path: PathBuf) -> usize {
	let mut session = SessionContext::device_session(
		sd_core::device::get_current_device_id(),
		sd_core::device::get_current_device_slug(),
	);
	session.current_library_id = Some(library.id());
	DirectoryListingQuery::from_input(DirectoryListingInput {
		path: SdPath::Physical {
			device_slug: sd_core::device::get_current_device_slug(),
			path,
		},
		limit: None,
		include_hidden: Some(false),
		sort_by: DirectorySortBy::Name,
		folders_first: Some(false),
		overlay: None,
	})
	.expect("input")
	.execute(core.context.clone(), session)
	.await
	.expect("listing")
	.files
	.len()
}

#[tokio::test]
async fn a_killed_first_walk_heals_on_a_hidden_volume() {
	let _ = tracing_subscriber::fmt::try_init();

	let root_dir = tempfile::Builder::new()
		.prefix("sd-hidden-heal")
		.tempdir()
		.expect("root dir");
	for directory in 0..DIRECTORIES {
		let path = root_dir.path().join(format!("d{directory:03}"));
		std::fs::create_dir(&path).expect("mkdir");
		for file in 0..FILES_PER_DIRECTORY {
			std::fs::write(path.join(format!("f{file:03}.txt")), b"x").expect("write");
		}
	}
	let records = DIRECTORIES * (FILES_PER_DIRECTORY + 1);

	let data_dir = tempfile::tempdir().expect("data dir");
	let core = boot(data_dir.path()).await;
	let library = library_of(&core).await;

	if cfg!(target_os = "linux") {
		let (volume, _) = core
			.context
			.volume_manager
			.locate_path(root_dir.path())
			.await
			.expect("the fixture sits on a detected volume");
		assert!(
			!volume.is_user_visible,
			"the fixture's volume {} is visible; this scenario needs a hidden one",
			volume.mount_point.display()
		);
	}

	let output = TrackSourceAction::from_input(TrackSourceInput {
		path: root_dir.path().to_path_buf(),
		name: None,
		overrides: Default::default(),
	})
	.expect("input")
	.execute(library.clone(), core.context.clone())
	.await
	.expect("track");
	let root = output.root.clone();
	assert!(output.job_id.is_some(), "tracking dispatched a walk");

	// Stop while the walk is still discovering, before any of it lands.
	drop(library);
	core.shutdown().await.expect("shutdown");
	drop(core);
	tokio::time::sleep(Duration::from_secs(2)).await;

	let core = boot(data_dir.path()).await;
	let library = library_of(&core).await;
	let cache = core.context.volume_index();
	let source = cache
		.sources_of(library.id())
		.into_iter()
		.find(|source| source.root == root)
		.expect("the source survives the restart");
	assert_eq!(
		source.entry_count, None,
		"the first walk finished before the stop, so this run proves nothing"
	);

	// The heal runs in the launch pass a desktop or \`sd index start\` requests.
	StartupIndexingAction::from_input(StartupIndexingInput { force: true })
		.expect("input")
		.execute(library.clone(), core.context.clone())
		.await
		.expect("startup pass");

	let mut held = 0;
	for _ in 0..600 {
		if let Some(store) = cache.store_for(&root).await {
			held = store
				.counts()
				.await
				.map(|counts| counts.records)
				.unwrap_or(0);
			if held as usize >= records && !cache.is_indexing(&root) {
				break;
			}
		}
		tokio::time::sleep(Duration::from_millis(100)).await;
	}
	assert_eq!(
		held as usize, records,
		"the healed store holds every directory and file"
	);
	assert_eq!(
		list_dir(&core, &library, root.clone()).await,
		DIRECTORIES,
		"the source root lists every directory"
	);
	assert_eq!(
		list_dir(&core, &library, root.join("d150")).await,
		FILES_PER_DIRECTORY,
		"a directory deep in the walk lists every file"
	);

	drop(library);
	core.shutdown().await.expect("shutdown");
}
