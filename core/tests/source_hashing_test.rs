//! Tracking a source walks it, then hashes what the walk recorded.

use sd_core::{
	infra::action::LibraryAction,
	ops::sources::track::{TrackSourceAction, TrackSourceInput},
	Core,
};
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::{sleep, Instant};

/// The hashing pass reads the records the walk writes, so it has to start
/// after the walk. Nothing else hashes a source whose files never change, so
/// a pass that ran early left every file on it without a content identity.
#[tokio::test]
async fn tracking_hashes_every_file_the_walk_records(
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
	let temp_dir = TempDir::new()?;
	let core = Core::new(temp_dir.path().join("core")).await?;
	let library = core
		.libraries
		.create_library("Hashing", None, core.context.clone())
		.await?;

	let source_dir = temp_dir.path().join("source");
	let mut files = 0;
	for dir in 0..20 {
		let subdir = source_dir.join(format!("dir_{dir}"));
		tokio::fs::create_dir_all(&subdir).await?;
		for file in 0..20 {
			tokio::fs::write(
				subdir.join(format!("file_{file}.txt")),
				format!("{dir} {file}"),
			)
			.await?;
			files += 1;
		}
	}

	let tracked = TrackSourceAction::from_input(TrackSourceInput {
		path: source_dir,
		name: None,
		unfiltered: false,
	})?
	.execute(library.clone(), core.context.clone())
	.await?;
	let store = core
		.context
		.volume_index()
		.store_for(&tracked.root)
		.await
		.ok_or("the tracked source has no store")?;

	// Every file holds different bytes, so a fully hashed source holds one
	// content per file.
	let deadline = Instant::now() + Duration::from_secs(30);
	loop {
		let contents = store.counts().await.map_or(0, |counts| counts.contents);
		if contents == files {
			break;
		}
		assert!(
			Instant::now() < deadline,
			"{contents} of {files} files were hashed"
		);
		sleep(Duration::from_millis(50)).await;
	}
	assert_eq!(store.files_needing_content_count().await?, 0);

	core.shutdown().await?;
	Ok(())
}
