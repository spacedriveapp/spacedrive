//! How long the startup reconcile takes on a large source whose disk holds
//! what the rules keep out of the map: system files, dev directories, and
//! files a `.gitignore` names. Each such entry is new to the map on every
//! pass, so the pass decides again whether to keep it.
//!
//! Run with `cargo test -p sd-core --release --test reconcile_scale_test --
//! --ignored --nocapture`.

mod helpers;

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use helpers::TestConfigBuilder;
use sd_core::{
	infra::action::LibraryAction,
	ops::sources::track::action::{TrackSourceAction, TrackSourceInput},
	Core,
};

const DIRECTORIES: usize = 2_000;
const FILES_PER_DIRECTORY: usize = 50;
const IGNORED_PER_DIRECTORY: usize = 5;

fn generate(root: &Path) {
	std::fs::create_dir(root.join(".git")).expect("git dir");
	std::fs::write(root.join(".gitignore"), "*.log\n").expect("gitignore");
	for directory in 0..DIRECTORIES {
		let dir = root.join(format!("d{:02}/e{directory:04}", directory % 40));
		std::fs::create_dir_all(dir.join("node_modules/pkg")).expect("mkdir");
		std::fs::write(dir.join("node_modules/pkg/index.js"), "x").expect("write");
		std::fs::write(dir.join(".DS_Store"), "x").expect("write");
		for file in 0..FILES_PER_DIRECTORY {
			std::fs::write(dir.join(format!("f{file:03}.txt")), "x").expect("write");
		}
		for file in 0..IGNORED_PER_DIRECTORY {
			std::fs::write(dir.join(format!("g{file:03}.log")), "x").expect("write");
		}
	}
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "measurement, not regression; run with --ignored --nocapture"]
async fn reconcile_of_a_large_filtered_tree() {
	let _ = tracing_subscriber::fmt()
		.with_env_filter(tracing_subscriber::EnvFilter::new(
			"reconcile_scale_test=info",
		))
		.try_init();
	let root_dir = tempfile::tempdir().expect("root dir");
	let started = Instant::now();
	generate(root_dir.path());
	let generated = started.elapsed();

	let data_dir = tempfile::tempdir().expect("data dir");
	let config = TestConfigBuilder::new(data_dir.path().to_path_buf())
		.build()
		.expect("config");
	config.save().expect("save config");
	let core = Arc::new(
		Core::new(data_dir.path().to_path_buf())
			.await
			.expect("core"),
	);
	let library = core
		.libraries
		.list()
		.await
		.into_iter()
		.next()
		.expect("default library");
	tracing::info!(?generated, "generated the tree");

	let started = Instant::now();
	let output = TrackSourceAction::from_input(TrackSourceInput {
		path: root_dir.path().to_path_buf(),
		name: None,
		overrides: Default::default(),
	})
	.expect("input")
	.execute(library.clone(), core.context.clone())
	.await
	.expect("track");
	if let Some(walk) = library
		.jobs()
		.get_job(sd_core::infra::job::prelude::JobId(
			output.job_id.expect("tracking dispatched a walk"),
		))
		.await
	{
		walk.wait().await.expect("walk");
	}
	tracing::info!(took = ?started.elapsed(), "walked");
	tokio::time::sleep(Duration::from_secs(2)).await;

	let started = Instant::now();
	let report = sd_core::ops::indexing::reconcile::reconcile_source(
		&core.context,
		Some(library.id()),
		&output.root,
		false,
	)
	.await
	.expect("the reconcile ran");
	tracing::info!(took = ?started.elapsed(), ?report, "reconciled");
	assert_eq!(report.added, 0, "nothing the rules keep out arrives");

	drop(library);
	core.shutdown().await.expect("shutdown");
}
