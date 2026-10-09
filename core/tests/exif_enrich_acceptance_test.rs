//! Acceptance for the EXIF metadata pass (`docs/core/indexing.mdx`, "Photo
//! metadata"): tracking a folder of photos leaves one `facet_image` row per
//! image, written from the file's EXIF once per content hash, rewritten when
//! the bytes change, and never written for a source added without content
//! identification.
//!
//! One `Core` over temporary directories, with the photos hand-built from an
//! EXIF segment so the tree carries no real images.

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

type FacetRow = (Option<String>, Option<String>, Option<f64>, Option<f64>);

/// `facet_image` by file name: content hash, capture time, latitude and
/// longitude.
async fn facet_rows(store: &SourceStore) -> Vec<(String, FacetRow)> {
	sqlx::query_as::<
		_,
		(
			String,
			Option<String>,
			Option<String>,
			Option<f64>,
			Option<f64>,
		),
	>(
		"SELECT r.title, i.content_hash, i.date_taken, i.latitude, i.longitude \
		 FROM facet_image i JOIN record r ON r.uuid = i.record_uuid ORDER BY r.title",
	)
	.fetch_all(store.db().pool())
	.await
	.unwrap()
	.into_iter()
	.map(|(title, hash, date, lat, lon)| (title, (hash, date, lat, lon)))
	.collect()
}

/// Wait until the pass has written `expected` facet rows, or fail.
async fn wait_for_facets(store: &SourceStore, expected: usize) -> Vec<(String, FacetRow)> {
	let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
	loop {
		let rows = facet_rows(store).await;
		if rows.len() == expected && rows.iter().all(|(_, (hash, ..))| hash.is_some()) {
			return rows;
		}
		assert!(
			tokio::time::Instant::now() < deadline,
			"{} of {expected} facet rows written: {rows:?}",
			rows.len()
		);
		tokio::time::sleep(Duration::from_millis(50)).await;
	}
}

async fn track(
	harness: &IndexingHarness,
	path: &Path,
	overrides: AddOverrides,
) -> Arc<SourceStore> {
	let output = TrackSourceAction::from_input(TrackSourceInput {
		path: path.to_path_buf(),
		name: None,
		overrides,
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

#[tokio::test(flavor = "multi_thread")]
async fn tracking_photos_fills_the_image_facet_once_per_content_hash() {
	let harness = IndexingHarnessBuilder::new("exif_enrich_acceptance")
		.disable_watcher()
		.build()
		.await
		.unwrap();
	let photos = harness.create_test_dir("photos").await.unwrap();
	let tokyo = exif_jpeg(1, Some("2024:03:12 10:00:00"), Some((35.6812, 139.7671)));
	std::fs::write(photos.path().join("a.jpg"), &tokyo).unwrap();
	std::fs::write(photos.path().join("a-copy.jpg"), &tokyo).unwrap();
	std::fs::write(
		photos.path().join("b.jpg"),
		exif_jpeg(2, Some("2024:03:19 09:00:00"), None),
	)
	.unwrap();
	std::fs::write(photos.path().join("none.jpg"), exif_jpeg(3, None, None)).unwrap();
	photos.write_file("notes.txt", "not a photo").await.unwrap();

	let store = track(&harness, photos.path(), AddOverrides::default()).await;

	// Row 1: every image has a row, the text file has none, and the two
	// copies of one photo share one row's worth of values under one hash.
	let rows = wait_for_facets(&store, 4).await;
	let names: Vec<&str> = rows.iter().map(|(name, _)| name.as_str()).collect();
	assert_eq!(names, ["a-copy.jpg", "a.jpg", "b.jpg", "none.jpg"]);
	let by_name = |name: &str| rows.iter().find(|(n, _)| n == name).unwrap().1.clone();
	let (a_hash, a_date, a_lat, a_lon) = by_name("a.jpg");
	assert_eq!(a_date.as_deref(), Some("2024-03-12T10:00:00+00:00"));
	assert!((a_lat.unwrap() - 35.6812).abs() < 0.001, "{a_lat:?}");
	assert!((a_lon.unwrap() - 139.7671).abs() < 0.001, "{a_lon:?}");
	assert_eq!(
		by_name("a-copy.jpg"),
		(a_hash.clone(), a_date, a_lat, a_lon)
	);
	let (b_hash, b_date, b_lat, _) = by_name("b.jpg");
	assert_ne!(b_hash, a_hash);
	assert_eq!(b_date.as_deref(), Some("2024-03-19T09:00:00+00:00"));
	assert_eq!(b_lat, None);
	let (none_hash, none_date, none_lat, _) = by_name("none.jpg");
	assert!(none_hash.is_some(), "a photo without EXIF is still read");
	assert_eq!((none_date, none_lat), (None, None));
	assert_eq!(store.files_needing_image_facets_count().await.unwrap(), 0);

	// Row 2: the same photos tracked again read nothing. The rows keep the
	// hash they were written under and the pass has no work.
	track(&harness, photos.path(), AddOverrides::default()).await;
	tokio::time::sleep(Duration::from_millis(300)).await;
	assert_eq!(store.files_needing_image_facets_count().await.unwrap(), 0);
	assert_eq!(wait_for_facets(&store, 4).await, rows);

	// Row 3: new bytes under a name re-run the pass for that file alone.
	std::fs::write(
		photos.path().join("b.jpg"),
		exif_jpeg(4, Some("2025:01:01 08:30:00"), Some((35.0116, 135.7681))),
	)
	.unwrap();
	track(&harness, photos.path(), AddOverrides::default()).await;
	let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
	let rows = loop {
		let rows = wait_for_facets(&store, 4).await;
		if by_name_in(&rows, "b.jpg").1.as_deref() == Some("2025-01-01T08:30:00+00:00") {
			break rows;
		}
		assert!(
			tokio::time::Instant::now() < deadline,
			"b.jpg was not re-read: {rows:?}"
		);
		tokio::time::sleep(Duration::from_millis(50)).await;
	};
	let (new_hash, _, new_lat, _) = by_name_in(&rows, "b.jpg");
	assert_ne!(new_hash, b_hash, "the row is keyed by the new bytes");
	assert!((new_lat.unwrap() - 35.0116).abs() < 0.001);
	assert_eq!(
		by_name_in(&rows, "a.jpg").0,
		a_hash,
		"untouched photos keep their rows"
	);

	// Row 4: a source added without content identification gets no facet
	// rows, since nothing names its images.
	let private = harness.create_test_dir("private").await.unwrap();
	std::fs::write(private.path().join("c.jpg"), &tokyo).unwrap();
	let private_store = track(
		&harness,
		private.path(),
		AddOverrides {
			identify_content: Some(false),
			..AddOverrides::default()
		},
	)
	.await;
	tokio::time::sleep(Duration::from_millis(500)).await;
	assert_eq!(
		private_store.counts().await.map(|c| c.contents),
		Some(0),
		"no identification"
	);
	assert!(facet_rows(&private_store).await.is_empty());

	harness.shutdown().await.unwrap();
}

fn by_name_in(rows: &[(String, FacetRow)], name: &str) -> FacetRow {
	rows.iter().find(|(n, _)| n == name).unwrap().1.clone()
}
