//! Changes made while the daemon is stopped reach the listing, search and the
//! store after a restart (`docs/core/acceptance/candidate-validation.md`,
//! step 4, and the warm start in `docs/core/design/zero-onboarding-startup.md`).
//!
//! A source is walked and hashed, the daemon stops, files are added, deleted,
//! modified and renamed, and the daemon starts again. The startup reconcile
//! compares the restored map with the disk and applies every difference
//! through the writer the watcher uses. Renamed files keep their records, and
//! an unchanged file is not hashed again. Both restore paths are covered: a
//! map restored from its snapshot, and one rebuilt from the store.
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
	infra::{action::LibraryAction, api::SessionContext, query::LibraryQuery},
	library::Library,
	ops::{
		files::query::directory_listing::{
			DirectoryListingInput, DirectoryListingQuery, DirectorySortBy,
		},
		search::{
			input::{
				FileSearchInput, PaginationOptions, SearchFilters, SearchMode, SearchScope,
				SortDirection, SortField, SortOptions,
			},
			query::FileSearchQuery,
		},
		sources::track::action::{TrackSourceAction, TrackSourceInput},
	},
	Core,
};
use tempfile::TempDir;
use uuid::Uuid;

const FILES: [&str; 8] = [
	"alpha.txt",
	"bravo.txt",
	"charlie.txt",
	"delta.txt",
	"india/juliet.txt",
	"lima/mike/november.txt",
	"lima/oscar.txt",
	"quebec/romeo.txt",
];

/// Enough unchanged files that the removals stay well under the sweep fuse.
const FILLER: usize = 150;

async fn boot(data_dir: &Path) -> Arc<Core> {
	let mut config = TestConfigBuilder::new(data_dir.to_path_buf())
		.build()
		.expect("config");
	config.services.fs_watcher_enabled = true;
	config.save().expect("save config");
	Arc::new(Core::new(data_dir.to_path_buf()).await.expect("core"))
}

async fn library_at(core: &Core, path: &Path) -> Arc<Library> {
	core.libraries
		.list()
		.await
		.into_iter()
		.find(|library| library.path() == path)
		.expect("the library loads")
}

fn session(library: &Library) -> SessionContext {
	let mut session = SessionContext::device_session(
		sd_core::device::get_current_device_id(),
		sd_core::device::get_current_device_slug(),
	);
	session.current_library_id = Some(library.id());
	session
}

/// Sorted names a listing returns. `File::name` is the stem.
async fn list_dir(core: &Arc<Core>, library: &Arc<Library>, path: PathBuf) -> Vec<String> {
	let listing = DirectoryListingQuery::from_input(DirectoryListingInput {
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
	.execute(core.context.clone(), session(library))
	.await
	.expect("listing");
	let mut names: Vec<String> = listing.files.iter().map(|file| file.name.clone()).collect();
	names.sort();
	names
}

/// Paths a search under the source finds for a word.
async fn search(core: &Arc<Core>, library: &Arc<Library>, root: &Path, word: &str) -> Vec<String> {
	let output = FileSearchQuery::new(FileSearchInput {
		query: word.to_string(),
		scope: SearchScope::Path {
			path: SdPath::local(root.to_path_buf()),
		},
		mode: SearchMode::Normal,
		filters: SearchFilters::default(),
		sort: SortOptions {
			field: SortField::Relevance,
			direction: SortDirection::Desc,
		},
		pagination: PaginationOptions {
			limit: 50,
			offset: 0,
		},
	})
	.execute(core.context.clone(), session(library))
	.await
	.expect("search");
	let mut found: Vec<String> = output
		.results
		.iter()
		.filter_map(|result| result.file.sd_path.as_local_path().map(Path::to_path_buf))
		.filter_map(|path| {
			path.strip_prefix(root)
				.ok()
				.map(|path| path.to_string_lossy().into_owned())
		})
		.collect();
	found.sort();
	found
}

struct Stopped {
	_root_dir: TempDir,
	root: PathBuf,
	data_dir: TempDir,
	library_path: PathBuf,
	snapshot_path: PathBuf,
	/// Record uuids and content uuids before the stop, by relative path.
	records: Vec<(&'static str, Uuid, Option<Uuid>)>,
}

impl Stopped {
	fn record(&self, file: &str) -> Uuid {
		self.records
			.iter()
			.find(|(path, ..)| *path == file)
			.map(|(_, uuid, _)| *uuid)
			.expect("fixture file")
	}

	fn content(&self, file: &str) -> Option<Uuid> {
		self.records
			.iter()
			.find(|(path, ..)| *path == file)
			.and_then(|(.., content)| *content)
	}
}

async fn wait_for_hashing(core: &Core, root: &Path) {
	let store = core
		.context
		.volume_index()
		.store_for(root)
		.await
		.expect("store");
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
}

/// Walk and hash a source, then stop the daemon cleanly. An empty source
/// holds nothing at all, not even the filler.
async fn walked_and_stopped(name: &str, empty: bool) -> Stopped {
	let root_dir = tempfile::Builder::new()
		.prefix(name)
		.tempdir()
		.expect("root dir");
	let files: &[&'static str] = if empty { &[] } else { &FILES };
	for file in files {
		let path = root_dir.path().join(file);
		std::fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
		std::fs::write(&path, file).expect("write");
	}
	let filler = if empty { 0 } else { FILLER };
	if !empty {
		std::fs::create_dir(root_dir.path().join("filler")).expect("mkdir");
		// A folder that holds only another, empty folder.
		std::fs::create_dir_all(root_dir.path().join("yankee/zulu")).expect("mkdir");
	}
	for index in 0..filler {
		std::fs::write(
			root_dir.path().join(format!("filler/papa{index:03}.txt")),
			format!("filler {index}"),
		)
		.expect("write");
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
		path: root_dir.path().to_path_buf(),
		name: None,
		overrides: Default::default(),
	})
	.expect("input")
	.execute(library.clone(), core.context.clone())
	.await
	.expect("track");
	let root = output.root.clone();
	if let Some(walk) = library
		.jobs()
		.get_job(sd_core::infra::job::prelude::JobId(
			output.job_id.expect("tracking dispatched a walk"),
		))
		.await
	{
		walk.wait().await.expect("walk");
	}
	wait_for_hashing(&core, &root).await;

	let cache = core.context.volume_index();
	let store = cache.store_for(&root).await.expect("store");
	let mut records = Vec::new();
	for &file in files {
		let uuid = store
			.db()
			.resolve_path(file)
			.await
			.expect("query")
			.unwrap_or_else(|| panic!("{file} is in the store"));
		records.push((file, uuid, store.content_of(uuid).await));
	}
	assert!(
		records.iter().all(|(.., content)| content.is_some()),
		"the fixture is hashed before the stop"
	);
	let snapshot_path = cache.snapshot_path_for(&root).expect("snapshot path");

	drop(store);
	drop(library);
	core.shutdown().await.expect("shutdown");
	drop(core);
	tokio::time::sleep(Duration::from_secs(2)).await;

	Stopped {
		_root_dir: root_dir,
		root,
		data_dir,
		library_path,
		snapshot_path,
		records,
	}
}

/// What a person does while the daemon is not running.
fn change_while_stopped(root: &Path) {
	std::fs::write(root.join("foxtrot.txt"), "new while stopped").expect("add");
	std::fs::create_dir(root.join("golf")).expect("add dir");
	std::fs::write(root.join("golf/hotel.txt"), "new in a new folder").expect("add");
	std::fs::remove_file(root.join("alpha.txt")).expect("delete");
	std::fs::remove_dir_all(root.join("lima/mike")).expect("delete dir");
	std::fs::write(root.join("bravo.txt"), "bravo, rewritten longer").expect("modify");
	std::fs::rename(root.join("delta.txt"), root.join("echo.txt")).expect("rename");
	std::fs::rename(root.join("india"), root.join("kilo")).expect("rename dir");
	std::fs::write(root.join("india"), "a file where the moved folder was").expect("reuse path");
	std::fs::remove_dir_all(root.join("quebec")).expect("replace dir");
	std::fs::write(root.join("quebec"), "a file where a folder was").expect("replace dir");
}

async fn restart_and_check(stopped: Stopped) {
	let core = boot(stopped.data_dir.path()).await;
	let library = library_at(&core, &stopped.library_path).await;
	let root = stopped.root.clone();

	let expected = vec![
		"bravo", "charlie", "echo", "filler", "foxtrot", "golf", "india", "kilo", "lima", "quebec",
		"yankee",
	];
	let mut listed = Vec::new();
	for _ in 0..300 {
		listed = list_dir(&core, &library, root.clone()).await;
		if listed == expected {
			break;
		}
		tokio::time::sleep(Duration::from_millis(100)).await;
	}
	assert_eq!(
		listed, expected,
		"the root lists every change made while stopped"
	);
	assert_eq!(
		list_dir(&core, &library, root.join("golf")).await,
		vec!["hotel"],
		"a folder created while stopped lists its file"
	);
	assert_eq!(
		list_dir(&core, &library, root.join("kilo")).await,
		vec!["juliet"],
		"a renamed folder lists its file"
	);
	assert_eq!(
		list_dir(&core, &library, root.join("lima")).await,
		vec!["oscar"],
		"a folder deleted while stopped is gone"
	);
	assert_eq!(
		list_dir(&core, &library, root.join("filler")).await.len(),
		FILLER,
		"unchanged files are all still listed"
	);

	for (word, found) in [
		("alpha", vec![]),
		("delta", vec![]),
		("november", vec![]),
		("romeo", vec![]),
		("bravo", vec!["bravo.txt"]),
		("echo", vec!["echo.txt"]),
		("foxtrot", vec!["foxtrot.txt"]),
		("hotel", vec!["golf/hotel.txt"]),
		("juliet", vec!["kilo/juliet.txt"]),
	] {
		assert_eq!(
			search(&core, &library, &root, word).await,
			found,
			"search for {word} reflects the changes"
		);
	}

	let cache = core.context.volume_index();
	{
		let index = cache.get_for_search(&root).expect("map");
		let index = index.read().await;
		assert_eq!(
			index.get_entry_uuid(&root.join("echo.txt")),
			Some(stopped.record("delta.txt")),
			"a file renamed while stopped keeps its record"
		);
		assert_eq!(
			index.get_entry_uuid(&root.join("kilo/juliet.txt")),
			Some(stopped.record("india/juliet.txt")),
			"a file in a renamed folder keeps its record"
		);
		assert_eq!(
			index.get_entry_uuid(&root.join("charlie.txt")),
			Some(stopped.record("charlie.txt")),
			"an unchanged file keeps its record"
		);
	}

	wait_for_hashing(&core, &root).await;
	let store = cache.store_for(&root).await.expect("store");
	let db = store.db();
	for gone in [
		"alpha.txt",
		"delta.txt",
		"india/juliet.txt",
		"lima/mike/november.txt",
		"quebec/romeo.txt",
	] {
		assert_eq!(
			db.resolve_path(gone).await.expect("query"),
			None,
			"{gone} left the store"
		);
	}
	assert_eq!(
		db.resolve_path("echo.txt").await.expect("query"),
		Some(stopped.record("delta.txt")),
		"the store moved the renamed file's record"
	);
	let foxtrot = db
		.resolve_path("foxtrot.txt")
		.await
		.expect("query")
		.expect("the new file is in the store");
	assert!(
		store.content_of(foxtrot).await.is_some(),
		"the new file is hashed"
	);
	let bravo = stopped.record("bravo.txt");
	assert_eq!(
		db.resolve_path("bravo.txt").await.expect("query"),
		Some(bravo),
		"a modified file keeps its record"
	);
	assert_ne!(
		store.content_of(bravo).await,
		stopped.content("bravo.txt"),
		"the modified file is hashed again"
	);
	assert_eq!(
		store.content_of(stopped.record("charlie.txt")).await,
		stopped.content("charlie.txt"),
		"the unchanged file keeps its content"
	);
	assert_eq!(
		store.content_of(stopped.record("delta.txt")).await,
		stopped.content("delta.txt"),
		"the renamed file keeps its content"
	);

	drop(store);
	drop(library);
	core.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn changes_while_stopped_reach_a_map_restored_from_its_snapshot() {
	let _ = tracing_subscriber::fmt::try_init();
	let stopped = walked_and_stopped("sd-reconcile-snapshot", false).await;
	assert!(
		stopped.snapshot_path.exists(),
		"the fixture left a snapshot"
	);
	change_while_stopped(&stopped.root);
	restart_and_check(stopped).await;
}

#[tokio::test]
async fn changes_while_stopped_reach_a_map_rebuilt_from_the_store() {
	let _ = tracing_subscriber::fmt::try_init();
	let stopped = walked_and_stopped("sd-reconcile-store", false).await;
	std::fs::remove_file(&stopped.snapshot_path).expect("delete snapshot");
	change_while_stopped(&stopped.root);
	restart_and_check(stopped).await;
}

/// R8's half-removal fuse holds for the reconcile too: a pass that would
/// remove most of what the map holds is evidence about the pass, not about
/// the files, so it removes nothing and leaves the next full walk to decide.
#[tokio::test]
async fn a_reconcile_that_would_remove_most_of_a_source_removes_nothing() {
	let _ = tracing_subscriber::fmt::try_init();
	let stopped = walked_and_stopped("sd-reconcile-fuse", false).await;
	std::fs::remove_dir_all(stopped.root.join("filler")).expect("delete most of the source");
	std::fs::write(stopped.root.join("foxtrot.txt"), "new while stopped").expect("add");

	let core = boot(stopped.data_dir.path()).await;
	let library = library_at(&core, &stopped.library_path).await;
	let root = stopped.root.clone();

	let mut listed = Vec::new();
	for _ in 0..300 {
		listed = list_dir(&core, &library, root.clone()).await;
		if listed.contains(&"foxtrot".to_string()) {
			break;
		}
		tokio::time::sleep(Duration::from_millis(100)).await;
	}
	assert!(
		listed.contains(&"foxtrot".to_string()),
		"additions still land: {listed:?}"
	);
	assert!(
		listed.contains(&"filler".to_string()) && listed.contains(&"alpha".to_string()),
		"the refused removals left the map as it was: {listed:?}"
	);
	let store = core
		.context
		.volume_index()
		.store_for(&root)
		.await
		.expect("store");
	store.flush().await.expect("flush");
	assert!(
		store
			.db()
			.resolve_path("filler/papa000.txt")
			.await
			.expect("query")
			.is_some(),
		"the store kept the records the fuse protected"
	);

	drop(store);
	drop(library);
	core.shutdown().await.expect("shutdown");
}

/// Moves are not removals. Renaming the folder that holds most of the source
/// moves every record in it, and the fuse that guards against a pass that
/// lost its footing must not hold the old paths in place.
#[tokio::test]
async fn renaming_most_of_a_source_is_not_a_mass_removal() {
	let _ = tracing_subscriber::fmt::try_init();
	let stopped = walked_and_stopped("sd-reconcile-bulk-move", false).await;
	let papa = stopped_record(&stopped, "filler/papa000.txt").await;
	std::fs::rename(stopped.root.join("filler"), stopped.root.join("moved"))
		.expect("rename most of the source");

	let core = boot(stopped.data_dir.path()).await;
	let library = library_at(&core, &stopped.library_path).await;
	let root = stopped.root.clone();

	let mut listed = Vec::new();
	for _ in 0..300 {
		listed = list_dir(&core, &library, root.clone()).await;
		if listed.contains(&"moved".to_string()) && !listed.contains(&"filler".to_string()) {
			break;
		}
		tokio::time::sleep(Duration::from_millis(100)).await;
	}
	assert!(
		listed.contains(&"moved".to_string()) && !listed.contains(&"filler".to_string()),
		"the renamed folder replaces the old one: {listed:?}"
	);
	assert_eq!(
		list_dir(&core, &library, root.join("moved")).await.len(),
		FILLER,
		"every moved file lists under the new name"
	);
	assert_eq!(
		search(&core, &library, &root, "papa000").await,
		vec!["moved/papa000.txt"],
		"search finds the file once, at its new path"
	);
	let index = core
		.context
		.volume_index()
		.get_for_search(&root)
		.expect("map");
	assert_eq!(
		index
			.read()
			.await
			.get_entry_uuid(&root.join("moved/papa000.txt")),
		Some(papa),
		"a moved file keeps its record"
	);

	drop(library);
	core.shutdown().await.expect("shutdown");
}

/// A source whose walk found it empty is covered, not a gap, so what was
/// added to it while the daemon was stopped is picked up.
#[tokio::test]
async fn an_empty_source_picks_up_files_added_while_stopped() {
	let _ = tracing_subscriber::fmt::try_init();
	let stopped = walked_and_stopped("sd-reconcile-empty", true).await;
	std::fs::write(stopped.root.join("sierra.txt"), "added while stopped").expect("add");
	std::fs::create_dir(stopped.root.join("tango")).expect("add dir");
	std::fs::write(
		stopped.root.join("tango/uniform.txt"),
		"added while stopped",
	)
	.expect("add");

	let core = boot(stopped.data_dir.path()).await;
	let library = library_at(&core, &stopped.library_path).await;
	let root = stopped.root.clone();

	let expected = vec!["sierra", "tango"];
	let mut listed = Vec::new();
	for _ in 0..300 {
		listed = list_dir(&core, &library, root.clone()).await;
		if listed == expected {
			break;
		}
		tokio::time::sleep(Duration::from_millis(100)).await;
	}
	assert_eq!(listed, expected, "the empty source lists what was added");
	// A restore of an empty root browses it one level, so the root lists
	// before the reconcile has reached the folder below it.
	let mut found = Vec::new();
	for _ in 0..300 {
		found = search(&core, &library, &root, "uniform").await;
		if !found.is_empty() {
			break;
		}
		tokio::time::sleep(Duration::from_millis(100)).await;
	}
	assert_eq!(
		found,
		vec!["tango/uniform.txt"],
		"search finds a file added to an empty source"
	);

	drop(library);
	core.shutdown().await.expect("shutdown");
}

/// A filler record's uuid from the stopped fixture's store.
async fn stopped_record(stopped: &Stopped, path: &str) -> Uuid {
	let core = boot(stopped.data_dir.path()).await;
	let store = core
		.context
		.volume_index()
		.store_for(&stopped.root)
		.await
		.expect("store");
	let uuid = store
		.db()
		.resolve_path(path)
		.await
		.expect("query")
		.expect("the record is in the store");
	drop(store);
	core.shutdown().await.expect("shutdown");
	tokio::time::sleep(Duration::from_secs(2)).await;
	uuid
}

/// A folder moved while stopped whose new home cannot be read: its own
/// identity resolves, but its files cannot arrive, so they are not removed.
#[cfg(unix)]
#[tokio::test]
async fn an_unreadable_moved_folder_keeps_its_records() {
	use std::os::unix::fs::PermissionsExt;

	let _ = tracing_subscriber::fmt::try_init();
	let stopped = walked_and_stopped("sd-reconcile-unreadable-move", false).await;
	let moved = stopped.root.join("moved");
	std::fs::rename(stopped.root.join("filler"), &moved).expect("rename");
	std::fs::set_permissions(&moved, std::fs::Permissions::from_mode(0o000)).expect("chmod");
	let folders = stopped.root.join("yankee-moved");
	std::fs::rename(stopped.root.join("yankee"), &folders).expect("rename");
	std::fs::set_permissions(&folders, std::fs::Permissions::from_mode(0o000)).expect("chmod");
	if std::fs::read_dir(&moved).is_ok() {
		std::fs::set_permissions(&folders, std::fs::Permissions::from_mode(0o755)).expect("chmod");
		std::fs::set_permissions(&moved, std::fs::Permissions::from_mode(0o755)).expect("chmod");
		eprintln!("skipping: this user can read a mode 000 directory");
		return;
	}

	let core = boot(stopped.data_dir.path()).await;
	let library = library_at(&core, &stopped.library_path).await;
	let root = stopped.root.clone();
	for _ in 0..300 {
		if list_dir(&core, &library, root.clone())
			.await
			.contains(&"moved".to_string())
		{
			break;
		}
		tokio::time::sleep(Duration::from_millis(100)).await;
	}
	tokio::time::sleep(Duration::from_secs(1)).await;

	let store = core
		.context
		.volume_index()
		.store_for(&root)
		.await
		.expect("store");
	store.flush().await.expect("flush");
	let mut kept = 0;
	for index in 0..FILLER {
		let db = store.db();
		if db
			.resolve_path(&format!("filler/papa{index:03}.txt"))
			.await
			.expect("query")
			.is_some()
			|| db
				.resolve_path(&format!("moved/papa{index:03}.txt"))
				.await
				.expect("query")
				.is_some()
		{
			kept += 1;
		}
	}
	let db = store.db();
	let zulu = db
		.resolve_path("yankee/zulu")
		.await
		.expect("query")
		.is_some()
		|| db
			.resolve_path("yankee-moved/zulu")
			.await
			.expect("query")
			.is_some();
	std::fs::set_permissions(&moved, std::fs::Permissions::from_mode(0o755)).expect("chmod");
	std::fs::set_permissions(&folders, std::fs::Permissions::from_mode(0o755)).expect("chmod");
	assert_eq!(
		kept, FILLER,
		"no record under the unreadable folder was removed"
	);
	assert!(
		zulu,
		"a folder under an unreadable moved folder keeps its record"
	);

	drop(store);
	drop(library);
	core.shutdown().await.expect("shutdown");
}
