//! Acceptance for a daemon that holds more than one library: every open
//! library lists its own sources, before and after a restart, whichever
//! order the library manager loads them in.
//!
//! The core keeps one volume index, and until the registry inside it became
//! per library, attaching a library replaced whatever the previous one had
//! adopted. Load order follows `read_dir` of the data directory, so a
//! two-library daemon listed one library's sources on one machine and the
//! other's on the next.

mod helpers;

use std::{path::Path, sync::Arc};

use helpers::TestConfigBuilder;
use sd_core::{
	infra::action::CoreAction,
	infra::{action::LibraryAction, api::SessionContext, query::LibraryQuery},
	library::Library,
	ops::{
		libraries::backup::{
			LibraryBackupAction, LibraryBackupInput, LibraryRestoreAction, LibraryRestoreInput,
			RestoreMode,
		},
		sources::{
			list::query::ListSourcesQuery,
			track::{TrackSourceAction, TrackSourceInput},
		},
	},
	Core,
};
use uuid::Uuid;

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

async fn track(core: &Arc<Core>, library: &Arc<Library>, root: &Path) {
	let output = TrackSourceAction::from_input(TrackSourceInput {
		path: root.to_path_buf(),
		name: None,
		overrides: Default::default(),
	})
	.expect("input")
	.execute(library.clone(), core.context.clone())
	.await
	.expect("sources.track");
	if let Some(job_id) = output.job_id {
		if let Some(walk) = library
			.jobs()
			.get_job(sd_core::infra::job::prelude::JobId(job_id))
			.await
		{
			walk.wait().await.expect("walk");
		}
	}
}

/// The roots `sources.list` answers for a library, and the roots the volume
/// index holds for it.
async fn roots_of(core: &Arc<Core>, library: &Arc<Library>) -> (Vec<String>, Vec<String>) {
	let listed = ListSourcesQuery::all()
		.execute(core.context.clone(), session(library))
		.await
		.expect("sources.list")
		.into_iter()
		.map(|info| info.root.unwrap_or_default())
		.collect();
	let indexed = core
		.context
		.volume_index()
		.sources_of(library.id())
		.into_iter()
		.map(|source| source.root.to_string_lossy().into_owned())
		.collect();
	(listed, indexed)
}

async fn open_library(core: &Arc<Core>, id: Uuid) -> Arc<Library> {
	core.libraries
		.get_library(id)
		.await
		.unwrap_or_else(|| panic!("library {id} is open"))
}

#[tokio::test]
async fn two_libraries_list_their_own_sources_across_a_restart() {
	// The library manager logs a library it could not reopen and carries
	// on, so without a subscriber a missing library fails the count below
	// with no reason attached.
	let _ = tracing_subscriber::fmt()
		.with_env_filter(
			tracing_subscriber::EnvFilter::try_from_default_env()
				.unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("sd_core=warn")),
		)
		.with_test_writer()
		.try_init();
	let data_dir = tempfile::tempdir().expect("data dir");
	let first_root = tempfile::tempdir().expect("first root");
	let second_root = tempfile::tempdir().expect("second root");
	for (root, name) in [(&first_root, "first"), (&second_root, "second")] {
		std::fs::write(root.path().join(format!("{name}.txt")), name).expect("write");
	}

	let core = boot(data_dir.path()).await;
	let first = core
		.libraries
		.list()
		.await
		.into_iter()
		.next()
		.expect("the daemon created a default library");
	let second = core
		.libraries
		.create_library("Second", None, core.context.clone())
		.await
		.expect("create the second library");
	let (first_id, second_id) = (first.id(), second.id());
	assert_ne!(first_id, second_id);

	track(&core, &first, first_root.path()).await;
	track(&core, &second, second_root.path()).await;

	let first_expected = first_root.path().to_string_lossy().into_owned();
	let second_expected = second_root.path().to_string_lossy().into_owned();

	let check =
		|core: &Arc<Core>, first: Arc<Library>, second: Arc<Library>, when: &'static str| {
			let core = core.clone();
			let first_expected = first_expected.clone();
			let second_expected = second_expected.clone();
			async move {
				let (listed, indexed) = roots_of(&core, &first).await;
				assert_eq!(listed, vec![first_expected.clone()], "{when}: first listed");
				assert_eq!(
					indexed,
					vec![first_expected.clone()],
					"{when}: first indexed"
				);
				let (listed, indexed) = roots_of(&core, &second).await;
				assert_eq!(
					listed,
					vec![second_expected.clone()],
					"{when}: second listed"
				);
				assert_eq!(
					indexed,
					vec![second_expected.clone()],
					"{when}: second indexed"
				);

				let index = core.context.volume_index();
				assert_eq!(
					index.source_id_for(Path::new(&first_expected)),
					index.sources_of(first.id()).first().map(|s| s.id),
					"{when}: the first root resolves to the first library's source"
				);
				assert_eq!(
					index.source_id_for(Path::new(&second_expected)),
					index.sources_of(second.id()).first().map(|s| s.id),
					"{when}: the second root resolves to the second library's source"
				);
			}
		};

	check(&core, first.clone(), second.clone(), "before restart").await;

	// A path is kept by one library on this device: the second library may
	// not track the first's root, nor a directory under it.
	for overlap in [
		first_root.path().to_path_buf(),
		first_root.path().join("nested"),
	] {
		std::fs::create_dir_all(&overlap).expect("mkdir");
		let refused = TrackSourceAction::from_input(TrackSourceInput {
			path: overlap.clone(),
			name: None,
			overrides: Default::default(),
		})
		.expect("input")
		.execute(second.clone(), core.context.clone())
		.await;
		assert!(
			refused.is_err(),
			"tracking {} in the second library is refused",
			overlap.display()
		);
	}
	assert_eq!(core.context.volume_index().sources_of(second_id).len(), 1);

	drop(first);
	drop(second);
	core.shutdown().await.expect("shutdown");
	drop(core);
	// Spawned service tasks let go of the key store after shutdown returns.
	tokio::time::sleep(std::time::Duration::from_secs(2)).await;

	// The restart is where the old behaviour flaked: whichever library
	// `read_dir` handed over last owned the index. Both must answer now.
	let core = boot(data_dir.path()).await;
	let open = core.libraries.list().await;
	assert_eq!(open.len(), 2, "both libraries reopened");
	let first = open_library(&core, first_id).await;
	let second = open_library(&core, second_id).await;
	assert_eq!(
		core.context.volume_index().sources().len(),
		2,
		"the index holds both libraries' sources"
	);
	check(&core, first.clone(), second.clone(), "after restart").await;

	// Restoring one library drops the drive partitions its sources share
	// with the other library; every source on those drives gets its map
	// back from its store, so a change below the second library's root
	// still reaches its store afterwards.
	let backup_dir = data_dir.path().join("first-backup");
	LibraryBackupAction::from_input(LibraryBackupInput {
		library_id: first_id,
		destination: backup_dir.clone(),
		include_sidecars: false,
		include_replicas: false,
	})
	.expect("backup input")
	.execute(first.clone(), core.context.clone())
	.await
	.expect("backup");
	drop(first);
	LibraryRestoreAction::from_input(LibraryRestoreInput {
		source: backup_dir,
		mode: RestoreMode::Replace,
		library_id: None,
		force: false,
	})
	.expect("restore input")
	.execute(core.context.clone())
	.await
	.expect("restore");
	let first = open_library(&core, first_id).await;
	check(&core, first.clone(), second.clone(), "after restore").await;
	let second_store = core
		.context
		.volume_index()
		.store_for(second_root.path())
		.await
		.expect("the second library's store");
	let before = second_store.db().revision().await.expect("revision").value;
	std::fs::create_dir_all(second_root.path().join("deeper")).expect("mkdir");
	std::fs::write(second_root.path().join("deeper/after-restore.txt"), "after").expect("write");
	let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
	loop {
		second_store.flush().await.expect("flush");
		if second_store.db().revision().await.expect("revision").value > before {
			break;
		}
		assert!(
			std::time::Instant::now() < deadline,
			"a change below the second library's root reaches its store after the first's restore"
		);
		tokio::time::sleep(std::time::Duration::from_millis(100)).await;
	}

	// Closing one library takes only its sources with it.
	drop(first);
	core.libraries
		.close_library(first_id)
		.await
		.expect("close the first library");
	let index = core.context.volume_index();
	assert!(index.sources_of(first_id).is_empty());
	assert_eq!(
		index
			.sources_of(second_id)
			.into_iter()
			.map(|source| source.root.to_string_lossy().into_owned())
			.collect::<Vec<_>>(),
		vec![second_expected.clone()]
	);

	drop(second);
	core.shutdown().await.expect("shutdown");
}
