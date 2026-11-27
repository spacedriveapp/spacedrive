//! Acceptance for the thin mount client (`docs/core/mount-client.mdx`).
//!
//! A daemon lends one folder through a grant and serves it on its agent
//! endpoint. The client mounts that folder knowing only the endpoint's URL
//! and the token, with no library of its own, then lists, stats and reads
//! it, is refused writes, sees changes made at the origin, and is refused
//! once the grant is revoked.
//!
//! Needs `/dev/fuse` and `fusermount3`. Without them the test skips with the
//! reason, unless `SD_REQUIRE_FUSE` is set, in which case it fails.

#![cfg(all(target_os = "linux", feature = "fuse"))]

mod helpers;

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use helpers::*;
use sd_core::{
	infra::action::LibraryAction,
	ops::grants::{
		create::{CreateGrantAction, CreateGrantInput},
		revoke::{RevokeGrantAction, RevokeGrantInput},
		GrantCap,
	},
	service::{
		agent_mcp::{self, Limits},
		mounts::{client, fuse},
	},
};
use tracing::warn;

const EROFS: i32 = 30;
const EACCES: i32 = 13;
const EIO: i32 = 5;

fn skip_reason() -> Option<String> {
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

/// Poll `check` on a blocking thread until it holds or `within` passes.
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
async fn a_thin_client_mounts_a_granted_folder_over_the_endpoint() -> anyhow::Result<()> {
	let _ = tracing_subscriber::fmt::try_init();
	if let Some(reason) = skip_reason() {
		warn!("Skipping: {reason}");
		return Ok(());
	}

	let harness = IndexingHarnessBuilder::new("mount_client")
		.disable_watcher()
		.build()
		.await?;
	let tracked = harness.create_test_dir("tracked").await?;
	tracked
		.write_file("shared/readme.md", "hello from the node\n")
		.await?;
	tracked
		.write_file("shared/deep/notes.txt", "0123456789")
		.await?;
	tracked.write_file("private/key.txt", "secret").await?;
	let base = std::fs::canonicalize(harness.temp_path())?;
	// Past one cache block and not UTF-8, so it crosses block edges and
	// travels as base64.
	let clip: Vec<u8> = (0..(2 * 1024 * 1024 + 300_000u32))
		.map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8)
		.collect();
	std::fs::write(base.join("tracked/shared/clip.bin"), &clip)?;
	tracked.track().await?;
	let context = harness.core.context.clone();

	let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
	let bound = agent_mcp::serve(context.clone(), listener, Limits::default())?;
	let url = format!("http://{bound}");
	let grant = CreateGrantAction::from_input(CreateGrantInput {
		path: base.join("tracked/shared"),
		caps: Some(GrantCap::Read),
		ttl_seconds: None,
		label: Some("thin mount".to_string()),
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), context.clone())
	.await?;

	assert!(
		client::connect(&url, "sdg_not-a-token", Some(base.join("cache")))
			.await
			.is_err(),
		"a token the node does not know is refused before mounting"
	);
	let fs = client::connect(&url, &grant.token, Some(base.join("cache"))).await?;
	let mnt = base.join("mnt");
	std::fs::create_dir(&mnt)?;
	let mounted = fuse::fs::mount(fs.clone(), &mnt).await?;

	{
		let (mnt, clip) = (mnt.clone(), clip.clone());
		blocking(move || {
			assert_eq!(names(&mnt), ["clip.bin", "deep", "readme.md"]);
			assert_eq!(names(&mnt.join("deep")), ["notes.txt"]);
			let meta = std::fs::metadata(mnt.join("readme.md")).unwrap();
			assert!(meta.is_file() && meta.permissions().readonly());
			assert_eq!(meta.len(), "hello from the node\n".len() as u64);
			assert!(std::fs::metadata(mnt.join("deep")).unwrap().is_dir());
			assert_eq!(
				std::fs::read_to_string(mnt.join("readme.md")).unwrap(),
				"hello from the node\n"
			);

			let mut notes = std::fs::File::open(mnt.join("deep/notes.txt")).unwrap();
			notes.seek(SeekFrom::Start(4)).unwrap();
			let mut tail = String::new();
			notes.read_to_string(&mut tail).unwrap();
			assert_eq!(tail, "456789", "reads start where they are asked to");

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
			assert_eq!(
				os_error(
					std::fs::OpenOptions::new()
						.append(true)
						.open(mnt.join("readme.md"))
				),
				Some(EROFS)
			);
			assert_eq!(
				os_error(std::fs::remove_file(mnt.join("readme.md"))),
				Some(EROFS)
			);
		})
		.await;
	}

	// Changes at the origin show through the mount: new bytes, a new file, a
	// removed file.
	std::fs::write(
		base.join("tracked/shared/readme.md"),
		"changed at the origin, longer\n",
	)?;
	std::fs::write(base.join("tracked/shared/added.txt"), "new")?;
	std::fs::remove_file(base.join("tracked/shared/deep/notes.txt"))?;
	{
		let mnt = mnt.clone();
		eventually(
			Duration::from_secs(10),
			"the changed file is read again",
			move || {
				std::fs::read_to_string(mnt.join("readme.md"))
					.ok()
					.as_deref() == Some("changed at the origin, longer\n")
			},
		)
		.await;
	}
	{
		let mnt = mnt.clone();
		eventually(
			Duration::from_secs(10),
			"the listing follows the origin",
			move || {
				names(&mnt) == ["added.txt", "clip.bin", "deep", "readme.md"]
					&& names(&mnt.join("deep")).is_empty()
			},
		)
		.await;
	}

	// Blocks of granted files are readable by this user alone.
	{
		use std::os::unix::fs::PermissionsExt;
		let mode = std::fs::metadata(base.join("cache"))?.permissions().mode();
		assert_eq!(mode & 0o777, 0o700, "the block cache is private: {mode:o}");
	}

	// A file opened and read while the grant held, so its bytes sit in the
	// block cache and the kernel's page cache.
	let held = {
		let (mnt, clip) = (mnt.clone(), clip.clone());
		blocking(move || {
			let mut file = std::fs::File::open(mnt.join("clip.bin")).unwrap();
			let mut head = vec![0u8; 4096];
			file.read_exact(&mut head).unwrap();
			assert_eq!(head, clip[..4096]);
			file
		})
		.await
	};
	let ended = tokio::spawn({
		let fs = fs.clone();
		async move { fs.grant_ended().await }
	});

	// Revoked: the client notices without being asked, and nothing is served
	// after that, not even bytes it already holds.
	RevokeGrantAction::from_input(RevokeGrantInput { id: grant.grant.id })
		.map_err(anyhow::Error::msg)?
		.execute(harness.library.clone(), context.clone())
		.await?;
	tokio::time::timeout(Duration::from_secs(15), ended)
		.await
		.expect("the client sees the grant end within its check interval")?;
	let held = blocking(move || {
		let mut held = held;
		held.seek(SeekFrom::Start(0)).unwrap();
		let mut head = vec![0u8; 4096];
		// Some kernels read through the page cache and report any refused
		// fill as EIO rather than passing EACCES on.
		let refused = os_error(held.read_exact(&mut head));
		assert!(
			matches!(refused, Some(EACCES) | Some(EIO)),
			"a descriptor opened before the revoke cannot read cached bytes: {refused:?}"
		);
		held
	})
	.await;
	drop(held);
	{
		let mnt = mnt.clone();
		blocking(move || {
			assert_eq!(os_error(std::fs::read(mnt.join("added.txt"))), Some(EACCES));
			assert_eq!(os_error(std::fs::read_dir(&mnt)), Some(EACCES));
		})
		.await;
	}

	blocking(move || mounted.umount_and_join()).await?;
	fs.close().await;
	assert!(
		names(&mnt).is_empty(),
		"unmounting leaves the empty directory"
	);
	Ok(())
}
