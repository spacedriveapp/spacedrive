//! Acceptance for the video half of the media metadata pass
//! (`docs/core/indexing.mdx`, "Photo and video metadata"): tracking a folder
//! of real clips leaves one `facet_video` row per video with the duration,
//! frame size and codec its container header names, read once per content
//! hash, and a listing of the folder carries them without any footage work.

mod helpers;

use std::{path::Path, sync::Arc, time::Duration};

use helpers::*;
use sd_core::{
	infra::action::LibraryAction,
	library::AddOverrides,
	ops::{
		indexing::SourceStore,
		sources::track::action::{TrackSourceAction, TrackSourceInput},
	},
};

type VideoRow = (
	String,
	Option<String>,
	Option<f64>,
	Option<i64>,
	Option<i64>,
	Option<String>,
);

async fn video_rows(store: &SourceStore) -> Vec<VideoRow> {
	sqlx::query_as(
		"SELECT r.title, v.content_hash, v.duration_seconds, v.width, v.height, v.codec \
		 FROM facet_video v JOIN record r ON r.uuid = v.record_uuid ORDER BY r.title",
	)
	.fetch_all(store.db().pool())
	.await
	.unwrap()
}

async fn wait_for_videos(store: &SourceStore, expected: usize) -> Vec<VideoRow> {
	let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
	loop {
		let rows = video_rows(store).await;
		if rows.len() == expected && rows.iter().all(|row| row.1.is_some()) {
			return rows;
		}
		assert!(
			tokio::time::Instant::now() < deadline,
			"{} of {expected} video rows written: {rows:?}",
			rows.len()
		);
		tokio::time::sleep(Duration::from_millis(50)).await;
	}
}

async fn track(harness: &IndexingHarness, path: &Path) -> Arc<SourceStore> {
	let output = TrackSourceAction::from_input(TrackSourceInput {
		path: path.to_path_buf(),
		name: None,
		overrides: AddOverrides::default(),
	})
	.map_err(anyhow::Error::msg)
	.unwrap()
	.execute(harness.library.clone(), harness.core.context.clone())
	.await
	.unwrap();
	if let Some(job) = output.job_id {
		if let Some(walk) = harness
			.library
			.jobs()
			.get_job(sd_core::infra::job::types::JobId(job))
			.await
		{
			walk.wait().await.unwrap();
		}
	}
	harness
		.core
		.context
		.volume_index()
		.store_for(&output.root)
		.await
		.expect("the tracked source has a store")
}

fn clip(name: &str) -> std::path::PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("../crates/media-metadata/tests/fixtures")
		.join(name)
}

#[tokio::test(flavor = "multi_thread")]
async fn tracking_clips_fills_the_video_facet_from_their_headers() {
	let harness = IndexingHarnessBuilder::new("video_header_acceptance")
		.disable_watcher()
		.build()
		.await
		.unwrap();
	let footage = harness.create_test_dir("footage").await.unwrap();
	std::fs::copy(clip("moov-at-end.mp4"), footage.path().join("walk.mp4")).unwrap();
	std::fs::copy(
		clip("moov-at-end.mp4"),
		footage.path().join("walk-copy.mp4"),
	)
	.unwrap();
	std::fs::copy(clip("clip.webm"), footage.path().join("screen.webm")).unwrap();
	footage
		.write_file("notes.txt", "not a video")
		.await
		.unwrap();

	let store = track(&harness, footage.path()).await;

	let rows = wait_for_videos(&store, 3).await;
	let names: Vec<&str> = rows.iter().map(|row| row.0.as_str()).collect();
	assert_eq!(names, ["screen.webm", "walk-copy.mp4", "walk.mp4"]);
	let walk = &rows[2];
	assert!((walk.2.unwrap() - 2.0).abs() < 0.05, "{walk:?}");
	assert_eq!(
		(walk.3, walk.4, walk.5.as_deref()),
		(Some(320), Some(240), Some("h264"))
	);
	assert_eq!(
		rows[1].1, walk.1,
		"both copies carry the row of their shared bytes"
	);
	let screen = &rows[0];
	assert!((screen.2.unwrap() - 1.0).abs() < 0.05, "{screen:?}");
	assert_eq!(
		(screen.3, screen.4, screen.5.as_deref()),
		(Some(256), Some(144), Some("vp9"))
	);

	// A listing reads the facet by record, with no footage sidecar.
	let pool = store.db().pool();
	let uuid = sd_store::read::resolve_path(pool, "walk.mp4")
		.await
		.unwrap()
		.expect("walk.mp4 is recorded");
	let facets = sd_store::video_facets_for_records(pool, &[uuid])
		.await
		.unwrap();
	let (_, video) = facets.first().expect("the listing reads the video facet");
	assert!((video.duration_seconds.unwrap() - 2.0).abs() < 0.05);
	assert_eq!(video.codec.as_deref(), Some("h264"));
}
