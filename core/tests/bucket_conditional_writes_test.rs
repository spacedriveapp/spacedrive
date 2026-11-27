//! Conditional writes against a real S3-compatible service.
//!
//! The service is named by `SD_TEST_S3_ENDPOINT` and `SD_TEST_S3_BUCKET`,
//! with `SD_TEST_S3_ACCESS_KEY` and `SD_TEST_S3_SECRET_KEY` when it checks
//! credentials. CI runs SeaweedFS for it. Without an endpoint the tests skip
//! with a reason, unless `SD_REQUIRE_S3` is set, in which case they fail.
//!
//! Each run writes under its own prefix, so a shared bucket never sees two
//! runs collide.

use std::path::{Path, PathBuf};

use bytes::Bytes;
use sd_core::volume::{CloudBackend, VolumeBackend, VolumeError, WriteCondition};
use uuid::Uuid;

struct Bucket {
	backend: CloudBackend,
	prefix: PathBuf,
}

impl Bucket {
	fn path(&self, name: &str) -> PathBuf {
		self.prefix.join(name)
	}
}

/// The test bucket, or `None` with the reason printed when none is set.
async fn bucket() -> Option<Bucket> {
	let Ok(endpoint) = std::env::var("SD_TEST_S3_ENDPOINT") else {
		let reason = "SD_TEST_S3_ENDPOINT is not set";
		if std::env::var_os("SD_REQUIRE_S3").is_some() {
			panic!("SD_REQUIRE_S3 is set but {reason}");
		}
		eprintln!("skipping: {reason}");
		return None;
	};
	let bucket = std::env::var("SD_TEST_S3_BUCKET").unwrap_or_else(|_| "spacedrive-test".into());
	let access = std::env::var("SD_TEST_S3_ACCESS_KEY").unwrap_or_else(|_| "test".into());
	let secret = std::env::var("SD_TEST_S3_SECRET_KEY").unwrap_or_else(|_| "test".into());
	let backend = CloudBackend::new_s3(&bucket, "us-east-1", &access, &secret, Some(endpoint))
		.await
		.expect("S3 backend");
	Some(Bucket {
		backend,
		prefix: Path::new("conditional").join(Uuid::now_v7().simple().to_string()),
	})
}

#[tokio::test]
async fn s3_reports_conditional_writes() {
	let Some(bucket) = bucket().await else { return };
	assert!(bucket.backend.conditional_writes());
}

#[tokio::test]
async fn a_create_never_replaces_an_object() {
	let Some(bucket) = bucket().await else { return };
	let path = bucket.path("notes.md");
	assert_eq!(bucket.backend.entity_tag(&path).await.unwrap(), None);

	bucket
		.backend
		.write_if(&path, Bytes::from("first"), WriteCondition::Absent)
		.await
		.expect("create on an empty key");
	let refused = bucket
		.backend
		.write_if(&path, Bytes::from("second"), WriteCondition::Absent)
		.await;
	assert!(
		matches!(refused, Err(VolumeError::PreconditionFailed(_))),
		"a second create must be refused, got {refused:?}"
	);
	assert_eq!(bucket.backend.read(&path).await.unwrap(), "first");
}

#[tokio::test]
async fn a_write_against_a_stale_tag_is_refused() {
	let Some(bucket) = bucket().await else { return };
	let path = bucket.path("plan.txt");
	bucket
		.backend
		.write_if(&path, Bytes::from("v1"), WriteCondition::Absent)
		.await
		.expect("create");
	let read = bucket
		.backend
		.entity_tag(&path)
		.await
		.unwrap()
		.expect("a tag for an object that exists");
	assert_eq!(
		bucket.backend.read_if_match(&path, &read).await.unwrap(),
		"v1"
	);

	bucket
		.backend
		.write_if(
			&path,
			Bytes::from("v2"),
			WriteCondition::Matches(read.clone()),
		)
		.await
		.expect("a write naming the current tag lands");
	let current = bucket
		.backend
		.entity_tag(&path)
		.await
		.unwrap()
		.expect("tag");
	assert_ne!(current, read);

	let stale = bucket
		.backend
		.write_if(
			&path,
			Bytes::from("v3"),
			WriteCondition::Matches(read.clone()),
		)
		.await;
	assert!(
		matches!(stale, Err(VolumeError::PreconditionFailed(_))),
		"a write naming an old tag must be refused, got {stale:?}"
	);
	let stale_read = bucket.backend.read_if_match(&path, &read).await;
	assert!(matches!(
		stale_read,
		Err(VolumeError::PreconditionFailed(_))
	));
	assert_eq!(bucket.backend.read(&path).await.unwrap(), "v2");
}

#[tokio::test]
async fn a_missing_object_reads_as_not_found() {
	let Some(bucket) = bucket().await else { return };
	let missing = bucket.backend.metadata(&bucket.path("absent.bin")).await;
	assert!(
		matches!(missing, Err(VolumeError::Io(ref e)) if e.kind() == std::io::ErrorKind::NotFound),
		"got {missing:?}"
	);
}
