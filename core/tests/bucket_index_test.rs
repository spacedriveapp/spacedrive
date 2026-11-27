//! Acceptance for indexing a bucket source against a real S3-compatible
//! service: tracking a prefix lists it into the source store, so the objects
//! under it are records the library's queries reach.
//!
//! The service is named by `SD_TEST_S3_ENDPOINT` (and optionally
//! `SD_TEST_S3_BUCKET`, `SD_TEST_S3_ACCESS_KEY`, `SD_TEST_S3_SECRET_KEY`).
//! Without an endpoint the test skips with a reason, unless `SD_REQUIRE_S3`
//! is set, in which case it fails.

#![cfg(unix)]

mod helpers;

use std::path::{Path, PathBuf};

use bytes::Bytes;
use helpers::*;
use sd_core::{
	domain::SdPath,
	infra::{action::LibraryAction, api::SessionContext, job::types::JobId, query::LibraryQuery},
	library::AddOverrides,
	ops::{
		search::{
			input::{FileSearchInput, SearchScope},
			query::FileSearchQuery,
		},
		sources::{
			list_records::query::{ListSourceRecordsInput, ListSourceRecordsQuery},
			rescan::rescan_soon,
			track::{TrackSourceAction, TrackSourceInput},
		},
		volumes::add_cloud::{CloudStorageConfig, VolumeAddCloudAction, VolumeAddCloudInput},
	},
	volume::{backend::CloudServiceType, CloudBackend, VolumeBackend},
};
use uuid::Uuid;

#[tokio::test]
async fn tracking_a_bucket_prefix_lists_its_objects_into_the_store() -> anyhow::Result<()> {
	let Ok(endpoint) = std::env::var("SD_TEST_S3_ENDPOINT") else {
		if std::env::var_os("SD_REQUIRE_S3").is_some() {
			panic!("SD_REQUIRE_S3 is set but SD_TEST_S3_ENDPOINT is not set");
		}
		eprintln!("skipping: SD_TEST_S3_ENDPOINT is not set");
		return Ok(());
	};
	let bucket = std::env::var("SD_TEST_S3_BUCKET").unwrap_or_else(|_| "spacedrive-test".into());
	let access = std::env::var("SD_TEST_S3_ACCESS_KEY").unwrap_or_else(|_| "test".into());
	let secret = std::env::var("SD_TEST_S3_SECRET_KEY").unwrap_or_else(|_| "test".into());
	let harness = IndexingHarnessBuilder::new("bucket_index")
		.disable_watcher()
		.build()
		.await?;

	let run = format!("index/{}", Uuid::now_v7().simple());
	let seed = CloudBackend::new_s3(
		&bucket,
		"us-east-1",
		&access,
		&secret,
		Some(endpoint.clone()),
	)
	.await?;
	for (key, body) in [
		("notes.md", "first\n"),
		("photos/beach.jpg", "jpeg"),
		("photos/2024/summit.png", "png"),
		(".spacedrive/trash/x/old.txt", "trashed\n"),
	] {
		seed.write(Path::new(&format!("{run}/{key}")), Bytes::from(body))
			.await?;
	}

	VolumeAddCloudAction::from_input(VolumeAddCloudInput {
		service: CloudServiceType::S3,
		display_name: "Test bucket".to_string(),
		config: CloudStorageConfig::S3 {
			bucket: bucket.clone(),
			region: "us-east-1".to_string(),
			access_key_id: access.clone(),
			secret_access_key: secret.clone(),
			endpoint: Some(endpoint.clone()),
		},
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?;
	let root = format!("s3://{bucket}/{run}");
	let tracked = TrackSourceAction::from_input(TrackSourceInput {
		path: PathBuf::from(&root),
		name: Some("Indexed bucket".to_string()),
		overrides: AddOverrides::default(),
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?;
	let walk = tracked.job_id.expect("tracking a bucket lists it");
	if let Some(walk) = harness.library.jobs().get_job(JobId(walk)).await {
		walk.wait().await?;
	}

	let mut session = SessionContext::device_session(
		sd_core::device::get_current_device_id(),
		sd_core::device::get_current_device_slug(),
	);
	session.current_library_id = Some(harness.library.id());
	let records = ListSourceRecordsQuery::from_input(ListSourceRecordsInput {
		source_id: tracked.id.to_string(),
		limit: 100,
		offset: 0,
	})
	.unwrap()
	.execute(harness.core.context.clone(), session.clone())
	.await?;
	let mut titles: Vec<&str> = records
		.iter()
		.map(|record| record["title"].as_str().unwrap())
		.collect();
	titles.sort();
	assert_eq!(
		titles,
		["beach.jpg", "notes.md", "summit.png"],
		"every object under the prefix is a record, and the trash is not: {records:?}"
	);

	let found = FileSearchQuery::from_input(FileSearchInput::simple("summit".to_string()))
		.unwrap()
		.execute(harness.core.context.clone(), session.clone())
		.await?;
	let paths: Vec<String> = found
		.files
		.iter()
		.map(|file| file.sd_path.to_string())
		.collect();
	assert_eq!(
		paths,
		[format!("{root}/photos/2024/summit.png")],
		"a filename search reaches the bucket"
	);
	let scoped = FileSearchQuery::from_input(FileSearchInput {
		scope: SearchScope::Path {
			path: SdPath::from_uri(&format!("{root}/photos"))?,
		},
		..FileSearchInput::simple("beach".to_string())
	})
	.unwrap()
	.execute(harness.core.context.clone(), session.clone())
	.await?;
	assert_eq!(
		scoped
			.files
			.iter()
			.map(|file| file.sd_path.to_string())
			.collect::<Vec<_>>(),
		[format!("{root}/photos/beach.jpg")],
		"a search scoped to a prefix reaches it"
	);

	// A change lands in the records once the source is listed again: a new
	// object appears and a deleted one is gone.
	seed.write(Path::new(&format!("{run}/later.txt")), Bytes::from("new"))
		.await?;
	seed.delete(Path::new(&format!("{run}/notes.md"))).await?;
	rescan_soon(
		harness.library.clone(),
		Path::new(&format!("{root}/later.txt")),
	);
	let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
	loop {
		let records = ListSourceRecordsQuery::from_input(ListSourceRecordsInput {
			source_id: tracked.id.to_string(),
			limit: 100,
			offset: 0,
		})
		.unwrap()
		.execute(harness.core.context.clone(), session.clone())
		.await?;
		let mut titles: Vec<String> = records
			.iter()
			.map(|record| record["title"].as_str().unwrap().to_string())
			.collect();
		titles.sort();
		if titles == ["beach.jpg", "later.txt", "summit.png"] {
			break;
		}
		assert!(
			tokio::time::Instant::now() < deadline,
			"the rescan did not land: {titles:?}"
		);
		tokio::time::sleep(std::time::Duration::from_millis(250)).await;
	}
	Ok(())
}
