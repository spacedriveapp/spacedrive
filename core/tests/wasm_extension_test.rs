//! WASM extension runtime
//!
//! The daemon finds extensions under `<data dir>/extensions`, instantiates
//! them, registers the jobs they declare, and runs those jobs through the job
//! manager. The fixture is `extensions/test-extension/test_extension.wasm`,
//! built from that crate with `cargo build --release` (its `.cargo/config.toml`
//! targets wasm32-unknown-unknown).
//!
//! The plugin manager only exists in a build with the `wasm` feature, so this
//! whole file goes with it.
#![cfg(feature = "wasm")]

mod helpers;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use helpers::exif_jpeg;
use sd_core::{
	domain::{ContentKind, File, SdPath},
	filetype::{FileTypeRegistry, KindConflict, PreviewSpec},
	infra::{
		action::LibraryAction,
		api::SessionContext,
		job::{database::checkpoint, types::JobStatus},
		query::CoreQuery,
	},
	ops::{
		extensions::{
			ListExtensionsInput, ListExtensionsQuery, RunExtensionJobAction, RunExtensionJobInput,
		},
		sources::track::{TrackSourceAction, TrackSourceInput},
	},
	Core,
};
use sea_orm::EntityTrait;
use serde::Deserialize;
use tempfile::TempDir;
use uuid::Uuid;

/// The test extension's job state, as `extensions/test-extension/src/lib.rs`
/// declares it.
#[derive(Debug, Deserialize)]
struct CounterState {
	current: u32,
	processed: Vec<String>,
}

fn install_extension(data_dir: &Path, dir: &str, wasm: &str) {
	let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.unwrap()
		.join("extensions")
		.join(dir);
	let target = data_dir.join("extensions").join(dir);
	std::fs::create_dir_all(&target).unwrap();
	for file in ["manifest.json", wasm] {
		std::fs::copy(source.join(file), target.join(file)).unwrap();
	}
	// The viewer half ships beside the module when the extension has one.
	if source.join("ui_manifest.json").exists() {
		std::fs::copy(
			source.join("ui_manifest.json"),
			target.join("ui_manifest.json"),
		)
		.unwrap();
	}
	if let Ok(bundles) = std::fs::read_dir(source.join("ui")) {
		std::fs::create_dir_all(target.join("ui")).unwrap();
		for bundle in bundles.flatten() {
			std::fs::copy(bundle.path(), target.join("ui").join(bundle.file_name())).unwrap();
		}
	}
}

fn install_test_extension(data_dir: &Path) {
	install_extension(data_dir, "test-extension", "test_extension.wasm");
}

/// A second extension claiming the test extension's `.fake` kind, under a
/// directory name that sorts after it. It runs the test extension's module;
/// only its manifest differs.
fn install_second_kind_extension(data_dir: &Path) {
	let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("tests/fixtures/extensions/zz-second-kind/manifest.json");
	let wasm = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.unwrap()
		.join("extensions/test-extension/test_extension.wasm");
	let target = data_dir.join("extensions/zz-second-kind");
	std::fs::create_dir_all(&target).unwrap();
	std::fs::copy(manifest, target.join("manifest.json")).unwrap();
	std::fs::copy(wasm, target.join("test_extension.wasm")).unwrap();
}

/// Track `dir` in `library` and wait until every file in it has a content
/// identity. Returns the store and its file entries by name.
async fn track_and_identify(
	core: &Core,
	library: &Arc<sd_core::library::Library>,
	dir: PathBuf,
	expected: u64,
) -> (
	Arc<sd_core::ops::indexing::SourceStore>,
	Vec<sd_store::FsEntry>,
) {
	let tracked = TrackSourceAction::from_input(TrackSourceInput {
		path: dir,
		name: None,
		overrides: Default::default(),
	})
	.unwrap()
	.execute(library.clone(), core.context.clone())
	.await
	.unwrap();
	let store = core
		.context
		.volume_index()
		.store_for(&tracked.root)
		.await
		.expect("the tracked source has a store");
	let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
	loop {
		let contents = store.counts().await.map_or(0, |counts| counts.contents);
		if contents == expected {
			break;
		}
		assert!(
			tokio::time::Instant::now() < deadline,
			"{contents} of {expected} files were hashed"
		);
		tokio::time::sleep(Duration::from_millis(50)).await;
	}
	(store.clone(), store_files(&store).await)
}

/// The directory listing the explorer asks for, by file name.
async fn list_directory(
	core: &Core,
	library: &Arc<sd_core::library::Library>,
	dir: &Path,
) -> std::collections::HashMap<String, File> {
	use sd_core::infra::query::LibraryQuery;
	use sd_core::ops::files::query::directory_listing::{
		DirectoryListingInput, DirectoryListingQuery, DirectorySortBy,
	};
	let session =
		SessionContext::device_session(Uuid::now_v7(), sd_core::device::get_current_device_slug())
			.with_library(library.id());
	let listing = DirectoryListingQuery::from_input(DirectoryListingInput {
		path: SdPath::local(dir.to_path_buf()),
		folders_first: Some(false),
		limit: None,
		include_hidden: Some(false),
		sort_by: DirectorySortBy::Name,
		overlay: None,
	})
	.unwrap()
	.execute(core.context.clone(), session)
	.await
	.unwrap();
	listing
		.files
		.into_iter()
		.map(|f| {
			let name = match &f.extension {
				Some(ext) => format!("{}.{ext}", f.name),
				None => f.name.clone(),
			};
			(name, f)
		})
		.collect()
}

async fn store_files(store: &sd_core::ops::indexing::SourceStore) -> Vec<sd_store::FsEntry> {
	store.flush().await.unwrap();
	let mut files = sd_store::read::files_beneath(
		store.db().pool(),
		"",
		sd_store::read::Start::First,
		None,
		false,
		100,
	)
	.await
	.unwrap();
	files.sort_by(|a, b| a.name.cmp(&b.name));
	files
}

fn kind_of(files: &[sd_store::FsEntry], name: &str) -> (Option<i64>, Option<String>) {
	let entry = files
		.iter()
		.find(|f| f.name == name)
		.unwrap_or_else(|| panic!("{name} is in the store"));
	(entry.content_kind, entry.content_kind_name.clone())
}

/// Start an extension job and wait for it to end, whichever way.
async fn run_to_end(
	core: &Core,
	library: &Arc<sd_core::library::Library>,
	job: &str,
	state: serde_json::Value,
) -> sd_core::infra::job::JobInfo {
	let started = RunExtensionJobAction::from_input(RunExtensionJobInput {
		job: job.into(),
		state: Some(state),
	})
	.unwrap()
	.execute(library.clone(), core.context.clone())
	.await
	.unwrap();
	let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
	loop {
		let info = library
			.jobs()
			.get_job_info(started.job_id)
			.await
			.unwrap()
			.unwrap();
		if matches!(
			info.status,
			JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
		) {
			return info;
		}
		assert!(
			tokio::time::Instant::now() < deadline,
			"job {job} never ended"
		);
		tokio::time::sleep(Duration::from_millis(20)).await;
	}
}

/// A library with one tracked source holding a dozen JPEGs (distinct bytes,
/// JPEG magic first) and two text files, hashed through, so every record
/// has a content identity.
async fn fixture_library(
	core: &Core,
	root: &Path,
) -> (Arc<sd_core::library::Library>, Vec<sd_store::FsEntry>) {
	let library = core
		.libraries
		.create_library("Photos", None, core.context.clone())
		.await
		.unwrap();
	let source_dir = root.join("photos");
	std::fs::create_dir_all(source_dir.join("trip")).unwrap();
	for i in 0..12u8 {
		let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE0, i];
		bytes.extend(std::iter::repeat_n(i, 1000 + i as usize * 7));
		let dir = if i % 2 == 0 { "" } else { "trip/" };
		std::fs::write(source_dir.join(format!("{dir}IMG_{i:04}.JPG")), bytes).unwrap();
	}
	std::fs::write(source_dir.join("notes.txt"), b"not a photo").unwrap();
	std::fs::write(source_dir.join("trip/itinerary.txt"), b"day one").unwrap();

	let tracked = TrackSourceAction::from_input(TrackSourceInput {
		path: source_dir,
		name: None,
		overrides: Default::default(),
	})
	.unwrap()
	.execute(library.clone(), core.context.clone())
	.await
	.unwrap();
	let store = core
		.context
		.volume_index()
		.store_for(&tracked.root)
		.await
		.expect("the tracked source has a store");
	let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
	loop {
		let contents = store.counts().await.map_or(0, |counts| counts.contents);
		if contents == 14 {
			break;
		}
		assert!(
			tokio::time::Instant::now() < deadline,
			"{contents} of 14 files were hashed"
		);
		tokio::time::sleep(Duration::from_millis(50)).await;
	}
	let mut files = sd_store::read::files_beneath(
		store.db().pool(),
		"",
		sd_store::read::Start::First,
		None,
		false,
		100,
	)
	.await
	.unwrap();
	files.sort_by(|a, b| a.name.cmp(&b.name));
	assert_eq!(files.len(), 14);
	(library, files)
}

fn digest_sidecar(library: &sd_core::library::Library, content_uuid: Uuid) -> PathBuf {
	library
		.path()
		.join("sidecars")
		.join(sd_sidecar_path::relative_path(
			&content_uuid,
			&sd_sidecar_path::extension_kind_directory("test-extension", "digest"),
			"default",
			"json",
		))
}

async fn wait_for_status(
	library: &sd_core::library::Library,
	job_id: Uuid,
	wanted: JobStatus,
	timeout: Duration,
) -> sd_core::infra::job::JobInfo {
	let deadline = tokio::time::Instant::now() + timeout;
	loop {
		let info = library.jobs().get_job_info(job_id).await.unwrap();
		if let Some(info) = info {
			if info.status == wanted {
				return info;
			}
			assert!(
				!matches!(info.status, JobStatus::Failed | JobStatus::Cancelled),
				"job {job_id} ended as {:?} ({:?}) while waiting for {wanted:?}",
				info.status,
				info.error_message
			);
		}
		assert!(
			tokio::time::Instant::now() < deadline,
			"job {job_id} never reached {wanted:?}"
		);
		tokio::time::sleep(Duration::from_millis(20)).await;
	}
}

async fn counter_checkpoint(
	library: &sd_core::library::Library,
	job_id: Uuid,
) -> Option<CounterState> {
	let row = checkpoint::Entity::find_by_id(job_id.to_string())
		.one(library.jobs().database().conn())
		.await
		.unwrap()?;
	let state_json: String = rmp_serde::from_slice(&row.checkpoint_data).unwrap();
	Some(serde_json::from_str(&state_json).unwrap())
}

#[tokio::test(flavor = "multi_thread")]
async fn daemon_discovers_the_extension_and_runs_its_job() {
	guest_log();

	let temp_dir = TempDir::new().unwrap();
	install_test_extension(temp_dir.path());

	// Discovery happens at startup, before any library loads
	let core = Core::new(temp_dir.path().to_path_buf()).await.unwrap();
	let pm = core.plugin_manager.as_ref().expect("plugin manager");
	let loaded = pm.read().await.list_plugins().await;
	assert_eq!(loaded, vec!["test-extension".to_string()]);
	let manifest = pm
		.read()
		.await
		.get_manifest("test-extension")
		.await
		.unwrap();
	assert_eq!(manifest.name, "Test Extension");

	// plugin_init registered the counter job under <extension>:<name>
	let registry = pm.read().await.job_registry();
	let job = registry
		.get_job("test-extension:counter")
		.expect("counter job registered");
	assert_eq!(job.export_fn, "execute_test_counter");
	assert!(job.resumable);

	let library = core
		.libraries
		.create_library("Extensions", None, core.context.clone())
		.await
		.unwrap();

	let started = RunExtensionJobAction::from_input(RunExtensionJobInput {
		job: "test-extension:counter".into(),
		state: Some(serde_json::json!({ "current": 0, "target": 25, "processed": [] })),
	})
	.unwrap()
	.execute(library.clone(), core.context.clone())
	.await
	.unwrap();

	let info = wait_for_status(
		&library,
		started.job_id,
		JobStatus::Completed,
		Duration::from_secs(30),
	)
	.await;
	assert_eq!(info.name, "wasm_job");
	assert!(
		counter_checkpoint(&library, started.job_id).await.is_none(),
		"a completed job keeps no checkpoint"
	);

	// An unknown job is refused before anything is dispatched
	let refused = RunExtensionJobAction::from_input(RunExtensionJobInput {
		job: "test-extension:nope".into(),
		state: None,
	})
	.unwrap()
	.execute(library.clone(), core.context.clone())
	.await;
	assert!(refused.is_err());

	core.shutdown().await.unwrap();
}

/// A paused extension job survives a daemon restart and continues from the
/// state the guest last checkpointed.
/// Collects every log line a guest emits through `spacedrive_log`, and every
/// line the job context logs on the guest's behalf (task attempts, host
/// operations).
struct GuestLog(Arc<Mutex<String>>);

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for GuestLog {
	fn on_event(
		&self,
		event: &tracing::Event<'_>,
		_ctx: tracing_subscriber::layer::Context<'_, S>,
	) {
		let target = event.metadata().target();
		if !target.ends_with("host_functions")
			&& !target.ends_with("job::context")
			&& !target.ends_with("extension::ops")
		{
			return;
		}
		let mut line = String::new();
		event.record(
			&mut |field: &tracing::field::Field, value: &dyn std::fmt::Debug| {
				if field.name() == "message" {
					line = format!("{value:?}");
				}
			},
		);
		let mut log = self.0.lock().unwrap();
		log.push_str(&line);
		log.push('\n');
	}
}

/// The process-wide subscriber, installed once whichever test runs first,
/// with the buffer the guest log layer writes to.
fn guest_log() -> Arc<Mutex<String>> {
	use tracing_subscriber::layer::SubscriberExt;
	static LOG: OnceLock<Arc<Mutex<String>>> = OnceLock::new();
	LOG.get_or_init(|| {
		let log = Arc::new(Mutex::new(String::new()));
		tracing::subscriber::set_global_default(
			tracing_subscriber::registry()
				.with(tracing_subscriber::EnvFilter::new("info,wasmer=warn"))
				.with(tracing_subscriber::fmt::layer().with_test_writer())
				.with(GuestLog(log.clone())),
		)
		.expect("no other subscriber installed");
		log
	})
	.clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn extension_job_resumes_after_restart() {
	let guest_log = guest_log();

	let temp_dir = TempDir::new().unwrap();
	install_test_extension(temp_dir.path());
	let target = 5000;

	let (library_id, job_id, paused_at) = {
		let core = Core::new(temp_dir.path().to_path_buf()).await.unwrap();
		// A fresh data dir gets a default library
		let library = core.libraries.list().await.into_iter().next().unwrap();

		let started = RunExtensionJobAction::from_input(RunExtensionJobInput {
			job: "test-extension:counter".into(),
			state: Some(serde_json::json!({ "current": 0, "target": target, "processed": [] })),
		})
		.unwrap()
		.execute(library.clone(), core.context.clone())
		.await
		.unwrap();
		let job_id = started.job_id;

		// The guest checkpoints every ten items; pause once one is on disk
		let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
		while counter_checkpoint(&library, job_id).await.is_none() {
			assert!(
				tokio::time::Instant::now() < deadline,
				"the guest never checkpointed"
			);
			tokio::time::sleep(Duration::from_millis(5)).await;
		}
		library.jobs().pause_job(job_id.into()).await.unwrap();
		wait_for_status(&library, job_id, JobStatus::Paused, Duration::from_secs(30)).await;

		let paused = counter_checkpoint(&library, job_id)
			.await
			.expect("paused job keeps its checkpoint");
		assert!(
			paused.current > 0 && paused.current < target,
			"paused mid-way, got {paused:?}"
		);
		assert_eq!(paused.processed.len(), paused.current as usize);
		println!("paused at {}/{}", paused.current, target);

		core.shutdown().await.unwrap();
		(library.id(), job_id, paused.current)
	};

	// The next process loads the extension, then the library, which resumes
	// the paused job.
	guest_log.lock().unwrap().clear();
	let core = Core::new(temp_dir.path().to_path_buf()).await.unwrap();
	let library = core
		.libraries
		.list()
		.await
		.into_iter()
		.find(|library| library.id() == library_id)
		.expect("library reopened");

	wait_for_status(
		&library,
		job_id,
		JobStatus::Completed,
		Duration::from_secs(60),
	)
	.await;
	assert!(counter_checkpoint(&library, job_id).await.is_none());
	let guest_log = guest_log.lock().unwrap().clone();

	// The guest logs its starting state through spacedrive_log
	let started_from = guest_log
		.lines()
		.find_map(|line| {
			line.split("Starting counter (current: ")
				.nth(1)
				.and_then(|rest| rest.split(',').next())
				.and_then(|n| n.parse::<u32>().ok())
		})
		.expect("the resumed guest logged its starting state");
	assert_eq!(
		started_from, paused_at,
		"the resumed run should continue from the checkpoint"
	);

	core.shutdown().await.unwrap();
}

/// The test extension's `catalog` job reads records and their bytes through
/// the library's source stores, writes one sidecar per content and one model
/// per file into the extension's own store, and leaves digested files alone
/// on a second run.
#[tokio::test(flavor = "multi_thread")]
async fn extension_job_reads_records_and_writes_sidecars_and_models() {
	let guest_log = guest_log();
	let temp_dir = TempDir::new().unwrap();
	install_test_extension(temp_dir.path().join("core").as_path());
	let core = Core::new(temp_dir.path().join("core")).await.unwrap();
	let (library, files) = fixture_library(&core, temp_dir.path()).await;
	let jpegs: Vec<&sd_store::FsEntry> = files
		.iter()
		.filter(|f| f.extension.as_deref() == Some("JPG"))
		.collect();
	assert_eq!(jpegs.len(), 12);

	guest_log.lock().unwrap().clear();
	let info = run_to_end(
		&core,
		&library,
		"test-extension:catalog",
		serde_json::json!({ "extensions": ["jpg"] }),
	)
	.await;
	assert_eq!(
		info.status,
		JobStatus::Completed,
		"{:?}",
		info.error_message
	);

	// One digest sidecar per photo, holding what the guest read
	for jpeg in &jpegs {
		let content_uuid = jpeg.content_uuid.expect("hashed");
		let path = digest_sidecar(&library, content_uuid);
		let digest: serde_json::Value =
			serde_json::from_slice(&std::fs::read(&path).unwrap_or_else(|e| {
				panic!("digest sidecar for {}: {e} ({})", jpeg.name, path.display())
			}))
			.unwrap();
		assert_eq!(digest["size"], jpeg.size.unwrap());
		assert_eq!(
			digest["first_bytes"],
			serde_json::json!([0xFF, 0xD8, 0xFF, 0xE0])
		);
		assert_eq!(digest["record_name"], jpeg.name);
	}
	// The text files were outside the query and got no sidecar
	for other in files
		.iter()
		.filter(|f| f.extension.as_deref() == Some("txt"))
	{
		assert!(!digest_sidecar(&library, other.content_uuid.unwrap()).exists());
	}

	// One CatalogEntry model per photo in the extension's store, read back
	// with its field types: the update after create landed too
	let store = sd_store::SourceManager::open_file_read_only(
		&library.path().join("extensions/test-extension/data.db"),
	)
	.await
	.unwrap();
	let mut entries = store.facet_rows("CatalogEntry", None, 100).await.unwrap();
	entries.sort_by_key(|e| e["name"].as_str().unwrap().to_string());
	assert_eq!(entries.len(), 12);
	for (entry, jpeg) in entries.iter().zip(&jpegs) {
		assert_eq!(entry["name"], jpeg.name);
		assert_eq!(entry["record"], jpeg.uuid.to_string());
		assert_eq!(entry["size"], jpeg.size.unwrap());
		assert_eq!(entry["seen_twice"], true);
		assert_eq!(
			entry["first_bytes"],
			serde_json::json!([0xFF, 0xD8, 0xFF, 0xE0])
		);
		Uuid::parse_str(entry["id"].as_str().unwrap()).expect("a v4 uuid from host entropy");
		chrono::DateTime::parse_from_rfc3339(entry["catalogued_at"].as_str().unwrap())
			.expect("a timestamp from the host clock");
	}
	let log = guest_log.lock().unwrap().clone();
	assert!(
		log.contains("Catalog holds 12 entries, 12 digested this run, 0 skipped"),
		"{log}"
	);

	// A second run finds every photo digested and writes nothing new
	guest_log.lock().unwrap().clear();
	let info = run_to_end(
		&core,
		&library,
		"test-extension:catalog",
		serde_json::json!({ "extensions": ["jpg"] }),
	)
	.await;
	assert_eq!(
		info.status,
		JobStatus::Completed,
		"{:?}",
		info.error_message
	);
	let log = guest_log.lock().unwrap().clone();
	assert_eq!(log.matches("already digested").count(), 12, "{log}");
	assert!(
		log.contains("Catalog holds 12 entries, 0 digested this run, 12 skipped"),
		"{log}"
	);
	assert_eq!(
		store
			.facet_rows("CatalogEntry", None, 100)
			.await
			.unwrap()
			.len(),
		12
	);
	store.pool().close().await;

	core.shutdown().await.unwrap();
}

/// The photos extension's `analyze_photos` runs end to end over the fixture:
/// every photo is read through its record, the host answers face detection
/// with not_available, the job warns once and completes with no faces
/// sidecar, so a later run with a detector picks the photos up. A record
/// outside the manifest's read_records glob is refused.
#[tokio::test(flavor = "multi_thread")]
async fn photos_analyze_photos_runs_end_to_end_without_a_detector() {
	let guest_log = guest_log();
	let temp_dir = TempDir::new().unwrap();
	install_test_extension(temp_dir.path().join("core").as_path());
	install_extension(
		temp_dir.path().join("core").as_path(),
		"photos",
		"photos.wasm",
	);

	let core = Core::new(temp_dir.path().join("core")).await.unwrap();
	let pm = core.plugin_manager.as_ref().expect("plugin manager");
	let mut loaded = pm.read().await.list_plugins().await;
	loaded.sort();
	assert_eq!(loaded, vec!["com.spacedrive.photos", "test-extension"]);

	let registry = pm.read().await.job_registry();
	let mut jobs: Vec<String> = registry
		.list_jobs_for_extension("com.spacedrive.photos")
		.into_iter()
		.map(|job| job.job_name)
		.collect();
	jobs.sort();
	assert_eq!(
		jobs,
		[
			"analyze_photos",
			"analyze_scenes",
			"create_moments",
			"identify_places"
		]
	);
	let models = pm.read().await.model_registry();
	let schema = models.schema_for("com.spacedrive.photos");
	let mut declared: Vec<&String> = schema.models.keys().collect();
	declared.sort();
	assert_eq!(
		declared,
		[
			"Album",
			"Moment",
			"Person",
			"Photo",
			"Place",
			"custom_field"
		],
		"the built-in custom field model rides along"
	);

	let (library, files) = fixture_library(&core, temp_dir.path()).await;
	let photo_ids: Vec<Uuid> = files
		.iter()
		.filter(|f| f.extension.as_deref() == Some("JPG"))
		.map(|f| f.uuid)
		.collect();

	guest_log.lock().unwrap().clear();
	let info = run_to_end(
		&core,
		&library,
		"com.spacedrive.photos:analyze_photos",
		serde_json::json!({ "photo_ids": photo_ids }),
	)
	.await;
	assert_eq!(
		info.status,
		JobStatus::Completed,
		"{:?}",
		info.error_message
	);
	let log = guest_log.lock().unwrap().clone();
	assert!(
		log.contains("Job analyze_photos_batch completed successfully"),
		"{log}"
	);
	assert!(!log.contains("guest panic"), "{log}");
	// No detector, so no faces sidecar anywhere under photos' namespace
	let photos_sidecars = walk(&library.path().join("sidecars"))
		.into_iter()
		.filter(|p| p.to_string_lossy().contains("com.spacedrive.photos"))
		.count();
	assert_eq!(photos_sidecars, 0);

	// The host logged each task attempt, and the detector's absence was the
	// reason the detection task failed; the policy did not retry a refusal
	assert_eq!(
		log.matches("task detect_faces_in_photo attempt 1/3 started")
			.count(),
		12,
		"{log}"
	);
	assert!(
		log.contains("attempt 1 failed after")
			&& log.contains("no face_detection provider is installed"),
		"{log}"
	);
	assert!(!log.contains("attempt 2/3"), "{log}");
	assert!(
		log.contains("task cluster_faces_into_people attempt 1/2 started")
			&& log.contains("task generate_face_tags attempt 1/1 started"),
		"{log}"
	);

	// A text file is outside photos' read_records glob
	let text = files
		.iter()
		.find(|f| f.extension.as_deref() == Some("txt"))
		.unwrap();
	guest_log.lock().unwrap().clear();
	let info = run_to_end(
		&core,
		&library,
		"com.spacedrive.photos:analyze_photos",
		serde_json::json!({ "photo_ids": [text.uuid] }),
	)
	.await;
	assert_eq!(info.status, JobStatus::Failed);
	let log = guest_log.lock().unwrap().clone();
	assert!(log.contains("Permission denied"), "{log}");

	core.shutdown().await.unwrap();
}

/// The test extension's `tag` job tags every photo by name, creating the
/// tag, records a custom field and reads it back, removes the tag from one
/// photo and tags its content instead, is refused a namespace the manifest
/// does not grant, and queues a counter job that then runs. A manifest
/// without the grants is refused at the first tag.
#[tokio::test(flavor = "multi_thread")]
async fn extension_job_tags_records_sets_fields_and_dispatches() {
	let guest_log = guest_log();
	let temp_dir = TempDir::new().unwrap();
	let data_dir = temp_dir.path().join("core");
	install_test_extension(&data_dir);
	install_second_kind_extension(&data_dir);
	let core = Core::new(data_dir).await.unwrap();
	let (library, files) = fixture_library(&core, temp_dir.path()).await;
	let jpegs: Vec<&sd_store::FsEntry> = files
		.iter()
		.filter(|f| f.extension.as_deref() == Some("JPG"))
		.collect();

	guest_log.lock().unwrap().clear();
	let info = run_to_end(
		&core,
		&library,
		"test-extension:tag",
		serde_json::json!({ "extensions": ["jpg"] }),
	)
	.await;
	assert_eq!(
		info.status,
		JobStatus::Completed,
		"{:?}",
		info.error_message
	);
	let log = guest_log.lock().unwrap().clone();
	assert!(
		log.contains("refused:") && log.contains("has no write_custom_fields grant for other"),
		"{log}"
	);
	assert!(log.contains("an unset field reads as None"), "{log}");
	assert!(
		log.contains("removing an undefined tag answers None"),
		"{log}"
	);
	assert!(
		log.contains("11 records still carry Catalog/Digested"),
		"{log}"
	);

	// Tags landed in the source store: eleven by record, one by content,
	// under definitions the guest created by name.
	let store = core
		.context
		.volume_index()
		.store_for(&temp_dir.path().join("photos"))
		.await
		.unwrap();
	let uuids: Vec<Uuid> = jpegs.iter().map(|j| j.uuid).collect();
	let tags = store.db().tags_for_records(&uuids).await.unwrap();
	let paths_of = |uuid: &Uuid| -> Vec<String> {
		let mut paths: Vec<String> = tags
			.get(uuid)
			.map(|t| t.iter().map(|t| t.path.clone()).collect())
			.unwrap_or_default();
		paths.sort();
		paths
	};
	let digested = uuids
		.iter()
		.filter(|u| paths_of(u) == ["Catalog/Digested"])
		.count();
	let untagged = uuids
		.iter()
		.filter(|u| paths_of(u) == ["Catalog/Bytes"])
		.count();
	assert_eq!((digested, untagged), (11, 1), "{tags:?}");
	let definitions = store.db().tag_definitions().await.unwrap();
	let mut defined: Vec<&str> = definitions.iter().map(|d| d.path.as_str()).collect();
	defined.sort();
	assert_eq!(defined, ["Catalog/Bytes", "Catalog/Digested"]);

	// Custom fields are rows in the extension's own store
	let ext_store = sd_store::SourceManager::open_file_read_only(
		&library.path().join("extensions/test-extension/data.db"),
	)
	.await
	.unwrap();
	let fields = ext_store
		.facet_rows("custom_field", None, 100)
		.await
		.unwrap();
	assert_eq!(fields.len(), 12);
	for field in &fields {
		assert_eq!(field["namespace"], "test");
		assert_eq!(field["name"], "size");
		let record = Uuid::parse_str(field["record"].as_str().unwrap()).unwrap();
		let jpeg = jpegs.iter().find(|j| j.uuid == record).expect("a photo");
		assert_eq!(field["value"], jpeg.size.unwrap().to_string());
	}
	ext_store.pool().close().await;

	// The dispatched counter ran as its own job and finished
	let dispatched = log
		.lines()
		.find_map(|line| line.split("dispatched counter as ").nth(1))
		.and_then(|id| Uuid::parse_str(id.trim()).ok())
		.expect("the guest logged the dispatched job id");
	assert_ne!(dispatched, info.id);
	let counter = wait_for_status(
		&library,
		dispatched,
		JobStatus::Completed,
		Duration::from_secs(30),
	)
	.await;
	assert_eq!(counter.name, "wasm_job");
	assert!(log.contains("Completed processing 5 items"), "{log}");

	// The same module under a manifest without the grants is refused
	guest_log.lock().unwrap().clear();
	let info = run_to_end(
		&core,
		&library,
		"zz-second-kind:tag",
		serde_json::json!({ "extensions": ["jpg"] }),
	)
	.await;
	assert_eq!(info.status, JobStatus::Failed);
	let log = guest_log.lock().unwrap().clone();
	assert!(log.contains("has no write_tags grant"), "{log}");

	core.shutdown().await.unwrap();
}

/// Capture times and places of the moments fixture: three outings (a
/// morning in Tokyo, a morning in Kyoto a week later, a spring day with no
/// GPS) and one photo with no EXIF at all.
const MOMENT_PHOTOS: [(&str, Option<(f64, f64)>); 12] = [
	("2024:03:12 10:00:00", Some((35.6812, 139.7671))),
	("2024:03:12 10:20:00", Some((35.6815, 139.7660))),
	("2024:03:12 11:05:00", Some((35.6900, 139.7000))),
	("2024:03:12 12:30:00", Some((35.6903, 139.7004))),
	("2024:03:12 13:00:00", None),
	("2024:03:19 09:00:00", Some((35.0116, 135.7681))),
	("2024:03:19 09:15:00", Some((35.0118, 135.7679))),
	("2024:03:19 09:40:00", Some((34.9949, 135.7850))),
	("2024:03:19 10:10:00", Some((34.9950, 135.7849))),
	("2024:04:02 15:00:00", None),
	("2024:04:02 16:00:00", None),
	("2024:04:02 18:30:00", None),
];

/// A library with one tracked source holding the twelve EXIF-dated JPEGs
/// of [`MOMENT_PHOTOS`] plus one JPEG with no EXIF, hashed through.
async fn moments_library(
	core: &Core,
	root: &Path,
) -> (Arc<sd_core::library::Library>, Vec<sd_store::FsEntry>) {
	let library = core
		.libraries
		.create_library("Moments", None, core.context.clone())
		.await
		.unwrap();
	let source_dir = root.join("moments");
	std::fs::create_dir_all(&source_dir).unwrap();
	for (i, (date, gps)) in MOMENT_PHOTOS.iter().enumerate() {
		std::fs::write(
			source_dir.join(format!("IMG_{i:04}.jpg")),
			exif_jpeg(i as u8, Some(date), *gps),
		)
		.unwrap();
	}
	std::fs::write(source_dir.join("IMG_9999.jpg"), exif_jpeg(99, None, None)).unwrap();
	let (store, files) = track_and_identify(core, &library, source_dir, 13).await;
	assert_eq!(files.len(), 13);

	// The metadata pass runs behind identification and writes one facet
	// row per photo, the no-EXIF one included, keyed by content hash.
	let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
	loop {
		let rows: i64 =
			sqlx::query_scalar("SELECT COUNT(*) FROM facet_image WHERE content_hash IS NOT NULL")
				.fetch_one(store.db().pool())
				.await
				.unwrap();
		if rows == 13 {
			break;
		}
		assert!(
			tokio::time::Instant::now() < deadline,
			"{rows} of 13 photos have a facet row"
		);
		tokio::time::sleep(Duration::from_millis(50)).await;
	}
	(library, files)
}

/// The photos extension's `create_moments` runs end to end from EXIF alone:
/// capture times and GPS come through `records.exif`, answered from the
/// image facet the metadata pass wrote rather than by parsing the file; the
/// twelve dated photos fall into three moments as `Moment` models, each
/// photo is tagged `Moments/<title>` and carries its moment id as a custom
/// field, the undated photo belongs to none, and a second run groups
/// nothing twice. `identify_places` and `analyze_scenes` do their
/// non-inference parts and take the `not_available` path where they need a
/// model.
#[tokio::test(flavor = "multi_thread")]
async fn photos_create_moments_from_exif_without_inference() {
	let guest_log = guest_log();
	let temp_dir = TempDir::new().unwrap();
	let data_dir = temp_dir.path().join("core");
	install_extension(&data_dir, "photos", "photos.wasm");
	let core = Core::new(data_dir).await.unwrap();
	let (library, files) = moments_library(&core, temp_dir.path()).await;

	guest_log.lock().unwrap().clear();
	let info = run_to_end(
		&core,
		&library,
		"com.spacedrive.photos:create_moments",
		serde_json::json!({}),
	)
	.await;
	assert_eq!(
		info.status,
		JobStatus::Completed,
		"{:?}",
		info.error_message
	);
	let log = guest_log.lock().unwrap().clone();
	assert!(
		log.contains("Created 3 moments over 12 photos (1 undated, 0 already in a moment)"),
		"{log}"
	);
	assert!(
		!log.contains("parsing EXIF on demand"),
		"every records.exif answered from the facet: {log}"
	);

	let ext_store = sd_store::SourceManager::open_file_read_only(
		&library
			.path()
			.join("extensions/com.spacedrive.photos/data.db"),
	)
	.await
	.unwrap();
	let mut moments = ext_store.facet_rows("Moment", None, 100).await.unwrap();
	moments.sort_by_key(|m| m["start_date"].as_str().unwrap().to_string());
	let titles: Vec<&str> = moments
		.iter()
		.map(|m| m["title"].as_str().unwrap())
		.collect();
	assert_eq!(
		titles,
		["March 12, 2024", "March 19, 2024", "April 2, 2024"]
	);
	let counts: Vec<i64> = moments
		.iter()
		.map(|m| m["photo_count"].as_i64().unwrap())
		.collect();
	assert_eq!(counts, [5, 4, 3]);
	assert_eq!(moments[0]["start_date"], "2024-03-12T10:00:00Z");
	assert_eq!(moments[0]["end_date"], "2024-03-12T13:00:00Z");

	// Every dated photo is tagged with its moment and carries its id
	let store = core
		.context
		.volume_index()
		.store_for(&temp_dir.path().join("moments"))
		.await
		.unwrap();
	let uuids: Vec<Uuid> = files.iter().map(|f| f.uuid).collect();
	let tags = store.db().tags_for_records(&uuids).await.unwrap();
	let fields = ext_store
		.facet_rows("custom_field", None, 100)
		.await
		.unwrap();
	assert_eq!(fields.len(), 12);
	for (i, file) in files.iter().enumerate() {
		let applied: Vec<String> = tags
			.get(&file.uuid)
			.map(|t| t.iter().map(|t| t.path.clone()).collect())
			.unwrap_or_default();
		let field = fields
			.iter()
			.find(|f| f["record"] == file.uuid.to_string())
			.map(|f| f["value"].as_str().unwrap().trim_matches('"').to_string());
		if i == 12 {
			assert!(applied.is_empty(), "{}: {applied:?}", file.name);
			assert_eq!(field, None, "{}", file.name);
			continue;
		}
		let moment = &moments[match i {
			0..=4 => 0,
			5..=8 => 1,
			_ => 2,
		}];
		assert_eq!(
			applied,
			[format!("Moments/{}", moment["title"].as_str().unwrap())],
			"{}",
			file.name
		);
		assert_eq!(field.as_deref(), moment["id"].as_str(), "{}", file.name);
	}

	// A second run leaves the grouping alone
	guest_log.lock().unwrap().clear();
	let info = run_to_end(
		&core,
		&library,
		"com.spacedrive.photos:create_moments",
		serde_json::json!({}),
	)
	.await;
	assert_eq!(info.status, JobStatus::Completed);
	let log = guest_log.lock().unwrap().clone();
	assert!(
		log.contains("Created 0 moments over 0 photos (1 undated, 12 already in a moment)"),
		"{log}"
	);
	assert_eq!(
		ext_store
			.facet_rows("Moment", None, 100)
			.await
			.unwrap()
			.len(),
		3
	);

	// Places: clusters, Place models, fields and tags from GPS; the name
	// needs a language model the host does not have
	guest_log.lock().unwrap().clear();
	let info = run_to_end(
		&core,
		&library,
		"com.spacedrive.photos:identify_places",
		serde_json::json!({}),
	)
	.await;
	assert_eq!(
		info.status,
		JobStatus::Completed,
		"{:?}",
		info.error_message
	);
	let log = guest_log.lock().unwrap().clone();
	assert!(
		log.contains("Placed 8 photos (5 without a location, 4 new places, 0 already placed)"),
		"{log}"
	);
	assert!(
		log.contains("task reverse_geocode attempt 1 failed")
			&& log.contains("no llm provider is installed"),
		"{log}"
	);
	let places = ext_store.facet_rows("Place", None, 100).await.unwrap();
	assert_eq!(places.len(), 4);
	assert!(places.iter().all(|p| p["name"] == "Unknown Location"));
	let tags = store.db().tags_for_records(&uuids).await.unwrap();
	let placed = uuids
		.iter()
		.filter(|u| {
			tags.get(u)
				.is_some_and(|t| t.iter().any(|t| t.path == "Places/Unknown Location"))
		})
		.count();
	assert_eq!(placed, 8);

	// A second run places nothing again and the counts stay put
	guest_log.lock().unwrap().clear();
	let info = run_to_end(
		&core,
		&library,
		"com.spacedrive.photos:identify_places",
		serde_json::json!({}),
	)
	.await;
	assert_eq!(info.status, JobStatus::Completed);
	let log = guest_log.lock().unwrap().clone();
	assert!(
		log.contains("Placed 0 photos (5 without a location, 0 new places, 8 already placed)"),
		"{log}"
	);
	let mut counts: Vec<i64> = ext_store
		.facet_rows("Place", None, 100)
		.await
		.unwrap()
		.iter()
		.map(|p| p["photo_count"].as_i64().unwrap())
		.collect();
	counts.sort();
	assert_eq!(counts, [2, 2, 2, 2]);

	// Scenes: every photo skipped after one warning, no sidecar written
	guest_log.lock().unwrap().clear();
	let info = run_to_end(
		&core,
		&library,
		"com.spacedrive.photos:analyze_scenes",
		serde_json::json!({}),
	)
	.await;
	assert_eq!(
		info.status,
		JobStatus::Completed,
		"{:?}",
		info.error_message
	);
	let log = guest_log.lock().unwrap().clone();
	assert!(log.contains("Scenes: 0 classified, 13 skipped"), "{log}");
	assert!(
		log.contains("no scene_classification provider is installed"),
		"{log}"
	);
	ext_store.pool().close().await;

	core.shutdown().await.unwrap();
}

fn walk(dir: &Path) -> Vec<PathBuf> {
	let mut out = Vec::new();
	let Ok(entries) = std::fs::read_dir(dir) else {
		return out;
	};
	for entry in entries.flatten() {
		let path = entry.path();
		if path.is_dir() {
			out.extend(walk(&path));
		} else {
			out.push(path);
		}
	}
	out
}

/// The test extension declares a `fake` kind over `.fake` with magic
/// bytes, and a second fixture loaded after it contests the extension. The
/// content identity phase stores the kind and its name for every file,
/// built-in kinds included; the row keeps its name after the extension
/// unloads while the registry forgets it; rows identified before an
/// extension existed are named when it loads and when a store opens after
/// a restart.
#[tokio::test(flavor = "multi_thread")]
async fn extension_kinds_are_stored_and_survive_unload() {
	guest_log();
	let temp_dir = TempDir::new().unwrap();
	let data_dir = temp_dir.path().join("core");
	install_test_extension(&data_dir);
	install_second_kind_extension(&data_dir);
	let core = Core::new(data_dir.clone()).await.unwrap();
	let pm = core
		.plugin_manager
		.as_ref()
		.expect("plugin manager")
		.clone();

	// Both loaded, in directory order; the list reports the kinds and the
	// one contested claim.
	let session =
		SessionContext::device_session(Uuid::now_v7(), sd_core::device::get_current_device_slug());
	let list = ListExtensionsQuery::from_input(ListExtensionsInput {})
		.unwrap()
		.execute(core.context.clone(), session.clone())
		.await
		.unwrap();
	let ids: Vec<&str> = list.extensions.iter().map(|e| e.id.as_str()).collect();
	assert_eq!(ids, ["test-extension", "zz-second-kind"]);
	let fake = &list.extensions[0].kinds[0];
	assert_eq!(fake.id, "test-extension:fake");
	assert_eq!(fake.display_name, "Fake file");
	assert_eq!(fake.parent, ContentKind::Text);
	assert_eq!(
		fake.preview,
		Some(PreviewSpec::Viewer("fake_viewer".into()))
	);
	assert_eq!(
		list.extensions[0]
			.viewers
			.iter()
			.map(|v| (v.id.as_str(), v.bundle.as_str()))
			.collect::<Vec<_>>(),
		[("fake_viewer", "ui/fake-viewer.js")],
		"the list carries the bundle the client mounts for the viewer"
	);
	assert!(list.extensions[1].viewers.is_empty());
	assert_eq!(
		list.conflicts,
		vec![KindConflict {
			extension: "fake".into(),
			kind: "zz-second-kind:other".into(),
			claimed_by: "test-extension:fake".into(),
		}]
	);
	assert_eq!(
		FileTypeRegistry::current()
			.type_by_extension(Path::new("x.fake2"))
			.unwrap()
			.id,
		"zz-second-kind:other",
		"the loser keeps its uncontested extension"
	);

	// Index a folder: the identity phase writes kind and kind_name.
	let library = core
		.libraries
		.create_library("Kinds", None, core.context.clone())
		.await
		.unwrap();
	let first = temp_dir.path().join("first");
	std::fs::create_dir_all(&first).unwrap();
	std::fs::write(first.join("a.fake"), b"FAKE".repeat(300)).unwrap();
	std::fs::write(first.join("b.fake"), b"OTHR".repeat(300)).unwrap();
	std::fs::write(first.join("c.fake"), vec![0u8; 1200]).unwrap();
	let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0];
	jpeg.extend(std::iter::repeat_n(7u8, 1200));
	std::fs::write(first.join("d.jpg"), jpeg).unwrap();
	std::fs::write(first.join("e.txt"), b"plain text".repeat(100)).unwrap();
	let (first_store, files) = track_and_identify(&core, &library, first.clone(), 5).await;

	let text = ContentKind::Text as i64;
	assert_eq!(
		kind_of(&files, "a.fake"),
		(Some(text), Some("test-extension:fake".into()))
	);
	assert_eq!(
		kind_of(&files, "b.fake"),
		(Some(text), Some("zz-second-kind:other".into())),
		"a lone magic match on the contested kind wins the file"
	);
	assert_eq!(
		kind_of(&files, "c.fake"),
		(Some(text), Some("test-extension:fake".into())),
		"no magic match keeps the extension holder"
	);
	assert_eq!(
		kind_of(&files, "d.jpg"),
		(Some(ContentKind::Image as i64), None),
		"a built-in kind is stored too"
	);
	assert_eq!(kind_of(&files, "e.txt"), (Some(text), None));

	let a = files.iter().find(|f| f.name == "a.fake").unwrap();
	let file = File::from_store_entry(a, SdPath::local(first.join("a.fake")));
	assert_eq!(file.content_kind, ContentKind::Text);
	assert_eq!(
		file.content_kind_name.as_deref(),
		Some("test-extension:fake")
	);

	// The browse listing answers from the arena, which knows only the
	// built-in kind; the stored name is laid over it so the client sees it.
	let listing = list_directory(&core, &library, &first).await;
	assert_eq!(
		listing
			.get("a.fake")
			.map(|f| (f.content_kind, f.content_kind_name.as_deref())),
		Some((ContentKind::Text, Some("test-extension:fake")))
	);
	assert_eq!(
		listing
			.get("b.fake")
			.map(|f| f.content_kind_name.as_deref()),
		Some(Some("zz-second-kind:other"))
	);
	assert_eq!(
		listing
			.get("d.jpg")
			.map(|f| (f.content_kind, f.content_kind_name.as_deref())),
		Some((ContentKind::Image, None))
	);

	// Unload the holder: the registry moves on, the rows do not.
	pm.write()
		.await
		.unload_plugin("test-extension")
		.await
		.unwrap();
	assert_eq!(
		FileTypeRegistry::current()
			.type_by_extension(Path::new("x.fake"))
			.unwrap()
			.id,
		"zz-second-kind:other",
		"the contested claim holds the extension once the winner is gone"
	);
	let files = store_files(&first_store).await;
	assert_eq!(
		kind_of(&files, "a.fake"),
		(Some(text), Some("test-extension:fake".into()))
	);
	let a = files.iter().find(|f| f.name == "a.fake").unwrap();
	let file = File::from_store_entry(a, SdPath::local(first.join("a.fake")));
	assert_eq!(file.content_kind, ContentKind::Text, "the stored parent");
	assert_eq!(
		file.content_kind_name.as_deref(),
		Some("test-extension:fake")
	);

	pm.write()
		.await
		.unload_plugin("zz-second-kind")
		.await
		.unwrap();
	assert!(
		FileTypeRegistry::current()
			.type_by_extension(Path::new("x.fake"))
			.is_none(),
		"no extension loaded, the built-in registry is back"
	);
	assert!(FileTypeRegistry::current().conflicts().is_empty());

	// Rows identified with no extension loaded have no kind; the load that
	// follows names them by extension without reading the bytes.
	let second = temp_dir.path().join("second");
	std::fs::create_dir_all(&second).unwrap();
	std::fs::write(second.join("f.FAKE"), vec![1u8; 1200]).unwrap();
	std::fs::write(second.join("g.txt"), b"plain".repeat(300)).unwrap();
	let (second_store, files) = track_and_identify(&core, &library, second.clone(), 2).await;
	assert_eq!(kind_of(&files, "f.FAKE"), (None, None));
	assert_eq!(kind_of(&files, "g.txt"), (Some(text), None));

	pm.write()
		.await
		.load_plugin("test-extension")
		.await
		.unwrap();
	let files = store_files(&second_store).await;
	assert_eq!(
		kind_of(&files, "f.FAKE"),
		(Some(text), Some("test-extension:fake".into())),
		"a load with kinds names the rows open stores hold, whatever the case of the extension"
	);
	assert_eq!(kind_of(&files, "g.txt"), (Some(text), None));

	// A store opened after a restart is named by the kinds loaded at startup.
	pm.write()
		.await
		.unload_plugin("test-extension")
		.await
		.unwrap();
	let third = temp_dir.path().join("third");
	std::fs::create_dir_all(&third).unwrap();
	std::fs::write(third.join("h.fake"), vec![2u8; 1200]).unwrap();
	let (_, files) = track_and_identify(&core, &library, third.clone(), 1).await;
	assert_eq!(kind_of(&files, "h.fake"), (None, None));
	core.shutdown().await.unwrap();

	let core = Core::new(data_dir).await.unwrap();
	let store = core
		.context
		.volume_index()
		.store_for(&third)
		.await
		.expect("the third source reopens");
	let files = store_files(&store).await;
	assert_eq!(
		kind_of(&files, "h.fake"),
		(Some(text), Some("test-extension:fake".into()))
	);
	core.shutdown().await.unwrap();
}

/// The viewer half of a preview. The test extension's `fake` kind previews
/// through `fake_viewer`, whose bundle `ui_manifest.json` names; the HTTP
/// route resolves that bundle and nothing else in the directory, and a
/// deleted bundle leaves a path whose open fails, which is what the client
/// turns into the parent renderer plus one warning. Photos declares `raw`
/// over the image renderer and a `photo_viewer` bundle, and loads beside the
/// built-in table with no conflict. A kind naming a viewer the UI manifest
/// does not declare refuses to load.
#[tokio::test(flavor = "multi_thread")]
async fn viewer_bundles_resolve_and_a_kind_needs_a_declared_viewer() {
	guest_log();
	let temp_dir = TempDir::new().unwrap();
	let data_dir = temp_dir.path().join("core");
	install_test_extension(&data_dir);
	install_extension(&data_dir, "photos", "photos.wasm");
	let core = Core::new(data_dir.clone()).await.unwrap();
	let session =
		SessionContext::device_session(Uuid::now_v7(), sd_core::device::get_current_device_slug());
	let list = ListExtensionsQuery::from_input(ListExtensionsInput {})
		.unwrap()
		.execute(core.context.clone(), session)
		.await
		.unwrap();

	let photos = &list.extensions[0];
	assert_eq!(photos.id, "com.spacedrive.photos");
	assert_eq!(photos.kinds.len(), 1);
	assert_eq!(photos.kinds[0].id, "com.spacedrive.photos:raw");
	assert_eq!(photos.kinds[0].parent, ContentKind::Image);
	assert_eq!(
		photos.kinds[0].preview,
		Some(PreviewSpec::Renderer("image".into()))
	);
	assert_eq!(photos.viewers.len(), 1);
	assert_eq!(photos.viewers[0].id, "photo_viewer");
	assert_eq!(photos.viewers[0].bundle, "ui/photo_viewer.js");
	assert!(list.conflicts.is_empty(), "{:?}", list.conflicts);
	assert_eq!(
		FileTypeRegistry::current()
			.type_by_extension(Path::new("IMG_0001.dng"))
			.unwrap()
			.id,
		"com.spacedrive.photos:raw"
	);

	let bundle = sd_extension_ui::resolve_bundle(&data_dir, "test-extension", "ui/fake-viewer.js")
		.await
		.expect("the declared bundle resolves");
	assert_eq!(
		bundle,
		data_dir.join("extensions/test-extension/ui/fake-viewer.js")
	);
	let module = std::fs::read_to_string(&bundle).unwrap();
	assert!(module.contains("export function mount(el, ctx)"));
	for path in [
		"manifest.json",
		"test_extension.wasm",
		"ui/../manifest.json",
	] {
		assert_eq!(
			sd_extension_ui::resolve_bundle(&data_dir, "test-extension", path).await,
			None,
			"{path} is not a declared bundle"
		);
	}
	assert!(sd_extension_ui::resolve_bundle(
		&data_dir,
		"com.spacedrive.photos",
		"ui/photo_viewer.js"
	)
	.await
	.is_some_and(|p| p.is_file()));

	std::fs::remove_file(&bundle).unwrap();
	let gone = sd_extension_ui::resolve_bundle(&data_dir, "test-extension", "ui/fake-viewer.js")
		.await
		.expect("the declaration still resolves");
	assert!(
		tokio::fs::File::open(&gone).await.is_err(),
		"the route's open fails, so the client gets 404 and falls back"
	);

	let broken = data_dir.join("extensions/broken-viewer");
	std::fs::create_dir_all(&broken).unwrap();
	std::fs::copy(
		data_dir.join("extensions/test-extension/test_extension.wasm"),
		broken.join("test_extension.wasm"),
	)
	.unwrap();
	std::fs::write(
		broken.join("manifest.json"),
		r#"{"id":"broken-viewer","name":"x","version":"1","wasm_file":"test_extension.wasm",
		"kinds":[{"name":"k","parent":"text","extensions":["brk"],"preview":{"viewer":"nope"}}]}"#,
	)
	.unwrap();
	let pm = core
		.plugin_manager
		.as_ref()
		.expect("plugin manager")
		.clone();
	let err = pm
		.write()
		.await
		.load_plugin("broken-viewer")
		.await
		.unwrap_err()
		.to_string();
	assert!(err.contains("does not declare"), "{err}");
	assert!(FileTypeRegistry::current()
		.type_by_extension(Path::new("x.brk"))
		.is_none());
}
