//! Acceptance for L3 and L4 of the locked volumes plan
//! (`docs/plans/2026-09-28-locked-volumes.md`,
//! `docs/core/acceptance/volumes.md`) on ZFS: a dataset whose key is not
//! loaded reads as locked rather than unmounted or empty, its source serves
//! from its snapshot with nothing dispatched at it, and loading the key and
//! mounting the dataset under the running daemon reattaches the source,
//! arms its watch on the mounted filesystem, and retries the
//! identifications that failed while it was away.
//!
//! The test builds a file-backed pool under `/tmp` mounted under `/mnt`, so
//! the dataset classifies as external the way a NAS pool does, with one
//! encrypted dataset whose passphrase lives in a key file. It skips with a
//! reason where there is no passwordless sudo, no zfs userland, or no zfs
//! kernel module.

mod helpers;

use std::{
	path::{Path, PathBuf},
	sync::Arc,
};

use helpers::TestConfigBuilder;
use sd_core::{
	infra::{action::LibraryAction, api::SessionContext, query::LibraryQuery},
	library::Library,
	ops::{
		indexing::content_identity::identify_every_source,
		sources::{
			list::query::ListSourcesQuery,
			track::{TrackSourceAction, TrackSourceInput},
		},
		volumes::list::query::{VolumeFilter, VolumeListQuery, VolumeListQueryInput},
	},
	volume::VolumeState,
	Core,
};
use tracing::warn;

async fn sudo(args: &[&str]) -> Result<String, String> {
	let output = tokio::process::Command::new("sudo")
		.arg("-n")
		.args(args)
		.output()
		.await
		.map_err(|e| format!("sudo {}: {e}", args.join(" ")))?;
	if output.status.success() {
		Ok(String::from_utf8_lossy(&output.stdout).into_owned())
	} else {
		Err(format!(
			"sudo {}: {}",
			args.join(" "),
			String::from_utf8_lossy(&output.stderr).trim()
		))
	}
}

/// Why a ZFS pool cannot be built here, or `None` when it can.
async fn skip_reason() -> Option<String> {
	if let Err(e) = sudo(&["true"]).await {
		return Some(format!("passwordless sudo required: {e}"));
	}
	if !["/sbin/zpool", "/usr/sbin/zpool", "/usr/local/sbin/zpool"]
		.iter()
		.any(|path| Path::new(path).exists())
	{
		return Some("zfs userland (zfsutils-linux) is not installed".into());
	}
	if !Path::new("/sys/module/zfs").exists() {
		if let Err(e) = sudo(&["modprobe", "zfs"]).await {
			return Some(format!("the zfs kernel module cannot load: {e}"));
		}
	}
	None
}

/// A file-backed pool with one encrypted dataset, destroyed on drop.
struct Pool {
	name: String,
	image: PathBuf,
	key_file: PathBuf,
	dataset: String,
	mount_point: PathBuf,
}

impl Pool {
	async fn create(tag: &str) -> Pool {
		let name = format!("sdzfs{tag}{}", std::process::id());
		let image = std::env::temp_dir().join(format!("{name}.img"));
		let key_file = std::env::temp_dir().join(format!("{name}.key"));
		std::fs::write(&key_file, "a passphrase of at least eight bytes\n").expect("key file");
		let file = std::fs::File::create(&image).expect("image");
		file.set_len(256 * 1024 * 1024).expect("size the image");
		drop(file);
		let pool_mount = PathBuf::from("/mnt").join(&name);
		sudo(&[
			"zpool",
			"create",
			"-m",
			pool_mount.to_str().unwrap(),
			&name,
			image.to_str().unwrap(),
		])
		.await
		.expect("zpool create");
		let dataset = format!("{name}/vault");
		let key_location = format!("file://{}", key_file.display());
		sudo(&[
			"zfs",
			"create",
			"-o",
			"encryption=on",
			"-o",
			"keyformat=passphrase",
			"-o",
			&format!("keylocation={key_location}"),
			&dataset,
		])
		.await
		.expect("zfs create");
		let mount_point = pool_mount.join("vault");
		sudo(&[
			"chown",
			"-R",
			&whoami::username(),
			mount_point.to_str().unwrap(),
		])
		.await
		.expect("chown");
		Pool {
			name,
			image,
			key_file,
			dataset,
			mount_point,
		}
	}

	async fn lock(&self) {
		sudo(&["zfs", "unmount", &self.dataset])
			.await
			.expect("zfs unmount");
		sudo(&["zfs", "unload-key", &self.dataset])
			.await
			.expect("zfs unload-key");
		assert!(
			self.mount_point.is_dir(),
			"the mount point stays behind on the pool"
		);
		let keystatus = sudo(&[
			"zfs",
			"get",
			"-H",
			"-o",
			"value",
			"keystatus",
			&self.dataset,
		])
		.await
		.expect("keystatus");
		assert_eq!(keystatus.trim(), "unavailable");
	}

	async fn unlock(&self) {
		sudo(&["zfs", "load-key", &self.dataset])
			.await
			.expect("zfs load-key");
		sudo(&["zfs", "mount", &self.dataset])
			.await
			.expect("zfs mount");
	}
}

impl Drop for Pool {
	fn drop(&mut self) {
		let _ = std::process::Command::new("sudo")
			.args(["-n", "zpool", "destroy", "-f", &self.name])
			.output();
		let _ = std::fs::remove_file(&self.image);
		let _ = std::fs::remove_file(&self.key_file);
		let _ = std::fs::remove_dir(PathBuf::from("/mnt").join(&self.name));
	}
}

/// A daemon over `data_dir`: detection on, watcher on, networking off.
async fn boot(data_dir: &Path) -> Arc<Core> {
	let mut config = TestConfigBuilder::new(data_dir.to_path_buf())
		.build()
		.expect("config");
	config.services.volume_monitoring_enabled = true;
	config.services.fs_watcher_enabled = true;
	config.save().expect("save config");
	Arc::new(Core::new(data_dir.to_path_buf()).await.expect("core"))
}

fn session(library: &Library) -> SessionContext {
	let mut session = SessionContext::device_session(
		sd_core::device::get_current_device_id(),
		sd_core::device::get_current_device_slug(),
	);
	session.current_library_id = Some(library.id());
	session
}

async fn job_count(library: &Library) -> usize {
	library.jobs().list_jobs(None).await.expect("jobs").len()
}

/// Poll `check` until it holds or ten seconds pass, for state the daemon
/// reaches through its event bus rather than on the caller's thread.
async fn eventually(what: &str, mut check: impl AsyncFnMut() -> bool) {
	for _ in 0..100 {
		if check().await {
			return;
		}
		tokio::time::sleep(std::time::Duration::from_millis(100)).await;
	}
	panic!("{what}: did not happen within ten seconds");
}

async fn listed_source(
	core: &Arc<Core>,
	library: &Library,
) -> sd_core::ops::sources::list::SourceInfo {
	ListSourcesQuery::all()
		.execute(core.context.clone(), session(library))
		.await
		.expect("sources.list")
		.into_iter()
		.find(|info| info.data_type == "filesystem" && info.device_id.is_none())
		.expect("the tracked source is listed")
}

async fn listed_volume(
	core: &Arc<Core>,
	library: &Library,
	mount_point: &Path,
) -> Option<sd_core::volume::Volume> {
	VolumeListQuery::from_input(VolumeListQueryInput {
		filter: VolumeFilter::All,
	})
	.expect("query")
	.execute(core.context.clone(), session(library))
	.await
	.expect("volumes.list")
	.volumes
	.into_iter()
	.find(|volume| volume.mount_point == mount_point)
}

/// L3 and L4: a dataset locked under the running daemon reads as locked in
/// `volumes.list` and `sources.list`, nothing is dispatched at it, its
/// listing serves from the snapshot with every record kept, and loading the
/// key and mounting it reattaches the source, arms the watch on the mounted
/// dataset, and retries the identification that failed while it was away.
#[tokio::test]
async fn a_locked_dataset_reads_as_locked_and_follows_its_key() {
	let _ = tracing_subscriber::fmt::try_init();
	if let Some(reason) = skip_reason().await {
		warn!("Skipping: {reason}");
		return;
	}

	let pool = Pool::create("L34").await;
	let root = pool.mount_point.clone();
	for n in 0..5 {
		std::fs::write(root.join(format!("file-{n}.txt")), n.to_string()).expect("write");
	}

	let data_dir = tempfile::tempdir().expect("data dir");
	let core = boot(data_dir.path()).await;
	let library = core
		.libraries
		.list()
		.await
		.into_iter()
		.next()
		.expect("default library");
	let cache = core.context.volume_index();
	let watcher = core.context.get_fs_watcher().await.expect("watcher");

	// Detection sees the mounted dataset as an ordinary ZFS volume.
	let mounted = listed_volume(&core, &library, &root)
		.await
		.expect("the dataset is a volume");
	assert_eq!(mounted.state(), VolumeState::Mounted);

	let output = TrackSourceAction::from_input(TrackSourceInput {
		path: root.clone(),
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
	let store = cache.store_for(&root).await.expect("store");
	eventually("the fixture's hashing pass finishes", async || {
		store.files_needing_content_count().await.unwrap() == 0
	})
	.await;
	let records = store.counts().await.expect("counts").records;
	assert!(records >= 5, "the walk recorded the files: {records}");
	eventually("the walked source is watched", async || {
		cache.is_watched(&root)
	})
	.await;
	assert_eq!(
		listed_source(&core, &library).await.volume_state,
		Some(VolumeState::Mounted)
	);

	// One record is left as if its read had failed while the dataset was
	// locked: no identity, and a reason recorded.
	let failed: (Vec<u8>,) =
		sqlx::query_as("SELECT uuid FROM record WHERE type = 'file' ORDER BY rowid LIMIT 1")
			.fetch_one(store.db().pool())
			.await
			.expect("a record");
	sqlx::query("UPDATE record SET content_id = NULL WHERE uuid = ?")
		.bind(&failed.0)
		.execute(store.db().pool())
		.await
		.expect("drop identity");
	sqlx::query("UPDATE facet_file SET content_error = 'volume locked' WHERE record_uuid = ?")
		.bind(&failed.0)
		.execute(store.db().pool())
		.await
		.expect("record failure");

	// The dataset is locked under the running daemon.
	pool.lock().await;
	let jobs_before = job_count(&library).await;
	core.volumes.refresh_volumes().await.expect("refresh");

	eventually("the source reads as locked", async || {
		listed_source(&core, &library).await.volume_state == Some(VolumeState::Locked)
	})
	.await;
	let listed = listed_source(&core, &library).await;
	assert!(!listed.attached, "a locked source is detached");
	assert_eq!(listed.item_count as u64, records, "no record was lost");
	let volume = listed_volume(&core, &library, &root)
		.await
		.expect("volumes.list keeps the locked dataset");
	assert_eq!(volume.state(), VolumeState::Locked);
	assert!(volume.locked && !volume.is_mounted);
	assert!(
		volume.is_tracked,
		"it is the tracked volume, not a stranger"
	);
	eventually("the watch is dropped with the mount", async || {
		!cache.is_watched(&root) && !watcher.watched_paths().await.contains(&root)
	})
	.await;

	// Nothing walks, hashes or thumbnails the directory left behind, and
	// the listing serves from the snapshot.
	let refusal = cache
		.dispatch_refusal(&root)
		.expect("dispatch at a locked dataset is refused");
	assert!(refusal.contains("not mounted"), "{refusal}");
	sd_core::ops::volumes::index::map_attached_volumes(&library, &core.context, false).await;
	identify_every_source(&library, &core.context).await;
	tokio::time::sleep(std::time::Duration::from_secs(1)).await;
	assert_eq!(
		job_count(&library).await,
		jobs_before,
		"no walk or hash job was dispatched at the locked dataset"
	);
	let child = root.join("file-0.txt");
	assert!(cache.ensure_restored(&child).await, "the map serves");
	assert!(cache.is_detached(&child), "read-only");
	assert!(cache
		.get_for_search(&child)
		.expect("map")
		.read()
		.await
		.get_entry_uuid(&child)
		.is_some());
	assert_eq!(store.counts().await.expect("counts").records, records);

	// The key loads and the dataset mounts again.
	pool.unlock().await;
	core.volumes.refresh_volumes().await.expect("refresh");
	eventually("the source reattaches within one refresh", async || {
		let listed = listed_source(&core, &library).await;
		listed.attached && listed.volume_state == Some(VolumeState::Mounted)
	})
	.await;
	assert_eq!(
		listed_volume(&core, &library, &root)
			.await
			.expect("volume")
			.state(),
		VolumeState::Mounted
	);
	eventually("the watch is armed on the mounted dataset", async || {
		cache.is_watched(&root) && watcher.watched_paths().await.contains(&root)
	})
	.await;

	let created = root.join("after-unlock.txt");
	std::fs::write(&created, "seen").expect("write");
	eventually(
		"a file created after the unlock reaches the map",
		async || match cache.get_for_search(&created) {
			Some(index) => index.read().await.get_entry_uuid(&created).is_some(),
			None => false,
		},
	)
	.await;
	eventually("the failed identification is retried", async || {
		let error: (Option<String>,) =
			sqlx::query_as("SELECT content_error FROM facet_file WHERE record_uuid = ?")
				.bind(&failed.0)
				.fetch_one(store.db().pool())
				.await
				.expect("content error");
		error.0.is_none() && store.files_needing_content_count().await.unwrap() == 0
	})
	.await;

	drop(library);
	core.shutdown().await.expect("shutdown");
	drop(core);
	tokio::time::sleep(std::time::Duration::from_secs(1)).await;
	drop(pool);
}
