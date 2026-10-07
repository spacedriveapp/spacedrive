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

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use sd_core::{
	infra::{
		action::LibraryAction,
		job::{database::checkpoint, types::JobStatus},
	},
	ops::{
		extensions::{RunExtensionJobAction, RunExtensionJobInput},
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
}

fn install_test_extension(data_dir: &Path) {
	install_extension(data_dir, "test-extension", "test_extension.wasm");
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
		unfiltered: false,
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
		if !target.ends_with("host_functions") && !target.ends_with("job::context") {
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
	let schema = models
		.schema_for("com.spacedrive.photos")
		.expect("photos declared its models");
	let mut declared: Vec<&String> = schema.models.keys().collect();
	declared.sort();
	assert_eq!(declared, ["Album", "Moment", "Person", "Photo", "Place"]);

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
