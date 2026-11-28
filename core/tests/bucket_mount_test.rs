//! Acceptance for mounting a bucket space with the thin client
//! (`sd mount`), against a real S3-compatible service. A prefix of a
//! bucket becomes a folder space, a read grant lends it, and the client
//! mounts it knowing only the endpoint's URL and the token. It lists the
//! prefix's directories, stats and reads objects across block edges, and
//! reads an object again after it changes in the bucket.
//!
//! Needs `SD_TEST_S3_ENDPOINT`, `/dev/fuse` and `fusermount3`. Without them the
//! test skips with the reason, unless `SD_REQUIRE_S3` or `SD_REQUIRE_FUSE`
//! is set, in which case it fails.

#![cfg(all(target_os = "linux", feature = "fuse"))]

mod helpers;

use std::{
	io::{Read, Seek, SeekFrom},
	path::{Path, PathBuf},
	sync::Arc,
	time::Duration,
};

use bytes::Bytes;
use helpers::*;
use sd_core::{
	domain::SpaceKind,
	infra::action::LibraryAction,
	ops::{
		grants::{
			create::{CreateGrantAction, CreateGrantInput},
			GrantCap,
		},
		spaces::{SpaceCreateAction, SpaceCreateInput},
		volumes::add_cloud::{CloudStorageConfig, VolumeAddCloudAction, VolumeAddCloudInput},
	},
	service::{
		agent_mcp::{self, Limits},
		mounts::{client, fuse},
	},
	volume::{backend::CloudServiceType, CloudBackend, VolumeBackend},
};
use uuid::Uuid;

const EROFS: i32 = 30;

fn fuse_missing() -> Option<String> {
	let reason = if let Err(err) = std::fs::OpenOptions::new()
		.read(true)
		.write(true)
		.open("/dev/fuse")
	{
		format!("/dev/fuse is not usable: {err}")
	} else if !std::process::Command::new("fusermount3")
		.arg("--version")
		.output()
		.is_ok_and(|out| out.status.success())
	{
		"fusermount3 is not installed (apt install fuse3)".to_string()
	} else {
		return None;
	};
	if std::env::var_os("SD_REQUIRE_FUSE").is_some() {
		panic!("SD_REQUIRE_FUSE is set but {reason}");
	}
	Some(reason)
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
	tokio::task::spawn_blocking(f).await.expect("blocking task")
}

fn names(dir: &Path) -> Vec<String> {
	let mut names: Vec<String> = std::fs::read_dir(dir)
		.unwrap_or_else(|err| panic!("list {}: {err}", dir.display()))
		.map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
		.collect();
	names.sort();
	names
}

fn os_error<T: std::fmt::Debug>(result: std::io::Result<T>) -> Option<i32> {
	result.expect_err("expected an error").raw_os_error()
}

async fn eventually(
	within: Duration,
	what: &str,
	check: impl Fn() -> bool + Send + Sync + 'static,
) {
	let check = Arc::new(check);
	let deadline = tokio::time::Instant::now() + within;
	loop {
		let check = check.clone();
		if blocking(move || check()).await {
			return;
		}
		assert!(tokio::time::Instant::now() < deadline, "{what}");
		tokio::time::sleep(Duration::from_millis(200)).await;
	}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_thin_client_mounts_a_bucket_space_and_reads_it() -> anyhow::Result<()> {
	let _ = tracing_subscriber::fmt::try_init();
	let Ok(endpoint) = std::env::var("SD_TEST_S3_ENDPOINT") else {
		if std::env::var_os("SD_REQUIRE_S3").is_some() {
			panic!("SD_REQUIRE_S3 is set but SD_TEST_S3_ENDPOINT is not set");
		}
		eprintln!("skipping: SD_TEST_S3_ENDPOINT is not set");
		return Ok(());
	};
	if let Some(reason) = fuse_missing() {
		eprintln!("skipping: {reason}");
		return Ok(());
	}
	let bucket = std::env::var("SD_TEST_S3_BUCKET").unwrap_or_else(|_| "spacedrive-test".into());
	let access = std::env::var("SD_TEST_S3_ACCESS_KEY").unwrap_or_else(|_| "test".into());
	let secret = std::env::var("SD_TEST_S3_SECRET_KEY").unwrap_or_else(|_| "test".into());
	let harness = IndexingHarnessBuilder::new("bucket_mount")
		.disable_watcher()
		.build()
		.await?;
	let base = std::fs::canonicalize(harness.temp_path())?;

	let run = format!("mounts/{}", Uuid::now_v7().simple());
	let seed = CloudBackend::new_s3(
		&bucket,
		"us-east-1",
		&access,
		&secret,
		Some(endpoint.clone()),
	)
	.await?;
	// Past one cache block and not UTF-8, so reads cross block edges.
	let clip: Vec<u8> = (0..(2 * 1024 * 1024 + 300_000u32))
		.map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8)
		.collect();
	for (key, body) in [
		("project/readme.md", Bytes::from("hello from the bucket\n")),
		("project/deep/notes.txt", Bytes::from("0123456789")),
		("project/clip.bin", Bytes::from(clip.clone())),
	] {
		seed.write(Path::new(&format!("{run}/{key}")), body).await?;
	}
	seed.create_directory(Path::new(&format!("{run}/project/empty")), true)
		.await?;
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

	let project = PathBuf::from(format!("s3://{bucket}/{run}/project"));
	SpaceCreateAction::from_input(SpaceCreateInput {
		name: None,
		kind: SpaceKind::Folder,
		path: Some(project.clone()),
		content_indexing: None,
		protected: None,
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?;
	let grant = CreateGrantAction::from_input(CreateGrantInput {
		path: project,
		caps: Some(GrantCap::Read),
		ttl_seconds: None,
		kind: None,
		token_format: None,
		label: Some("bucket mount".to_string()),
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?;
	let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
	let bound = agent_mcp::serve(harness.core.context.clone(), listener, Limits::default())?;

	let fs = client::connect(
		&format!("http://{bound}"),
		&grant.token,
		None,
		Some(base.join("cache")),
	)
	.await?;
	let mnt = base.join("mnt");
	std::fs::create_dir(&mnt)?;
	let mounted = fuse::fs::mount(fs.clone(), &mnt).await?;

	{
		let (mnt, clip) = (mnt.clone(), clip.clone());
		blocking(move || {
			assert_eq!(names(&mnt), ["clip.bin", "deep", "empty", "readme.md"]);
			assert_eq!(names(&mnt.join("deep")), ["notes.txt"]);
			assert!(names(&mnt.join("empty")).is_empty());
			assert!(std::fs::metadata(mnt.join("deep")).unwrap().is_dir());
			assert!(std::fs::metadata(mnt.join("empty")).unwrap().is_dir());
			let meta = std::fs::metadata(mnt.join("readme.md")).unwrap();
			assert!(meta.is_file() && meta.permissions().readonly());
			assert_eq!(meta.len(), "hello from the bucket\n".len() as u64);
			assert_eq!(
				std::fs::read_to_string(mnt.join("readme.md")).unwrap(),
				"hello from the bucket\n"
			);

			let mut notes = std::fs::File::open(mnt.join("deep/notes.txt")).unwrap();
			notes.seek(SeekFrom::Start(4)).unwrap();
			let mut tail = String::new();
			notes.read_to_string(&mut tail).unwrap();
			assert_eq!(tail, "456789");

			let read = std::fs::read(mnt.join("clip.bin")).unwrap();
			let first_wrong = read.iter().zip(clip.iter()).position(|(a, b)| a != b);
			assert!(
				read.len() == clip.len() && first_wrong.is_none(),
				"binary bytes arrive intact: read {} of {}, first wrong byte at {first_wrong:?}",
				read.len(),
				clip.len()
			);

			assert_eq!(
				std::fs::metadata(mnt.join("absent.md")).unwrap_err().kind(),
				std::io::ErrorKind::NotFound
			);
			assert_eq!(
				os_error(std::fs::write(mnt.join("new.md"), "x")),
				Some(EROFS)
			);
		})
		.await;
	}

	// An object replaced in the bucket is read afresh: a new open asks the
	// node, and block reads are pinned to the new entity tag.
	seed.write(
		Path::new(&format!("{run}/project/readme.md")),
		Bytes::from("changed in the bucket, longer\n"),
	)
	.await?;
	{
		let mnt = mnt.clone();
		eventually(
			Duration::from_secs(10),
			"the changed object is read again",
			move || {
				std::fs::read_to_string(mnt.join("readme.md"))
					.ok()
					.as_deref() == Some("changed in the bucket, longer\n")
			},
		)
		.await;
	}

	blocking(move || mounted.umount_and_join()).await?;
	fs.close().await;
	Ok(())
}
