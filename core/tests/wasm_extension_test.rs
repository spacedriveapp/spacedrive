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
	ops::extensions::{RunExtensionJobAction, RunExtensionJobInput},
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

fn install_test_extension(data_dir: &Path) {
	let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.unwrap()
		.join("extensions/test-extension");
	let target = data_dir.join("extensions/test-extension");
	std::fs::create_dir_all(&target).unwrap();
	for file in ["manifest.json", "test_extension.wasm"] {
		std::fs::copy(source.join(file), target.join(file)).unwrap();
	}
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
/// Collects every log line a guest emits through `spacedrive_log`.
struct GuestLog(Arc<Mutex<String>>);

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for GuestLog {
	fn on_event(
		&self,
		event: &tracing::Event<'_>,
		_ctx: tracing_subscriber::layer::Context<'_, S>,
	) {
		if !event.metadata().target().ends_with("host_functions") {
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

/// The photos extension (`extensions/photos`, fixture `photos.wasm`) loads
/// beside the test extension and registers its jobs. Its jobs stop at the
/// first SDK call with no host function behind it; the guest's panic reaches
/// the host log and the job fails instead of hanging.
#[tokio::test(flavor = "multi_thread")]
async fn photos_extension_loads_and_stops_at_the_first_missing_host_function() {
	let guest_log = guest_log();
	let temp_dir = TempDir::new().unwrap();
	install_test_extension(temp_dir.path());
	let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.unwrap()
		.join("extensions/photos");
	let target = temp_dir.path().join("extensions/photos");
	std::fs::create_dir_all(&target).unwrap();
	for file in ["manifest.json", "photos.wasm"] {
		std::fs::copy(source.join(file), target.join(file)).unwrap();
	}

	let core = Core::new(temp_dir.path().to_path_buf()).await.unwrap();
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

	// Twice: the second run lands on a fresh instance, since the first one
	// aborted mid-call.
	let library = core.libraries.list().await.into_iter().next().unwrap();
	for _ in 0..2 {
		guest_log.lock().unwrap().clear();
		let started = RunExtensionJobAction::from_input(RunExtensionJobInput {
			job: "com.spacedrive.photos:analyze_photos".into(),
			state: None,
		})
		.unwrap()
		.execute(library.clone(), core.context.clone())
		.await
		.unwrap();

		let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
		let info = loop {
			let info = library
				.jobs()
				.get_job_info(started.job_id)
				.await
				.unwrap()
				.unwrap();
			if info.status == JobStatus::Failed {
				break info;
			}
			assert_ne!(
				info.status,
				JobStatus::Completed,
				"nothing backs this job yet"
			);
			assert!(
				tokio::time::Instant::now() < deadline,
				"the job never ended"
			);
			tokio::time::sleep(Duration::from_millis(20)).await;
		};
		assert!(
			info.error_message
				.as_deref()
				.unwrap_or("")
				.contains("WASM trap"),
			"{:?}",
			info.error_message
		);
		let guest_log = guest_log.lock().unwrap().clone();
		assert!(
			guest_log.contains("guest panic:"),
			"the guest's panic should reach the host log:\n{guest_log}"
		);
	}
	assert_eq!(
		registry
			.list_jobs_for_extension("com.spacedrive.photos")
			.len(),
		4,
		"the reloaded extension registers its jobs again"
	);

	core.shutdown().await.unwrap();
}
