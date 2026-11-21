//! Acceptance for the read-only FUSE mount (`docs/core/fuse-mount.mdx`).
//!
//! The first test mounts a running daemon's sources through the ops the CLI
//! calls: it lists, stats and reads a tracked source, is refused writes,
//! narrows a second mount to a grant and watches it refuse and then go away
//! once the grant is revoked, and checks that shutdown unmounts what is left.
//! The second mounts a simulated peer source whose bytes come through the
//! block cache, so the remote read path is covered without a second device.
//!
//! Both need `/dev/fuse` and `fusermount3`. Without them they skip with the
//! reason, unless `SD_REQUIRE_FUSE` is set, in which case they fail.

#![cfg(all(target_os = "linux", feature = "fuse"))]

mod helpers;

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use helpers::*;
use sd_core::{
	infra::{
		action::{CoreAction, LibraryAction},
		source_dirs::SourceDirs,
	},
	ops::{
		config::app::{UpdateAppConfigAction, UpdateAppConfigInput},
		grants::{
			create::{CreateGrantAction, CreateGrantInput},
			revoke::{RevokeGrantAction, RevokeGrantInput},
			GrantCap,
		},
		indexing::{arena::Arena, metadata::EntryMetadata, state::EntryKind},
		mounts::{FuseMountAction, FuseMountInput, FuseUnmountAction, FuseUnmountInput},
	},
	service::mounts::{
		cache::{BlockCache, CachedProvider},
		fuse::{
			self,
			tree::{Layout, MountError, MountRoot, MountTree},
		},
		provider::{ByteError, ByteProvider, ByteStat, ByteTarget, ProviderClass},
	},
};
use tokio::sync::RwLock;
use tracing::warn;
use uuid::Uuid;

const EROFS: i32 = 30;
const EACCES: i32 = 13;
const ENOENT: i32 = 2;
const EIO: i32 = 5;

/// Why FUSE cannot run here, if it cannot.
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

/// Run blocking filesystem calls off the runtime that serves the mount.
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_daemon_mounts_lists_reads_and_unmounts() -> anyhow::Result<()> {
	let _ = tracing_subscriber::fmt::try_init();
	if let Some(reason) = skip_reason() {
		warn!("Skipping: {reason}");
		return Ok(());
	}

	let harness = IndexingHarnessBuilder::new("fuse_mount")
		.disable_watcher()
		.build()
		.await?;
	let tracked = harness.create_test_dir("tracked").await?;
	tracked
		.write_file("docs/readme.md", "hello from the mount\n")
		.await?;
	tracked
		.write_file("docs/deep/notes.txt", "0123456789")
		.await?;
	tracked.write_file("private/key.txt", "secret").await?;
	tracked.track().await?;
	let context = harness.core.context.clone();
	let base = std::fs::canonicalize(harness.temp_path())?;
	let all = base.join("mnt-all");
	let granted = base.join("mnt-grant");
	let at_shutdown = base.join("mnt-shutdown");
	for dir in [&all, &granted, &at_shutdown] {
		std::fs::create_dir(dir)?;
	}

	let mount = |path: &Path, grant_token: Option<String>| {
		FuseMountAction::from_input(FuseMountInput {
			path: path.to_path_buf(),
			grant_token,
		})
		.unwrap()
		.execute(context.clone())
	};

	// Off until the config allows it.
	let refused = mount(&all, None)
		.await
		.expect_err("mounts are off by default");
	assert!(refused.to_string().contains("fuse_enabled"), "{refused}");
	UpdateAppConfigAction::from_input(UpdateAppConfigInput {
		mounts_fuse_enabled: Some(true),
		..Default::default()
	})
	.map_err(anyhow::Error::msg)?
	.execute(context.clone())
	.await?;

	// A mount point must be an empty directory.
	assert!(mount(&base.join("tracked"), None).await.is_err());

	let info = mount(&all, None).await?;
	assert_eq!(info.path, all);
	assert!(info.grant_id.is_none());
	assert!(mount(&all, None).await.is_err(), "one mount per path");

	{
		let all = all.clone();
		blocking(move || {
			let shares = names(&all);
			let share = shares
				.iter()
				.find(|name| name.starts_with("tracked-"))
				.unwrap_or_else(|| panic!("the tracked source is listed: {shares:?}"));
			let root = all.join(share);
			assert_eq!(names(&root), ["docs", "private"]);
			assert_eq!(names(&root.join("docs")), ["deep", "readme.md"]);

			let readme = root.join("docs/readme.md");
			let meta = std::fs::metadata(&readme).unwrap();
			assert!(meta.is_file());
			assert_eq!(meta.len(), "hello from the mount\n".len() as u64);
			assert!(meta.permissions().readonly());
			assert!(std::fs::metadata(root.join("docs/deep")).unwrap().is_dir());
			assert_eq!(
				std::fs::read_to_string(&readme).unwrap(),
				"hello from the mount\n"
			);

			let mut notes = std::fs::File::open(root.join("docs/deep/notes.txt")).unwrap();
			notes.seek(SeekFrom::Start(4)).unwrap();
			let mut tail = String::new();
			notes.read_to_string(&mut tail).unwrap();
			assert_eq!(tail, "456789", "reads start where they are asked to");

			assert_eq!(
				std::fs::metadata(root.join("docs/absent.md"))
					.unwrap_err()
					.kind(),
				std::io::ErrorKind::NotFound
			);
			assert_eq!(
				os_error(std::fs::write(root.join("docs/new.md"), "x")),
				Some(EROFS)
			);
			assert_eq!(
				os_error(std::fs::OpenOptions::new().append(true).open(&readme)),
				Some(EROFS)
			);
			assert_eq!(os_error(std::fs::remove_file(&readme)), Some(EROFS));
		})
		.await;
	}

	// A grant narrows a mount to the directory it lends.
	let grant = CreateGrantAction::from_input(CreateGrantInput {
		path: base.join("tracked/docs"),
		caps: Some(GrantCap::Read),
		ttl_seconds: None,
		label: Some("fuse".to_string()),
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), context.clone())
	.await?;
	assert!(mount(&granted, Some("sdg_not-a-token".to_string()))
		.await
		.is_err());
	let info = mount(&granted, Some(grant.token.clone())).await?;
	assert_eq!(info.grant_id, Some(grant.grant.id));
	assert_eq!(info.granted_path, Some(base.join("tracked/docs")));
	{
		let granted = granted.clone();
		blocking(move || {
			assert_eq!(names(&granted), ["deep", "readme.md"]);
			assert_eq!(
				std::fs::read_to_string(granted.join("deep/notes.txt")).unwrap(),
				"0123456789"
			);
			assert_eq!(
				std::fs::metadata(granted.join("private"))
					.unwrap_err()
					.kind(),
				std::io::ErrorKind::NotFound,
				"nothing outside the granted directory is reachable"
			);
		})
		.await;
	}

	// Revoked: the next open is refused, and the mount goes away.
	RevokeGrantAction::from_input(RevokeGrantInput { id: grant.grant.id })
		.map_err(anyhow::Error::msg)?
		.execute(harness.library.clone(), context.clone())
		.await?;
	{
		let granted = granted.clone();
		let refused = blocking(move || os_error(std::fs::read(granted.join("readme.md")))).await;
		// ENOENT only if the mount was already taken down under the read.
		assert!(
			matches!(refused, Some(EACCES) | Some(ENOENT)),
			"a revoked grant is refused: {refused:?}"
		);
	}
	let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
	while fuse::list().await.iter().any(|m| m.path == granted) {
		assert!(
			tokio::time::Instant::now() < deadline,
			"the revoked grant's mount is taken down"
		);
		tokio::time::sleep(Duration::from_millis(250)).await;
	}
	{
		let granted = granted.clone();
		assert!(blocking(move || names(&granted)).await.is_empty());
	}

	// Unmounting leaves the empty directory behind.
	FuseUnmountAction::from_input(FuseUnmountInput { path: all.clone() })
		.map_err(anyhow::Error::msg)?
		.execute(context.clone())
		.await?;
	{
		let all = all.clone();
		assert!(blocking(move || names(&all)).await.is_empty());
	}
	assert!(
		FuseUnmountAction::from_input(FuseUnmountInput { path: all.clone() })
			.unwrap()
			.execute(context.clone())
			.await
			.is_err()
	);

	// Shutdown takes down whatever is still mounted.
	mount(&at_shutdown, None).await?;
	harness.shutdown().await?;
	assert!(fuse::list().await.is_empty());
	let mounts = std::fs::read_to_string("/proc/self/mounts")?;
	assert!(
		!mounts.contains(&*at_shutdown.to_string_lossy()),
		"shutdown left the mount in the kernel's table"
	);
	Ok(())
}

/// A peer that serves bytes from memory and counts what it is asked for.
struct SimulatedPeer {
	data: Option<Bytes>,
	reads: Arc<AtomicUsize>,
}

#[async_trait]
impl ByteProvider for SimulatedPeer {
	async fn stat(&self, _target: &ByteTarget) -> Result<ByteStat, ByteError> {
		match &self.data {
			Some(data) => Ok(ByteStat {
				size: data.len() as u64,
				modified: None,
			}),
			None => Err(offline()),
		}
	}

	async fn read_range(
		&self,
		_target: &ByteTarget,
		range: std::ops::Range<u64>,
	) -> Result<Bytes, ByteError> {
		self.reads.fetch_add(1, Ordering::SeqCst);
		let data = self.data.as_ref().ok_or_else(offline)?;
		let start = (range.start as usize).min(data.len());
		let end = (range.end as usize).min(data.len());
		Ok(data.slice(start..end))
	}

	fn class(&self) -> ProviderClass {
		ProviderClass::Peer
	}
}

fn offline() -> ByteError {
	ByteError::PeerUnavailable {
		device: "studio".to_string(),
		reason: "device is not connected".to_string(),
	}
}

/// One replicated source from a paired device, as the mount sees it.
struct PeerTree {
	root: MountRoot,
	arena: Arc<RwLock<Arena>>,
	files: HashMap<PathBuf, Option<Bytes>>,
	cache: Arc<BlockCache>,
	reads: Arc<AtomicUsize>,
}

#[async_trait]
impl MountTree for PeerTree {
	async fn layout(&self) -> Layout {
		Layout::Shares(vec![self.root.clone()])
	}

	async fn arena(&self, source: Uuid, _abs: &Path) -> Option<Arc<RwLock<Arena>>> {
		(source == self.root.source_id).then(|| self.arena.clone())
	}

	async fn open(&self, _source: Uuid, abs: &Path) -> Result<Arc<dyn ByteProvider>, MountError> {
		let data = self.files.get(abs).cloned().ok_or(MountError::NotFound)?;
		let peer = Arc::new(SimulatedPeer {
			data,
			reads: self.reads.clone(),
		});
		Ok(Arc::new(CachedProvider::new(peer, self.cache.clone())))
	}
}

fn entry(path: &Path, kind: EntryKind, size: u64) -> EntryMetadata {
	EntryMetadata {
		path: path.to_path_buf(),
		kind,
		size,
		modified: None,
		accessed: None,
		created: None,
		inode: None,
		permissions: None,
		uid: None,
		gid: None,
		link_target: None,
		is_hidden: false,
	}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_peer_source_reads_through_the_block_cache() -> anyhow::Result<()> {
	let _ = tracing_subscriber::fmt::try_init();
	if let Some(reason) = skip_reason() {
		warn!("Skipping: {reason}");
		return Ok(());
	}

	let dir = tempfile::tempdir()?;
	let data_dir = dir.path().join("data");
	let mountpoint = dir.path().join("mnt");
	std::fs::create_dir_all(&data_dir)?;
	std::fs::create_dir_all(&mountpoint)?;

	let root = PathBuf::from("/srv/pool/footage");
	let clip = root.join("clips/take-1.bin");
	let gone = root.join("clips/take-2.bin");
	let clip_bytes: Bytes = (0..3 * 1024 * 1024u32).map(|i| (i % 251) as u8).collect();

	let mut arena = Arena::new()?;
	for (path, kind, size) in [
		(root.clone(), EntryKind::Directory, 0),
		(root.join("clips"), EntryKind::Directory, 0),
		(clip.clone(), EntryKind::File, clip_bytes.len() as u64),
		(gone.clone(), EntryKind::File, 64),
	] {
		arena.add_entry(path.clone(), Uuid::now_v7(), entry(&path, kind, size))?;
	}

	let reads = Arc::new(AtomicUsize::new(0));
	let cache = Arc::new(BlockCache::new(
		SourceDirs::under_data_dir(&data_dir)?,
		64 * 1024 * 1024,
	));
	let tree = Arc::new(PeerTree {
		root: MountRoot {
			name: "footage@studio-a1b2c3".to_string(),
			source_id: Uuid::now_v7(),
			path: root.clone(),
			remote: true,
		},
		arena: Arc::new(RwLock::new(arena)),
		files: HashMap::from([
			(clip.clone(), Some(clip_bytes.clone())),
			(gone.clone(), None),
		]),
		cache: cache.clone(),
		reads: reads.clone(),
	});
	let session = fuse::fs::mount(tree, &mountpoint).await?;

	let share = mountpoint.join("footage@studio-a1b2c3");
	{
		let (mountpoint, share, clip_bytes) =
			(mountpoint.clone(), share.clone(), clip_bytes.clone());
		blocking(move || {
			assert_eq!(names(&mountpoint), ["footage@studio-a1b2c3"]);
			assert_eq!(names(&share.join("clips")), ["take-1.bin", "take-2.bin"]);
			assert_eq!(
				std::fs::metadata(share.join("clips/take-1.bin"))
					.unwrap()
					.len(),
				clip_bytes.len() as u64
			);
			assert_eq!(
				std::fs::read(share.join("clips/take-1.bin")).unwrap(),
				clip_bytes,
				"bytes arrive intact through the cache"
			);
		})
		.await;
	}
	// Let any read-ahead the first pass started settle before counting.
	tokio::time::sleep(Duration::from_millis(500)).await;
	let fetched = reads.load(Ordering::SeqCst);
	assert!(fetched > 0, "the first read went to the peer");
	let misses = cache.snapshot().misses;

	// Opening again drops the kernel's page cache, so this read reaches the
	// mount, and the block cache answers it without asking the peer.
	{
		let (share, clip_bytes) = (share.clone(), clip_bytes.clone());
		blocking(move || {
			let mut file = std::fs::File::open(share.join("clips/take-1.bin")).unwrap();
			file.seek(SeekFrom::Start(1024 * 1024 + 7)).unwrap();
			let mut buf = vec![0u8; 4096];
			file.read_exact(&mut buf).unwrap();
			assert_eq!(buf, clip_bytes[1024 * 1024 + 7..1024 * 1024 + 7 + 4096]);
		})
		.await;
	}
	let snapshot = cache.snapshot();
	assert_eq!(snapshot.misses, misses, "the second read missed nothing");
	assert!(snapshot.l1_hits + snapshot.l2_hits > 0);

	// A peer that cannot answer is an I/O error, not an empty file.
	{
		let share = share.clone();
		let failed =
			blocking(move || os_error(std::fs::read(share.join("clips/take-2.bin")))).await;
		assert_eq!(failed, Some(EIO));
	}

	blocking(move || session.umount_and_join()).await?;
	assert!(names(&mountpoint).is_empty());
	Ok(())
}
